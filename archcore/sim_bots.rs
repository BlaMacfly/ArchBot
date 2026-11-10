//! `archcore sim bots [compte] [mdp] [perso] [n]` : un client simulé (compte MJ) observe et commande les bots à
//! travers le vrai protocole, et rend un verdict par point : PASS, FAIL, BLOCKED ou NOT RUN.
//!
//! ```text
//!   1. création et visibilité   /bot spawn n, /bot list, les bots amenés devant le MJ (/move "nom") : AGPMCHARACTER op 0
//!   2. déplacement              AGPMOPTIMIZEDCHARMOVE (0x40) d'un bot en vue avec une destination
//!   3. compagnon                /bot hire <nom> : réponse et invitation de groupe du bot, groupe formé, suivi du MJ qui
//!                               s'éloigne (0x40 du bot vers lui), « stay » chuchoté (réponse), « status », « come »
//!   4. combat                   le MJ attaque un monstre au point de chasse : le compagnon (défensif) frappe la même
//!                               cible (0x41 du bot) ; « aggressive » puis le bot engage de lui-même
//!   5. métriques                /bot stats
//!   6. départ                   /bot dismiss, /bot despawn <nom> : retrait de la vue (AGPMCHARACTER op 3/2)
//! ```
//! `archcore sim bots <mj> <mdp> <perso MJ> companion <compte> <mdp> <perso> [minutes]` : essai long d'un compagnon
//! engagé par un joueur ordinaire (sans droits MJ) ; le MJ ne sert qu'à le poser au point de chasse et à relever
//! les métriques. Le joueur engage un bot (`/bot hire`), accepte son invitation, chasse avec lui (attitude défensive :
//! le compagnon frappe la cible du joueur), puis se promène (attitude offensive : le compagnon chasse autour de lui) ;
//! à la fin, le MJ donne 5 000 000 000 points d'expérience au compagnon (`/exp`, au-delà de 2³²) et son statut doit
//! les compter. Relevés : déplacements et coups du compagnon, cibles, distance au maître, `/bot stats` chaque minute.
//!
//! Tous les paquets utiles (personnages, retraits, déplacements, coups, vues de monstres) sont collectés pendant les
//! attentes : rien n'est écarté. Le serveur doit tourner avec ArchBot (compte MJ de niveau ArchBot.CommandLevel au
//! moins). Rien ici ne prouve le rendu dans le client ArchLord lui-même : seulement ce que le serveur envoie à un client.

use super::{enter_world, step, InWorld, Recv, SeenMonster};
use archcore_net::schema::{self, chat_type, mini};
use archcore_net::{decode_mini, Fields, Vec3};
use std::time::Duration;

/// Point de chasse : Vers verts au nord d'Anchorville (comme `sim combat`).
const HUNT: Vec3 = Vec3 { x: -466_468.0, y: 4_700.0, z: -50_010.0 };
/// Devant le village d'Anchorville : point d'apparition des personnages humains (StartPosition.1).
const VILLAGE: Vec3 = Vec3 { x: -475_955.0, y: 4_608.5, z: -66_219.0 };
const STOP: i64 = 0x04;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    Pass,
    Fail,
    Blocked,
    NotRun,
}

struct Report(Vec<(String, Verdict, String)>);

impl Report {
    fn note(&mut self, what: &str, v: Verdict, detail: impl Into<String>) {
        let detail = detail.into();
        let tag = match v {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Blocked => "BLOCKED",
            Verdict::NotRun => "NOT RUN",
        };
        println!("  [{tag}] {what}{}", if detail.is_empty() { String::new() } else { format!(" — {detail}") });
        self.0.push((what.to_string(), v, detail));
    }
}

/// Ce que le client a vu passer : personnages (CID, nom), retraits, déplacements (CID, destination), coups
/// (attaquant, cible, résultat), monstres.
#[derive(Default)]
struct Obs {
    players: Vec<(i64, String)>,
    removed: Vec<i64>,
    moves: Vec<(i64, Vec3)>,
    actions: Vec<(i64, i64, i64)>,
    monsters: Vec<SeenMonster>,
}

impl Obs {
    fn absorb(&mut self, q: &Recv) -> bool {
        let (c, mv, act, ov) = (&schema::CHARACTER, &schema::OPT_CHAR_MOVE, &schema::OPT_CHAR_ACTION, &schema::OPT_VIEW);
        if q.is(c, Some(0)) {
            let cid = q.int(1).unwrap_or(0);
            if (cid as u32) < crate::world::npcs::NPC_CID_BASE && !self.players.iter().any(|p| p.0 == cid) {
                self.players.push((cid, q.fields().text(3).unwrap_or_default().trim_end_matches('\0').to_string()));
            }
            return true;
        }
        if q.is(c, Some(2)) || q.is(c, Some(3)) {
            self.removed.push(q.int(1).unwrap_or(0));
            return true;
        }
        if q.is(mv, None) {
            if let Some(d) = q.fields().vec3(2).filter(|_| q.int(5).is_some_and(|f| f & STOP == 0)) {
                self.moves.push((q.int(0).unwrap_or(0), d));
            }
            return true;
        }
        if q.is(act, None) {
            self.actions.push((q.int(0).unwrap_or(0), q.int(1).unwrap_or(0), q.int(2).unwrap_or(0)));
            return true;
        }
        if q.is(ov, Some(1)) {
            if let Ok(view) = decode_mini(&mini::OPT_VIEW_CHAR, q.fields().bytes(1).unwrap_or_default()) {
                if view.int(0).unwrap_or(0) >= crate::world::monsters::MONSTER_CID_BASE as i64 {
                    self.monsters.push(SeenMonster::from_view(&view));
                }
            }
            return true;
        }
        false
    }

    fn name(&self, cid: i64) -> String {
        self.players.iter().find(|p| p.0 == cid).map(|p| p.1.clone()).unwrap_or_else(|| format!("{cid:#x}"))
    }
}

/// Attend le premier paquet qui satisfait `pred` sous `ms`, en collectant au passage tout ce qui intéresse `obs`.
async fn wait(w: &mut InWorld, obs: &mut Obs, pred: impl Fn(&Recv) -> bool, ms: u64, what: &str) -> anyhow::Result<Recv> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now()).as_millis() as u64;
        if left == 0 {
            anyhow::bail!("attente dépassée : {what}");
        }
        let q = w.world.expect_where(|q| pred(q) || is_interesting(q), left, what).await?;
        if pred(&q) {
            obs.absorb(&q);
            return Ok(q);
        }
        obs.absorb(&q);
    }
}

fn is_interesting(q: &Recv) -> bool {
    let (c, mv, act, ov) = (&schema::CHARACTER, &schema::OPT_CHAR_MOVE, &schema::OPT_CHAR_ACTION, &schema::OPT_VIEW);
    q.is(c, Some(0)) || q.is(c, Some(2)) || q.is(c, Some(3)) || q.is(mv, None) || q.is(act, None) || q.is(ov, Some(1))
}

/// Collecte pendant `ms` sans rien attendre de précis.
async fn listen(w: &mut InWorld, obs: &mut Obs, ms: u64) {
    let _ = wait(w, obs, |_| false, ms, "écoute").await;
}

fn say(ch: &'static archcore_net::Schema, cid: u32, text: &str) -> Fields {
    Fields::new(ch).with(0, 0).with(1, cid).with(2, chat_type::NORMAL).with(6, crate::tools::to_cp1252(text))
}

fn whisper(ch: &'static archcore_net::Schema, cid: u32, to: &str, text: &str) -> Fields {
    Fields::new(ch).with(0, 0).with(1, cid).with(2, chat_type::WHISPER).with(5, to).with(6, crate::tools::to_cp1252(text))
}

fn text(q: &Recv) -> String {
    q.fields().text(6).unwrap_or_default()
}

fn is_system(q: &Recv) -> bool {
    q.ptype == schema::CHATTING.id && q.int(2) == Some(chat_type::WHOLE_WORLD)
}

fn is_whisper_from(q: &Recv, who: &str) -> bool {
    q.ptype == schema::CHATTING.id && q.int(2) == Some(chat_type::WHISPER) && q.fields().text(4).as_deref() == Some(who)
}

async fn system_reply(w: &mut InWorld, obs: &mut Obs, needle: &str, ms: u64) -> anyhow::Result<String> {
    let r = wait(w, obs, |q| is_system(q) && text(q).contains(needle), ms, &format!("réponse « {needle} »")).await?;
    Ok(text(&r))
}

async fn system_lines(w: &mut InWorld, obs: &mut Obs, ms: u64) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(r) = wait(w, obs, is_system, ms, "lignes").await {
        out.push(text(&r));
    }
    out
}

async fn chat(w: &mut InWorld, cmd: &str) -> anyhow::Result<()> {
    tokio::time::sleep(Duration::from_millis(150)).await;
    let ch = &schema::CHATTING;
    let cid = w.cid;
    w.world.send(ch, say(ch, cid, cmd)).await
}

/// Noms des bots d'après `/bot list` (« Nom (niv. … »).
fn names_of(list: &[String]) -> Vec<String> {
    list.iter().filter(|l| l.contains(" (niv.")).filter_map(|l| l.split(" (niv.").next().map(str::trim).map(str::to_string)).filter(|n| !n.is_empty() && !n.contains(' ')).collect()
}

pub async fn run(user: &str, pass: &str, char_name: &str, args: &[String]) -> anyhow::Result<()> {
    if args.first().is_some_and(|a| a == "companion") {
        let a = |i: usize, d: &str| args.get(i).cloned().unwrap_or_else(|| d.to_string());
        let minutes: u64 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(5).clamp(1, 120);
        return Box::pin(companion(user, pass, char_name, &a(1, "test"), &a(2, "test"), &a(3, "Testeur"), minutes)).await;
    }
    if args.first().is_some_and(|a| a == "load") {
        return load(user, pass, char_name, args.get(1).map(String::as_str).unwrap_or("1,10,25,50,100")).await;
    }
    let n: usize = args.first().and_then(|v| v.parse().ok()).unwrap_or(3).clamp(1, 50);
    let cfg = crate::config::load()?;
    let (act, ch, pm, pt) = (&schema::OPT_CHAR_ACTION, &schema::CHATTING, &schema::PARTY_MANAGEMENT, &schema::PARTY);
    let mut report = Report(Vec::new());
    let mut obs = Obs::default();
    obs.monsters = Vec::new();

    step(&format!("{char_name} entre en jeu et se place devant Anchorville"));
    let mut w = enter_world(&cfg, user, pass, char_name, "").await?;
    let me = w.cid;
    chat(&mut w, &format!("/move {},{}", VILLAGE.x, VILLAGE.z)).await?;
    system_reply(&mut w, &mut obs, "porté en", 5000).await?;
    // Le MJ ne doit pas être tué par les monstres : niveau 20.
    chat(&mut w, "/level 20").await?;
    let _ = system_lines(&mut w, &mut obs, 800).await;

    step("1. création et visibilité : /bot stats, /bot spawn, bots amenés devant le MJ");
    chat(&mut w, "/bot stats").await?;
    match system_reply(&mut w, &mut obs, "ArchBot :", 5000).await {
        Ok(line) => report.note("métriques accessibles (/bot stats)", Verdict::Pass, line),
        Err(e) => {
            report.note("métriques accessibles (/bot stats)", Verdict::Blocked, format!("{e} — ArchBot désactivé ou niveau MJ insuffisant"));
            println!("\narrêt : le module ne répond pas");
            return Ok(());
        }
    }
    let _ = system_lines(&mut w, &mut obs, 500).await;
    chat(&mut w, &format!("/bot spawn {n}")).await?;
    let spawned = system_reply(&mut w, &mut obs, "entré(s) en jeu", 20_000).await?;
    report.note(&format!("/bot spawn {n}"), Verdict::Pass, spawned.clone());
    chat(&mut w, "/bot list").await?;
    let list = system_lines(&mut w, &mut obs, 1500).await;
    for l in &list {
        println!("    {l}");
    }
    let names = names_of(&list);
    // Les bots apparaissent au village de leur race : ceux qu'on va observer sont amenés devant le MJ.
    let brought: Vec<String> = names.iter().take(n).cloned().collect();
    for (i, name) in brought.iter().enumerate() {
        chat(&mut w, &format!("/move {},{} \"{name}\"", VILLAGE.x + 300.0 * (i as f32 + 1.0), VILLAGE.z + 200.0)).await?;
        let _ = system_reply(&mut w, &mut obs, "porté en", 5000).await;
    }
    listen(&mut w, &mut obs, 1500).await;
    let seen_names: Vec<String> = obs.players.iter().map(|p| p.1.clone()).collect();
    let missing: Vec<&String> = brought.iter().filter(|b| !seen_names.contains(b)).collect();
    if missing.is_empty() && !brought.is_empty() {
        report.note("bots visibles du client (AGPMCHARACTER op 0)", Verdict::Pass, format!("{} personnage(s) reçus : {}", obs.players.len(), seen_names.join(", ")));
    } else {
        report.note("bots visibles du client (AGPMCHARACTER op 0)", Verdict::Fail, format!("attendus {brought:?}, reçus {seen_names:?}"));
    }
    let bot_cid = |obs: &Obs, name: &str| obs.players.iter().find(|p| p.1 == name).map(|p| p.0);
    let brought_cids: Vec<i64> = brought.iter().filter_map(|b| bot_cid(&obs, b)).collect();

    step("2. déplacement : un bot en vue se déplace (0x40 avec destination)");
    let started = tokio::time::Instant::now();
    while started.elapsed() < Duration::from_secs(45) && !obs.moves.iter().any(|m| brought_cids.contains(&m.0)) {
        listen(&mut w, &mut obs, 1000).await;
    }
    match obs.moves.iter().find(|m| brought_cids.contains(&m.0)) {
        Some((cid, dest)) => report.note("déplacement d'un bot reçu", Verdict::Pass, format!("{} vers {:.0}, {:.0}", obs.name(*cid), dest.x, dest.z)),
        None if brought_cids.is_empty() => report.note("déplacement d'un bot reçu", Verdict::NotRun, "aucun bot en vue"),
        None => report.note("déplacement d'un bot reçu", Verdict::Fail, "aucun 0x40 d'un bot en 45 s"),
    }

    step("3. compagnon : /bot hire <nom>, groupe, suivi, ordres");
    let companion = brought.first().cloned().unwrap_or_default();
    let companion_cid = bot_cid(&obs, &companion);
    chat(&mut w, &format!("/bot hire {companion}")).await?;
    let mut hired: Option<String> = None;
    let mut invite_from: Option<i64> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while hired.is_none() && tokio::time::Instant::now() < deadline {
        let Ok(q) = wait(&mut w, &mut obs, |q| q.is(pm, Some(0)) || (is_system(q) && text(q).contains(" : ")), 10_000, "réponse à /bot hire").await else { break };
        if q.is(pm, Some(0)) {
            invite_from = q.int(1);
        } else {
            hired = Some(text(&q));
        }
    }
    let hired = hired.unwrap_or_default();
    let ok = hired.contains("À vos ordres");
    report.note(&format!("/bot hire {companion}"), if ok { Verdict::Pass } else { Verdict::Fail }, hired.clone());
    if ok {
        if invite_from.is_none() {
            if let Ok(q) = wait(&mut w, &mut obs, |q| q.is(pm, Some(0)), 3000, "invitation").await {
                invite_from = q.int(1);
            }
        }
        match invite_from {
            Some(bcid) => {
                w.world.send(pm, Fields::new(pm).with(0, 2).with(1, me).with(2, bcid)).await?;
                match wait(&mut w, &mut obs, |q| q.is(pt, Some(0)), 5000, "groupe formé").await {
                    Ok(_) => report.note("groupe formé avec le compagnon", Verdict::Pass, format!("{companion} (CID {bcid:#x})")),
                    Err(e) => report.note("groupe formé avec le compagnon", Verdict::Fail, e.to_string()),
                }
            }
            None => report.note("invitation de groupe du compagnon", Verdict::Fail, "aucune invitation reçue (ArchBot.AllowParty = 0 ?)"),
        }
        let bcid = companion_cid.or(invite_from).unwrap_or(0);
        // Le MJ s'éloigne de 2500 unités : le compagnon le suit (0x40 du bot vers une destination proche du MJ).
        let away = Vec3::new(VILLAGE.x + 2500.0, VILLAGE.y, VILLAGE.z);
        chat(&mut w, &format!("/move {},{}", away.x, away.z)).await?;
        let _ = system_reply(&mut w, &mut obs, "porté en", 5000).await;
        let near_me = |d: Vec3| ((d.x - away.x) as f64).hypot((d.z - away.z) as f64);
        let mv = &schema::OPT_CHAR_MOVE;
        match wait(&mut w, &mut obs, |q| q.is(mv, None) && q.int(0) == Some(bcid) && q.fields().vec3(2).is_some_and(|d| near_me(d) < 1200.0), 15_000, "suivi").await {
            Ok(p) => {
                let d = p.fields().vec3(2).unwrap_or_default();
                report.note("le compagnon suit le MJ", Verdict::Pass, format!("destination {:.0}, {:.0} à {:.0} du MJ", d.x, d.z, near_me(d)));
            }
            Err(e) => report.note("le compagnon suit le MJ", Verdict::Fail, e.to_string()),
        }
        for (order, needle) in [("stay", "reste"), ("status", "compagnon de")] {
            tokio::time::sleep(Duration::from_millis(200)).await;
            w.world.send(ch, whisper(ch, me, &companion, order)).await?;
            match wait(&mut w, &mut obs, |q| is_whisper_from(q, &companion) && text(q).contains(needle), 5000, order).await {
                Ok(r) => report.note(&format!("ordre chuchoté « {order} »"), Verdict::Pass, text(&r)),
                Err(e) => report.note(&format!("ordre chuchoté « {order} »"), Verdict::Fail, e.to_string()),
            }
        }
        chat(&mut w, "/bot come").await?;
        match wait(&mut w, &mut obs, |q| is_whisper_from(q, &companion) && text(q).contains("arrive"), 5000, "come").await {
            Ok(r) => report.note("ordre « /bot come »", Verdict::Pass, text(&r)),
            Err(e) => report.note("ordre « /bot come »", Verdict::Fail, e.to_string()),
        }

        step("4. combat : au point de chasse, le MJ attaque un monstre ; le compagnon frappe la même cible");
        chat(&mut w, &format!("/move {},{}", HUNT.x, HUNT.z)).await?;
        let _ = system_reply(&mut w, &mut obs, "porté en", 5000).await;
        chat(&mut w, &format!("/move {},{} \"{companion}\"", HUNT.x + 200.0, HUNT.z + 200.0)).await?;
        let _ = system_reply(&mut w, &mut obs, "porté en", 5000).await;
        listen(&mut w, &mut obs, 2000).await;
        let dist = |p: Vec3| (((p.x - HUNT.x) as f64).powi(2) + ((p.z - HUNT.z) as f64).powi(2)).sqrt();
        let worm = |m: &SeenMonster| m.name == "Ver vert" || m.name == "Green Worm";
        let target = obs
            .monsters
            .iter()
            .filter(|m| m.hp > 0 && dist(m.pos) < 6000.0)
            .min_by_key(|m| (if worm(m) { 0 } else { 1 }, dist(m.pos) as i64))
            .cloned();
        match target {
            None => report.note("compagnon : attaque la cible du MJ", Verdict::Blocked, "aucun monstre vivant en vue au point de chasse"),
            Some(t) => {
                let tcid = t.cid as i64;
                println!("  cible : {} (CID {tcid:#x}), {} PV, à {:.0} unités", t.name, t.hp, dist(t.pos));
                w.world.send(act, Fields::new(act).with(0, me).with(1, t.cid).with(6, 1)).await?;
                match wait(&mut w, &mut obs, |q| q.is(act, None) && q.int(0) == Some(bcid) && q.int(1) == Some(tcid), 30_000, "coup du compagnon").await {
                    Ok(p) => report.note("compagnon : attaque la cible du MJ", Verdict::Pass, format!("coup sur {} (résultat {})", t.name, p.int(2).unwrap_or(0))),
                    Err(e) => report.note("compagnon : attaque la cible du MJ", Verdict::Fail, e.to_string()),
                }
                chat(&mut w, "/bot aggressive").await?;
                let _ = wait(&mut w, &mut obs, |q| is_whisper_from(q, &companion) && text(q).contains("aggressive"), 5000, "attitude").await;
                match wait(&mut w, &mut obs, |q| q.is(act, None) && q.int(0) == Some(bcid) && q.int(1) != Some(tcid), 60_000, "coup d'initiative").await {
                    Ok(p) => report.note("compagnon offensif : engage de lui-même", Verdict::Pass, format!("cible {:#x}", p.int(1).unwrap_or(0))),
                    Err(e) => report.note("compagnon offensif : engage de lui-même", Verdict::Fail, e.to_string()),
                }
            }
        }
    } else {
        report.note("compagnon : attaque la cible du MJ", Verdict::NotRun, "pas de compagnon");
    }

    step("5. métriques");
    chat(&mut w, "/bot stats").await?;
    let lines = system_lines(&mut w, &mut obs, 1500).await;
    for l in &lines {
        println!("    {l}");
    }
    report.note("/bot stats après activité", if lines.iter().any(|l| l.contains("tour(s) d'IA")) { Verdict::Pass } else { Verdict::Fail }, lines.first().cloned().unwrap_or_default());

    step("6. départ : /bot dismiss puis /bot despawn <nom>");
    chat(&mut w, "/bot dismiss").await?;
    match system_reply(&mut w, &mut obs, "Au revoir", 5000).await {
        Ok(l) => report.note("/bot dismiss", Verdict::Pass, l),
        Err(e) => report.note("/bot dismiss", if ok { Verdict::Fail } else { Verdict::NotRun }, e.to_string()),
    }
    // L'ancien compagnon est à côté du MJ : son départ doit retirer son personnage de la vue.
    let bcid = companion_cid.unwrap_or(0);
    obs.removed.clear();
    chat(&mut w, &format!("/bot despawn {companion}")).await?;
    match system_reply(&mut w, &mut obs, "sorti(s) du monde", 15_000).await {
        Ok(l) => report.note(&format!("/bot despawn {companion}"), Verdict::Pass, l),
        Err(e) => report.note(&format!("/bot despawn {companion}"), Verdict::Fail, e.to_string()),
    }
    listen(&mut w, &mut obs, 1500).await;
    if obs.removed.contains(&bcid) {
        report.note("retrait du bot de la vue (AGPMCHARACTER op 2/3)", Verdict::Pass, format!("{companion} (CID {bcid:#x})"));
    } else {
        report.note("retrait du bot de la vue (AGPMCHARACTER op 2/3)", if companion_cid.is_some() { Verdict::Fail } else { Verdict::NotRun }, format!("retraits reçus : {:?}", obs.removed));
    }
    chat(&mut w, &format!("/bot despawn {}", n.saturating_sub(1))).await?;
    let _ = system_reply(&mut w, &mut obs, "sorti(s) du monde", 15_000).await;

    println!("\n=== bilan « sim bots »");
    let count = |v: Verdict| report.0.iter().filter(|(_, x, _)| *x == v).count();
    for (what, v, _) in &report.0 {
        println!("  {:8} {what}", format!("{v:?}").to_uppercase());
    }
    println!("  PASS {} · FAIL {} · BLOCKED {} · NOT RUN {}", count(Verdict::Pass), count(Verdict::Fail), count(Verdict::Blocked), count(Verdict::NotRun));
    anyhow::ensure!(count(Verdict::Fail) == 0, "{} point(s) en échec", count(Verdict::Fail));
    Ok(())
}

/// Points de la promenade autour du point de chasse (rayon 1500).
fn walk_point(i: usize) -> Vec3 {
    let a = i as f32 * std::f32::consts::FRAC_PI_3;
    Vec3::new(HUNT.x + 1500.0 * a.cos(), HUNT.y, HUNT.z + 1500.0 * a.sin())
}

fn dist2(a: Vec3, b: Vec3) -> f64 {
    ((a.x - b.x) as f64).hypot((a.z - b.z) as f64)
}

/// Chiffre « … et N point(s) d'expérience gagnés » d'une ligne de statut.
fn exp_in_status(line: &str) -> Option<u64> {
    let head = line.split(" point(s) d'expérience gagnés").next()?;
    head.rsplit(' ').next()?.parse().ok()
}

/// Essai long d'un compagnon engagé par un joueur ordinaire (voir l'en-tête).
async fn companion(gm_user: &str, gm_pass: &str, gm_char: &str, user: &str, pass: &str, char_name: &str, minutes: u64) -> anyhow::Result<()> {
    let cfg = crate::config::load()?;
    let (act, pm, pt, mv) = (&schema::OPT_CHAR_ACTION, &schema::PARTY_MANAGEMENT, &schema::PARTY, &schema::OPT_CHAR_MOVE);
    let mut report = Report(Vec::new());
    let (mut gobs, mut obs) = (Obs::default(), Obs::default());

    step(&format!("{gm_char} (MJ) et {char_name} (joueur) entrent en jeu"));
    let mut gm = enter_world(&cfg, gm_user, gm_pass, gm_char, "MJ-").await?;
    let mut pl = enter_world(&cfg, user, pass, char_name, "J-").await?;
    let me = pl.cid;
    let level0 = pl.level;
    // Le joueur ne doit pas mourir en quelques coups : niveau 15 le temps de l'essai (rétabli à la fin).
    chat(&mut gm, &format!("/level 15 \"{char_name}\"")).await?;
    let _ = system_lines(&mut gm, &mut gobs, 800).await;
    super::gm_place(&mut gm, char_name, HUNT).await?;
    pl.world.send(mv, Fields::new(mv).with(0, me).with(1, HUNT).with(2, HUNT).with(5, STOP).with(6, 0)).await?;
    listen(&mut pl, &mut obs, 2000).await;
    chat(&mut gm, "/bot stats reset").await?;
    let _ = system_reply(&mut gm, &mut gobs, "remises", 5000).await;

    step(&format!("{char_name} engage un compagnon (/bot hire), accepte son invitation"));
    chat(&mut pl, "/bot hire").await?;
    let (mut hired, mut invite_from) = (None::<String>, None::<i64>);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while (hired.is_none() || invite_from.is_none()) && tokio::time::Instant::now() < deadline {
        let Ok(q) = wait(&mut pl, &mut obs, |q| q.is(pm, Some(0)) || (is_system(q) && text(q).contains(" : ")), 15_000, "réponse à /bot hire").await else { break };
        if q.is(pm, Some(0)) {
            invite_from = q.int(1);
        } else {
            hired = Some(text(&q));
        }
    }
    let hired = hired.unwrap_or_default();
    let companion = hired.split(" : ").next().unwrap_or_default().trim().to_string();
    let ok = hired.contains("À vos ordres") && !companion.is_empty();
    report.note("/bot hire (joueur sans droits MJ)", if ok { Verdict::Pass } else { Verdict::Fail }, hired.clone());
    let Some(bcid) = invite_from.filter(|_| ok) else {
        report.note("invitation de groupe du compagnon", Verdict::Fail, "aucune invitation");
        return finish(report);
    };
    pl.world.send(pm, Fields::new(pm).with(0, 2).with(1, me).with(2, bcid)).await?;
    match wait(&mut pl, &mut obs, |q| q.is(pt, Some(0)), 5000, "groupe formé").await {
        Ok(_) => report.note("groupe formé avec le compagnon", Verdict::Pass, format!("{companion} (CID {bcid:#x})")),
        Err(e) => report.note("groupe formé avec le compagnon", Verdict::Fail, e.to_string()),
    }
    let comp_level = bot_level(&mut gm, &mut gobs, &companion).await?;
    println!("  compagnon {companion}, niveau {comp_level}");

    // Le compagnon rejoint son maître (il peut venir de loin : bot libre le plus proche, 20 000 unités au plus).
    let joined = {
        let t0 = tokio::time::Instant::now();
        loop {
            listen(&mut pl, &mut obs, 1000).await;
            let near = obs.moves.iter().rev().find(|m| m.0 == bcid).is_some_and(|m| dist2(m.1, HUNT) < 1500.0) || obs.players.iter().any(|p| p.0 == bcid);
            if near || t0.elapsed() > Duration::from_secs(90) {
                break near;
            }
        }
    };
    report.note("le compagnon rejoint son maître", if joined { Verdict::Pass } else { Verdict::Fail }, format!("{} déplacement(s) du compagnon reçus", obs.moves.iter().filter(|m| m.0 == bcid).count()));

    let total = Duration::from_secs(minutes * 60);
    let half = total / 2;
    let t0 = tokio::time::Instant::now();
    let (mut asked, mut next_minute, mut walk_i) = (0u64, Duration::from_secs(60), 0usize);
    let mut my_pos = HUNT;
    let mut max_gap = 0f64;
    let mut aggressive = false;
    step(&format!("chasse : {minutes} min (moitié défensive, le joueur attaque ; moitié offensive, le joueur se promène)"));
    while t0.elapsed() < total {
        let elapsed = t0.elapsed();
        if elapsed < half {
            // Le joueur attaque le monstre vivant le plus proche toutes les 15 s.
            if elapsed.as_secs() / 15 >= asked {
                asked += 1;
                if let Some(t) = obs.monsters.iter().filter(|m| m.hp > 0 && dist2(m.pos, my_pos) < 2500.0).min_by_key(|m| dist2(m.pos, my_pos) as i64).cloned() {
                    pl.world.send(act, Fields::new(act).with(0, me).with(1, t.cid).with(6, 1)).await?;
                    my_pos = t.pos;
                }
            }
        } else {
            if !aggressive {
                aggressive = true;
                chat(&mut pl, "/bot aggressive").await?;
            }
            // Promenade : un point toutes les 30 s.
            if (elapsed - half).as_secs() / 30 >= walk_i as u64 {
                let dest = walk_point(walk_i);
                walk_i += 1;
                pl.world.send(mv, Fields::new(mv).with(0, me).with(1, my_pos).with(2, dest).with(5, 0).with(6, 0)).await?;
                my_pos = dest;
            }
        }
        let before = obs.moves.len();
        listen(&mut pl, &mut obs, 1000).await;
        for m in obs.moves[before..].iter().filter(|m| m.0 == bcid) {
            max_gap = max_gap.max(dist2(m.1, my_pos));
        }
        if t0.elapsed() >= next_minute {
            next_minute += Duration::from_secs(60);
            chat(&mut gm, "/bot stats").await?;
            let lines = system_lines(&mut gm, &mut gobs, 1200).await;
            println!("  [{} s] {}", t0.elapsed().as_secs(), lines.iter().take(4).cloned().collect::<Vec<_>>().join(" | "));
            gobs = Obs::default();
        }
    }
    let moves = obs.moves.iter().filter(|m| m.0 == bcid).count();
    let hits: Vec<&(i64, i64, i64)> = obs.actions.iter().filter(|a| a.0 == bcid).collect();
    let mut targets: Vec<i64> = hits.iter().map(|a| a.1).collect();
    targets.sort_unstable();
    targets.dedup();
    report.note("le compagnon se déplace avec son maître", if moves > 0 { Verdict::Pass } else { Verdict::Fail }, format!("{moves} déplacement(s), écart maximal destination/maître {max_gap:.0} unités"));
    report.note("le compagnon combat", if !hits.is_empty() { Verdict::Pass } else { Verdict::Fail }, format!("{} coup(s) sur {} cible(s)", hits.len(), targets.len()));

    step("statut, expérience en 64 bits, départ");
    w_status(&mut pl, &mut obs, me, &companion, &mut report, "statut après la chasse", None).await?;
    // Niveau atteint pendant la chasse, rétabli après le don d'expérience.
    let comp_level = bot_level(&mut gm, &mut gobs, &companion).await?;
    chat(&mut gm, &format!("/exp 5000000000 \"{companion}\"")).await?;
    match system_reply(&mut gm, &mut gobs, "point(s) d'expérience", 5000).await {
        Ok(l) => println!("  {l}"),
        Err(e) => report.note("/exp 5000000000 au compagnon", Verdict::Fail, e.to_string()),
    }
    // Le cerveau voit le gain au tour suivant (une pensée par seconde au plus avec beaucoup de bots).
    tokio::time::sleep(Duration::from_secs(3)).await;
    w_status(&mut pl, &mut obs, me, &companion, &mut report, "statut : 5 000 000 000 points d'expérience comptés (64 bits)", Some(5_000_000_000)).await?;
    chat(&mut pl, "/bot dismiss").await?;
    match system_reply(&mut pl, &mut obs, "Au revoir", 5000).await {
        Ok(l) => report.note("/bot dismiss", Verdict::Pass, l),
        Err(e) => report.note("/bot dismiss", Verdict::Fail, e.to_string()),
    }
    chat(&mut gm, "/bot stats").await?;
    for l in system_lines(&mut gm, &mut gobs, 1500).await {
        println!("    {l}");
    }
    // Niveaux rétablis (joueur de test, compagnon).
    chat(&mut gm, &format!("/level {} \"{char_name}\"", level0.max(1))).await?;
    let _ = system_lines(&mut gm, &mut gobs, 800).await;
    chat(&mut gm, &format!("/level {comp_level} \"{companion}\"")).await?;
    let _ = system_lines(&mut gm, &mut gobs, 800).await;
    finish(report)
}

/// Niveau du bot `name` d'après `/bot list` (MJ) ; 1 s'il n'y figure pas.
async fn bot_level(gm: &mut InWorld, obs: &mut Obs, name: &str) -> anyhow::Result<i64> {
    chat(gm, "/bot list").await?;
    let list = system_lines(gm, obs, 1500).await;
    Ok(list.iter().find(|l| l.starts_with(&format!("{name} (niv. "))).and_then(|l| l.split("(niv. ").nth(1)?.split(',').next()?.parse().ok()).unwrap_or(1))
}

/// Statut du compagnon chuchoté ; avec `min_exp`, l'expérience gagnée qu'il annonce doit l'atteindre.
async fn w_status(pl: &mut InWorld, obs: &mut Obs, me: u32, companion: &str, report: &mut Report, what: &str, min_exp: Option<u64>) -> anyhow::Result<()> {
    let ch = &schema::CHATTING;
    tokio::time::sleep(Duration::from_millis(200)).await;
    pl.world.send(ch, whisper(ch, me, companion, "status")).await?;
    match wait(pl, obs, |q| is_whisper_from(q, companion) && text(q).contains("compagnon de"), 5000, "status").await {
        Ok(r) => {
            let line = text(&r);
            let exp = exp_in_status(&line).unwrap_or(0);
            let v = if min_exp.is_none_or(|m| exp >= m) { Verdict::Pass } else { Verdict::Fail };
            report.note(what, v, line);
        }
        Err(e) => report.note(what, Verdict::Fail, e.to_string()),
    }
    Ok(())
}

fn finish(report: Report) -> anyhow::Result<()> {
    println!("\n=== bilan « sim bots companion »");
    let count = |v: Verdict| report.0.iter().filter(|(_, x, _)| *x == v).count();
    for (what, v, _) in &report.0 {
        println!("  {:8} {what}", format!("{v:?}").to_uppercase());
    }
    println!("  PASS {} · FAIL {} · BLOCKED {} · NOT RUN {}", count(Verdict::Pass), count(Verdict::Fail), count(Verdict::Blocked), count(Verdict::NotRun));
    anyhow::ensure!(count(Verdict::Fail) == 0, "{} point(s) en échec", count(Verdict::Fail));
    Ok(())
}

/// `archcore sim bots load [paliers]` : test de charge. Pour chaque palier, la population visée est fixée
/// (`/bot population n`), on attend que les bots soient en jeu, les métriques sont remises à zéro, le serveur
/// tourne 60 s, puis `/bot stats` est relevé. La mesure du CPU et de la mémoire du processus se fait à côté
/// (Get-Process, top…) ; voir docs/TESTS.md.
async fn load(user: &str, pass: &str, char_name: &str, steps: &str) -> anyhow::Result<()> {
    let cfg = crate::config::load()?;
    let mut obs = Obs::default();
    step(&format!("{char_name} entre en jeu (test de charge, paliers {steps})"));
    let mut w = enter_world(&cfg, user, pass, char_name, "").await?;
    let bots_in = |line: &str| line.split("ArchBot : ").nth(1).and_then(|r| r.split(' ').next()).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
    let mut rows: Vec<String> = Vec::new();
    for n in steps.split(',').filter_map(|s| s.trim().parse::<usize>().ok()) {
        step(&format!("palier {n} bot(s)"));
        chat(&mut w, &format!("/bot population {n}")).await?;
        let r = system_reply(&mut w, &mut obs, "visée", 5000).await?;
        println!("  {r}");
        // Entrée progressive (une par ArchBot.SpawnIntervalMs) ou sortie des bots en trop : on attend le compte exact
        // (le compagnon éventuel compte en plus, la population visée ne portant que sur les bots autonomes).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60 + 2 * n as u64);
        let mut count = 0;
        while tokio::time::Instant::now() < deadline {
            chat(&mut w, "/bot stats").await?;
            let line = system_reply(&mut w, &mut obs, "ArchBot :", 5000).await?;
            let _ = system_lines(&mut w, &mut obs, 300).await;
            count = bots_in(&line);
            if count == n {
                break;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        println!("  {count} bot(s) en jeu");
        chat(&mut w, "/bot stats reset").await?;
        let _ = system_reply(&mut w, &mut obs, "remises", 5000).await;
        let t0 = chrono::Local::now().format("%H:%M:%S").to_string();
        listen(&mut w, &mut obs, 60_000).await;
        let t1 = chrono::Local::now().format("%H:%M:%S").to_string();
        chat(&mut w, "/bot stats").await?;
        let line = system_reply(&mut w, &mut obs, "ArchBot :", 5000).await?;
        let rest = system_lines(&mut w, &mut obs, 500).await;
        println!("  {t0} → {t1} : {line}");
        for l in &rest {
            println!("    {l}");
        }
        rows.push(format!("| {count} | {t0}–{t1} | {} | {} |", line.split(" : ").nth(1).unwrap_or(""), rest.iter().find(|l| l.starts_with("Entrées")).cloned().unwrap_or_default()));
    }
    println!("
=== synthèse (métriques du module sur 60 s par palier)");
    println!("| bots | fenêtre | tours d'IA | activité |");
    println!("|---|---|---|---|");
    for r in &rows {
        println!("{r}");
    }
    Ok(())
}
