//! Gestionnaire des bots : population autonome, boucle d'IA à cadence réglée, répartition des bots entre les
//! tours, invitations de groupe, rotation des sessions, métriques.
//!
//! La boucle tourne dans une tâche tokio à part (comme la boucle de combat d'ArchCore) et prend le verrou du
//! monde à chaque tour ; elle ne traite que `ArchBot.BotsPerTick` bots par tour, les autres attendant le tour
//! suivant, et un bot sans vrai joueur à `ArchBot.FarFromPlayersUnits` ne pense qu'un tour sur
//! `ArchBot.FarTickDivisor`. Une entrée en jeu au plus par `ArchBot.SpawnIntervalMs`.

use super::persistence::{self, BotRow};
use super::{actions, lifecycle, perception, Shared};
use crate::world::{World, WorldState};
use crate::{info, warn};
use archbot::population::SpawnPacer;
use archbot::world::Pos;
use archbot::{BotConfig, Brain, Metrics};
use archcore_net::Vec3;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Attente au plus avant de faire sortir un bot en combat ou mort quand la population visée baisse.
const SURPLUS_PATIENCE: Duration = Duration::from_secs(30);

/// Un bot en jeu.
pub struct BotEntry {
    pub cid: u32,
    pub name: String,
    pub brain: Brain,
    /// Point d'ancrage (exploration, refuge).
    pub home: Pos,
    pub entered_at: Instant,
    /// Fin de session (rotation), s'il y en a une.
    pub session_end: Option<Instant>,
    /// Compteurs déjà reportés dans les métriques : victoires, morts, niveaux, expérience (64 bits).
    pub counted: (u32, u32, u32, u64),
    pub row: BotRow,
}

pub struct Manager {
    pub cfg: BotConfig,
    /// Bots en jeu, par identifiant de session.
    pub bots: HashMap<u32, BotEntry>,
    /// Population de bots autonomes visée.
    pub target_population: usize,
    /// Personnages de bots en base, pas en jeu.
    pub pool: Vec<BotRow>,
    pacer: SpawnPacer,
    /// Depuis quand la population dépasse la cible sans bot au calme à faire sortir (patience de 30 s).
    surplus_since: Option<Instant>,
    rng: StdRng,
    pub metrics: Metrics,
    tick: u64,
}

impl Manager {
    pub fn new(cfg: BotConfig, pool: Vec<BotRow>) -> Self {
        let target = if cfg.autonomous { cfg.initial_population.min(cfg.max_bots) } else { 0 };
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(1);
        Manager {
            pacer: SpawnPacer::new(Duration::from_millis(cfg.spawn_interval_ms)),
            surplus_since: None,
            cfg,
            bots: HashMap::new(),
            target_population: target,
            pool,
            rng: StdRng::seed_from_u64(seed),
            metrics: Metrics::default(),
            tick: 0,
        }
    }

    pub fn is_bot(&self, sid: u32) -> bool {
        self.bots.contains_key(&sid)
    }

    pub fn sid_of(&self, cid: u32) -> Option<u32> {
        self.bots.iter().find(|(_, e)| e.cid == cid).map(|(s, _)| *s)
    }

    /// Sessions des compagnons du maître `master` (CID), dans un ordre stable.
    pub fn companions_of(&self, master: u32) -> Vec<u32> {
        let mut v: Vec<u32> = self.bots.iter().filter(|(_, e)| e.brain.companion().is_some_and(|c| c.master == master)).map(|(s, _)| *s).collect();
        v.sort_unstable();
        v
    }

    pub fn autonomous_count(&self) -> usize {
        self.bots.values().filter(|e| !e.brain.is_companion()).count()
    }

    /// Fait entrer un bot : un personnage de la réserve, sinon un personnage créé. Rend sa session.
    pub async fn spawn_one(&mut self, world: &World, st: &mut WorldState) -> anyhow::Result<Option<u32>> {
        if self.bots.len() >= self.cfg.max_bots {
            return Ok(None);
        }
        let row = match self.pool.pop() {
            Some(r) => r,
            None => match lifecycle::create_bot(&world.ctx, &self.cfg, &mut self.rng).await? {
                Some(r) => r,
                None => return Ok(None),
            },
        };
        let Some(sid) = lifecycle::enter(world, st, &row).await? else {
            warn!("[archbot] {} n'a pas pu entrer en jeu", row.name);
            return Ok(None);
        };
        let now = Instant::now();
        let home = if row.home == Vec3::default() { st.players.get(&sid).map(|p| perception::to_pos(p.ch.pos())).unwrap_or_default() } else { perception::to_pos(row.home) };
        let session_end = archbot::population::session_length(&mut self.rng, self.cfg.rotation_minutes).map(|d| now + d);
        let (cid, name) = (row.character_id, row.name.clone());
        self.bots.insert(sid, BotEntry { cid, name: name.clone(), brain: Brain::new(cid as u64), home, entered_at: now, session_end, counted: (0, 0, 0, 0), row });
        self.metrics.spawned += 1;
        info!("[archbot] {name} entre en jeu (session #{sid}, {} bot(s))", self.bots.len());
        Ok(Some(sid))
    }

    /// Fait entrer un bot et le place près de `at` (compagnon engagé loin de tout bot libre).
    pub async fn spawn_near(&mut self, world: &World, st: &mut WorldState, at: Vec3) -> anyhow::Result<Option<u32>> {
        let Some(sid) = self.spawn_one(world, st).await? else { return Ok(None) };
        let x = at.x as f64 + 150.0;
        let z = at.z as f64 + 150.0;
        crate::world::teleport(&world.ctx, st, sid, x, None, z).await?;
        if let Some(e) = self.bots.get_mut(&sid) {
            e.home = perception::to_pos(at);
        }
        Ok(Some(sid))
    }

    /// Sort un bot du monde ; son personnage retourne dans la réserve (ou est marqué supprimé sans persistance).
    pub async fn despawn(&mut self, world: &World, st: &mut WorldState, sid: u32) {
        let Some(entry) = self.bots.remove(&sid) else { return };
        let (kills, deaths) = (entry.brain.stats.kills, entry.brain.stats.deaths);
        lifecycle::leave(&world.ctx, st, sid).await;
        if let Err(e) = persistence::touch(&world.ctx.db, entry.cid, kills, deaths).await {
            warn!("[archbot] {} : enregistrement : {e:#}", entry.name);
        }
        if self.cfg.persist_characters {
            self.pool.push(entry.row);
        } else if let Err(e) = persistence::forget(&world.ctx.db, entry.cid).await {
            warn!("[archbot] {} : suppression : {e:#}", entry.name);
        }
        self.metrics.despawned += 1;
        info!("[archbot] {} quitte le monde ({} bot(s))", entry.name, self.bots.len());
    }

    pub async fn despawn_all(&mut self, world: &World, st: &mut WorldState) {
        let mut sids: Vec<u32> = self.bots.keys().copied().collect();
        sids.sort_unstable();
        for sid in sids {
            self.despawn(world, st, sid).await;
        }
    }

    /// Un tour de la boucle d'IA.
    pub async fn tick(&mut self, world: &World, st: &mut WorldState) {
        let started = Instant::now();
        let now = started;
        self.tick += 1;
        // Bots dont le personnage a disparu du monde (expulsé, remplacé) : oubliés.
        let gone: Vec<u32> = self.bots.keys().copied().filter(|sid| !st.players.get(sid).is_some_and(|p| p.in_game)).collect();
        for sid in gone {
            if let Some(e) = self.bots.remove(&sid) {
                warn!("[archbot] {} n'est plus en jeu : retiré du gestionnaire", e.name);
                if self.cfg.persist_characters {
                    self.pool.push(e.row);
                }
            }
        }
        // Population autonome.
        if self.cfg.autonomous && self.autonomous_count() < self.target_population && self.bots.len() < self.cfg.max_bots && self.pacer.take(now) {
            match self.spawn_one(world, st).await {
                Ok(_) => {}
                Err(e) => {
                    self.metrics.record_error();
                    warn!("[archbot] entrée d'un bot : {e:#}");
                }
            }
        }
        // Population visée en baisse (/bot population n) : les bots autonomes en trop se déconnectent comme des joueurs
        // (groupe quitté, position enregistrée, personnage rendu à la réserve), au calme d'abord, deux par tour au plus ;
        // ceux qui combattent ou sont morts ne partent qu'après 30 s sans bot au calme.
        let surplus = self.autonomous_count().saturating_sub(self.target_population);
        if surplus == 0 {
            self.surplus_since = None;
        } else {
            let candidates: Vec<archbot::population::Leaver> = self
                .bots
                .iter()
                .filter(|(_, e)| !e.brain.is_companion())
                .filter_map(|(sid, _)| st.players.get(sid).map(|p| archbot::population::Leaver { id: *sid, in_combat: now < p.timers.combat_until, dead: p.dead }))
                .collect();
            let since = *self.surplus_since.get_or_insert(now);
            let patience_out = now.duration_since(since) >= SURPLUS_PATIENCE;
            let leaving = archbot::population::leavers(&candidates, self.autonomous_count(), self.target_population, 2, patience_out);
            if !leaving.is_empty() {
                self.surplus_since = None;
            }
            for sid in leaving {
                self.despawn(world, st, sid).await;
            }
        }
        // Rotation : un bot autonome en fin de session, hors combat, cède sa place.
        let rotate: Vec<u32> = self
            .bots
            .iter()
            .filter(|(sid, e)| !e.brain.is_companion() && e.session_end.is_some_and(|t| now >= t) && st.players.get(sid).is_some_and(|p| now >= p.timers.combat_until && !p.dead))
            .map(|(s, _)| *s)
            .take(1)
            .collect();
        for sid in rotate {
            self.despawn(world, st, sid).await;
        }
        // Invitations de groupe reçues par des bots : acceptées (un vrai joueur invite un bot).
        if self.cfg.allow_party {
            let pending: Vec<(u32, u32)> = st.parties.invitations.iter().flat_map(|(from, list)| list.iter().map(move |to| (*from, *to))).collect();
            for (from, to) in pending {
                if let Some(bsid) = self.sid_of(to) {
                    if let Err(e) = actions::accept_party_invitation(&world.ctx, st, bsid, from) {
                        warn!("[archbot] invitation : {e:#}");
                    }
                }
            }
        }
        // Pensée : une tranche des bots par tour.
        let mut sids: Vec<u32> = self.bots.keys().copied().collect();
        sids.sort_unstable();
        let n = sids.len();
        let slices = n.div_ceil(self.cfg.bots_per_tick.max(1)).max(1) as u64;
        let my_slice = self.tick % slices;
        let terrain = perception::MapTerrain { ctx: &world.ctx };
        let mut thought = 0usize;
        for (i, sid) in sids.into_iter().enumerate() {
            if (i as u64) % slices != my_slice {
                continue;
            }
            let (home, master) = match self.bots.get(&sid) {
                Some(e) => (e.home, e.brain.companion().map(|c| c.master)),
                None => continue,
            };
            let actions = {
                let Some(p) = perception::build(&world.ctx, st, sid, home, master, &self.cfg, now, &terrain) else { continue };
                if !p.real_player_near && self.cfg.far_tick_divisor > 1 && (self.tick / slices) % self.cfg.far_tick_divisor as u64 != 0 {
                    continue;
                }
                let e = self.bots.get_mut(&sid).unwrap();
                e.brain.think(&p, &self.cfg)
            };
            thought += 1;
            for a in actions {
                if let Err(e) = actions::execute(world, st, sid, a, &mut self.metrics, self.cfg.debug).await {
                    self.metrics.record_error();
                    warn!("[archbot] #{sid} : {e:#}");
                }
            }
            if let Some(e) = self.bots.get_mut(&sid) {
                let s = e.brain.stats;
                self.metrics.kills += (s.kills - e.counted.0) as u64;
                self.metrics.deaths += (s.deaths - e.counted.1) as u64;
                self.metrics.level_ups += (s.level_ups - e.counted.2) as u64;
                self.metrics.exp_gained = self.metrics.exp_gained.saturating_add(s.exp_gained - e.counted.3);
                e.counted = (s.kills, s.deaths, s.level_ups, s.exp_gained);
            }
        }
        self.metrics.set_states(self.bots.values().map(|e| e.brain.state));
        self.metrics.record_tick(started.elapsed(), thought);
        if self.cfg.debug && self.tick % 20 == 0 {
            info!("[archbot] {}", self.metrics.report().replace('\n', " | "));
        }
    }
}

/// Lance la boucle d'IA.
pub fn spawn_ticker(sh: Arc<Shared>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(sh.cfg.ai_interval());
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let mut st = sh.world.state.lock().await;
            let mut m = sh.manager.lock().await;
            m.tick(&sh.world, &mut st).await;
        }
    });
}
