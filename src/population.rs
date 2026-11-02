//! Population : noms, modèles et cadence d'apparition des bots autonomes.

use rand::Rng;
use std::time::{Duration, Instant};

const ONSETS: &[&str] = &["", "b", "br", "c", "d", "dr", "f", "g", "gr", "h", "k", "kr", "l", "m", "n", "p", "r", "s", "sh", "t", "th", "v", "z"];
const NUCLEI: &[&str] = &["a", "e", "i", "o", "u", "ae", "ia", "ei", "ou", "y"];
const CODAS: &[&str] = &["", "", "l", "n", "r", "s", "th", "m", "k", "d", "nd", "rn", "x"];

/// Nom plausible de 4 à 10 lettres, première en majuscule, syllabes au hasard.
pub fn generate_name(rng: &mut impl Rng) -> String {
    loop {
        let syllables = rng.gen_range(2..=3);
        let mut name = String::new();
        for i in 0..syllables {
            name.push_str(ONSETS[rng.gen_range(0..ONSETS.len())]);
            name.push_str(NUCLEI[rng.gen_range(0..NUCLEI.len())]);
            if i + 1 == syllables || rng.gen_bool(0.4) {
                name.push_str(CODAS[rng.gen_range(0..CODAS.len())]);
            }
        }
        if (4..=10).contains(&name.len()) {
            let mut c = name.chars();
            return c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default();
        }
    }
}

/// Modèle à créer : un des `allowed` s'ils sont parmi `available`, sinon un des `available`.
pub fn pick_template(rng: &mut impl Rng, allowed: &[u32], available: &[u32]) -> Option<u32> {
    let pool: Vec<u32> = if allowed.is_empty() { available.to_vec() } else { allowed.iter().copied().filter(|t| available.contains(t)).collect() };
    if pool.is_empty() {
        return None;
    }
    Some(pool[rng.gen_range(0..pool.len())])
}

/// Niveau de départ : tiré entre `min` et `max` (0 : niveau du modèle, rendu `None`).
pub fn random_level(rng: &mut impl Rng, min: i32, max: i32) -> Option<i32> {
    let (lo, hi) = (min.max(0), max.max(min).max(0));
    if hi == 0 {
        return None;
    }
    Some(rng.gen_range(lo.max(1)..=hi))
}

/// Durée de session d'un bot autonome : entre 50 % et 150 % de `rotation_minutes` (0 : sans fin).
pub fn session_length(rng: &mut impl Rng, rotation_minutes: u64) -> Option<Duration> {
    if rotation_minutes == 0 {
        return None;
    }
    let secs = rotation_minutes * 60;
    Some(Duration::from_secs(rng.gen_range(secs / 2..=secs * 3 / 2)))
}

/// Cadence des entrées en jeu : une au plus par intervalle.
#[derive(Clone, Debug)]
pub struct SpawnPacer {
    interval: Duration,
    next_at: Option<Instant>,
}

impl SpawnPacer {
    pub fn new(interval: Duration) -> Self {
        SpawnPacer { interval, next_at: None }
    }

    /// Vrai si une entrée peut avoir lieu maintenant (et en note une).
    pub fn take(&mut self, now: Instant) -> bool {
        if self.next_at.is_some_and(|t| now < t) {
            return false;
        }
        self.next_at = Some(now + self.interval);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn noms() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        for _ in 0..200 {
            let n = generate_name(&mut rng);
            assert!((4..=10).contains(&n.len()), "{n}");
            assert!(n.chars().next().unwrap().is_ascii_uppercase() && n.chars().all(|c| c.is_ascii_alphabetic()), "{n}");
        }
    }

    #[test]
    fn modeles_niveaux_cadence() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        assert_eq!(pick_template(&mut rng, &[999], &[96, 1]), None, "aucun modèle permis disponible");
        assert_eq!(pick_template(&mut rng, &[96, 999], &[96, 1]), Some(96));
        assert!(matches!(pick_template(&mut rng, &[], &[96, 1]), Some(96 | 1)));
        assert_eq!(random_level(&mut rng, 0, 0), None);
        for _ in 0..50 {
            assert!((5..=9).contains(&random_level(&mut rng, 5, 9).unwrap()));
        }
        assert_eq!(session_length(&mut rng, 0), None);
        let d = session_length(&mut rng, 10).unwrap();
        assert!((300..=900).contains(&d.as_secs()));
        let t0 = Instant::now();
        let mut p = SpawnPacer::new(Duration::from_secs(1));
        assert!(p.take(t0));
        assert!(!p.take(t0 + Duration::from_millis(500)));
        assert!(p.take(t0 + Duration::from_millis(1000)));
    }
}
