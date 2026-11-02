//! Recherche de chemin sur la grille du terrain. ArchCore n'a pas de NavMesh : sa carte serveur dit, par
//! segment de 400 unités, si le sol est infranchissable. Le serveur fait marcher un personnage en ligne
//! droite jusqu'à sa destination sans tenir compte de ces segments (le client gère lui-même les obstacles) ;
//! un bot n'a pas de client, il contourne donc les segments bloqués lui-même : A* à 8 voisins sur les
//! cellules, puis lissage par visée directe (« string pulling »).

use crate::world::{Pos, Terrain};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Nœuds explorés au plus par recherche, pour borner le temps CPU d'un tour d'IA.
pub const DEFAULT_MAX_NODES: usize = 3000;

pub type Cell = (i64, i64);

fn cell_of(cell: f64, p: Pos) -> Cell {
    ((p.x as f64 / cell).floor() as i64, (p.z as f64 / cell).floor() as i64)
}

fn center(cell: f64, c: Cell, y: f32) -> Pos {
    Pos::new(((c.0 as f64 + 0.5) * cell) as f32, y, ((c.1 as f64 + 0.5) * cell) as f32)
}

fn blocked(terrain: &dyn Terrain, cell: f64, c: Cell) -> bool {
    let p = center(cell, c, 0.0);
    terrain.is_blocked(p.x as f64, p.z as f64)
}

/// Distance octile entre deux cellules, en dixièmes de cellule (10 droit, 14 diagonal).
fn heuristic(a: Cell, b: Cell) -> i64 {
    let (dx, dz) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
    10 * (dx + dz) - 6 * dx.min(dz)
}

/// Cellules traversées par le segment `a`→`b` (parcours de grille d'Amanatides et Woo) ; au passage exact
/// d'un coin, les deux cellules voisines comptent aussi : aucun effleurement n'échappe au contrôle.
pub fn cells_on_segment(cell: f64, a: Pos, b: Pos) -> Vec<Cell> {
    let (x0, z0) = (a.x as f64 / cell, a.z as f64 / cell);
    let (x1, z1) = (b.x as f64 / cell, b.z as f64 / cell);
    let (mut cx, mut cz) = (x0.floor() as i64, z0.floor() as i64);
    let end = (x1.floor() as i64, z1.floor() as i64);
    let (dx, dz) = (x1 - x0, z1 - z0);
    let (step_x, step_z) = (dx.signum() as i64, dz.signum() as i64);
    let mut t_max_x = if dx > 0.0 { (cx as f64 + 1.0 - x0) / dx } else if dx < 0.0 { (x0 - cx as f64) / -dx } else { f64::INFINITY };
    let mut t_max_z = if dz > 0.0 { (cz as f64 + 1.0 - z0) / dz } else if dz < 0.0 { (z0 - cz as f64) / -dz } else { f64::INFINITY };
    let (t_delta_x, t_delta_z) = ((1.0 / dx).abs(), (1.0 / dz).abs());
    let mut out = vec![(cx, cz)];
    let budget = ((end.0 - cx).abs() + (end.1 - cz).abs()) as usize * 2 + 4;
    for _ in 0..budget {
        if (cx, cz) == end {
            break;
        }
        if (t_max_x - t_max_z).abs() < 1e-9 {
            // Coin exact : les deux cellules adjacentes sont effleurées.
            out.push((cx + step_x, cz));
            out.push((cx, cz + step_z));
            cx += step_x;
            cz += step_z;
            t_max_x += t_delta_x;
            t_max_z += t_delta_z;
        } else if t_max_x < t_max_z {
            cx += step_x;
            t_max_x += t_delta_x;
        } else {
            cz += step_z;
            t_max_z += t_delta_z;
        }
        out.push((cx, cz));
    }
    out
}

/// Le segment `a`→`b` ne traverse aucune cellule bloquée.
pub fn line_clear(terrain: &dyn Terrain, a: Pos, b: Pos) -> bool {
    let cell = terrain.cell_size();
    cells_on_segment(cell, a, b).into_iter().all(|c| !blocked(terrain, cell, c))
}

/// Chemin de `from` à `to` : points de passage (centres de cellules), le dernier étant `to` exactement.
/// `None` si la destination est bloquée ou hors de portée de `max_nodes` nœuds. Un trajet en ligne droite
/// dégagé rend `[to]` directement.
pub fn find_path(terrain: &dyn Terrain, from: Pos, to: Pos, max_nodes: usize) -> Option<Vec<Pos>> {
    if !from.is_finite() || !to.is_finite() {
        return None;
    }
    let cell = terrain.cell_size();
    if terrain.is_blocked(to.x as f64, to.z as f64) {
        return None;
    }
    if line_clear(terrain, from, to) {
        return Some(vec![to]);
    }
    let (start, goal) = (cell_of(cell, from), cell_of(cell, to));
    if start == goal {
        return Some(vec![to]);
    }
    let mut open: BinaryHeap<Reverse<(i64, u64, Cell)>> = BinaryHeap::new();
    let mut came: HashMap<Cell, Cell> = HashMap::new();
    let mut g: HashMap<Cell, i64> = HashMap::new();
    let mut seq = 0u64;
    g.insert(start, 0);
    open.push(Reverse((heuristic(start, goal), seq, start)));
    let mut expanded = 0usize;
    while let Some(Reverse((_, _, cur))) = open.pop() {
        if cur == goal {
            let mut path = vec![goal];
            let mut c = goal;
            while let Some(&prev) = came.get(&c) {
                if prev == start {
                    break;
                }
                path.push(prev);
                c = prev;
            }
            path.reverse();
            let y = |c: Cell| {
                let p = center(cell, c, from.y);
                terrain.height(p.x as f64, p.z as f64).unwrap_or(from.y)
            };
            let mut out: Vec<Pos> = path.iter().map(|&c| center(cell, c, y(c))).collect();
            // Le dernier point est la destination exacte, pas le centre de sa cellule.
            out.pop();
            out.push(to);
            return Some(out);
        }
        expanded += 1;
        if expanded > max_nodes {
            return None;
        }
        let gc = g[&cur];
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let next = (cur.0 + dx, cur.1 + dz);
            if blocked(terrain, cell, next) {
                continue;
            }
            let diagonal = dx != 0 && dz != 0;
            // Pas de coupe de coin : les deux cellules orthogonales doivent être libres.
            if diagonal && (blocked(terrain, cell, (cur.0 + dx, cur.1)) || blocked(terrain, cell, (cur.0, cur.1 + dz))) {
                continue;
            }
            let ng = gc + if diagonal { 14 } else { 10 };
            if g.get(&next).is_none_or(|&old| ng < old) {
                g.insert(next, ng);
                came.insert(next, cur);
                seq += 1;
                open.push(Reverse((ng + heuristic(next, goal), seq, next)));
            }
        }
    }
    None
}

/// Lissage : depuis chaque point, on vise le point le plus lointain atteignable en ligne droite.
pub fn smooth(terrain: &dyn Terrain, from: Pos, path: &[Pos]) -> Vec<Pos> {
    if path.len() <= 1 {
        return path.to_vec();
    }
    let mut out = Vec::new();
    let mut anchor = from;
    let mut i = 0;
    while i < path.len() {
        let mut far = i;
        for j in (i + 1..path.len()).rev() {
            if line_clear(terrain, anchor, path[j]) {
                far = j;
                break;
            }
        }
        out.push(path[far]);
        anchor = path[far];
        i = far + 1;
    }
    out
}

/// Chemin trouvé puis lissé.
pub fn plan(terrain: &dyn Terrain, from: Pos, to: Pos, max_nodes: usize) -> Option<Vec<Pos>> {
    find_path(terrain, from, to, max_nodes).map(|p| smooth(terrain, from, &p))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Terrain de test : cellules bloquées (indices de cellule), hauteur constante.
    pub struct GridTerrain {
        pub blocked: HashSet<Cell>,
        pub cell: f64,
    }

    impl GridTerrain {
        pub fn new(cell: f64) -> Self {
            GridTerrain { blocked: HashSet::new(), cell }
        }
        /// Mur vertical en x = `cx` cellules, de `z0` à `z1` (cellules).
        pub fn wall_x(&mut self, cx: i64, z0: i64, z1: i64) {
            for z in z0..=z1 {
                self.blocked.insert((cx, z));
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

    #[test]
    fn ligne_droite_sans_obstacle() {
        let t = GridTerrain::new(400.0);
        let path = plan(&t, Pos::new(0.0, 0.0, 0.0), Pos::new(5000.0, 0.0, 1000.0), 1000).unwrap();
        assert_eq!(path, vec![Pos::new(5000.0, 0.0, 1000.0)]);
    }

    #[test]
    fn contourne_un_mur() {
        let mut t = GridTerrain::new(400.0);
        // Mur en x ∈ [2000, 2400) de z = -2000 à 2000 ; le trajet de (0, 0) à (4000, 0) doit le contourner.
        t.wall_x(5, -5, 5);
        let from = Pos::new(100.0, 0.0, 100.0);
        let to = Pos::new(4100.0, 0.0, 100.0);
        assert!(!line_clear(&t, from, to));
        let raw = find_path(&t, from, to, 3000).expect("chemin");
        let path = smooth(&t, from, &raw);
        assert_eq!(*path.last().unwrap(), to);
        assert!(path.len() >= 2, "au moins un détour : {path:?}");
        // Chaque tronçon est dégagé et aucun point n'est bloqué.
        let mut prev = from;
        for p in &path {
            assert!(line_clear(&t, prev, *p), "tronçon {prev:?} → {p:?} traverse le mur");
            prev = *p;
        }
        // Le détour passe au-delà du bout du mur (z > 2400 ou z < -2000).
        assert!(path.iter().any(|p| p.z >= 2400.0 || p.z < -2000.0), "{path:?}");
    }

    #[test]
    fn destination_bloquee_ou_enfermee() {
        let mut t = GridTerrain::new(400.0);
        t.blocked.insert((10, 10));
        assert!(find_path(&t, Pos::new(0.0, 0.0, 0.0), Pos::new(4200.0, 0.0, 4200.0), 1000).is_none(), "cible bloquée");
        // Cible entourée.
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            t.blocked.insert((20 + dx, 20 + dz));
        }
        assert!(find_path(&t, Pos::new(0.0, 0.0, 0.0), Pos::new(8200.0, 0.0, 8200.0), 5000).is_none(), "enfermée");
        // Budget épuisé : rien.
        assert!(find_path(&t, Pos::new(0.0, 0.0, 0.0), Pos::new(8200.0, 0.0, 8200.0), 2).is_none());
    }

    #[test]
    fn cellules_traversees() {
        // Diagonale exacte de (0,0) à (2,2) : coins effleurés comptés.
        let cells = cells_on_segment(1.0, Pos::new(0.5, 0.0, 0.5), Pos::new(2.5, 0.0, 2.5));
        for c in [(0, 0), (1, 0), (0, 1), (1, 1), (2, 1), (1, 2), (2, 2)] {
            assert!(cells.contains(&c), "{c:?} manque dans {cells:?}");
        }
        // Segment horizontal.
        let cells = cells_on_segment(400.0, Pos::new(100.0, 0.0, 100.0), Pos::new(1300.0, 0.0, 100.0));
        assert_eq!(cells, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        // Point unique.
        assert_eq!(cells_on_segment(400.0, Pos::new(50.0, 0.0, 50.0), Pos::new(50.0, 0.0, 50.0)), vec![(0, 0)]);
        // Un mur effleuré en coin est détecté.
        let mut t = GridTerrain::new(400.0);
        t.blocked.insert((1, 1));
        assert!(!line_clear(&t, Pos::new(0.0, 0.0, 800.0), Pos::new(800.0, 0.0, 0.0)), "diagonale par le coin (1,1)");
        assert!(line_clear(&t, Pos::new(0.0, 0.0, 0.0), Pos::new(1200.0, 0.0, 0.0)));
    }

    #[test]
    fn heuristique_octile() {
        assert_eq!(heuristic((0, 0), (3, 0)), 30);
        assert_eq!(heuristic((0, 0), (3, 3)), 42);
        assert_eq!(heuristic((0, 0), (3, 1)), 34);
    }
}
