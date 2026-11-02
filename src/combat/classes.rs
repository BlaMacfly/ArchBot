//! Stratégies par classe.
//!
//! ArchLord n'a pas de rotations complexes : chaque classe a une attaque normale et des compétences actives
//! aux portées, coûts et relances propres (SKILL_SPEC / SKILL_CONST, lues par le serveur). Le moteur ne
//! connaît que les compétences **réellement apprises** par le bot ([`SkillView`]), avec leurs valeurs réelles ;
//! il n'en invente aucune. La seule différence de classe est la distance d'engagement : un archer ou un
//! lanceur se tient au bout de sa portée au lieu d'aller au contact.

use crate::world::{ClassKind, SelfView, SkillView};

/// Profil de combat d'une classe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassProfile {
    pub kind: ClassKind,
    /// Se tient à distance plutôt qu'au contact.
    pub keeps_distance: bool,
    /// Se soigne (compétence de soin) sous cette part de PV.
    pub heal_below: f64,
    /// Garde cette part de PM en réserve (pas de compétence offensive en dessous).
    pub mp_reserve: f64,
}

pub fn profile(kind: ClassKind) -> ClassProfile {
    match kind {
        ClassKind::Melee | ClassKind::Unknown => ClassProfile { kind, keeps_distance: false, heal_below: 0.5, mp_reserve: 0.0 },
        ClassKind::Ranged => ClassProfile { kind, keeps_distance: true, heal_below: 0.5, mp_reserve: 0.0 },
        ClassKind::Caster => ClassProfile { kind, keeps_distance: true, heal_below: 0.6, mp_reserve: 0.1 },
    }
}

/// Distance à laquelle le bot cherche à se tenir de sa cible (hors rayon du corps de celle-ci) : la portée
/// d'attaque, moins une marge pour les classes à distance (la cible avance pendant l'approche).
pub fn engagement_distance(kind: ClassKind, attack_range: f64) -> f64 {
    let range = attack_range.max(50.0);
    if profile(kind).keeps_distance {
        range * 0.9
    } else {
        range
    }
}

fn affordable(me: &SelfView, s: &SkillView, reserve: f64) -> bool {
    s.ready && s.cost_mp <= me.mp - (me.mp_max as f64 * reserve) as i32
}

/// Compétence offensive à lancer sur une cible à `distance` : prête, payable, à portée (0 : portée d'attaque),
/// la plus haute de niveau puis la plus coûteuse (généralement la plus forte).
pub fn pick_offensive_skill<'a>(me: &SelfView, skills: &'a [SkillView], distance: f64) -> Option<&'a SkillView> {
    let reserve = profile(me.class).mp_reserve;
    skills
        .iter()
        .filter(|s| s.offensive && !s.on_self && affordable(me, s, reserve))
        .filter(|s| {
            let range = if s.range > 0.0 { s.range } else { me.attack_range.max(50.0) };
            distance <= range + 100.0
        })
        .max_by_key(|s| (s.level, s.cost_mp))
}

/// Compétence de soin sur soi, si les PV sont sous le seuil du profil.
pub fn pick_heal_skill<'a>(me: &SelfView, skills: &'a [SkillView]) -> Option<&'a SkillView> {
    if me.hp_ratio() >= profile(me.class).heal_below {
        return None;
    }
    skills.iter().filter(|s| s.heals && !s.offensive && affordable(me, s, 0.0)).max_by_key(|s| s.level)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: u32, level: u32, cost: i32, range: f64) -> SkillView {
        SkillView { id, tid: id, level, offensive: true, ready: true, needs_target: true, cost_mp: cost, range, ..Default::default() }
    }

    #[test]
    fn distances() {
        assert_eq!(engagement_distance(ClassKind::Melee, 80.0), 80.0);
        assert_eq!(engagement_distance(ClassKind::Melee, 0.0), 50.0);
        assert_eq!(engagement_distance(ClassKind::Ranged, 1000.0), 900.0);
        assert!(profile(ClassKind::Caster).keeps_distance && !profile(ClassKind::Unknown).keeps_distance);
    }

    #[test]
    fn choix_de_competence() {
        let me = SelfView { mp: 30, mp_max: 100, attack_range: 100.0, class: ClassKind::Melee, ..Default::default() };
        let skills = vec![skill(1, 1, 10, 0.0), skill(2, 3, 25, 0.0), skill(3, 5, 50, 0.0), SkillView { ready: false, ..skill(4, 9, 5, 0.0) }, skill(5, 2, 5, 2000.0)];
        assert_eq!(pick_offensive_skill(&me, &skills, 120.0).map(|s| s.id), Some(2), "la plus haute payable à portée");
        assert_eq!(pick_offensive_skill(&me, &skills, 1500.0).map(|s| s.id), Some(5), "seule à longue portée");
        assert_eq!(pick_offensive_skill(&SelfView { mp: 0, ..me.clone() }, &skills, 120.0), None);
        // Lanceur : réserve de 10 % de PM.
        let caster = SelfView { class: ClassKind::Caster, mp: 12, mp_max: 100, ..me.clone() };
        assert_eq!(pick_offensive_skill(&caster, &skills, 120.0), None, "12 - 10 < 10");
        // Soin.
        let heal = SkillView { heals: true, offensive: false, on_self: true, ..skill(9, 1, 10, 0.0) };
        let hurt = SelfView { hp: 30, hp_max: 100, ..me.clone() };
        assert_eq!(pick_heal_skill(&hurt, std::slice::from_ref(&heal)).map(|s| s.id), Some(9));
        assert_eq!(pick_heal_skill(&me, &[heal]), None, "PV pleins");
    }
}
