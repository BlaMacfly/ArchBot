//! Groupe : rôles et formation des compagnons.
//!
//! Les règles de groupe sont celles d'ArchCore (5 membres au plus, chef en tête, partage de l'expérience par
//! dégâts ou par niveau, visibilité mutuelle) ; le moteur ne fait que placer les compagnons autour du maître
//! et choisir un rôle d'après la classe.

use crate::world::{ClassKind, Pos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Au contact, prend les coups.
    Tank,
    /// Dégâts à distance.
    Damage,
    /// Lanceur : dégâts, soins quand il en a.
    Support,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Tank => "tank",
            Role::Damage => "damage",
            Role::Support => "support",
        }
    }
}

pub fn role_of(kind: ClassKind) -> Role {
    match kind {
        ClassKind::Melee | ClassKind::Unknown => Role::Tank,
        ClassKind::Ranged => Role::Damage,
        ClassKind::Caster => Role::Support,
    }
}

/// Décalage (dx, dz) du compagnon `index` parmi `count`, en arc derrière le maître (relativement à la
/// direction maître → bot), pour que plusieurs compagnons ne se superposent pas.
pub fn formation_offset(index: usize, count: usize, distance: f64) -> (f64, f64) {
    let count = count.max(1);
    let spread = std::f64::consts::FRAC_PI_2; // arc de 90°
    let angle = if count == 1 { 0.0 } else { -spread / 2.0 + spread * index as f64 / (count - 1) as f64 };
    (distance * angle.sin(), distance * angle.cos())
}

/// Point de suivi : à `distance` du maître, du côté du bot, décalé selon sa place dans la formation.
pub fn follow_point(master: Pos, me: Pos, distance: f64, index: usize, count: usize) -> Pos {
    let (mut dx, mut dz) = ((me.x - master.x) as f64, (me.z - master.z) as f64);
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1.0 {
        dx = 0.0;
        dz = 1.0;
    } else {
        dx /= len;
        dz /= len;
    }
    let (ox, oz) = formation_offset(index, count, distance);
    // Base (dx, dz) et sa perpendiculaire (-dz, dx).
    let x = master.x as f64 + dz * -ox + dx * oz;
    let z = master.z as f64 + dx * ox + dz * oz;
    Pos::new(x as f32, master.y, z as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles() {
        assert_eq!(role_of(ClassKind::Melee), Role::Tank);
        assert_eq!(role_of(ClassKind::Ranged), Role::Damage);
        assert_eq!(role_of(ClassKind::Caster), Role::Support);
    }

    #[test]
    fn formation() {
        let master = Pos::new(0.0, 0.0, 0.0);
        let me = Pos::new(0.0, 0.0, 1000.0);
        let p = follow_point(master, me, 300.0, 0, 1);
        assert!((p.dist2d(master) - 300.0).abs() < 1e-3 && p.z > 0.0, "{p:?}");
        // Trois compagnons : trois points distincts, tous à 300 du maître.
        let pts: Vec<Pos> = (0..3).map(|i| follow_point(master, me, 300.0, i, 3)).collect();
        for p in &pts {
            assert!((p.dist2d(master) - 300.0).abs() < 1e-3);
        }
        assert!(pts[0].dist2d(pts[1]) > 100.0 && pts[1].dist2d(pts[2]) > 100.0);
        // Bot sur le maître : direction par défaut.
        assert!(follow_point(master, master, 300.0, 0, 1).dist2d(master) > 299.0);
    }
}
