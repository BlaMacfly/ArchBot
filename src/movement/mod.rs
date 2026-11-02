//! Navigation d'un bot : destination, points de passage, détection de blocage.
//!
//! Le serveur avance lui-même le personnage vers la destination reçue (`presence::move_step` d'ArchCore) ;
//! le moteur n'a donc qu'à donner le prochain point de passage quand le précédent est atteint, et à
//! constater qu'un bot n'avance plus (position inchangée alors qu'il a une destination).

pub mod pathfinding;

use crate::world::{Pos, Terrain};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Tolérance d'arrivée à un point, en unités.
pub const ARRIVAL_TOLERANCE: f64 = 150.0;
/// Position inchangée pendant ce délai avec une destination : blocage.
pub const STUCK_AFTER: Duration = Duration::from_millis(3000);
/// Déplacement minimal qui compte comme une progression.
const PROGRESS_UNITS: f64 = 15.0;
/// Replanifications tolérées avant d'abandonner un but.
pub const MAX_STRIKES: u32 = 3;

/// Ce que la navigation demande ce tour-ci.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavStep {
    /// Pas de but.
    Idle,
    /// La marche en cours continue.
    Continue,
    /// Donner cette destination au serveur.
    Move(Pos),
    Arrived,
    /// Le bot n'avance plus (le but est gardé : à replanifier ou abandonner).
    Stuck,
}

#[derive(Clone, Debug, Default)]
pub struct Navigator {
    goal: Option<Pos>,
    path: VecDeque<Pos>,
    last_pos: Option<Pos>,
    last_progress: Option<Instant>,
    pub strikes: u32,
    pub replans: u32,
}

impl Navigator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn goal(&self) -> Option<Pos> {
        self.goal
    }

    pub fn has_goal(&self) -> bool {
        self.goal.is_some()
    }

    pub fn clear(&mut self) {
        self.goal = None;
        self.path.clear();
        self.strikes = 0;
        self.last_pos = None;
        self.last_progress = None;
    }

    /// Fixe un but et planifie le chemin ; faux si aucun chemin n'est trouvé (le but n'est pas gardé).
    pub fn set_goal(&mut self, terrain: &dyn Terrain, from: Pos, goal: Pos, now: Instant) -> bool {
        match pathfinding::plan(terrain, from, goal, pathfinding::DEFAULT_MAX_NODES) {
            Some(path) => {
                self.goal = Some(goal);
                self.path = path.into_iter().collect();
                self.strikes = 0;
                self.last_pos = Some(from);
                self.last_progress = Some(now);
                true
            }
            None => {
                self.clear();
                false
            }
        }
    }

    /// Replanifie depuis `from` vers le but courant (après un blocage) ; faux si impossible ou trop d'essais.
    pub fn replan(&mut self, terrain: &dyn Terrain, from: Pos, now: Instant) -> bool {
        let Some(goal) = self.goal else { return false };
        self.strikes += 1;
        self.replans += 1;
        if self.strikes > MAX_STRIKES {
            self.clear();
            return false;
        }
        match pathfinding::plan(terrain, from, goal, pathfinding::DEFAULT_MAX_NODES) {
            Some(path) => {
                self.path = path.into_iter().collect();
                self.last_pos = Some(from);
                self.last_progress = Some(now);
                true
            }
            None => {
                self.clear();
                false
            }
        }
    }

    /// Un tour de navigation : `pos` et `dest` sont la position et la destination que le serveur tient. Un
    /// point de passage atteint dans la tolérance n'est sauté que si le suivant est en vue directe depuis la
    /// position actuelle (sinon on rejoint le point exact, pour ne pas couper un coin de mur).
    pub fn step(&mut self, terrain: &dyn Terrain, pos: Pos, dest: Option<Pos>, now: Instant) -> NavStep {
        let Some(goal) = self.goal else { return NavStep::Idle };
        if pos.dist2d(goal) <= ARRIVAL_TOLERANCE {
            self.clear();
            return NavStep::Arrived;
        }
        // Progression.
        match self.last_pos {
            Some(last) if pos.dist2d(last) >= PROGRESS_UNITS => {
                self.last_pos = Some(pos);
                self.last_progress = Some(now);
            }
            None => {
                self.last_pos = Some(pos);
                self.last_progress = Some(now);
            }
            Some(_) => {
                if dest.is_some() && self.last_progress.is_some_and(|t| now.saturating_duration_since(t) >= STUCK_AFTER) {
                    self.last_progress = Some(now);
                    return NavStep::Stuck;
                }
            }
        }
        // Point de passage courant.
        while self.path.front().is_some_and(|wp| pos.dist2d(*wp) <= ARRIVAL_TOLERANCE) {
            let next_clear = match self.path.get(1) {
                Some(next) => pathfinding::line_clear(terrain, pos, *next),
                None => pathfinding::line_clear(terrain, pos, goal),
            };
            if !next_clear && pos.dist2d(*self.path.front().unwrap()) > 1.0 {
                break;
            }
            self.path.pop_front();
        }
        if self.path.is_empty() {
            self.path.push_back(goal);
        }
        let wp = *self.path.front().unwrap();
        match dest {
            Some(d) if d.dist2d(wp) <= 1.0 => NavStep::Continue,
            _ => NavStep::Move(wp),
        }
    }

    /// Points de passage restants.
    pub fn remaining(&self) -> usize {
        self.path.len()
    }
}

#[cfg(test)]
mod tests {
    use super::pathfinding::tests::GridTerrain;
    use super::*;

    #[test]
    fn marche_par_etapes_et_arrivee() {
        let t = GridTerrain::new(400.0);
        let mut nav = Navigator::new();
        let t0 = Instant::now();
        let goal = Pos::new(3000.0, 0.0, 0.0);
        assert!(nav.set_goal(&t, Pos::new(0.0, 0.0, 0.0), goal, t0));
        assert_eq!(nav.step(&t, Pos::new(0.0, 0.0, 0.0), None, t0), NavStep::Move(goal));
        assert_eq!(nav.step(&t, Pos::new(500.0, 0.0, 0.0), Some(goal), t0 + Duration::from_millis(500)), NavStep::Continue);
        assert_eq!(nav.step(&t, Pos::new(2900.0, 0.0, 0.0), None, t0 + Duration::from_secs(3)), NavStep::Arrived);
        assert_eq!(nav.step(&t, Pos::new(2900.0, 0.0, 0.0), None, t0), NavStep::Idle);
    }

    #[test]
    fn blocage_detecte_puis_replanifie() {
        let t = GridTerrain::new(400.0);
        let mut nav = Navigator::new();
        let t0 = Instant::now();
        let goal = Pos::new(3000.0, 0.0, 0.0);
        let here = Pos::new(100.0, 0.0, 0.0);
        nav.set_goal(&t, here, goal, t0);
        assert_eq!(nav.step(&t, here, None, t0), NavStep::Move(goal));
        // Position inchangée avec une destination pendant 3 s : blocage.
        assert_eq!(nav.step(&t, here, Some(goal), t0 + Duration::from_secs(1)), NavStep::Continue);
        assert_eq!(nav.step(&t, here, Some(goal), t0 + Duration::from_secs(4)), NavStep::Stuck);
        assert!(nav.replan(&t, here, t0 + Duration::from_secs(4)));
        assert_eq!(nav.strikes, 1);
        for i in 0..MAX_STRIKES {
            let ok = nav.replan(&t, here, t0);
            assert_eq!(ok, i + 1 < MAX_STRIKES, "essai {i}");
        }
        assert!(!nav.has_goal(), "but abandonné après trop d'essais");
    }

    #[test]
    fn chemin_avec_detour() {
        let mut t = GridTerrain::new(400.0);
        t.wall_x(5, -5, 5);
        let mut nav = Navigator::new();
        let from = Pos::new(100.0, 0.0, 100.0);
        let goal = Pos::new(4100.0, 0.0, 100.0);
        assert!(nav.set_goal(&t, from, goal, Instant::now()));
        assert!(nav.remaining() >= 2);
        let NavStep::Move(first) = nav.step(&t, from, None, Instant::now()) else { panic!("premier point") };
        assert_ne!(first, goal, "le premier point contourne le mur");
    }
}
