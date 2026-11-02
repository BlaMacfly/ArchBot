//! État du monde → [`Perception`] du moteur. Tout vient de `WorldState` et de `WorldCtx` : positions que le
//! serveur tient, agresseurs réels des monstres, objets au sol et leur propriétaire, compétences apprises
//! avec les valeurs de SKILL_CONST.

use crate::world::{skills, WorldCtx, WorldState};
use archbot::world::{GroundItemView, MonsterView, PartyView, Perception, PlayerView, Pos, SelfView, SkillView, Terrain};
use archbot::{BotConfig, ClassKind};
use archcore_net::Vec3;
use std::time::Instant;

/// Terrain du serveur : carte des segments (sol infranchissable, hauteur) et régions sûres.
pub struct MapTerrain<'a> {
    pub ctx: &'a WorldCtx,
}

impl Terrain for MapTerrain<'_> {
    fn is_blocked(&self, x: f64, z: f64) -> bool {
        self.ctx.map.is_blocked(x, z)
    }
    fn height(&self, x: f64, z: f64) -> Option<f32> {
        self.ctx.map.height(x, z)
    }
    fn is_safe(&self, x: f64, z: f64) -> bool {
        self.ctx.map.region(x, z).is_some_and(|r| self.ctx.regions.is_safe(r))
    }
    fn cell_size(&self) -> f64 {
        crate::world::map::SEGMENT_SIZE
    }
}

pub fn to_pos(v: Vec3) -> Pos {
    Pos::new(v.x, v.y, v.z)
}

pub fn to_vec3(p: Pos) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

/// Potions de vie et de mana de l'inventaire ordinaire (exemplaires).
pub fn potions(ctx: &WorldCtx, p: &crate::world::Player) -> (u32, u32) {
    let (mut hp, mut mp) = (0u32, 0u32);
    for it in p.inventory.items.values() {
        if let Some(potion) = ctx.catalog.get(it.tid).and_then(|d| d.potion) {
            if potion.hp > 0 {
                hp += it.count.max(0) as u32;
            } else if potion.mp > 0 {
                mp += it.count.max(0) as u32;
            }
        }
    }
    (hp, mp)
}

/// Perception du bot `sid` ; `None` s'il n'est plus en jeu. `home` : point d'ancrage ; `master` : CID du maître
/// d'un compagnon.
pub fn build<'a>(ctx: &'a WorldCtx, st: &WorldState, sid: u32, home: Pos, master: Option<u32>, cfg: &BotConfig, now: Instant, terrain: &'a MapTerrain<'a>) -> Option<Perception<'a>> {
    let p = st.players.get(&sid).filter(|p| p.in_game)?;
    let cid = p.ch.id;
    let pos = p.ch.pos();
    let (hp_potions, mp_potions) = potions(ctx, p);
    let party = st.parties.party_of(cid).map(|pt| PartyView { leader: pt.members[0], members: pt.members.clone() });
    let party_members: Vec<u32> = party.as_ref().map(|pt| pt.members.clone()).unwrap_or_default();
    let f = &p.factors;
    let me = SelfView {
        id: cid,
        name: p.ch.name.clone(),
        pos: to_pos(pos),
        dest: p.dest.map(to_pos),
        hp: f.point[crate::character::POINT_HP],
        hp_max: f.point_max[0],
        mp: f.point[1],
        mp_max: f.point_max[1],
        level: p.ch.level,
        dead: p.dead,
        invincible: p.invincible_until.is_some_and(|t| now < t),
        in_combat: now < p.timers.combat_until,
        attacking: p.attack.map(|a| a.target),
        casting: p.skills.cast.is_some(),
        class: ClassKind::from_archlord(f.ftype[2]),
        attack_range: f.attack[0].max(50) as f64,
        run_speed: f.status[9].max(1) as f64 / 10.0,
        hp_potions,
        mp_potions,
        free_slots: p.inventory.free_count() as u32,
        party,
        home,
        money: p.ch.money,
        skill_points: f.dirt[skills::DIRT_SKILL_POINT],
    };
    let mut out = Perception::new(now, me, terrain);
    // Monstres visibles (vivants ou cadavres) à portée de perception, avec leurs agresseurs réels.
    for m in st.monsters.near(pos, cfg.scan_radius).filter(|m| m.is_visible()) {
        let (mut aggro_me, mut aggro_master, mut aggro_party, mut aggro_others) = (false, false, false, false);
        for a in &m.aggro {
            if a.cid == cid {
                aggro_me = true;
            } else if Some(a.cid) == master {
                aggro_master = true;
            } else if party_members.contains(&a.cid) {
                aggro_party = true;
            } else {
                aggro_others = true;
            }
        }
        out.monsters.push(MonsterView {
            id: m.cid,
            pos: to_pos(m.pos),
            level: m.level as i32,
            hp: m.hp,
            hp_max: m.max_hp,
            alive: m.is_alive(),
            attackable: st.monsters.attackable(m),
            aggro_me,
            aggro_master,
            aggro_party,
            aggro_others,
            hit_range: st.monsters.stats_for(m).hit_range as f64,
        });
    }
    // Joueurs (vrais et bots) à portée ; le maître à toute distance.
    let far = cfg.far_from_players_units;
    let mut real_player_near = false;
    for (osid, o) in st.players.iter().filter(|(osid, o)| **osid != sid && o.in_game) {
        let d = crate::world::combat::dist2d(o.ch.pos(), pos);
        if !o.sess.is_detached() && d <= far {
            real_player_near = true;
        }
        let view = PlayerView { id: o.ch.id, name: o.ch.name.clone(), pos: to_pos(o.ch.pos()), level: o.ch.level, dead: o.dead, attacking: o.attack.map(|a| a.target), is_bot: o.sess.is_detached() };
        if Some(o.ch.id) == master {
            out.master = Some(view.clone());
        }
        if d <= cfg.scan_radius {
            out.players.push(view);
        }
        let _ = osid;
    }
    out.real_player_near = real_player_near;
    // Objets au sol : à portée, avec le droit de les ramasser maintenant.
    for g in st.ground.items.values() {
        if crate::world::combat::dist2d(g.pos, pos) <= cfg.scan_radius {
            out.items.push(GroundItemView { id: g.iid, pos: to_pos(g.pos), mine: g.owner == cid || now >= g.owner_until, gold: g.tid == crate::world::loot::GOLD_TID });
        }
    }
    // Compétences actives connues, avec les valeurs de leur niveau.
    for o in &p.skills.list {
        let Some(t) = ctx.skills.get(o.tid) else { continue };
        if t.auto_attack || t.nature().passive || t.is_product() || t.is_evolution() || t.archlord_only() || t.is_mutation() {
            continue;
        }
        let Some(lv) = t.level(o.level) else { continue };
        out.skills.push(SkillView {
            id: o.id,
            tid: o.tid,
            name: t.name.clone(),
            level: o.level,
            offensive: t.offensive(),
            heals: !lv.heal.is_empty(),
            on_self: t.on_self(),
            needs_target: t.needs_target(),
            ready: o.ready_at.is_none_or(|r| now >= r),
            range: lv.range,
            cost_mp: lv.cost_mp,
            cast_ms: lv.cast_ms,
        });
    }
    Some(out)
}
