//! Contrat entre le moteur et l'hôte : ce qu'un bot perçoit ([`Perception`]) et ce qu'il peut faire
//! ([`Action`]). L'hôte construit la perception à partir de l'état réel du serveur (rien n'est inventé ici) et
//! exécute les actions avec les fonctions du serveur.

use std::time::Instant;

/// Identifiant d'une entité du monde : CID d'un personnage, d'un monstre, IID d'un objet au sol.
pub type EntityId = u32;

/// Position dans le monde (unités du jeu ; X/Z au sol, Y la hauteur).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Pos {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Pos { x, y, z }
    }

    /// Distance horizontale (X/Z), comme le serveur la mesure.
    pub fn dist2d(self, o: Pos) -> f64 {
        let (dx, dz) = ((self.x - o.x) as f64, (self.z - o.z) as f64);
        (dx * dx + dz * dz).sqrt()
    }

    /// Point situé à `distance` unités de `self` en direction de `toward` (ou `toward` si plus proche).
    pub fn step_toward(self, toward: Pos, distance: f64) -> Pos {
        let total = self.dist2d(toward);
        if total <= distance || total <= 0.0 {
            return toward;
        }
        let k = (distance / total) as f32;
        Pos::new(self.x + (toward.x - self.x) * k, toward.y, self.z + (toward.z - self.z) * k)
    }

    /// Point situé à `distance` unités de `target`, du côté de `self` (pour se tenir à portée sans coller).
    pub fn at_distance_from(self, target: Pos, distance: f64) -> Pos {
        let total = self.dist2d(target);
        if total <= distance || total <= 0.0 {
            return self;
        }
        let k = ((total - distance) / total) as f32;
        Pos::new(self.x + (target.x - self.x) * k, target.y, self.z + (target.z - self.z) * k)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// Terrain tel que le serveur le connaît : sol infranchissable, hauteur, régions sûres. ArchCore n'a pas de
/// NavMesh : sa carte est une grille de segments de 400 unités avec un bit « infranchissable ».
pub trait Terrain {
    /// Sol infranchissable (ou hors du monde) en (x, z).
    fn is_blocked(&self, x: f64, z: f64) -> bool;
    /// Hauteur du sol en (x, z), si elle est connue.
    fn height(&self, _x: f64, _z: f64) -> Option<f32> {
        None
    }
    /// Région sûre (village) en (x, z).
    fn is_safe(&self, _x: f64, _z: f64) -> bool {
        false
    }
    /// Côté d'une cellule de la grille, en unités.
    fn cell_size(&self) -> f64 {
        400.0
    }
}

/// Terrain sans obstacle (tests, repli).
pub struct OpenTerrain;

impl Terrain for OpenTerrain {
    fn is_blocked(&self, _x: f64, _z: f64) -> bool {
        false
    }
}

/// Nature d'une classe d'ArchLord, d'après le champ `Class` de CharacterDataTable : 1 combattant (Knight,
/// Berserker, Swashbuckler, Slayer), 2 archer (Archer, Hunter, Ranger, Orbiter), 3 Scion, 4 lanceur (Mage,
/// Sorcerer, Elementalist, Summoner).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ClassKind {
    Melee,
    Ranged,
    Caster,
    #[default]
    Unknown,
}

impl ClassKind {
    pub fn from_archlord(class: i32) -> Self {
        match class {
            1 | 3 => ClassKind::Melee,
            2 => ClassKind::Ranged,
            4 => ClassKind::Caster,
            _ => ClassKind::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ClassKind::Melee => "melee",
            ClassKind::Ranged => "ranged",
            ClassKind::Caster => "caster",
            ClassKind::Unknown => "unknown",
        }
    }
}

/// Groupe du bot, tel que le serveur le tient.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyView {
    pub leader: EntityId,
    pub members: Vec<EntityId>,
}

/// Ce que le bot sait de lui-même.
#[derive(Clone, Debug, PartialEq)]
pub struct SelfView {
    pub id: EntityId,
    pub name: String,
    pub pos: Pos,
    /// Destination de la marche en cours (le serveur avance le personnage lui-même).
    pub dest: Option<Pos>,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub level: i32,
    pub dead: bool,
    pub invincible: bool,
    /// Un coup donné ou reçu il y a moins de 10 s.
    pub in_combat: bool,
    /// Cible de l'attaque en cours.
    pub attacking: Option<EntityId>,
    pub casting: bool,
    pub class: ClassKind,
    /// Portée d'attaque (facteurs), en unités.
    pub attack_range: f64,
    /// Vitesse de course, en unités par seconde.
    pub run_speed: f64,
    pub hp_potions: u32,
    pub mp_potions: u32,
    pub free_slots: u32,
    pub party: Option<PartyView>,
    /// Point d'ancrage (lieu d'apparition ou village) : centre de l'exploration, refuge de la fuite.
    pub home: Pos,
    pub money: u64,
    /// Points de compétence disponibles (facteur « dirt » 5) : des compétences peuvent être apprises.
    pub skill_points: i32,
}

impl Default for SelfView {
    fn default() -> Self {
        SelfView {
            id: 0,
            name: String::new(),
            pos: Pos::default(),
            dest: None,
            hp: 100,
            hp_max: 100,
            mp: 50,
            mp_max: 50,
            level: 1,
            dead: false,
            invincible: false,
            in_combat: false,
            attacking: None,
            casting: false,
            class: ClassKind::Melee,
            attack_range: 50.0,
            run_speed: 1200.0,
            hp_potions: 0,
            mp_potions: 0,
            free_slots: 64,
            party: None,
            home: Pos::default(),
            money: 0,
            skill_points: 0,
        }
    }
}

impl SelfView {
    pub fn hp_ratio(&self) -> f64 {
        if self.hp_max <= 0 {
            1.0
        } else {
            (self.hp.max(0) as f64 / self.hp_max as f64).min(1.0)
        }
    }
    pub fn mp_ratio(&self) -> f64 {
        if self.mp_max <= 0 {
            1.0
        } else {
            (self.mp.max(0) as f64 / self.mp_max as f64).min(1.0)
        }
    }
    pub fn moving(&self) -> bool {
        self.dest.is_some()
    }
}

/// Un monstre vu par le bot.
#[derive(Clone, Debug, PartialEq)]
pub struct MonsterView {
    pub id: EntityId,
    pub pos: Pos,
    pub level: i32,
    pub hp: i32,
    pub hp_max: i32,
    pub alive: bool,
    /// Attaquable et ciblable (type de personnage, forme).
    pub attackable: bool,
    /// Il a le bot, le maître du bot, un membre du groupe, ou quelqu'un d'autre parmi ses agresseurs.
    pub aggro_me: bool,
    pub aggro_master: bool,
    pub aggro_party: bool,
    pub aggro_others: bool,
    /// Rayon du corps (ajouté à la portée de l'attaquant).
    pub hit_range: f64,
}

impl Default for MonsterView {
    fn default() -> Self {
        MonsterView {
            id: 0,
            pos: Pos::default(),
            level: 1,
            hp: 100,
            hp_max: 100,
            alive: true,
            attackable: true,
            aggro_me: false,
            aggro_master: false,
            aggro_party: false,
            aggro_others: false,
            hit_range: 100.0,
        }
    }
}

/// Un joueur (vrai ou bot) vu par le bot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerView {
    pub id: EntityId,
    pub name: String,
    pub pos: Pos,
    pub level: i32,
    pub dead: bool,
    pub attacking: Option<EntityId>,
    pub is_bot: bool,
}

/// Un objet au sol.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroundItemView {
    pub id: EntityId,
    pub pos: Pos,
    /// Le bot a le droit de le ramasser maintenant (propriétaire, ou délai de propriété écoulé).
    pub mine: bool,
    pub gold: bool,
}

/// Une compétence connue du bot, avec les valeurs réelles de son niveau.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillView {
    /// Identifiant de session (celui que le serveur attend pour lancer).
    pub id: u32,
    pub tid: u32,
    pub name: String,
    pub level: u32,
    pub offensive: bool,
    /// Soigne (à l'application).
    pub heals: bool,
    pub on_self: bool,
    pub needs_target: bool,
    pub ready: bool,
    /// Portée en unités ; 0 : portée d'attaque du personnage.
    pub range: f64,
    pub cost_mp: i32,
    pub cast_ms: u64,
}

/// Tout ce que le moteur sait d'un bot à un tour donné.
pub struct Perception<'a> {
    pub now: Instant,
    pub me: SelfView,
    pub monsters: Vec<MonsterView>,
    pub players: Vec<PlayerView>,
    pub items: Vec<GroundItemView>,
    pub skills: Vec<SkillView>,
    /// Le maître d'un compagnon, s'il est en jeu et à portée de perception.
    pub master: Option<PlayerView>,
    pub terrain: &'a dyn Terrain,
    /// Un vrai joueur est assez près pour voir le bot (sinon l'hôte peut ralentir sa pensée).
    pub real_player_near: bool,
}

impl<'a> Perception<'a> {
    pub fn new(now: Instant, me: SelfView, terrain: &'a dyn Terrain) -> Self {
        Perception {
            now,
            me,
            monsters: Vec::new(),
            players: Vec::new(),
            items: Vec::new(),
            skills: Vec::new(),
            master: None,
            terrain,
            real_player_near: true,
        }
    }

    pub fn monster(&self, id: EntityId) -> Option<&MonsterView> {
        self.monsters.iter().find(|m| m.id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PotionKind {
    Hp,
    Mp,
}

/// Ce qu'un bot demande à l'hôte. L'hôte applique les règles du serveur : une action impossible est
/// simplement sans effet (le moteur le constate au tour suivant).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Courir jusqu'à un point (le serveur avance le personnage).
    MoveTo(Pos),
    Stop,
    /// Prendre un monstre pour cible : le serveur s'approche et frappe au rythme de la vitesse d'attaque.
    Attack(EntityId),
    StopAttack,
    Cast { skill: u32, target: EntityId },
    Potion(PotionKind),
    PickUp(EntityId),
    /// Résurrection au village (ou au point de la zone), comme le bouton du client.
    Resurrect,
    /// Équiper les meilleurs objets portables de l'inventaire.
    EquipUpgrades,
    /// Apprendre et améliorer les compétences de sa classe avec les points disponibles.
    LearnSkills,
    /// Se reposer (immobile) ou reprendre.
    Rest(bool),
    Say(String),
    Whisper { to: EntityId, text: String },
    PartyInvite(EntityId),
    PartyLeave,
    /// Trace pour le journal de l'hôte (mode debug).
    Note(String),
}

impl Action {
    pub fn kind(&self) -> &'static str {
        match self {
            Action::MoveTo(_) => "move",
            Action::Stop => "stop",
            Action::Attack(_) => "attack",
            Action::StopAttack => "stop_attack",
            Action::Cast { .. } => "cast",
            Action::Potion(_) => "potion",
            Action::PickUp(_) => "pickup",
            Action::Resurrect => "resurrect",
            Action::EquipUpgrades => "equip",
            Action::LearnSkills => "learn",
            Action::Rest(_) => "rest",
            Action::Say(_) => "say",
            Action::Whisper { .. } => "whisper",
            Action::PartyInvite(_) => "party_invite",
            Action::PartyLeave => "party_leave",
            Action::Note(_) => "note",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometrie() {
        let a = Pos::new(0.0, 0.0, 0.0);
        let b = Pos::new(3000.0, 10.0, 4000.0);
        assert_eq!(a.dist2d(b), 5000.0);
        assert_eq!(a.step_toward(b, 1000.0), Pos::new(600.0, 10.0, 800.0));
        assert_eq!(a.step_toward(b, 9000.0), b, "plus près que la distance : on y est");
        let c = a.at_distance_from(b, 1000.0);
        assert!((c.dist2d(b) - 1000.0).abs() < 1e-3);
        assert_eq!(b.at_distance_from(b, 100.0), b);
    }

    #[test]
    fn classes() {
        assert_eq!(ClassKind::from_archlord(1), ClassKind::Melee);
        assert_eq!(ClassKind::from_archlord(2), ClassKind::Ranged);
        assert_eq!(ClassKind::from_archlord(3), ClassKind::Melee, "Scion");
        assert_eq!(ClassKind::from_archlord(4), ClassKind::Caster);
        assert_eq!(ClassKind::from_archlord(9), ClassKind::Unknown);
    }

    #[test]
    fn jauges() {
        let me = SelfView { hp: 25, hp_max: 100, mp: 0, mp_max: 0, ..Default::default() };
        assert_eq!(me.hp_ratio(), 0.25);
        assert_eq!(me.mp_ratio(), 1.0, "sans PM : jauge pleine");
    }
}
