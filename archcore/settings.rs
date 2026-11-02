//! Lecture de `config/archbot.conf` : même format et mêmes règles que les `.conf` d'ArchCore (modèle `.dist`
//! copié au premier lancement, variables d'environnement `ARCHCORE_<CLÉ>` prioritaires, clés inconnues
//! signalées). Le fichier est à part de `world.conf` pour que le module s'installe sans toucher à la
//! configuration du serveur, comme `playerbots.conf` chez mod-playerbots.

use crate::{info, warn};
use archbot::config::{Parsed, KEYS};
use archbot::BotConfig;
use std::path::Path;

pub const FILE_NAME: &str = "archbot.conf";

/// Dossiers où chercher le modèle `archbot.conf.dist` : la configuration du serveur, puis le module lui-même.
fn dist_candidates(root: &Path, conf_dir: &Path) -> Vec<std::path::PathBuf> {
    vec![conf_dir.join(format!("{FILE_NAME}.dist")), root.join("modules/ArchBot/config").join(format!("{FILE_NAME}.dist"))]
}

/// Lit la configuration ; sans fichier ni modèle, les valeurs par défaut s'appliquent (avec un avertissement).
pub fn load(root: &Path) -> Parsed {
    let dir = crate::config::conf_dir(root);
    let path = dir.join(FILE_NAME);
    if !path.exists() {
        match dist_candidates(root, &dir).into_iter().find(|d| d.exists()) {
            Some(dist) => match std::fs::copy(&dist, &path) {
                Ok(_) => info!("[archbot] configuration : {} créé à partir de {}", path.display(), dist.display()),
                Err(e) => warn!("[archbot] configuration : copie de {} impossible ({e}) ; valeurs par défaut", dist.display()),
            },
            None => warn!("[archbot] configuration : ni {} ni son modèle .dist ; valeurs par défaut", path.display()),
        }
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut pairs = crate::config::parse_conf(&text);
    // Variables d'environnement par-dessus le fichier (ARCHCORE_ARCH_BOT_MAX_BOTS…), comme pour world.conf.
    for key in KEYS {
        if let Ok(v) = std::env::var(crate::config::env_name(key)) {
            pairs.push(((*key).to_string(), v));
        }
    }
    BotConfig::from_pairs(&pairs)
}
