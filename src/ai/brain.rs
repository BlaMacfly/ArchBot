//! Cerveau d'un bot : décide des actions d'un tour à partir d'une perception.
//!
//! Priorités d'un tour, de la plus haute à la plus basse :
//! mort / résurrection → fuite → potions → combat (riposte, puis cible selon le mode) → butin →
//! progression (équipement, compétences) → repos → suivi (compagnon) ou exploration (autonome).
//! Chaque décision ne s'appuie que sur la [`Perception`] : aucune donnée du serveur n'est devinée.

use crate::ai::state::BotState;
use crate::combat::{classes, survival, targeting};
use crate::commands::{Command, HELP};
use crate::config::BotConfig;
use crate::movement::{NavStep, Navigator, ARRIVAL_TOLERANCE};
use crate::party;
use crate::world::{Action, ClassKind, EntityId, MonsterView, Perception, Pos, PotionKind, SelfView};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::{Duration, Instant};

/// Attitude d'un compagnon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stance {
    /// N'attaque jamais de lui-même.
    Passive,
    /// Défend son maître et attaque la cible de celui-ci.
    #[default]
    Defensive,
    /// En plus, attaque les monstres proches du maître.
    Aggressive,
}

impl Stance {
    pub fn name(self) -> &'static str {
        match self {
            Stance::Passive => "passive",
            Stance::Defensive => "defensive",
            Stance::Aggressive => "aggressive",
        }
    }
}

/// Ordre de position d'un compagnon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Order {
    Follow,
    Stay(Pos),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Companion {
    pub master: EntityId,
    pub master_name: String,
    pub stance: Stance,
    pub follow_distance: f64,
    pub order: Order,
    /// Place dans la formation (index, nombre de compagnons du même maître).
    pub slot: (usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Autonomous,
    Companion(Companion),
}

/// Compteurs d'un cerveau.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrainStats {
    pub transitions: u32,
    pub kills: u32,
    pub deaths: u32,
    pub flees: u32,
    pub loots: u32,
    pub explorations: u32,
    pub stuck: u32,
    /// Niveaux gagnés depuis l'entrée en jeu.
    pub level_ups: u32,
    /// Expérience gagnée et perdue (pénalité de mort) depuis l'entrée en jeu, en 64 bits.
    pub exp_gained: u64,
    pub exp_lost: u64,
}

/// Délais internes.
const LOOT_TIMEOUT: Duration = Duration::from_secs(15);
const LOOT_BACKOFF: Duration = Duration::from_secs(20);
const TARGET_TIMEOUT: Duration = Duration::from_secs(25);
const BLACKLIST_FOR: Duration = Duration::from_secs(90);
const EQUIP_EVERY: Duration = Duration::from_secs(30);
const LEARN_EVERY: Duration = Duration::from_secs(60);
const FLEE_CLEAR_AFTER: Duration = Duration::from_secs(3);
const RESURRECT_RETRY: Duration = Duration::from_secs(5);
/// Rayon de « laisse » d'un compagnon offensif autour de son maître.
const AGGRESSIVE_LEASH: f64 = 1500.0;
/// Marge de portée (positions arrondies, cible qui bouge).
const REACH_MARGIN: f64 = 50.0;

pub struct Brain {
    pub state: BotState,
    pub mode: Mode,
    pub stats: BrainStats,
    pub entered_at: Option<Instant>,
    pub reason: &'static str,
    nav: Navigator,
    target: Option<EntityId>,
    target_hp: (i32, Option<Instant>),
    blacklist: Vec<(EntityId, Instant)>,
    idle_until: Option<Instant>,
    last_potion: Option<Instant>,
    last_equip: Option<Instant>,
    last_learn: Option<Instant>,
    loot_since: Option<Instant>,
    loot_blocked_until: Option<Instant>,
    died_at: Option<Instant>,
    resurrect_asked: Option<Instant>,
    flee_clear_since: Option<Instant>,
    flee_point: Option<Pos>,
    /// Ordre « attack » reçu : la cible du maître est prise même en attitude passive.
    attack_order: bool,
    /// Terrain de chasse trouvé (dernier monstre tué) : centre de l'exploration à la place du point d'ancrage.
    camp: Option<Pos>,
    /// Facteur du rayon d'exploration : grandit tant que les explorations ne croisent aucun monstre éligible
    /// (un village est sûr, il faut s'en éloigner pour chasser), retombe quand on en trouve.
    explore_scale: f64,
    /// Dernier (niveau, expérience cumulée) vu : base des gains d'expérience.
    progress: Option<(i32, u64)>,
    rng: StdRng,
}

/// Facteur d'exploration au plus (× ExploreRadius).
const MAX_EXPLORE_SCALE: f64 = 6.0;

impl Brain {
    /// `seed` : identifiant du bot, pour des tirages reproductibles.
    pub fn new(seed: u64) -> Self {
        Brain {
            state: BotState::Idle,
            mode: Mode::Autonomous,
            stats: BrainStats::default(),
            entered_at: None,
            reason: "new",
            nav: Navigator::new(),
            target: None,
            target_hp: (0, None),
            blacklist: Vec::new(),
            idle_until: None,
            last_potion: None,
            last_equip: None,
            last_learn: None,
            loot_since: None,
            loot_blocked_until: None,
            died_at: None,
            resurrect_asked: None,
            flee_clear_since: None,
            flee_point: None,
            attack_order: false,
            camp: None,
            explore_scale: 1.0,
            progress: None,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// Terrain de chasse courant (dernier monstre tué), s'il y en a un.
    pub fn camp(&self) -> Option<Pos> {
        self.camp
    }

    /// Facteur courant du rayon d'exploration.
    pub fn explore_scale(&self) -> f64 {
        self.explore_scale
    }

    pub fn target(&self) -> Option<EntityId> {
        self.target
    }

    /// But de la navigation en cours (exploration, fuite planifiée).
    pub fn navigation_goal(&self) -> Option<Pos> {
        self.nav.goal()
    }

    pub fn is_companion(&self) -> bool {
        matches!(self.mode, Mode::Companion(_))
    }

    pub fn companion(&self) -> Option<&Companion> {
        match &self.mode {
            Mode::Companion(c) => Some(c),
            Mode::Autonomous => None,
        }
    }

    /// Change de mode : l'état repart d'Idle, la navigation et la cible sont oubliées.
    pub fn set_mode(&mut self, mode: Mode, now: Instant) {
        self.mode = mode;
        self.nav.clear();
        self.target = None;
        self.idle_until = None;
        self.attack_order = false;
        if !self.state.is_dead() {
            self.enter(BotState::Idle, now, "mode");
        }
    }

    /// Devient le compagnon de `master`.
    pub fn hire(&mut self, master: EntityId, master_name: &str, follow_distance: f64, slot: (usize, usize), now: Instant) {
        self.set_mode(Mode::Companion(Companion { master, master_name: master_name.to_string(), stance: Stance::Defensive, follow_distance, order: Order::Follow, slot }), now);
    }

    /// Redevient autonome autour de `home` (le point d'ancrage est tenu par l'hôte dans la perception).
    pub fn dismiss(&mut self, now: Instant) {
        self.set_mode(Mode::Autonomous, now);
    }

    /// Applique une commande du maître ; rend la réponse à lui envoyer (si les bots parlent).
    pub fn command(&mut self, cmd: &Command, me_pos: Pos, _now: Instant) -> String {
        let Mode::Companion(c) = &mut self.mode else {
            return "Je ne suis le compagnon de personne.".into();
        };
        match cmd {
            Command::Follow(d) => {
                if let Some(d) = d {
                    c.follow_distance = d.clamp(100.0, 3000.0);
                }
                c.order = Order::Follow;
                self.nav.clear();
                format!("Je vous suis à {:.0} unités.", c.follow_distance)
            }
            Command::Stay => {
                c.order = Order::Stay(me_pos);
                self.nav.clear();
                "Je reste ici.".into()
            }
            Command::Come => {
                c.order = Order::Follow;
                self.nav.clear();
                "J'arrive.".into()
            }
            Command::Attack => {
                self.attack_order = true;
                "À l'attaque !".into()
            }
            Command::Stance(s) => {
                c.stance = *s;
                if *s == Stance::Passive {
                    self.target = None;
                    self.attack_order = false;
                }
                format!("Attitude : {}.", s.name())
            }
            Command::Stop => {
                self.target = None;
                self.attack_order = false;
                c.order = Order::Follow;
                "Je cesse le combat.".into()
            }
            Command::Status => self.status_line(),
            Command::Help => HELP.into(),
            Command::Hire | Command::Dismiss => String::new(),
        }
    }

    /// Une ligne d'état lisible.
    pub fn status_line(&self) -> String {
        let mode = match &self.mode {
            Mode::Autonomous => "autonome".to_string(),
            Mode::Companion(c) => format!("compagnon de {} ({}, {})", c.master_name, c.stance.name(), match c.order { Order::Follow => "suit".to_string(), Order::Stay(_) => "reste".to_string() }),
        };
        format!(
            "{mode}, état {} ({}), cible {}, {} victoire(s), {} mort(s), {} fuite(s), {} ramassage(s), {} niveau(x) et {} point(s) d'expérience gagnés",
            self.state,
            self.reason,
            self.target.map(|t| format!("{t:#x}")).unwrap_or_else(|| "aucune".into()),
            self.stats.kills,
            self.stats.deaths,
            self.stats.flees,
            self.stats.loots,
            self.stats.level_ups,
            self.stats.exp_gained
        )
    }

    /// Gains d'expérience et de niveaux, par différence avec le tour précédent (l'expérience cumulée de l'hôte
    /// couvre les montées de niveau ; une baisse est une pénalité de mort). Un hôte qui ne tient pas le cumul
    /// (`exp_total` à 0) ne compte que les niveaux.
    fn track_progress(&mut self, me: &SelfView) {
        if let Some((level, total)) = self.progress {
            if me.level > level {
                self.stats.level_ups += (me.level - level) as u32;
            }
            if me.exp_total > total {
                self.stats.exp_gained = self.stats.exp_gained.saturating_add(me.exp_total - total);
            } else if me.exp_total < total && me.level <= level {
                self.stats.exp_lost = self.stats.exp_lost.saturating_add(total - me.exp_total);
            }
        }
        self.progress = Some((me.level, me.exp_total));
    }

    fn enter(&mut self, next: BotState, now: Instant, reason: &'static str) {
        if next != self.state {
            self.state = next;
            self.entered_at = Some(now);
            self.reason = reason;
            self.stats.transitions += 1;
        }
    }

    fn blacklisted(&self) -> Vec<EntityId> {
        self.blacklist.iter().map(|(id, _)| *id).collect()
    }

    fn blacklist(&mut self, id: EntityId, now: Instant) {
        self.blacklist.retain(|(i, _)| *i != id);
        self.blacklist.push((id, now + BLACKLIST_FOR));
    }

    // ---------------------------------------------------------------------------------------------
    // Un tour
    // ---------------------------------------------------------------------------------------------

    /// Décide les actions de ce tour.
    pub fn think(&mut self, p: &Perception, cfg: &BotConfig) -> Vec<Action> {
        let now = p.now;
        let mut out = Vec::new();
        self.blacklist.retain(|(_, until)| *until > now);
        self.track_progress(&p.me);

        if p.me.dead {
            return self.think_dead(cfg, now);
        }
        if self.state.is_dead() {
            // Ressuscité : tout repart de zéro.
            self.enter(BotState::Idle, now, "revived");
            self.nav.clear();
            self.target = None;
            self.flee_point = None;
            self.resurrect_asked = None;
        }

        let threat = targeting::assess(&p.me, &p.monsters);

        // Fuite.
        if self.state == BotState::Fleeing {
            self.think_flee(p, cfg, &threat, &mut out);
            return out;
        }
        if survival::should_flee(&p.me, &threat, cfg) {
            self.enter(BotState::Fleeing, now, "low hp");
            self.stats.flees += 1;
            self.target = None;
            self.nav.clear();
            self.flee_point = None;
            self.flee_clear_since = None;
            out.push(Action::StopAttack);
            self.think_flee(p, cfg, &threat, &mut out);
            return out;
        }

        // Potions : en combat, ou mana à tout moment.
        if let Some(kind) = survival::potion_wanted(&p.me, cfg, self.last_potion, now) {
            if p.me.in_combat || threat.count > 0 || kind == PotionKind::Mp {
                out.push(Action::Potion(kind));
                self.last_potion = Some(now);
            }
        }

        // Cible tuée depuis le tour précédent : comptée tout de suite (le bot peut enchaîner sur une autre).
        let killed = self.target.and_then(|t| p.monster(t)).is_some_and(|m| !m.alive);
        if killed {
            self.stats.kills += 1;
            // Un terrain de chasse : l'exploration se recentre ici.
            self.camp = Some(p.me.pos);
            self.explore_scale = 1.0;
            self.target = None;
        }

        // Butin à portée, sans agresseur : ramassé avant d'engager un nouveau monstre.
        let loot_first = threat.count == 0 && cfg.allow_loot && p.me.free_slots > 0 && self.loot_blocked_until.is_none_or(|t| now >= t) && p.items.iter().any(|i| i.mine);

        // Combat.
        if let Some(t) = if loot_first { None } else { self.choose_target(p, cfg, &threat) } {
            if self.state != BotState::Combat {
                self.enter(BotState::Combat, now, if threat.count > 0 { "attacked" } else { "target" });
                self.nav.clear();
                self.loot_since = None;
            }
            self.combat_tick(p, t, &mut out);
            return out;
        }
        if self.state == BotState::Combat {
            self.target = None;
            self.attack_order = false;
            self.enter(BotState::Idle, now, if killed { "kill" } else { "target lost" });
        }
        if p.me.attacking.is_some() && !self.state.is_dead() {
            // Attaque qui traîne sans cible décidée (cible devenue inattaquable) : on l'arrête.
            out.push(Action::StopAttack);
        }

        // Butin.
        if cfg.allow_loot && p.me.free_slots > 0 && self.loot_blocked_until.is_none_or(|t| now >= t) {
            if let Some(item) = p.items.iter().filter(|i| i.mine).min_by(|a, b| p.me.pos.dist2d(a.pos).total_cmp(&p.me.pos.dist2d(b.pos))) {
                if self.state != BotState::Looting {
                    self.enter(BotState::Looting, now, "loot");
                    self.loot_since = Some(now);
                    self.nav.clear();
                }
                if self.loot_since.is_some_and(|t| now.saturating_duration_since(t) > LOOT_TIMEOUT) {
                    self.loot_blocked_until = Some(now + LOOT_BACKOFF);
                    self.enter(BotState::Idle, now, "loot timeout");
                    return out;
                }
                let d = p.me.pos.dist2d(item.pos);
                if d <= cfg.pickup_radius {
                    if p.me.moving() {
                        out.push(Action::Stop);
                    }
                    out.push(Action::PickUp(item.id));
                    self.stats.loots += 1;
                } else {
                    let spot = p.me.pos.at_distance_from(item.pos, cfg.pickup_radius * 0.5);
                    if p.me.dest.is_none_or(|dst| dst.dist2d(spot) > ARRIVAL_TOLERANCE) {
                        out.push(Action::MoveTo(spot));
                    }
                }
                return out;
            }
        }
        if self.state == BotState::Looting {
            self.enter(BotState::Idle, now, "loot done");
        }

        // Progression, à l'écart du combat.
        if cfg.allow_progression && !p.me.in_combat {
            if self.last_equip.is_none_or(|t| now.saturating_duration_since(t) >= EQUIP_EVERY) {
                self.last_equip = Some(now);
                out.push(Action::EquipUpgrades);
            }
            if p.me.skill_points > 0 && self.last_learn.is_none_or(|t| now.saturating_duration_since(t) >= LEARN_EVERY) {
                self.last_learn = Some(now);
                out.push(Action::LearnSkills);
            }
        }

        // Repos.
        let resting = self.state == BotState::Recovering;
        let master_far = match (&self.mode, &p.master) {
            (Mode::Companion(c), Some(m)) => p.me.pos.dist2d(m.pos) > c.follow_distance * 2.0,
            _ => false,
        };
        if survival::should_rest(&p.me, cfg, resting) && threat.count == 0 && !master_far {
            if !resting {
                self.enter(BotState::Recovering, now, "rest");
                self.nav.clear();
                if p.me.moving() {
                    out.push(Action::Stop);
                }
                out.push(Action::Rest(true));
            }
            return out;
        }
        if resting {
            self.enter(BotState::Idle, now, "rested");
            out.push(Action::Rest(false));
        }

        // Comportement du mode.
        match &self.mode {
            Mode::Companion(_) => self.companion_tick(p, &mut out),
            Mode::Autonomous => self.explore_tick(p, cfg, &mut out),
        }
        out
    }

    fn think_dead(&mut self, cfg: &BotConfig, now: Instant) -> Vec<Action> {
        if !self.state.is_dead() {
            self.enter(BotState::Dead, now, "died");
            self.died_at = Some(now);
            self.stats.deaths += 1;
            self.target = None;
            self.attack_order = false;
            self.nav.clear();
            self.flee_point = None;
            return Vec::new();
        }
        if self.state == BotState::Dead && self.died_at.is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_secs(cfg.resurrect_delay_secs)) {
            self.enter(BotState::Resurrecting, now, "resurrect");
        }
        if self.state == BotState::Resurrecting && self.resurrect_asked.is_none_or(|t| now.saturating_duration_since(t) >= RESURRECT_RETRY) {
            self.resurrect_asked = Some(now);
            return vec![Action::Resurrect];
        }
        Vec::new()
    }

    fn think_flee(&mut self, p: &Perception, cfg: &BotConfig, threat: &targeting::Threat, out: &mut Vec<Action>) {
        let now = p.now;
        if let Some(kind) = survival::potion_wanted(&p.me, cfg, self.last_potion, now) {
            out.push(Action::Potion(kind));
            self.last_potion = Some(now);
        }
        if threat.count == 0 {
            let since = *self.flee_clear_since.get_or_insert(now);
            if now.saturating_duration_since(since) >= FLEE_CLEAR_AFTER {
                self.flee_point = None;
                self.nav.clear();
                if p.me.moving() {
                    out.push(Action::Stop);
                }
                self.enter(BotState::Recovering, now, "escaped");
                out.push(Action::Rest(true));
                return;
            }
        } else {
            self.flee_clear_since = None;
        }
        let threats: Vec<&MonsterView> = p.monsters.iter().filter(|m| m.alive && m.aggro_me).collect();
        let mut fp = self.flee_point.unwrap_or_else(|| survival::flee_point(&p.me, &threats, p.terrain));
        if p.me.pos.dist2d(fp) < ARRIVAL_TOLERANCE {
            fp = survival::flee_point(&p.me, &threats, p.terrain);
        }
        self.flee_point = Some(fp);
        if p.me.dest.is_none_or(|d| d.dist2d(fp) > ARRIVAL_TOLERANCE) {
            out.push(Action::MoveTo(fp));
        }
    }

    /// Cible de ce tour : la cible courante si elle vaut encore, sinon selon les menaces et le mode.
    fn choose_target(&mut self, p: &Perception, cfg: &BotConfig, threat: &targeting::Threat) -> Option<EntityId> {
        if !cfg.allow_combat {
            return None;
        }
        let now = p.now;
        let bl = self.blacklisted();
        let me = &p.me;
        if let Some(t) = self.target {
            if let Some(m) = p.monster(t) {
                let riposte_elsewhere = threat.count > 0 && !m.aggro_me;
                // Compagnon passif : il ne garde une cible que si elle le frappe ou sur ordre.
                let passive_drop = matches!(&self.mode, Mode::Companion(c) if c.stance == Stance::Passive) && !m.aggro_me && !self.attack_order;
                if m.alive && m.attackable && !bl.contains(&t) && !riposte_elsewhere && !passive_drop && me.pos.dist2d(m.pos) <= cfg.scan_radius * 1.5 {
                    // Cible qui ne perd pas de PV depuis trop longtemps (inaccessible ?) : écartée.
                    if m.hp != self.target_hp.0 {
                        self.target_hp = (m.hp, Some(now));
                    } else if self.target_hp.1.is_some_and(|t0| now.saturating_duration_since(t0) > TARGET_TIMEOUT) {
                        self.blacklist(t, now);
                        self.stats.stuck += 1;
                        self.target = None;
                        return None;
                    }
                    return Some(t);
                }
            }
            self.target = None;
        }
        let pick = if threat.count > 0 {
            targeting::most_threatening(me, &p.monsters, &bl)
        } else {
            match &self.mode {
                Mode::Autonomous => {
                    let fit = me.hp_ratio() * 100.0 >= cfg.engage_hp_pct && self.state != BotState::Recovering;
                    if fit {
                        targeting::pick_autonomous(me, &p.monsters, cfg, &bl)
                    } else {
                        None
                    }
                }
                Mode::Companion(c) => {
                    let master = p.master.as_ref();
                    match c.stance {
                        Stance::Passive if self.attack_order => targeting::pick_defensive(master, &p.monsters, &bl),
                        Stance::Passive => None,
                        Stance::Defensive => targeting::pick_defensive(master, &p.monsters, &bl),
                        Stance::Aggressive => targeting::pick_aggressive(me, master, &p.monsters, cfg, &bl, AGGRESSIVE_LEASH),
                    }
                }
            }
        };
        if let Some(t) = pick {
            self.target = Some(t);
            self.target_hp = (p.monster(t).map(|m| m.hp).unwrap_or(0), Some(now));
        }
        pick
    }

    fn combat_tick(&mut self, p: &Perception, t: EntityId, out: &mut Vec<Action>) {
        let me = &p.me;
        let Some(m) = p.monster(t) else { return };
        let dist = me.pos.dist2d(m.pos);
        if !me.casting {
            if let Some(s) = classes::pick_heal_skill(me, &p.skills) {
                out.push(Action::Cast { skill: s.id, target: me.id });
            } else if let Some(s) = classes::pick_offensive_skill(me, &p.skills, dist) {
                out.push(Action::Cast { skill: s.id, target: t });
            }
        }
        let reach = classes::engagement_distance(me.class, me.attack_range) + m.hit_range;
        let keeps_distance = classes::profile(me.class).keeps_distance;
        if keeps_distance && dist > reach + REACH_MARGIN {
            // Classe à distance : se placer au bout de sa portée plutôt que de laisser le serveur l'amener au contact.
            let spot = me.pos.at_distance_from(m.pos, reach * 0.85);
            if me.dest.is_none_or(|d| d.dist2d(spot) > 100.0) {
                out.push(Action::MoveTo(spot));
            }
            return;
        }
        if me.attacking != Some(t) {
            out.push(Action::Attack(t));
        }
    }

    fn companion_tick(&mut self, p: &Perception, out: &mut Vec<Action>) {
        let now = p.now;
        let Mode::Companion(c) = &self.mode else { return };
        let Some(master) = &p.master else {
            // Maître hors de vue : on attend.
            if self.state != BotState::Idle {
                self.enter(BotState::Idle, now, "master away");
            }
            return;
        };
        match c.order {
            Order::Stay(spot) => {
                if p.me.pos.dist2d(spot) > ARRIVAL_TOLERANCE * 2.0 {
                    if p.me.dest.is_none_or(|d| d.dist2d(spot) > ARRIVAL_TOLERANCE) {
                        out.push(Action::MoveTo(spot));
                    }
                } else if self.state != BotState::Idle {
                    self.enter(BotState::Idle, now, "staying");
                }
            }
            Order::Follow => {
                let d = p.me.pos.dist2d(master.pos);
                let (follow_distance, slot) = (c.follow_distance, c.slot);
                if d > follow_distance * 1.5 {
                    if self.state != BotState::Following {
                        self.enter(BotState::Following, now, "follow");
                    }
                    let want = party::follow_point(master.pos, p.me.pos, follow_distance, slot.0, slot.1);
                    if p.me.dest.is_none_or(|dst| dst.dist2d(want) > follow_distance * 0.5) {
                        out.push(Action::MoveTo(want));
                    }
                } else if d <= follow_distance && self.state == BotState::Following {
                    self.enter(BotState::Idle, now, "near master");
                    if p.me.moving() {
                        out.push(Action::Stop);
                    }
                }
            }
        }
    }

    fn explore_tick(&mut self, p: &Perception, cfg: &BotConfig, out: &mut Vec<Action>) {
        let now = p.now;
        if self.state == BotState::Exploring {
            match self.nav.step(p.terrain, p.me.pos, p.me.dest, now) {
                NavStep::Move(wp) => out.push(Action::MoveTo(wp)),
                NavStep::Continue => {}
                NavStep::Arrived | NavStep::Idle => {
                    self.stats.explorations += 1;
                    // Rien à chasser par ici : la prochaine exploration ira plus loin.
                    let bl = self.blacklisted();
                    let prey_here = p.monsters.iter().any(|m| targeting::eligible(&p.me, m, cfg, &bl));
                    if prey_here {
                        self.explore_scale = 1.0;
                    } else if self.camp.is_none() {
                        self.explore_scale = (self.explore_scale * 1.5).min(MAX_EXPLORE_SCALE);
                    }
                    self.idle_until = Some(now + Duration::from_millis(self.rng.gen_range(2000..8000)));
                    self.enter(BotState::Idle, now, "arrived");
                }
                NavStep::Stuck => {
                    self.stats.stuck += 1;
                    if self.nav.replan(p.terrain, p.me.pos, now) {
                        if let NavStep::Move(wp) = self.nav.step(p.terrain, p.me.pos, None, now) {
                            out.push(Action::MoveTo(wp));
                        }
                    } else {
                        out.push(Action::Stop);
                        self.idle_until = Some(now + Duration::from_secs(3));
                        self.enter(BotState::Idle, now, "stuck");
                    }
                }
            }
            return;
        }
        if cfg.explore_radius <= 0.0 || self.idle_until.is_some_and(|t| now < t) {
            if self.state != BotState::Idle {
                self.enter(BotState::Idle, now, "waiting");
            }
            return;
        }
        if let Some(goal) = self.pick_explore_goal(p, cfg) {
            if self.nav.set_goal(p.terrain, p.me.pos, goal, now) {
                self.enter(BotState::Exploring, now, "explore");
                if let NavStep::Move(wp) = self.nav.step(p.terrain, p.me.pos, p.me.dest, now) {
                    out.push(Action::MoveTo(wp));
                }
                return;
            }
        }
        self.idle_until = Some(now + Duration::from_secs(5));
        if self.state != BotState::Idle {
            self.enter(BotState::Idle, now, "no goal");
        }
    }

    /// Point d'exploration : dans le disque d'exploration autour du terrain de chasse (sinon du point d'ancrage,
    /// avec le facteur d'éloignement), hors sol bloqué, à plus de 500 unités d'ici ; huit tirages au plus.
    fn pick_explore_goal(&mut self, p: &Perception, cfg: &BotConfig) -> Option<Pos> {
        // Bot déplacé très loin de son centre d'exploration (téléporté par un MJ, portail, résurrection ailleurs) :
        // un but tiré autour d'un centre à des dizaines de milliers d'unités n'aurait pas de chemin et le bot
        // resterait inactif. Il chasse alors là où il est.
        let center = self.camp.unwrap_or(p.me.home);
        if center.dist2d(p.me.pos) > cfg.explore_radius * MAX_EXPLORE_SCALE * 2.0 {
            self.camp = Some(p.me.pos);
            self.explore_scale = 1.0;
        }
        let (home, scale) = match self.camp {
            Some(c) => (c, 1.0),
            None => (p.me.home, self.explore_scale),
        };
        for _ in 0..8 {
            let angle = self.rng.gen_range(0.0..std::f64::consts::TAU);
            let r = cfg.explore_radius * scale * self.rng.gen_range(0.0f64..1.0).sqrt();
            let x = home.x as f64 + r * angle.cos();
            let z = home.z as f64 + r * angle.sin();
            let goal = Pos::new(x as f32, p.terrain.height(x, z).unwrap_or(p.me.pos.y), z as f32);
            if !p.terrain.is_blocked(x, z) && goal.dist2d(p.me.pos) > 500.0 {
                return Some(goal);
            }
        }
        None
    }

    /// Classe du bot pour les rapports.
    pub fn class_name(p: &Perception) -> &'static str {
        match p.me.class {
            ClassKind::Melee => "melee",
            ClassKind::Ranged => "ranged",
            ClassKind::Caster => "caster",
            ClassKind::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{GroundItemView, OpenTerrain, PlayerView};

    fn perception(now: Instant, me: SelfView) -> Perception<'static> {
        Perception::new(now, me, &OpenTerrain)
    }

    fn mob(id: u32, x: f32, aggro_me: bool) -> MonsterView {
        MonsterView { id, pos: Pos::new(x, 0.0, 0.0), level: 1, aggro_me, ..Default::default() }
    }

    #[test]
    fn riposte_puis_butin_puis_exploration() {
        let cfg = BotConfig::default();
        let mut b = Brain::new(1);
        let t0 = Instant::now();
        let me = SelfView { id: 7, pos: Pos::new(0.0, 0.0, 0.0), home: Pos::new(0.0, 0.0, 0.0), ..Default::default() };
        // Attaqué par un monstre : combat, attaque demandée.
        let mut p = perception(t0, me.clone());
        p.monsters.push(mob(0x5000_0001, 300.0, true));
        let out = b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Combat);
        assert!(out.contains(&Action::Attack(0x5000_0001)), "{out:?}");
        // Déjà en train d'attaquer : rien à redemander.
        let mut p = perception(t0 + Duration::from_secs(1), SelfView { attacking: Some(0x5000_0001), in_combat: true, ..me.clone() });
        p.monsters.push(mob(0x5000_0001, 300.0, true));
        assert!(b.think(&p, &cfg).is_empty());
        // Monstre mort, butin à nous : ramassage.
        let mut p = perception(t0 + Duration::from_secs(2), me.clone());
        p.monsters.push(MonsterView { alive: false, ..mob(0x5000_0001, 300.0, false) });
        p.items.push(GroundItemView { id: 0x3000_0001, pos: Pos::new(350.0, 0.0, 0.0), mine: true, gold: true });
        let out = b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Looting);
        assert_eq!(b.stats.kills, 1);
        assert!(out.contains(&Action::PickUp(0x3000_0001)), "{out:?}");
        // Plus rien : exploration (après l'équipement), un déplacement est demandé.
        let p = perception(t0 + Duration::from_secs(3), me.clone());
        let out = b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Exploring, "{out:?}");
        assert!(out.iter().any(|a| matches!(a, Action::MoveTo(_))) && out.contains(&Action::EquipUpgrades), "{out:?}");
    }

    #[test]
    fn mort_et_resurrection() {
        let cfg = BotConfig { resurrect_delay_secs: 8, ..Default::default() };
        let mut b = Brain::new(2);
        let t0 = Instant::now();
        let dead = SelfView { dead: true, hp: 0, ..Default::default() };
        assert!(b.think(&perception(t0, dead.clone()), &cfg).is_empty());
        assert_eq!((b.state, b.stats.deaths), (BotState::Dead, 1));
        assert!(b.think(&perception(t0 + Duration::from_secs(3), dead.clone()), &cfg).is_empty(), "on attend");
        let out = b.think(&perception(t0 + Duration::from_secs(9), dead.clone()), &cfg);
        assert_eq!((b.state, out), (BotState::Resurrecting, vec![Action::Resurrect]));
        assert!(b.think(&perception(t0 + Duration::from_secs(10), dead.clone()), &cfg).is_empty(), "pas de double demande");
        assert_eq!(b.think(&perception(t0 + Duration::from_secs(15), dead), &cfg), vec![Action::Resurrect], "relance");
        b.think(&perception(t0 + Duration::from_secs(16), SelfView::default()), &cfg);
        assert!(!b.state.is_dead());
    }

    #[test]
    fn fuite_et_repos() {
        let cfg = BotConfig::default();
        let mut b = Brain::new(3);
        let t0 = Instant::now();
        let me = SelfView { hp: 10, hp_max: 100, level: 5, home: Pos::new(3000.0, 0.0, 0.0), ..Default::default() };
        let mut p = perception(t0, me.clone());
        p.monsters.push(MonsterView { level: 5, ..mob(1, 100.0, true) });
        p.monsters.push(MonsterView { level: 5, ..mob(2, 200.0, true) });
        let out = b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Fleeing);
        assert!(out.contains(&Action::StopAttack) && out.contains(&Action::MoveTo(me.home)), "{out:?}");
        // Les monstres lâchent prise : après 3 s calmes, repos.
        let calm = perception(t0 + Duration::from_secs(1), SelfView { pos: Pos::new(2000.0, 0.0, 0.0), ..me.clone() });
        b.think(&calm, &cfg);
        assert_eq!(b.state, BotState::Fleeing);
        let calm = perception(t0 + Duration::from_secs(5), SelfView { pos: Pos::new(2900.0, 0.0, 0.0), ..me.clone() });
        let out = b.think(&calm, &cfg);
        assert_eq!(b.state, BotState::Recovering, "{out:?}");
        assert!(out.contains(&Action::Rest(true)));
        // Repos jusqu'à 85 %.
        b.think(&perception(t0 + Duration::from_secs(20), SelfView { hp: 70, ..me.clone() }), &cfg);
        assert_eq!(b.state, BotState::Recovering);
        let out = b.think(&perception(t0 + Duration::from_secs(40), SelfView { hp: 90, ..me }), &cfg);
        assert_ne!(b.state, BotState::Recovering);
        assert!(out.contains(&Action::Rest(false)));
    }

    #[test]
    fn compagnon_suit_et_obeit() {
        let cfg = BotConfig::default();
        let mut b = Brain::new(4);
        let t0 = Instant::now();
        b.hire(42, "Maitre", 300.0, (0, 1), t0);
        let me = SelfView { id: 7, pos: Pos::new(0.0, 0.0, 0.0), ..Default::default() };
        let master = PlayerView { id: 42, name: "Maitre".into(), pos: Pos::new(2000.0, 0.0, 0.0), ..Default::default() };
        let mut p = perception(t0, me.clone());
        p.master = Some(master.clone());
        let out = b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Following);
        let Some(Action::MoveTo(dest)) = out.iter().find(|a| matches!(a, Action::MoveTo(_))) else { panic!("{out:?}") };
        assert!((dest.dist2d(master.pos) - 300.0).abs() < 1.0);
        // Arrivé près du maître : à l'arrêt.
        let mut p = perception(t0 + Duration::from_secs(2), SelfView { pos: Pos::new(1750.0, 0.0, 0.0), ..me.clone() });
        p.master = Some(master.clone());
        b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Idle);
        // Le maître attaque : en défense, le compagnon prend sa cible.
        let mut p = perception(t0 + Duration::from_secs(3), SelfView { pos: Pos::new(1750.0, 0.0, 0.0), ..me.clone() });
        p.master = Some(PlayerView { attacking: Some(0x5000_0009), ..master.clone() });
        p.monsters.push(mob(0x5000_0009, 2200.0, false));
        let out = b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Combat);
        assert!(out.contains(&Action::Attack(0x5000_0009)));
        // « stay » puis « passive » : n'attaque plus, reste.
        assert!(b.command(&Command::Stance(Stance::Passive), me.pos, t0).contains("passive"));
        assert!(b.command(&Command::Stay, Pos::new(1750.0, 0.0, 0.0), t0).contains("reste"));
        let mut p = perception(t0 + Duration::from_secs(4), SelfView { pos: Pos::new(1750.0, 0.0, 0.0), ..me.clone() });
        p.master = Some(PlayerView { attacking: Some(0x5000_0009), ..master.clone() });
        p.monsters.push(mob(0x5000_0009, 2200.0, false));
        let out = b.think(&p, &cfg);
        assert!(!out.iter().any(|a| matches!(a, Action::Attack(_))), "{out:?}");
        assert_ne!(b.state, BotState::Combat);
        // « attack » force la cible du maître même en passif.
        b.command(&Command::Attack, me.pos, t0);
        let mut p = perception(t0 + Duration::from_secs(5), SelfView { pos: Pos::new(1750.0, 0.0, 0.0), ..me });
        p.master = Some(PlayerView { attacking: Some(0x5000_0009), ..master });
        p.monsters.push(mob(0x5000_0009, 2200.0, false));
        assert!(b.think(&p, &cfg).contains(&Action::Attack(0x5000_0009)));
        assert!(b.status_line().contains("compagnon de Maitre"));
        b.dismiss(t0);
        assert!(!b.is_companion());
    }

    #[test]
    fn exploration_s_eloigne_sans_proie_puis_campe() {
        let cfg = BotConfig { explore_radius: 1000.0, ..Default::default() };
        let mut b = Brain::new(11);
        let t0 = Instant::now();
        let me = SelfView { home: Pos::new(0.0, 0.0, 0.0), ..Default::default() };
        // Exploration sans aucun monstre : le facteur grandit à chaque arrivée.
        let mut t = t0;
        let mut arrivals = 0;
        while arrivals < 3 {
            let goal = b.navigation_goal();
            let pos = goal.unwrap_or(me.pos);
            t += Duration::from_secs(10);
            b.think(&perception(t, SelfView { pos, ..me.clone() }), &cfg);
            if goal.is_some() && b.navigation_goal().is_none() {
                arrivals += 1;
            }
        }
        assert!(b.explore_scale() > 1.5, "facteur {}", b.explore_scale());
        // Un monstre tué : le terrain de chasse devient le centre.
        let mut p = perception(t + Duration::from_secs(20), SelfView { pos: Pos::new(5000.0, 0.0, 0.0), ..me.clone() });
        p.monsters.push(mob(1, 5200.0, true));
        b.think(&p, &cfg);
        assert_eq!(b.state, BotState::Combat);
        let mut p = perception(t + Duration::from_secs(21), SelfView { pos: Pos::new(5000.0, 0.0, 0.0), ..me });
        p.monsters.push(MonsterView { alive: false, ..mob(1, 5200.0, false) });
        b.think(&p, &cfg);
        assert_eq!(b.camp(), Some(Pos::new(5000.0, 0.0, 0.0)));
        assert_eq!(b.explore_scale(), 1.0);
    }

    #[test]
    fn classe_a_distance_se_place() {
        let cfg = BotConfig::default();
        let mut b = Brain::new(5);
        let t0 = Instant::now();
        let me = SelfView { class: ClassKind::Ranged, attack_range: 1000.0, ..Default::default() };
        let mut p = perception(t0, me);
        p.monsters.push(mob(1, 3000.0, true));
        let out = b.think(&p, &cfg);
        // Trop loin : se place à ~85 % de (900 + 100), sans encore attaquer.
        let Some(Action::MoveTo(spot)) = out.iter().find(|a| matches!(a, Action::MoveTo(_))) else { panic!("{out:?}") };
        assert!((spot.dist2d(Pos::new(3000.0, 0.0, 0.0)) - 850.0).abs() < 1.0, "{spot:?}");
        assert!(!out.iter().any(|a| matches!(a, Action::Attack(_))));
    }

    #[test]
    fn cible_inaccessible_ecartee() {
        let cfg = BotConfig::default();
        let mut b = Brain::new(6);
        let t0 = Instant::now();
        let me = SelfView { level: 1, ..Default::default() };
        for s in 0..40 {
            let mut p = perception(t0 + Duration::from_secs(s), SelfView { attacking: Some(1), ..me.clone() });
            p.monsters.push(mob(1, 2000.0, false));
            b.think(&p, &cfg);
        }
        assert_eq!(b.stats.stuck, 1, "PV inchangés 25 s : écartée");
        let mut p = perception(t0 + Duration::from_secs(41), me);
        p.monsters.push(mob(1, 2000.0, false));
        b.think(&p, &cfg);
        assert_ne!(b.target(), Some(1));
    }

    #[test]
    fn progression_en_64_bits() {
        // Courbe de Webzen : au-delà du niveau 100, l'expérience dépasse 2³² ; gains et montées lus par différence.
        let cfg = BotConfig::default();
        let mut b = Brain::new(7);
        let t0 = Instant::now();
        let base = 600_000_000_000u64;
        let me = SelfView { level: 119, exp: 5_000_000_000, exp_next: 9_000_000_000, exp_total: base, ..Default::default() };
        b.think(&perception(t0, me.clone()), &cfg);
        assert_eq!((b.stats.level_ups, b.stats.exp_gained), (0, 0), "premier tour : référence seulement");
        // 5 000 000 000 points d'un coup, passage au niveau 120.
        b.think(&perception(t0 + Duration::from_secs(1), SelfView { level: 120, exp: 1_000_000_000, exp_next: 0, exp_total: base + 5_000_000_000, ..me.clone() }), &cfg);
        assert_eq!((b.stats.level_ups, b.stats.exp_gained), (1, 5_000_000_000));
        // Pénalité de mort : perte comptée à part.
        b.think(&perception(t0 + Duration::from_secs(2), SelfView { level: 120, exp: 400_000_000, exp_total: base + 4_400_000_000, ..me }), &cfg);
        assert_eq!((b.stats.exp_gained, b.stats.exp_lost), (5_000_000_000, 600_000_000));
        assert!(b.status_line().contains("5000000000 point(s) d'expérience"), "{}", b.status_line());
    }

    #[test]
    fn deplace_loin_du_point_d_ancrage() {
        // Point d'ancrage dans un autre village, à un million d'unités : l'exploration repart d'ici.
        let cfg = BotConfig::default();
        let mut b = Brain::new(8);
        let t0 = Instant::now();
        let me = SelfView { pos: Pos::new(-475_000.0, 0.0, -66_000.0), home: Pos::new(517_000.0, 0.0, 38_000.0), ..Default::default() };
        let out = b.think(&perception(t0, me.clone()), &cfg);
        let Some(Action::MoveTo(wp)) = out.iter().find(|a| matches!(a, Action::MoveTo(_))) else { panic!("{out:?} ; {}", b.status_line()) };
        assert!(wp.dist2d(me.pos) <= cfg.explore_radius + 1.0, "{wp:?}");
        assert_eq!(b.camp(), Some(me.pos));
    }
}
