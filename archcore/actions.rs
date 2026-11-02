//! [`Action`] du moteur → fonctions d'ArchCore. Chaque action passe par le même chemin qu'un paquet du client
//! (portée, cadence, règles de propriété du butin, délais des potions…) : le bot ne contourne aucune règle.

use super::chat;
use super::perception::to_vec3;
use crate::world::{combat, equipment, loot, party, presence, recovery, skills, World, WorldState};
use crate::{debug, info};
use archbot::world::{Action, PotionKind};
use archbot::Metrics;
use archcore_net::schema;
use archcore_net::{build_packet, decode_packet, Fields, Packet, Schema};
use std::time::Instant;

/// Paquet « comme venu du client », pour réutiliser un gestionnaire qui lit un `Packet`.
fn packet(s: &'static Schema, fields: Fields) -> anyhow::Result<Packet> {
    Ok(decode_packet(s, &build_packet(s, &fields))?)
}

/// Exécute une action du bot `sid`.
pub async fn execute(world: &World, st: &mut WorldState, sid: u32, action: Action, metrics: &mut Metrics, debug_log: bool) -> anyhow::Result<()> {
    let ctx = &world.ctx;
    metrics.record_action(action.kind());
    if debug_log {
        if let Some(p) = st.players.get(&sid) {
            debug!("[archbot] {} : {action:?}", p.ch.name);
        }
    }
    match action {
        Action::MoveTo(p) => {
            let Some(pl) = st.players.get_mut(&sid) else { return Ok(()) };
            if pl.dead || !pl.in_game {
                return Ok(());
            }
            // Une marche voulue par le bot remplace l'attaque et l'incantation en cours, comme un déplacement du client.
            pl.attack = None;
            pl.skills.cast = None;
            pl.portal = None;
            let mut dest = to_vec3(p);
            if let Some(y) = ctx.map.height(dest.x as f64, dest.z as f64) {
                dest.y = y;
            }
            presence::run_to(st, sid, dest, Instant::now());
        }
        Action::Stop => presence::stop(st, sid),
        Action::Attack(target) => combat::start_attack(ctx, st, sid, target, false),
        Action::StopAttack => {
            if let Some(pl) = st.players.get_mut(&sid) {
                pl.attack = None;
                pl.skills.cast = None;
            }
        }
        Action::Cast { skill, target } => skills::start_cast(ctx, st, sid, skill, target, false),
        Action::Potion(kind) => {
            let Some(pl) = st.players.get(&sid) else { return Ok(()) };
            let iid = pl.inventory.items.values().find(|it| {
                ctx.catalog.get(it.tid).and_then(|d| d.potion).is_some_and(|po| match kind {
                    PotionKind::Hp => po.hp > 0,
                    PotionKind::Mp => po.mp > 0 && po.hp <= 0,
                })
            });
            if let Some(it) = iid {
                let s = &schema::ITEM;
                let p = packet(s, Fields::new(s).with(0, recovery::USE_ITEM).with(2, it.iid))?;
                recovery::use_item(ctx, st, sid, &p, &ctx.catalog).await?;
            }
        }
        Action::PickUp(iid) => {
            let s = &schema::ITEM;
            let p = packet(s, Fields::new(s).with(0, loot::PICKUP_ITEM).with(2, iid))?;
            loot::on_item_packet(ctx, st, sid, &p).await?;
        }
        Action::Resurrect => combat::resurrect(ctx, st, sid).await?,
        Action::EquipUpgrades => equip_upgrades(world, st, sid).await?,
        Action::LearnSkills => learn_skills(world, st, sid).await?,
        Action::Rest(_) => {}
        Action::Say(text) => chat::say(ctx, st, sid, &text),
        Action::Whisper { to, text } => {
            if let Some(&tsid) = st.by_cid.get(&to) {
                chat::whisper(st, sid, tsid, &text);
            }
        }
        Action::PartyInvite(cid) => {
            let s = &schema::PARTY_MANAGEMENT;
            let p = packet(s, Fields::new(s).with(0, party::mgmt::INVITE).with(2, cid))?;
            party::on_management(st, sid, &p);
        }
        Action::PartyLeave => {
            let s = &schema::PARTY_MANAGEMENT;
            let p = packet(s, Fields::new(s).with(0, party::mgmt::LEAVE))?;
            party::on_management(st, sid, &p);
        }
        Action::Note(text) => {
            if let Some(p) = st.players.get(&sid) {
                debug!("[archbot] {} : {text}", p.ch.name);
            }
        }
    }
    Ok(())
}

/// Accepte une invitation de groupe reçue par le bot `sid` de la part de `inviter` (CID).
pub fn accept_party_invitation(st: &mut WorldState, sid: u32, inviter: u32) -> anyhow::Result<()> {
    let s = &schema::PARTY_MANAGEMENT;
    let p = packet(s, Fields::new(s).with(0, party::mgmt::INVITE_ACCEPT).with(2, inviter))?;
    party::on_management(st, sid, &p);
    Ok(())
}

/// Porte les meilleurs objets portables de l'inventaire : pour chaque partie du corps, un objet de niveau
/// d'objet supérieur à celui porté (ou rien de porté), permis au personnage (niveau, race, classe, sexe) ; les
/// montures sont laissées de côté. Trois changements au plus par appel.
pub async fn equip_upgrades(world: &World, st: &mut WorldState, sid: u32) -> anyhow::Result<()> {
    let ctx = &world.ctx;
    let Some(p) = st.players.get(&sid) else { return Ok(()) };
    if p.dead || !p.in_game {
        return Ok(());
    }
    let f = &p.factors;
    let (level, race, class, gender) = (f.status[7] as i64, f.ftype[0] as i64, f.ftype[2] as i64, f.ftype[1] as i64);
    let mut candidates: Vec<(u32, i64, i64)> = Vec::new();
    for it in p.inventory.items.values() {
        let Some(e) = ctx.catalog.get(it.tid).and_then(|d| d.equip.as_ref()) else { continue };
        if e.kind == equipment::KIND_MOUNT || e.part == equipment::part::RIDE || e.part == equipment::part::LANCER {
            continue;
        }
        if equipment::check(e, level, race, class, gender).is_err() {
            continue;
        }
        let at = equipment::target_part(&p.inventory, e);
        let worn_level = p.inventory.equipment.get(&at).and_then(|w| ctx.catalog.get(w.tid)).and_then(|d| d.equip.as_ref()).map(|w| w.level);
        if worn_level.is_none_or(|w| e.level > w) {
            candidates.push((it.iid, at, e.level));
        }
    }
    // Le meilleur par partie, les parties les plus utiles d'abord.
    candidates.sort_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)));
    candidates.dedup_by(|a, b| a.1 == b.1);
    let name = p.ch.name.clone();
    let mut changed = 0;
    for (iid, at, lvl) in candidates.into_iter().take(3) {
        if equipment::equip(ctx, st, sid, iid, true).await? {
            changed += 1;
            info!("[archbot] {name} s'équipe (partie {at}, niveau d'objet {lvl})");
        }
    }
    let _ = changed;
    Ok(())
}

/// Apprend les compétences de sa classe que ses points permettent (du niveau requis le plus bas au plus haut),
/// puis améliore celles qu'il connaît. Cinq apprentissages ou améliorations au plus par appel ; le serveur
/// applique prix, charisme, points et arbres de maîtrise.
pub async fn learn_skills(world: &World, st: &mut WorldState, sid: u32) -> anyhow::Result<()> {
    let ctx = &world.ctx;
    let Some(p) = st.players.get(&sid) else { return Ok(()) };
    if p.dead || !p.in_game || p.factors.dirt[skills::DIRT_SKILL_POINT] <= 0 {
        return Ok(());
    }
    let class_col = skills::class_column(p.factors.ftype[0], p.factors.ftype[2]);
    let level = p.ch.level;
    let (points, money) = (p.factors.dirt[skills::DIRT_SKILL_POINT], p.ch.money as i64);
    let known: Vec<u32> = p.skills.list.iter().map(|s| s.tid).collect();
    // Candidates : compétences de la classe, de l'arbre ordinaire (pas héroïque), dont les conditions sont déjà
    // remplies (niveau, points, prix, arbre de maîtrise) ; le serveur refait tous les contrôles.
    let mut learnable: Vec<(i32, u32)> = ctx
        .skills
        .all()
        .filter(|t| !t.auto_attack && !t.is_product() && !t.is_evolution() && !t.archlord_only() && !t.is_mutation())
        .filter(|t| !known.contains(&t.tid))
        .filter_map(|t| {
            let lv = t.level(1)?;
            let fits = lv.allows(class_col) && lv.require_level <= level && lv.require_point > 0 && lv.require_point <= points && lv.require_heroic == 0 && lv.learn_cost <= money;
            let tree_ok = ctx.skills.mastery(t.tid, class_col).is_none_or(|m| !m.heroic && skills::mastery_unmet(&ctx.skills, &p.skills, m).is_none());
            (fits && tree_ok).then_some((lv.require_level, t.tid))
        })
        .collect();
    learnable.sort_unstable();
    let name = p.ch.name.clone();
    let mut done = 0;
    for (_, tid) in learnable {
        if done >= 5 || st.players.get(&sid).is_none_or(|p| p.factors.dirt[skills::DIRT_SKILL_POINT] <= 0) {
            break;
        }
        let r = skills::learn(ctx, st, sid, tid).await?;
        if r == skills::learn_result::SUCCESS {
            done += 1;
            info!("[archbot] {name} apprend la compétence {tid}");
        }
    }
    // Améliorations, les plus basses d'abord.
    let mut owned: Vec<(u32, u32)> = st.players.get(&sid).map(|p| p.skills.list.iter().map(|s| (s.level, s.id)).collect()).unwrap_or_default();
    owned.sort_unstable();
    for (_, id) in owned {
        if done >= 5 || st.players.get(&sid).is_none_or(|p| p.factors.dirt[skills::DIRT_SKILL_POINT] <= 0) {
            break;
        }
        if skills::upgrade(ctx, st, sid, id).await?.is_some_and(|r| r == skills::upgrade_result::SUCCESS) {
            done += 1;
        }
    }
    Ok(())
}
