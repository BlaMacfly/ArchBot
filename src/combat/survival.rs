//! Survie : fuite, potions, repos.
//!
//! Faits d'ArchCore utilisés : hors combat (10 s sans coup), un personnage régénère 1,5 % de ses PV et PM
//! maximum + taux + 5 toutes les 10 s ; une potion de vie et une potion de mana ont chacune leur délai ;
//! un monstre abandonne la poursuite au-delà de 5000 unités et oublie ses agresseurs 30 s après le dernier
//! coup ; un village (région sûre) est le refuge naturel.

use super::targeting::Threat;
use crate::config::BotConfig;
use crate::world::{MonsterView, Pos, PotionKind, SelfView, Terrain};
use std::time::{Duration, Instant};

/// Délai minimal entre deux potions demandées (le serveur a aussi le sien).
pub const POTION_COOLDOWN: Duration = Duration::from_millis(2500);
/// Distance de fuite par bond.
pub const FLEE_LEG: f64 = 3500.0;
/// Au-delà, le refuge est trop loin : on fuit à l'opposé des menaces.
pub const HOME_REACH: f64 = 8000.0;

/// Fuir : PV sous le seuil, sans invincibilité, et la situation ne se retourne pas (plus de potion, plusieurs
/// agresseurs, ou un agresseur nettement plus haut).
pub fn should_flee(me: &SelfView, threat: &Threat, cfg: &BotConfig) -> bool {
    if me.invincible || threat.count == 0 || cfg.flee_hp_pct <= 0.0 {
        return false;
    }
    me.hp_ratio() * 100.0 < cfg.flee_hp_pct && (me.hp_potions == 0 || threat.count >= 2 || threat.max_level > me.level + 2)
}

/// Potion à boire maintenant : vie d'abord, mana ensuite (lanceurs), si le délai est passé.
pub fn potion_wanted(me: &SelfView, cfg: &BotConfig, last: Option<Instant>, now: Instant) -> Option<PotionKind> {
    if last.is_some_and(|t| now.saturating_duration_since(t) < POTION_COOLDOWN) {
        return None;
    }
    if me.hp_potions > 0 && me.hp_ratio() * 100.0 < cfg.potion_hp_pct {
        return Some(PotionKind::Hp);
    }
    if me.mp_potions > 0 && me.mp_max > 0 && me.mp_ratio() * 100.0 < cfg.potion_mp_pct {
        return Some(PotionKind::Mp);
    }
    None
}

/// Se reposer : PV sous le seuil de repos ; déjà au repos, on y reste jusqu'au seuil de reprise.
pub fn should_rest(me: &SelfView, cfg: &BotConfig, resting: bool) -> bool {
    let pct = me.hp_ratio() * 100.0;
    if resting {
        pct < cfg.rest_until_pct
    } else {
        pct < cfg.rest_hp_pct
    }
}

/// Point de fuite : le refuge s'il est à portée, sinon un bond à l'opposé du barycentre des menaces, en
/// essayant plusieurs angles pour éviter le sol bloqué.
pub fn flee_point(me: &SelfView, threats: &[&MonsterView], terrain: &dyn Terrain) -> Pos {
    if me.home.dist2d(me.pos) <= HOME_REACH && me.home.dist2d(me.pos) > 100.0 {
        return me.home;
    }
    let (mut cx, mut cz) = (0.0f64, 0.0f64);
    for m in threats {
        cx += m.pos.x as f64;
        cz += m.pos.z as f64;
    }
    let n = threats.len().max(1) as f64;
    let (cx, cz) = (cx / n, cz / n);
    let (mut dx, mut dz) = (me.pos.x as f64 - cx, me.pos.z as f64 - cz);
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1.0 {
        dx = 1.0;
        dz = 0.0;
    } else {
        dx /= len;
        dz /= len;
    }
    for angle in [0.0f64, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5, std::f64::consts::PI] {
        let (s, c) = angle.sin_cos();
        let (rx, rz) = (dx * c - dz * s, dx * s + dz * c);
        let x = me.pos.x as f64 + rx * FLEE_LEG;
        let z = me.pos.z as f64 + rz * FLEE_LEG;
        if !terrain.is_blocked(x, z) {
            let y = terrain.height(x, z).unwrap_or(me.pos.y);
            return Pos::new(x as f32, y, z as f32);
        }
    }
    me.home
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::OpenTerrain;

    #[test]
    fn fuite() {
        let cfg = BotConfig::default();
        let low = SelfView { hp: 15, hp_max: 100, level: 10, hp_potions: 0, ..Default::default() };
        let one = Threat { count: 1, max_level: 10, nearest: 100.0 };
        assert!(should_flee(&low, &one, &cfg), "sans potion");
        assert!(!should_flee(&SelfView { hp_potions: 2, ..low.clone() }, &one, &cfg), "une potion peut suffire");
        assert!(should_flee(&SelfView { hp_potions: 2, ..low.clone() }, &Threat { count: 2, ..one }, &cfg), "deux agresseurs");
        assert!(should_flee(&SelfView { hp_potions: 2, ..low.clone() }, &Threat { max_level: 13, ..one }, &cfg), "trop haut");
        assert!(!should_flee(&SelfView { invincible: true, ..low.clone() }, &one, &cfg));
        assert!(!should_flee(&low, &Threat::default(), &cfg), "personne");
        assert!(!should_flee(&SelfView { hp: 50, ..low }, &one, &cfg));
    }

    #[test]
    fn potions_et_repos() {
        let cfg = BotConfig::default();
        let t0 = Instant::now();
        let me = SelfView { hp: 30, hp_max: 100, mp: 5, mp_max: 100, hp_potions: 1, mp_potions: 1, ..Default::default() };
        assert_eq!(potion_wanted(&me, &cfg, None, t0), Some(PotionKind::Hp));
        assert_eq!(potion_wanted(&me, &cfg, Some(t0), t0 + Duration::from_secs(1)), None, "délai");
        assert_eq!(potion_wanted(&SelfView { hp: 90, ..me.clone() }, &cfg, None, t0), Some(PotionKind::Mp));
        assert_eq!(potion_wanted(&SelfView { hp_potions: 0, mp_potions: 0, ..me.clone() }, &cfg, None, t0), None);
        assert!(should_rest(&me, &cfg, false));
        assert!(should_rest(&SelfView { hp: 70, ..me.clone() }, &cfg, true), "au repos : jusqu'à 85 %");
        assert!(!should_rest(&SelfView { hp: 70, ..me.clone() }, &cfg, false));
        assert!(!should_rest(&SelfView { hp: 90, ..me }, &cfg, true));
    }

    #[test]
    fn point_de_fuite() {
        let me = SelfView { pos: Pos::new(0.0, 0.0, 0.0), home: Pos::new(4000.0, 0.0, 0.0), ..Default::default() };
        assert_eq!(flee_point(&me, &[], &OpenTerrain), me.home, "refuge à portée");
        let far = SelfView { home: Pos::new(100_000.0, 0.0, 0.0), ..me };
        let mob = MonsterView { pos: Pos::new(1000.0, 0.0, 0.0), ..Default::default() };
        let p = flee_point(&far, &[&mob], &OpenTerrain);
        assert!(p.x < -3000.0 && p.z.abs() < 1.0, "à l'opposé : {p:?}");
    }
}
