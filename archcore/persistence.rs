//! Persistance propre au module : la table `archbot_character` de la base des personnages marque les
//! personnages qui sont des bots (leur compte, leur point d'ancrage, quelques compteurs). Tout le reste —
//! niveau, expérience, position, inventaire, compétences, argent — est dans les tables ordinaires d'ArchCore,
//! écrites par ses propres fonctions. La table est créée au démarrage si elle manque (le module ne touche pas au
//! système de migrations d'ArchCore).

use crate::db::{col, Databases};
use archbot::BotConfig;
use archcore_net::Vec3;

pub const TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS archbot_character (\n  character_id INT UNSIGNED NOT NULL,\n  account_id   INT UNSIGNED NOT NULL,\n  home_x       DOUBLE NOT NULL DEFAULT 0,\n  home_y       DOUBLE NOT NULL DEFAULT 0,\n  home_z       DOUBLE NOT NULL DEFAULT 0,\n  sessions     INT UNSIGNED NOT NULL DEFAULT 0,\n  kills        INT UNSIGNED NOT NULL DEFAULT 0,\n  deaths       INT UNSIGNED NOT NULL DEFAULT 0,\n  created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,\n  last_seen    DATETIME NULL,\n  PRIMARY KEY (character_id)\n) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4";

/// Un personnage de bot connu en base.
#[derive(Clone, Debug, PartialEq)]
pub struct BotRow {
    pub character_id: u32,
    pub account_id: u32,
    pub account_name: String,
    pub name: String,
    pub template_id: u32,
    pub level: i32,
    pub home: Vec3,
}

pub async fn ensure_tables(db: &Databases) -> anyhow::Result<()> {
    db.characters.exec(TABLE_SQL, ()).await?;
    Ok(())
}

pub async fn register(db: &Databases, character_id: u32, account_id: u32, home: Vec3) -> anyhow::Result<()> {
    db.characters
        .exec(
            "INSERT INTO archbot_character (character_id, account_id, home_x, home_y, home_z) VALUES (?, ?, ?, ?, ?) ON DUPLICATE KEY UPDATE account_id = VALUES(account_id)",
            (character_id, account_id, home.x as f64, home.y as f64, home.z as f64),
        )
        .await?;
    Ok(())
}

/// Personnages de bots présents en base (non supprimés), avec le nom de leur compte, pour les reprendre au
/// démarrage. Les comptes sont ceux du préfixe configuré.
pub async fn load_pool(db: &Databases, cfg: &BotConfig) -> anyhow::Result<Vec<BotRow>> {
    let rows = db
        .characters
        .rows(
            "SELECT b.character_id, b.account_id, b.home_x, b.home_y, b.home_z, c.name, c.template_id, c.level FROM archbot_character b JOIN characters c ON c.id = b.character_id WHERE c.deleted_at IS NULL ORDER BY b.character_id",
            (),
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        let account_id: u32 = col(r, "account_id");
        let Some(acc) = db.auth.first("SELECT username FROM account WHERE id = ?", (account_id,)).await? else { continue };
        let account_name: String = col(&acc, "username");
        if !account_name.starts_with(&cfg.account_prefix) {
            continue;
        }
        out.push(BotRow {
            character_id: col(r, "character_id"),
            account_id,
            account_name,
            name: col(r, "name"),
            template_id: col(r, "template_id"),
            level: col(r, "level"),
            home: Vec3::new(col::<f64>(r, "home_x") as f32, col::<f64>(r, "home_y") as f32, col::<f64>(r, "home_z") as f32),
        });
    }
    Ok(out)
}

/// Fin de session d'un bot : compteurs et date.
pub async fn touch(db: &Databases, character_id: u32, kills: u32, deaths: u32) -> anyhow::Result<()> {
    db.characters
        .exec("UPDATE archbot_character SET sessions = sessions + 1, kills = kills + ?, deaths = deaths + ?, last_seen = NOW() WHERE character_id = ?", (kills, deaths, character_id))
        .await?;
    Ok(())
}

/// Bot non persistant : son personnage est marqué supprimé (comme une suppression par le joueur), sa ligne retirée.
pub async fn forget(db: &Databases, character_id: u32) -> anyhow::Result<()> {
    db.characters.exec("UPDATE characters SET deleted_at = NOW() WHERE id = ?", (character_id,)).await?;
    db.characters.exec("DELETE FROM archbot_character WHERE character_id = ?", (character_id,)).await?;
    Ok(())
}
