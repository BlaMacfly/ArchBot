//! Monde simulé pour les scénarios du moteur : un bot, des monstres, des objets au sol, un terrain à grille.
//!
//! Il reproduit les règles d'ArchCore utiles au moteur (marche du serveur vers la destination, approche
//! automatique en attaque, riposte et poursuite des monstres jusqu'à 5000 unités, régénération hors combat,
//! butin au tueur), de façon volontairement simple : il ne remplace pas le serveur, il vérifie que le
//! moteur prend des décisions cohérentes face à ces règles.

#![allow(dead_code)]

use archbot::world::*;
use archbot::{Action, BotConfig, Brain, BotState};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub struct GridTerrain {
    pub blocked: HashSet<(i64, i64)>,
    pub cell: f64,
}

impl GridTerrain {
    pub fn open() -> Self {
        GridTerrain { blocked: HashSet::new(), cell: 400.0 }
    }
    pub fn block_cell(&mut self, cx: i64, cz: i64) {
        self.blocked.insert((cx, cz));
    }
    pub fn wall_x(&mut self, cx: i64, z0: i64, z1: i64) {
        for z in z0..=z1 {
            self.blocked.insert((cx, z));
        }
    }
    pub fn wall_z(&mut self, cz: i64, x0: i64, x1: i64) {
        for x in x0..=x1 {
            self.blocked.insert((x, cz));
        }
    }
}

impl Terrain for GridTerrain {
    fn is_blocked(&self, x: f64, z: f64) -> bool {
        self.blocked.contains(&((x / self.cell).floor() as i64, (z / self.cell).floor() as i64))
    }
    fn cell_size(&self) -> f64 {
        self.cell
    }
}

#[derive(Clone, Debug)]
pub struct Mob {
    pub id: EntityId,
    pub pos: Pos,
    pub home: Pos,
    pub level: i32,
    pub hp: i32,
    pub hp_max: i32,
    pub alive: bool,
    pub aggro: Vec<EntityId>,
    pub hit_range: f64,
    pub damage: i32,
    pub next_hit: f64,
    pub respawn_at: Option<f64>,
    /// Instant de la mort : le cadavre reste visible 20 s, comme sur le serveur.
    pub died_at: Option<f64>,
}

impl Mob {
    pub fn new(id: EntityId, pos: Pos, level: i32, hp: i32, damage: i32) -> Self {
        Mob { id, pos, home: pos, level, hp, hp_max: hp, alive: true, aggro: Vec::new(), hit_range: 100.0, damage, next_hit: 0.0, respawn_at: None, died_at: None }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: EntityId,
    pub pos: Pos,
    pub owner: EntityId,
}

/// Le monde simulé.
pub struct Sim {
    pub me: SelfView,
    pub mobs: Vec<Mob>,
    pub items: Vec<Item>,
    pub terrain: GridTerrain,
    pub master: Option<PlayerView>,
    pub t: f64,
    start: Instant,
    pub my_damage: i32,
    last_hit: f64,
    last_combat: f64,
    regen_at: f64,
    next_item: EntityId,
    pub potions_drunk: u32,
    pub resurrections: u32,
    pub picked: u32,
    pub casts: u32,
    pub actions: HashMap<&'static str, u32>,
    pub states: HashSet<BotState>,
    pub respawn_secs: Option<f64>,
    /// Expérience donnée par monstre tué (64 bits ; 0 : aucune) ; montée de niveau quand `me.exp` atteint `me.exp_next`.
    pub exp_per_kill: u64,
    pub trace: Vec<String>,
    pub blocked_steps: u32,
    pub max_state_stay: HashMap<BotState, f64>,
    state_since: (BotState, f64),
}

impl Sim {
    pub fn new(me: SelfView) -> Self {
        Sim {
            me,
            mobs: Vec::new(),
            items: Vec::new(),
            terrain: GridTerrain::open(),
            master: None,
            t: 0.0,
            start: Instant::now(),
            my_damage: 20,
            last_hit: -10.0,
            last_combat: -100.0,
            regen_at: 10.0,
            next_item: 0x3000_0000,
            potions_drunk: 0,
            resurrections: 0,
            picked: 0,
            casts: 0,
            actions: HashMap::new(),
            states: HashSet::new(),
            respawn_secs: None,
            exp_per_kill: 0,
            trace: Vec::new(),
            blocked_steps: 0,
            max_state_stay: HashMap::new(),
            state_since: (BotState::Idle, 0.0),
        }
    }

    pub fn now(&self) -> Instant {
        self.start + Duration::from_secs_f64(self.t)
    }

    pub fn perceive(&self) -> Perception<'_> {
        let in_combat = self.t - self.last_combat < 10.0;
        let mut me = self.me.clone();
        me.in_combat = in_combat;
        let mut p = Perception::new(self.now(), me, &self.terrain);
        let master = self.master.as_ref().map(|m| m.id);
        for m in &self.mobs {
            if !m.alive && m.died_at.is_none_or(|d| self.t - d > 20.0) {
                continue;
            }
            p.monsters.push(MonsterView {
                id: m.id,
                pos: m.pos,
                level: m.level,
                hp: m.hp,
                hp_max: m.hp_max,
                alive: m.alive,
                attackable: true,
                aggro_me: m.aggro.contains(&self.me.id),
                aggro_master: master.is_some_and(|id| m.aggro.contains(&id)),
                aggro_party: false,
                aggro_others: m.aggro.iter().any(|a| *a != self.me.id && Some(*a) != master),
                hit_range: m.hit_range,
            });
        }
        for i in &self.items {
            p.items.push(GroundItemView { id: i.id, pos: i.pos, mine: i.owner == self.me.id, gold: false });
        }
        p.master = self.master.clone();
        p
    }

    pub fn apply(&mut self, actions: &[Action]) {
        for a in actions {
            *self.actions.entry(a.kind()).or_default() += 1;
            match a {
                Action::MoveTo(d) => {
                    if !self.me.dead {
                        self.me.dest = Some(*d);
                    }
                }
                Action::Stop => self.me.dest = None,
                Action::Attack(id) => {
                    if self.mobs.iter().any(|m| m.id == *id && m.alive) {
                        self.me.attacking = Some(*id);
                    }
                }
                Action::StopAttack => self.me.attacking = None,
                Action::PickUp(id) => {
                    if let Some(i) = self.items.iter().position(|i| i.id == *id) {
                        let item = &self.items[i];
                        if item.owner == self.me.id && self.me.pos.dist2d(item.pos) <= 2000.0 {
                            self.items.remove(i);
                            self.picked += 1;
                        }
                    }
                }
                Action::Resurrect => {
                    if self.me.dead {
                        self.me.dead = false;
                        self.me.hp = self.me.hp_max;
                        self.me.pos = self.me.home;
                        self.me.dest = None;
                        self.me.invincible = true;
                        self.resurrections += 1;
                    }
                }
                Action::Potion(PotionKind::Hp) => {
                    if self.me.hp_potions > 0 {
                        self.me.hp_potions -= 1;
                        self.me.hp = (self.me.hp + 75).min(self.me.hp_max);
                        self.potions_drunk += 1;
                    }
                }
                Action::Potion(PotionKind::Mp) => {
                    if self.me.mp_potions > 0 {
                        self.me.mp_potions -= 1;
                        self.me.mp = (self.me.mp + 45).min(self.me.mp_max);
                        self.potions_drunk += 1;
                    }
                }
                Action::Cast { .. } => self.casts += 1,
                _ => {}
            }
        }
    }

    /// Avance le monde de `dt` secondes.
    pub fn step(&mut self, dt: f64) {
        let me_id = self.me.id;
        if self.me.invincible && self.t - self.last_combat > 10.0 {
            self.me.invincible = false;
        }
        // Marche du serveur.
        if let Some(d) = self.me.dest {
            if self.me.dead {
                self.me.dest = None;
            } else {
                let next = self.me.pos.step_toward(d, self.me.run_speed * dt);
                if self.terrain.is_blocked(next.x as f64, next.z as f64) {
                    self.blocked_steps += 1;
                }
                self.me.pos = next;
                if self.me.pos.dist2d(d) < 1.0 {
                    self.me.dest = None;
                }
            }
        }
        // Attaque du bot : approche automatique, coups toutes les secondes.
        if let Some(target) = self.me.attacking {
            match self.mobs.iter_mut().find(|m| m.id == target && m.alive) {
                None => self.me.attacking = None,
                Some(m) => {
                    let reach = self.me.attack_range.max(50.0) + m.hit_range + 50.0;
                    let dist = self.me.pos.dist2d(m.pos);
                    if dist > reach {
                        if self.me.dest.is_none() {
                            self.me.dest = Some(self.me.pos.at_distance_from(m.pos, m.hit_range / 2.0));
                        }
                    } else if self.t - self.last_hit >= 1.0 {
                        self.me.dest = None;
                        self.last_hit = self.t;
                        self.last_combat = self.t;
                        m.hp -= self.my_damage;
                        if !m.aggro.contains(&me_id) {
                            m.aggro.push(me_id);
                        }
                        if m.hp <= 0 {
                            m.hp = 0;
                            m.alive = false;
                            m.aggro.clear();
                            m.respawn_at = self.respawn_secs.map(|s| self.t + s);
                            m.died_at = Some(self.t);
                            self.me.attacking = None;
                            let id = self.next_item;
                            self.next_item += 1;
                            self.items.push(Item { id, pos: m.pos, owner: me_id });
                            if self.exp_per_kill > 0 {
                                self.me.exp += self.exp_per_kill;
                                self.me.exp_total += self.exp_per_kill;
                                while self.me.exp_next > 0 && self.me.exp >= self.me.exp_next {
                                    self.me.exp -= self.me.exp_next;
                                    self.me.level += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        // Monstres : poursuite et coups ; réapparition.
        let me_pos = self.me.pos;
        let mut damage_taken = 0;
        for m in &mut self.mobs {
            if !m.alive {
                if m.respawn_at.is_some_and(|r| self.t >= r) {
                    m.alive = true;
                    m.hp = m.hp_max;
                    m.pos = m.home;
                    m.respawn_at = None;
                    m.died_at = None;
                }
                continue;
            }
            if !m.aggro.contains(&me_id) {
                continue;
            }
            let dist = m.pos.dist2d(me_pos);
            if dist > 5000.0 || self.me.dead || self.me.invincible {
                m.aggro.retain(|a| *a != me_id);
                continue;
            }
            if dist > m.hit_range + 50.0 {
                m.pos = m.pos.step_toward(me_pos, 350.0 * dt);
            } else if self.t >= m.next_hit {
                m.next_hit = self.t + 1.5;
                damage_taken += m.damage;
            }
        }
        if damage_taken > 0 && !self.me.dead {
            self.last_combat = self.t;
            self.me.hp -= damage_taken;
            if self.me.hp <= 0 {
                self.me.hp = 0;
                self.me.dead = true;
                self.me.attacking = None;
                self.me.dest = None;
                for m in &mut self.mobs {
                    m.aggro.clear();
                }
            }
        }
        // Régénération hors combat (1,5 % + 5 toutes les 10 s).
        if !self.me.dead && self.t - self.last_combat >= 10.0 && self.t >= self.regen_at {
            self.regen_at = self.t + 10.0;
            self.me.hp = (self.me.hp + (self.me.hp_max as f64 * 0.015) as i32 + 5).min(self.me.hp_max);
            self.me.mp = (self.me.mp + (self.me.mp_max as f64 * 0.015) as i32 + 5).min(self.me.mp_max);
        }
        self.t += dt;
    }

    /// Fait vivre le bot `seconds` secondes par pas de `dt`.
    pub fn run(&mut self, brain: &mut Brain, cfg: &BotConfig, seconds: f64, dt: f64) {
        let mut elapsed = 0.0;
        while elapsed < seconds {
            let actions = {
                let p = self.perceive();
                brain.think(&p, cfg)
            };
            self.apply(&actions);
            self.states.insert(brain.state);
            if brain.state != self.state_since.0 {
                let stay = self.t - self.state_since.1;
                let e = self.max_state_stay.entry(self.state_since.0).or_default();
                *e = e.max(stay);
                self.trace.push(format!("t={:.1} {} → {} ({})", self.t, self.state_since.0, brain.state, brain.reason));
                self.state_since = (brain.state, self.t);
            }
            self.step(dt);
            elapsed += dt;
        }
        let stay = self.t - self.state_since.1;
        let e = self.max_state_stay.entry(self.state_since.0).or_default();
        *e = e.max(stay);
    }
}

pub fn warrior(id: u32, pos: Pos) -> SelfView {
    SelfView { id, name: format!("Bot{id}"), pos, home: pos, hp: 120, hp_max: 120, mp: 32, mp_max: 32, level: 3, class: ClassKind::Melee, attack_range: 80.0, run_speed: 1200.0, ..Default::default() }
}
