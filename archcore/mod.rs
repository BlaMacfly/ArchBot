//! ArchBot — adaptateur ArchCore.
//!
//! Ce dossier est compilé **à l'intérieur** de la crate `archcore` (déclaré par `#[path]` dans `main.rs` sous la
//! feature `archbot`) : il voit l'état du monde et les fonctions des systèmes existants, et relie le moteur
//! indépendant (crate `archbot`, dossier `src/` du dépôt) au serveur.
//!
//! ```text
//!   settings.rs     lecture de config/archbot.conf (clés ArchBot.*, variables ARCHCORE_ARCH_BOT_*)
//!   manager.rs      gestionnaire : population, boucle d'IA, répartition par tour, métriques
//!   lifecycle.rs    comptes et personnages des bots, entrée dans le monde (session détachée), sortie
//!   perception.rs   état du monde → Perception du moteur (terrain, soi, monstres, joueurs, objets, compétences)
//!   actions.rs      Action du moteur → fonctions d'ArchCore (marche, combat, compétences, potions, butin…)
//!   chat.rs         commandes /bot …, chuchotements et messages de groupe adressés aux bots, parole des bots
//!   persistence.rs  table archbot_character (base des personnages)
//!   sim_bots.rs     scénario `archcore sim bots` : observation des bots par un client simulé
//! ```
//!
//! Un bot est un `Player` ordinaire de `WorldState`, admis par `world::admit` sur une `SessionHandle::detached` :
//! visibilité, déplacement, combat, expérience, butin, groupe et sauvegarde passent par les systèmes du serveur,
//! sans copie. Ordre des verrous : état du monde, puis gestionnaire.

pub mod actions;
pub mod chat;
pub mod lifecycle;
pub mod manager;
pub mod perception;
pub mod persistence;
pub mod settings;

use crate::world::World;
use crate::{info, warn};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

/// Ce que le module partage entre la boucle d'IA, les commandes de discussion et l'arrêt.
pub struct Shared {
    pub world: Arc<World>,
    pub cfg: archbot::BotConfig,
    pub manager: Mutex<manager::Manager>,
}

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

/// Le module, s'il est démarré.
pub fn shared() -> Option<Arc<Shared>> {
    SHARED.get().cloned()
}

/// Démarre le module au lancement du serveur monde : configuration, tables, gestionnaire, boucle d'IA.
pub async fn start(world: Arc<World>) -> anyhow::Result<()> {
    let parsed = settings::load(&world.ctx.config.root);
    for w in &parsed.warnings {
        warn!("[archbot] configuration : {w}");
    }
    for k in &parsed.unknown {
        warn!("[archbot] configuration : clé inconnue {k}");
    }
    let cfg = parsed.config;
    if !cfg.enabled {
        info!("[archbot] module désactivé (ArchBot.Enable = 0)");
        return Ok(());
    }
    persistence::ensure_tables(&world.ctx.db).await?;
    let pool = persistence::load_pool(&world.ctx.db, &cfg).await?;
    info!("[archbot] v{} : {} ; {} personnage(s) de bot en base", archbot::VERSION, cfg.summary(), pool.len());
    let manager = manager::Manager::new(cfg.clone(), pool);
    let shared = Arc::new(Shared { world, cfg, manager: Mutex::new(manager) });
    if SHARED.set(shared.clone()).is_err() {
        anyhow::bail!("ArchBot déjà démarré");
    }
    manager::spawn_ticker(shared);
    Ok(())
}

/// Arrêt du serveur : les bots quittent le monde (positions enregistrées par `presence::leave`).
pub async fn shutdown() {
    let Some(sh) = shared() else { return };
    let mut st = sh.world.state.lock().await;
    let mut m = sh.manager.lock().await;
    // Population visée à 0 : la boucle d'IA, qui tourne encore pendant l'annonce d'arrêt d'ArchCore (1 s),
    // ne fait plus entrer de bot après leur sortie.
    m.target_population = 0;
    let n = m.bots.len();
    m.despawn_all(&sh.world, &mut st).await;
    info!("[archbot] {n} bot(s) sorti(s) du monde ; {}", m.metrics.report().lines().next().unwrap_or_default());
}
