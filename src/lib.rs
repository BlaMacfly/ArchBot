//! ArchBot : moteur de PlayerBots pour ArchCore (émulateur ArchLord), en Rust.
//!
//! Cette crate est **indépendante d'ArchCore** : elle ne connaît ni ses paquets, ni sa base, ni son état du
//! monde. Chaque tour d'IA, l'hôte (l'adaptateur compilé dans ArchCore, dossier `archcore/` du dépôt) lui
//! donne une [`world::Perception`] — ce que le bot peut savoir de lui-même et de ce qui l'entoure — et la
//! crate rend des [`world::Action`]s que l'hôte exécute avec les systèmes existants du serveur (déplacement,
//! combat, butin, groupe, compétences). Le moteur est donc synchrone, déterministe à graine égale, et se
//! teste sans serveur (voir `tests/`).
//!
//! ```text
//!   ai/          cerveau d'un bot : machine à états (Idle, Exploring, Following, Combat, Fleeing,
//!                Recovering, Looting, Dead, Resurrecting), modes autonome et compagnon
//!   movement/    navigation : A* sur la grille de segments du terrain, lissage, détection de blocage
//!   combat/      choix de la cible, survie (fuite, potions, repos), stratégies par classe
//!   party/       rôles et formation des compagnons
//!   commands/    commandes de discussion comprises par les bots (follow, stay, attack…)
//!   population   noms, modèles, cadence d'apparition
//!   diagnostics  métriques du module
//!   config       réglages `ArchBot.*` (archbot.conf), validés
//!   world        perception, actions, terrain : le contrat entre le moteur et l'hôte
//! ```

pub mod ai;
pub mod combat;
pub mod commands;
pub mod config;
pub mod diagnostics;
pub mod movement;
pub mod party;
pub mod population;
pub mod world;

pub use ai::brain::{Brain, Companion, Mode, Order, Stance};
pub use ai::state::BotState;
pub use config::BotConfig;
pub use diagnostics::Metrics;
pub use world::{Action, ClassKind, EntityId, Perception, Pos, PotionKind, Terrain};

/// Version du moteur (celle de Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
