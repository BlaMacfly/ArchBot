//! Cycle de vie d'un bot : compte et personnage (créés comme le ferait l'écran de création), entrée dans le
//! monde sur une session détachée (mêmes étapes que `GAME_ENTER` puis `ENTER_WORLD`), sortie.

use super::persistence::{self, BotRow};
use crate::db::col;
use crate::net::SessionHandle;
use crate::world::{Account, World, WorldCtx, WorldState};
use crate::{info, warn};
use archbot::BotConfig;
use archcore_net::Vec3;
use rand::Rng;

/// Position de départ du modèle `race` (`StartPosition.<race>`, sinon celle des humains).
pub fn start_position(ctx: &WorldCtx, race: i32) -> Vec3 {
    let starts = &ctx.config.world.start_positions;
    match starts.get(&race.to_string()).or_else(|| starts.get("1")) {
        Some(p) => Vec3::new(p.x as f32, p.y as f32, p.z as f32),
        None => Vec3::default(),
    }
}

/// Crée un compte et un personnage de bot : modèle jouable permis par la configuration, nom tiré et libre,
/// niveau tiré, points pleins, équipement de départ du modèle, ligne `archbot_character`. `None` si aucun modèle
/// ne convient.
pub async fn create_bot(ctx: &WorldCtx, cfg: &BotConfig, rng: &mut impl Rng) -> anyhow::Result<Option<BotRow>> {
    let Some(tid) = archbot::population::pick_template(rng, &cfg.classes, &ctx.templates.base) else {
        warn!("[archbot] aucun modèle jouable ne correspond à ArchBot.Classes = {:?}", cfg.classes);
        return Ok(None);
    };
    let Some(tpl) = ctx.templates.get(tid).cloned() else { return Ok(None) };
    let db = &ctx.db;
    // Nom libre (48 caractères au plus en base ; les noms tirés font 4 à 10 lettres).
    let mut name = None;
    for _ in 0..30 {
        let candidate = format!("{}{}", cfg.name_prefix, archbot::population::generate_name(rng));
        if db.characters.first("SELECT id FROM characters WHERE name = ?", (candidate.clone(),)).await?.is_none() {
            name = Some(candidate);
            break;
        }
    }
    let Some(name) = name else {
        warn!("[archbot] aucun nom libre trouvé en 30 tirages");
        return Ok(None);
    };
    // Un compte par bot (la banque et l'unicité de session sont par compte), mot de passe aléatoire inconnu de tous.
    let account_name = format!("{}{}", cfg.account_prefix, name.to_lowercase());
    let mut secret = [0u8; 32];
    rng.fill_bytes(&mut secret);
    let hash = crate::login::sha256_hex(&secret);
    let account_id = match db.auth.first("SELECT id FROM account WHERE username = ?", (account_name.clone(),)).await? {
        Some(r) => col(&r, "id"),
        None => db.auth.exec("INSERT INTO account (username, password_hash, gm_level) VALUES (?, ?, 0)", (account_name.clone(), hash)).await? as u32,
    };
    let level = archbot::population::random_level(rng, cfg.min_level, cfg.max_level).unwrap_or(tpl.level.max(1)).clamp(1, 120);
    let (hp, mp, sp) = tpl.max_points(level);
    let start = start_position(ctx, tpl.race);
    let character_id = db
        .characters
        .exec(
            "INSERT INTO characters (account_id, slot, name, template_id, level, hp, mp, sp, pos_x, pos_y, pos_z, hair, face) VALUES (?, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0)",
            (account_id, name.clone(), tid, level, hp, mp, sp, start.x as f64, start.y as f64, start.z as f64),
        )
        .await? as u32;
    crate::login::give_starting_items(db, character_id, tid).await?;
    persistence::register(db, character_id, account_id, start).await?;
    info!("[archbot] personnage créé : {name} ({}, niveau {level}, compte {account_name}, cid {character_id})", tpl.name);
    Ok(Some(BotRow { character_id, account_id, account_name, name, template_id: tid, level, home: start }))
}

/// Fait entrer un bot dans le monde : session détachée, chargement (`admit`) puis entrée (`enter_world`). Rend
/// l'identifiant de session, ou `None` si le personnage est déjà en jeu ou introuvable.
pub async fn enter(world: &World, st: &mut WorldState, row: &BotRow) -> anyhow::Result<Option<u32>> {
    let ctx = &world.ctx;
    if st.by_cid.contains_key(&row.character_id) {
        return Ok(None);
    }
    let Some(crow) = ctx.db.characters.first("SELECT * FROM characters WHERE id = ? AND deleted_at IS NULL", (row.character_id,)).await? else {
        warn!("[archbot] personnage {} ({}) absent de la base", row.name, row.character_id);
        return Ok(None);
    };
    let s = SessionHandle::detached("world");
    let account = Account { id: row.account_id, username: row.account_name.clone() };
    if !crate::world::admit(ctx, st, &s, account, 0, &crow).await? {
        return Ok(None);
    }
    crate::world::enter_world(ctx, st, &s);
    ctx.db.characters.exec("UPDATE characters SET last_played = NOW() WHERE id = ?", (row.character_id,)).await?;
    Ok(Some(s.id()))
}

/// Sort un bot du monde (groupe quitté, position enregistrée), comme une déconnexion.
pub async fn leave(ctx: &WorldCtx, st: &mut WorldState, sid: u32) {
    crate::world::remove_player(ctx, st, sid).await;
}
