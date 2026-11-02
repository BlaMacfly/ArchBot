//! Discussion : commandes `/bot …` d'un joueur, chuchotements et messages de groupe adressés aux bots, et parole
//! des bots (réponses chuchotées, phrases autour d'eux).
//!
//! ```text
//!   /bot help                          aide
//!   /bot list                          bots en jeu
//!   /bot hire [nom]                    un bot libre (le plus proche, ou nommé) devient votre compagnon ; à défaut, un
//!                                      nouveau bot est créé à côté de vous (si la population le permet)
//!   /bot dismiss [nom]                 vos compagnons (ou celui nommé) redeviennent autonomes
//!   /bot follow [d] | stay | come | attack | passive | defensive | aggressive | stop | status
//!                                      ordre à vos compagnons (aussi par chuchotement au bot, ou « /p ordre » dans le groupe)
//!   /bot stats                         métriques du module (MJ de niveau ArchBot.CommandLevel)
//!   /bot spawn <n> | despawn <n|all> | population <n>   population des bots autonomes (MJ)
//! ```

use super::{manager::Manager, shared};
use crate::world::{chat, send_system, World, WorldState};
use crate::{info, warn};
use archbot::commands::{self, Command, HELP};
use archcore_net::schema::chat_type;
use std::sync::Arc;
use std::time::Instant;

/// Point d'entrée appelé par `chat::on_chatting` avant tout autre traitement. Rend vrai si le message est consommé.
pub async fn on_chat(world: &Arc<World>, st: &mut WorldState, sid: u32, kind: i64, text: &str, whisper_target: Option<&str>) -> anyhow::Result<bool> {
    let Some(sh) = shared() else { return Ok(false) };
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    // Commandes /bot …
    if kind != chat_type::WHISPER && (lower == "/bot" || lower.starts_with("/bot ")) {
        let mut m = sh.manager.lock().await;
        let args: Vec<&str> = trimmed.split_whitespace().skip(1).collect();
        slash(world, st, &mut m, sid, &args).await?;
        return Ok(true);
    }
    // Chuchotement à un bot : commande de compagnon, ou simple réponse.
    if kind == chat_type::WHISPER {
        if let Some(tsid) = whisper_target.and_then(|n| st.by_name(n)) {
            let mut m = sh.manager.lock().await;
            if m.is_bot(tsid) {
                direct(st, &mut m, sid, tsid, trimmed);
                return Ok(true);
            }
        }
        return Ok(false);
    }
    // Message de groupe (« /p ordre ») : ordre à ses compagnons du groupe ; le message continue son chemin.
    if let Some(rest) = lower.strip_prefix("/p ") {
        if let Some(cmd) = commands::parse(rest) {
            let mut m = sh.manager.lock().await;
            let Some(me) = st.players.get(&sid).map(|p| p.ch.id) else { return Ok(false) };
            let mine: Vec<u32> = m.companions_of(me).into_iter().filter(|bsid| st.players.get(bsid).is_some_and(|b| st.parties.same(me, b.ch.id))).collect();
            for bsid in mine {
                apply_command(st, &mut m, sid, bsid, &cmd);
            }
        }
    }
    Ok(false)
}

/// Chuchotement d'un joueur au bot `bsid`.
fn direct(st: &mut WorldState, m: &mut Manager, from: u32, bsid: u32, text: &str) {
    match commands::parse(text) {
        Some(Command::Hire) => {
            let reply = hire_named(st, m, from, bsid);
            whisper(st, bsid, from, &reply);
        }
        Some(Command::Dismiss) => {
            let reply = dismiss_one(st, m, from, bsid);
            whisper(st, bsid, from, &reply);
        }
        Some(cmd) => apply_command(st, m, from, bsid, &cmd),
        None => {
            if m.cfg.chatter {
                whisper(st, bsid, from, HELP);
            }
        }
    }
}

/// Applique un ordre au compagnon `bsid` si `from` est son maître ; répond par chuchotement.
fn apply_command(st: &mut WorldState, m: &mut Manager, from: u32, bsid: u32, cmd: &Command) {
    let Some(me) = st.players.get(&from).map(|p| p.ch.id) else { return };
    let Some(entry) = m.bots.get_mut(&bsid) else { return };
    let reply = match entry.brain.companion() {
        Some(c) if c.master == me => {
            let pos = st.players.get(&bsid).map(|b| super::perception::to_pos(b.ch.pos())).unwrap_or_default();
            entry.brain.command(cmd, pos, Instant::now())
        }
        Some(c) => format!("Je suis le compagnon de {}.", c.master_name),
        None => "Je ne suis le compagnon de personne : dites-moi « hire ».".to_string(),
    };
    if m.cfg.chatter && !reply.is_empty() {
        whisper(st, bsid, from, &reply);
    }
}

/// `bsid` devient le compagnon de `from` (s'il est libre et si les compagnons sont permis).
fn hire_named(st: &mut WorldState, m: &mut Manager, from: u32, bsid: u32) -> String {
    if !m.cfg.companions {
        return "Les compagnons sont désactivés sur ce serveur.".into();
    }
    let Some((me, my_name)) = st.players.get(&from).map(|p| (p.ch.id, p.ch.name.clone())) else { return String::new() };
    let follow = m.cfg.follow_distance;
    let slot_count = m.companions_of(me).len() + 1;
    let Some(entry) = m.bots.get_mut(&bsid) else { return "Ce personnage n'est pas un bot.".into() };
    if let Some(c) = entry.brain.companion() {
        return if c.master == me { "Je vous suis déjà.".into() } else { format!("Je suis déjà le compagnon de {}.", c.master_name) };
    }
    entry.brain.hire(me, &my_name, follow, (slot_count - 1, slot_count), Instant::now());
    // Formation : chaque compagnon du même maître reçoit sa place.
    let companions = m.companions_of(me);
    for (i, s) in companions.iter().enumerate() {
        if let Some(c) = m.bots.get_mut(s).and_then(|e| match &mut e.brain.mode {
            archbot::Mode::Companion(c) => Some(c),
            _ => None,
        }) {
            c.slot = (i, companions.len());
        }
    }
    info!("[archbot] {} devient le compagnon de {my_name}", m.bots[&bsid].name);
    // Groupe : le bot invite son maître (le client affiche l'invitation), s'il n'est pas déjà de son groupe.
    if m.cfg.allow_party && st.players.get(&bsid).is_some_and(|b| !st.parties.same(b.ch.id, me)) {
        if let Ok(p) = packet_invite(me) {
            crate::world::party::on_management(st, bsid, &p);
        }
    }
    "À vos ordres ! Dites « help » pour mes commandes.".into()
}

fn packet_invite(target: u32) -> anyhow::Result<archcore_net::Packet> {
    let s = &archcore_net::schema::PARTY_MANAGEMENT;
    Ok(archcore_net::decode_packet(s, &archcore_net::build_packet(s, &archcore_net::Fields::new(s).with(0, crate::world::party::mgmt::INVITE).with(2, target)))?)
}

/// `bsid` quitte le service de `from`.
fn dismiss_one(st: &mut WorldState, m: &mut Manager, from: u32, bsid: u32) -> String {
    let Some(me) = st.players.get(&from).map(|p| p.ch.id) else { return String::new() };
    let Some(entry) = m.bots.get_mut(&bsid) else { return String::new() };
    match entry.brain.companion() {
        Some(c) if c.master == me => {
            entry.brain.dismiss(Instant::now());
            let home = st.players.get(&bsid).map(|b| super::perception::to_pos(b.ch.pos())).unwrap_or(entry.home);
            entry.home = home;
            if let Some(cid) = st.players.get(&bsid).map(|b| b.ch.id) {
                if st.parties.of.contains_key(&cid) {
                    crate::world::party::remove_member(st, cid);
                }
            }
            info!("[archbot] {} quitte le service de {}", entry.name, st.players.get(&from).map(|p| p.ch.name.as_str()).unwrap_or("?"));
            "Au revoir, et bonne chasse.".into()
        }
        Some(_) => "Je ne suis pas votre compagnon.".into(),
        None => "Je ne suis le compagnon de personne.".into(),
    }
}

/// Commandes `/bot …`.
async fn slash(world: &Arc<World>, st: &mut WorldState, m: &mut Manager, sid: u32, args: &[&str]) -> anyhow::Result<()> {
    let Some((me, my_name, gm_level, my_pos)) = st.players.get(&sid).map(|p| (p.ch.id, p.ch.name.clone(), p.gm_level, p.ch.pos())) else { return Ok(()) };
    let admin = m.cfg.command_level > 0 && gm_level >= m.cfg.command_level;
    let head = args.first().map(|s| s.to_ascii_lowercase()).unwrap_or_else(|| "help".into());
    match head.as_str() {
        "help" | "aide" => {
            let mut text = "ArchBot : /bot list, /bot hire [nom], /bot dismiss [nom], /bot follow [d] | stay | come | attack | passive | defensive | aggressive | stop | status".to_string();
            if admin {
                text.push_str("\n/bot stats, /bot spawn <n>, /bot despawn <n|all>, /bot population <n>");
            }
            send_system(st, sid, &text);
        }
        "list" => {
            let mut lines: Vec<String> = m
                .bots
                .iter()
                .filter_map(|(bsid, e)| st.players.get(bsid).map(|p| (p, e)))
                .map(|(p, e)| format!("{} (niv. {}, en jeu depuis {} min) : {}", p.ch.name, p.ch.level, e.entered_at.elapsed().as_secs() / 60, e.brain.status_line()))
                .collect();
            lines.sort();
            let total = lines.len();
            lines.truncate(20);
            let mut text = format!("{total} bot(s) en jeu");
            for l in lines {
                text.push('\n');
                text.push_str(&l);
            }
            send_system(st, sid, &text);
        }
        "hire" => {
            if !m.cfg.companions {
                send_system(st, sid, "Les compagnons sont désactivés sur ce serveur.");
                return Ok(());
            }
            let chosen = match args.get(1) {
                Some(name) => st.by_name(name).filter(|s| m.is_bot(*s)),
                None => m
                    .bots
                    .iter()
                    .filter(|(_, e)| !e.brain.is_companion())
                    .filter_map(|(bsid, _)| st.players.get(bsid).filter(|p| p.in_game && !p.dead).map(|p| (*bsid, crate::world::combat::dist2d(p.ch.pos(), my_pos))))
                    .filter(|(_, d)| *d <= 20_000.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(s, _)| s),
            };
            let bsid = match chosen {
                Some(s) => Some(s),
                None if args.get(1).is_some() => {
                    send_system(st, sid, "Aucun bot de ce nom en jeu.");
                    None
                }
                None => {
                    // Personne de libre à proximité : un nouveau bot est créé à côté du joueur.
                    match m.spawn_near(world, st, my_pos).await? {
                        Some(s) => Some(s),
                        None => {
                            send_system(st, sid, "Aucun bot disponible (population au maximum ou création impossible).");
                            None
                        }
                    }
                }
            };
            if let Some(bsid) = bsid {
                let reply = hire_named(st, m, sid, bsid);
                let bot_name = m.bots.get(&bsid).map(|e| e.name.clone()).unwrap_or_default();
                send_system(st, sid, &format!("{bot_name} : {reply}"));
            }
        }
        "dismiss" | "leave" | "release" => {
            let targets: Vec<u32> = match args.get(1) {
                Some(name) => st.by_name(name).into_iter().collect(),
                None => m.companions_of(me),
            };
            if targets.is_empty() {
                send_system(st, sid, "Vous n'avez aucun compagnon.");
            }
            for bsid in targets {
                let reply = dismiss_one(st, m, sid, bsid);
                let bot_name = m.bots.get(&bsid).map(|e| e.name.clone()).unwrap_or_default();
                send_system(st, sid, &format!("{bot_name} : {reply}"));
            }
        }
        "stats" if admin => {
            if args.get(1).is_some_and(|v| v.eq_ignore_ascii_case("reset")) {
                m.metrics = archbot::Metrics::default();
                send_system(st, sid, "Métriques remises à zéro.");
                return Ok(());
            }
            let text = m.metrics.report();
            send_system(st, sid, &text);
        }
        "spawn" if admin => {
            let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(1).min(200);
            let mut done = 0;
            for _ in 0..n {
                if m.bots.len() >= m.cfg.max_bots {
                    break;
                }
                if m.spawn_one(world, st).await?.is_some() {
                    done += 1;
                }
            }
            m.target_population = m.target_population.max(m.autonomous_count());
            send_system(st, sid, &format!("{done} bot(s) entré(s) en jeu ({} au total, population visée {}).", m.bots.len(), m.target_population));
        }
        "despawn" if admin => {
            // /bot despawn <n> | all | <nom>
            let arg = args.get(1).copied().unwrap_or("1");
            let named = st.by_name(arg).filter(|s| m.is_bot(*s));
            let all = arg.eq_ignore_ascii_case("all");
            let n: usize = if all { usize::MAX } else { arg.parse().unwrap_or(1) };
            let mut victims: Vec<u32> = match named {
                Some(s) => vec![s],
                None => m.bots.iter().filter(|(_, e)| !e.brain.is_companion()).map(|(s, _)| *s).collect(),
            };
            victims.sort_unstable();
            if named.is_none() {
                victims.truncate(n);
            }
            let count = victims.len();
            for bsid in victims {
                m.despawn(world, st, bsid).await;
            }
            m.target_population = m.target_population.saturating_sub(count).min(m.autonomous_count());
            send_system(st, sid, &format!("{count} bot(s) sorti(s) du monde ({} restant(s), population visée {}).", m.bots.len(), m.target_population));
        }
        "population" if admin => {
            let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(m.target_population).min(m.cfg.max_bots);
            m.target_population = n;
            send_system(st, sid, &format!("Population de bots autonomes visée : {n}."));
        }
        "stats" | "spawn" | "despawn" | "population" => {
            send_system(st, sid, &format!("Niveau MJ {} requis pour cette commande.", m.cfg.command_level));
        }
        other => match commands::parse(other) {
            Some(cmd) => {
                let mine = m.companions_of(me);
                if mine.is_empty() {
                    send_system(st, sid, "Vous n'avez aucun compagnon (/bot hire).");
                }
                for bsid in mine {
                    apply_command(st, m, sid, bsid, &cmd);
                }
            }
            None => {
                warn!("[archbot] {my_name} : commande /bot {other} inconnue");
                send_system(st, sid, &format!("Commande /bot {other} inconnue (/bot help)."));
            }
        },
    }
    Ok(())
}

/// Le bot `from` chuchote `text` au joueur `to`.
pub fn whisper(st: &WorldState, from: u32, to: u32, text: &str) {
    let (Some(b), Some(t)) = (st.players.get(&from), st.players.get(&to)) else { return };
    for line in text.split('\n') {
        t.send_raw(chat::chat_packet(chat_type::WHISPER, Some(b.ch.id), Some(&b.ch.name), Some(&t.ch.name), line));
    }
}

/// Le bot `sid` parle autour de lui (discussion normale, portée du rayon de vue).
pub fn say(ctx: &crate::world::WorldCtx, st: &WorldState, sid: u32, text: &str) {
    let Some(b) = st.players.get(&sid) else { return };
    let pkt = chat::chat_packet(chat_type::NORMAL, Some(b.ch.id), Some(&b.ch.name), None, text);
    let me = b.ch.pos();
    let r2 = ctx.view_radius * ctx.view_radius;
    for (osid, o) in &st.players {
        if *osid == sid || o.sess.is_detached() {
            continue;
        }
        let (dx, dz) = ((o.ch.pos_x - me.x as f64), (o.ch.pos_z - me.z as f64));
        if dx * dx + dz * dz <= r2 {
            o.send_raw(pkt.clone());
        }
    }
}
