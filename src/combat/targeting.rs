//! Choix de la cible.
//!
//! Règles d'ArchLord retenues (ArchCore) : un monstre ne s'en prend qu'à ceux qui l'ont frappé (sauf les
//! monstres « préventifs »), poursuit jusqu'à 5000 unités et retient ses agresseurs 30 s ; le butin
//! appartient 30 s au tueur. Un bot n'attaque donc pas un monstre déjà engagé par un autre joueur (hors
//! groupe), choisit des monstres de niveau proche du sien, et riposte toujours à qui le frappe.

use crate::config::BotConfig;
use crate::world::{EntityId, MonsterView, PlayerView, SelfView};

/// Résumé des menaces : monstres vivants qui ont le bot parmi leurs agresseurs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Threat {
    pub count: usize,
    pub max_level: i32,
    /// Distance du plus proche.
    pub nearest: f64,
}

pub fn assess(me: &SelfView, monsters: &[MonsterView]) -> Threat {
    let mut t = Threat { count: 0, max_level: 0, nearest: f64::INFINITY };
    for m in monsters.iter().filter(|m| m.alive && m.aggro_me) {
        t.count += 1;
        t.max_level = t.max_level.max(m.level);
        t.nearest = t.nearest.min(me.pos.dist2d(m.pos));
    }
    t
}

/// Le monstre peut être pris pour cible par un bot autonome : vivant, attaquable, dans la fenêtre de niveau,
/// pas déjà engagé par quelqu'un d'autre (sauf le groupe), à portée de perception, non écarté.
pub fn eligible(me: &SelfView, m: &MonsterView, cfg: &BotConfig, blacklist: &[EntityId]) -> bool {
    m.alive
        && m.attackable
        && !blacklist.contains(&m.id)
        && m.level >= me.level - cfg.target_level_below
        && m.level <= me.level + cfg.target_level_above
        && (!m.aggro_others || m.aggro_party || m.aggro_me)
        && me.pos.dist2d(m.pos) <= cfg.scan_radius
}

/// Parmi les agresseurs : le plus proche, à PV égaux le plus faible (il tombe plus vite).
pub fn most_threatening(me: &SelfView, monsters: &[MonsterView], blacklist: &[EntityId]) -> Option<EntityId> {
    monsters
        .iter()
        .filter(|m| m.alive && m.attackable && m.aggro_me && !blacklist.contains(&m.id))
        .min_by(|a, b| {
            let (da, db) = (me.pos.dist2d(a.pos), me.pos.dist2d(b.pos));
            da.total_cmp(&db).then(a.hp.cmp(&b.hp))
        })
        .map(|m| m.id)
}

/// Cible d'un bot autonome : d'abord les monstres qui attaquent le groupe, sinon le plus proche des
/// éligibles, un écart de niveau coûtant 400 unités par niveau.
pub fn pick_autonomous(me: &SelfView, monsters: &[MonsterView], cfg: &BotConfig, blacklist: &[EntityId]) -> Option<EntityId> {
    let score = |m: &MonsterView| me.pos.dist2d(m.pos) + 400.0 * (m.level - me.level).unsigned_abs() as f64 - if m.aggro_party { 5000.0 } else { 0.0 };
    monsters.iter().filter(|m| eligible(me, m, cfg, blacklist)).min_by(|a, b| score(a).total_cmp(&score(b))).map(|m| m.id)
}

/// Cible d'un compagnon en défense : qui attaque le maître (le plus proche du maître), sinon la cible que le
/// maître attaque, si elle est un monstre vivant et attaquable.
pub fn pick_defensive(master: Option<&PlayerView>, monsters: &[MonsterView], blacklist: &[EntityId]) -> Option<EntityId> {
    let master = master?;
    if let Some(m) = monsters
        .iter()
        .filter(|m| m.alive && m.attackable && (m.aggro_master || m.aggro_party) && !blacklist.contains(&m.id))
        .min_by(|a, b| master.pos.dist2d(a.pos).total_cmp(&master.pos.dist2d(b.pos)))
    {
        return Some(m.id);
    }
    let t = master.attacking?;
    monsters.iter().find(|m| m.id == t && m.alive && m.attackable && !blacklist.contains(&m.id)).map(|m| m.id)
}

/// Cible d'un compagnon offensif : comme en défense, sinon un monstre éligible à moins de `leash` du maître.
pub fn pick_aggressive(me: &SelfView, master: Option<&PlayerView>, monsters: &[MonsterView], cfg: &BotConfig, blacklist: &[EntityId], leash: f64) -> Option<EntityId> {
    if let Some(t) = pick_defensive(master, monsters, blacklist) {
        return Some(t);
    }
    let anchor = master.map(|m| m.pos).unwrap_or(me.pos);
    monsters
        .iter()
        .filter(|m| eligible(me, m, cfg, blacklist) && anchor.dist2d(m.pos) <= leash)
        .min_by(|a, b| me.pos.dist2d(a.pos).total_cmp(&me.pos.dist2d(b.pos)))
        .map(|m| m.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Pos;

    fn mob(id: u32, x: f32, level: i32) -> MonsterView {
        MonsterView { id, pos: Pos::new(x, 0.0, 0.0), level, ..Default::default() }
    }

    #[test]
    fn eligibilite_et_choix() {
        let cfg = BotConfig::default();
        let me = SelfView { level: 10, ..Default::default() };
        let mobs = vec![
            mob(1, 3000.0, 10),
            mob(2, 1000.0, 30),                                          // trop haut
            mob(3, 1500.0, 2),                                           // trop bas (10 - 6 = 4)
            MonsterView { aggro_others: true, ..mob(4, 500.0, 10) },     // pris par un autre joueur
            MonsterView { alive: false, ..mob(5, 100.0, 10) },           // mort
            MonsterView { attackable: false, ..mob(6, 100.0, 10) },      // pas attaquable
            mob(7, 2000.0, 12),
        ];
        assert!(eligible(&me, &mobs[0], &cfg, &[]));
        assert!(!eligible(&me, &mobs[1], &cfg, &[]) && !eligible(&me, &mobs[2], &cfg, &[]));
        assert!(!eligible(&me, &mobs[3], &cfg, &[]) && !eligible(&me, &mobs[4], &cfg, &[]) && !eligible(&me, &mobs[5], &cfg, &[]));
        // 7 : 2000 + 800 = 2800 < 3000 : préféré malgré l'écart de niveau.
        assert_eq!(pick_autonomous(&me, &mobs, &cfg, &[]), Some(7));
        assert_eq!(pick_autonomous(&me, &mobs, &cfg, &[7]), Some(1), "écarté : le suivant");
        // Un monstre qui attaque le groupe passe devant.
        let mut mobs2 = mobs.clone();
        mobs2[0].aggro_party = true;
        mobs2[0].aggro_others = true;
        assert_eq!(pick_autonomous(&me, &mobs2, &cfg, &[]), Some(1));
    }

    #[test]
    fn menaces() {
        let me = SelfView { level: 5, ..Default::default() };
        let mobs = vec![MonsterView { aggro_me: true, ..mob(1, 800.0, 7) }, MonsterView { aggro_me: true, hp: 10, ..mob(2, 800.0, 3) }, mob(3, 100.0, 5)];
        let t = assess(&me, &mobs);
        assert_eq!((t.count, t.max_level, t.nearest), (2, 7, 800.0));
        assert_eq!(most_threatening(&me, &mobs, &[]), Some(2), "même distance : le plus faible");
        assert_eq!(assess(&me, &[]).count, 0);
    }

    #[test]
    fn compagnon() {
        let cfg = BotConfig::default();
        let me = SelfView { level: 10, pos: Pos::new(0.0, 0.0, 0.0), ..Default::default() };
        let master = PlayerView { id: 42, pos: Pos::new(1000.0, 0.0, 0.0), attacking: Some(9), ..Default::default() };
        let mobs = vec![mob(9, 1200.0, 10), MonsterView { aggro_master: true, ..mob(8, 2000.0, 10) }, mob(7, 1300.0, 10)];
        assert_eq!(pick_defensive(Some(&master), &mobs, &[]), Some(8), "qui attaque le maître d'abord");
        let calm: Vec<MonsterView> = mobs.iter().cloned().map(|mut m| { m.aggro_master = false; m }).collect();
        assert_eq!(pick_defensive(Some(&master), &calm, &[]), Some(9), "sinon la cible du maître");
        assert_eq!(pick_defensive(None, &mobs, &[]), None);
        let idle = PlayerView { attacking: None, ..master.clone() };
        assert_eq!(pick_defensive(Some(&idle), &calm, &[]), None);
        assert_eq!(pick_aggressive(&me, Some(&idle), &calm, &cfg, &[], 1500.0), Some(9), "offensif : le plus proche du bot près du maître");
        assert_eq!(pick_aggressive(&me, Some(&idle), &calm, &cfg, &[], 100.0), None, "rien dans la laisse");
    }
}
