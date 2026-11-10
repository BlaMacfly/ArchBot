//! Scénarios du moteur dans un monde simulé (voir `common/mod.rs`). Ce sont des **tests unitaires du
//! moteur** : ils ne prouvent rien sur le serveur réel ni sur le client ArchLord (voir `docs/TESTS.md`).

mod common;

use archbot::ai::brain::Order;
use archbot::commands::Command;
use archbot::world::*;
use archbot::{BotConfig, BotState, Brain};
use common::*;

fn cfg() -> BotConfig {
    BotConfig { explore_radius: 3000.0, ..Default::default() }
}

/// Test 2 — Déplacement : un bot autonome se fixe un but d'exploration et l'atteint dans la tolérance.
#[test]
fn deplacement_atteint_la_destination() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    let mut brain = Brain::new(1);
    let cfg = cfg();
    let mut goal = None;
    let mut reached = false;
    for _ in 0..600 {
        let actions = {
            let p = sim.perceive();
            brain.think(&p, &cfg)
        };
        sim.apply(&actions);
        if let Some(g) = brain.navigation_goal() {
            goal = Some(g);
        }
        if let Some(g) = goal {
            if brain.state != BotState::Exploring && sim.me.pos.dist2d(g) <= 150.0 {
                reached = true;
                break;
            }
        }
        sim.step(0.5);
    }
    assert!(goal.is_some(), "un but d'exploration a été choisi");
    assert!(reached, "destination atteinte dans la tolérance : position {:?}, but {:?}", sim.me.pos, goal);
}

/// Test 2 bis — Obstacles : un mur entre le bot et son but ; il le contourne sans jamais marcher sur une case bloquée.
#[test]
fn deplacement_contourne_les_obstacles() {
    let mut sim = Sim::new(warrior(1, Pos::new(200.0, 0.0, 200.0)));
    // Enceinte autour du point d'ancrage avec une ouverture au nord : tout but hors de l'enceinte oblige à passer par la porte.
    sim.terrain.wall_x(-4, -4, 4);
    sim.terrain.wall_x(4, -4, 4);
    sim.terrain.wall_z(-4, -4, 4);
    sim.terrain.wall_z(4, -4, 1);
    sim.terrain.wall_z(4, 3, 4);
    let mut brain = Brain::new(7);
    let cfg = BotConfig { explore_radius: 5000.0, ..Default::default() };
    sim.run(&mut brain, &cfg, 400.0, 0.5);
    assert_eq!(sim.blocked_steps, 0, "le bot a traversé un mur {} fois ; trace : {:?}", sim.blocked_steps, sim.trace);
    assert!(brain.stats.explorations >= 2, "plusieurs explorations : {:?}", brain.stats);
}

/// Test 3 — Combat : le bot détecte un monstre, s'en approche, l'attaque, le tue et ramasse le butin.
#[test]
fn combat_detecte_attaque_tue_et_ramasse() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    sim.mobs.push(Mob::new(0x5000_0001, Pos::new(1500.0, 0.0, 0.0), 3, 60, 4));
    let mut brain = Brain::new(2);
    sim.run(&mut brain, &cfg(), 40.0, 0.5);
    assert!(!sim.mobs[0].alive, "monstre tué ; trace : {:?}", sim.trace);
    assert_eq!(brain.stats.kills, 1);
    assert_eq!(sim.picked, 1, "butin ramassé ; trace : {:?} ; actions {:?} ; objets {:?} ; pos {:?}", sim.trace, sim.actions, sim.items, sim.me.pos);
    assert!(sim.states.contains(&BotState::Combat) && sim.states.contains(&BotState::Looting), "{:?}", sim.states);
    assert!(sim.actions.get("attack").copied().unwrap_or(0) >= 1);
}

/// Test 3 bis — Riposte : attaqué pendant qu'il explore, le bot se retourne contre l'agresseur.
#[test]
fn riposte_a_une_attaque() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    let mut mob = Mob::new(0x5000_0002, Pos::new(300.0, 0.0, 0.0), 30, 40, 3);
    mob.aggro.push(1);
    sim.mobs.push(mob);
    let mut brain = Brain::new(3);
    // Niveau 30 : hors fenêtre de cible, mais il nous frappe.
    sim.run(&mut brain, &cfg(), 20.0, 0.5);
    assert!(!sim.mobs[0].alive, "agresseur abattu ; trace : {:?}", sim.trace);
}

/// Test 4 — Autonomie : plusieurs cycles d'IA avec des monstres qui réapparaissent ; aucun état ne dure
/// anormalement et le bot alterne exploration, combat, butin, repos.
#[test]
fn autonomie_sur_la_duree() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    sim.respawn_secs = Some(20.0);
    for i in 0..6u32 {
        let angle = i as f64;
        sim.mobs.push(Mob::new(0x5000_0010 + i, Pos::new((2000.0 * angle.cos()) as f32, 0.0, (2000.0 * angle.sin()) as f32), 3, 60, 6));
    }
    // Expérience de la courbe de Webzen (64 bits) : chaque victoire dépasse 2³² points cumulés en deux coups.
    sim.exp_per_kill = 2_500_000_000;
    sim.me.exp_next = 10_000_000_000;
    let mut brain = Brain::new(4);
    sim.run(&mut brain, &cfg(), 900.0, 0.5);
    assert!(brain.stats.kills >= 5, "{:?} ; trace : {:?}", brain.stats, sim.trace);
    // La dernière victoire peut tomber après le dernier tour de pensée : au plus une d'écart.
    let seen = brain.stats.exp_gained;
    assert!(seen > u32::MAX as u64 && sim.me.exp_total - seen <= sim.exp_per_kill, "expérience vue {seen}, cumul {}", sim.me.exp_total);
    assert!(brain.stats.level_ups >= 1 && brain.stats.level_ups as i32 <= sim.me.level - 3, "{:?}, niveau {}", brain.stats, sim.me.level);
    assert!(sim.states.contains(&BotState::Exploring) && sim.states.contains(&BotState::Combat), "{:?}", sim.states);
    for (state, stay) in &sim.max_state_stay {
        let limit = match state {
            BotState::Recovering => 300.0,
            BotState::Dead | BotState::Resurrecting => 30.0,
            _ => 120.0,
        };
        assert!(*stay <= limit, "état {state} tenu {stay:.0} s ; trace : {:?}", sim.trace);
    }
    assert!(sim.actions.get("equip").copied().unwrap_or(0) >= 1, "vérification d'équipement périodique");
}

/// Test 5 — Compagnon : suit le joueur, reste sur ordre, revient sur ordre, défend le maître.
#[test]
fn compagnon_suit_et_repond_aux_commandes() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    let mut brain = Brain::new(5);
    let cfg = cfg();
    brain.hire(42, "Maitre", 300.0, (0, 1), sim.now());
    sim.master = Some(PlayerView { id: 42, name: "Maitre".into(), pos: Pos::new(4000.0, 0.0, 0.0), level: 10, ..Default::default() });
    sim.run(&mut brain, &cfg, 15.0, 0.5);
    let d = sim.me.pos.dist2d(sim.master.as_ref().unwrap().pos);
    assert!(d <= 450.0, "à portée de suivi : {d:.0} ; trace : {:?}", sim.trace);
    // Le maître s'éloigne : le compagnon suit.
    sim.master.as_mut().unwrap().pos = Pos::new(4000.0, 0.0, 3000.0);
    sim.run(&mut brain, &cfg, 15.0, 0.5);
    assert!(sim.me.pos.dist2d(Pos::new(4000.0, 0.0, 3000.0)) <= 450.0, "{:?}", sim.me.pos);
    // « stay » : le maître repart, le compagnon ne bouge pas.
    let reply = brain.command(&Command::Stay, sim.me.pos, sim.now());
    assert!(reply.contains("reste"));
    let here = sim.me.pos;
    sim.master.as_mut().unwrap().pos = Pos::new(0.0, 0.0, 0.0);
    sim.run(&mut brain, &cfg, 10.0, 0.5);
    assert!(sim.me.pos.dist2d(here) < 10.0, "resté sur place : {:?}", sim.me.pos);
    assert!(matches!(brain.companion().unwrap().order, Order::Stay(_)));
    // « come » : il revient.
    brain.command(&Command::Come, sim.me.pos, sim.now());
    sim.run(&mut brain, &cfg, 15.0, 0.5);
    assert!(sim.me.pos.dist2d(Pos::new(0.0, 0.0, 0.0)) <= 450.0, "{:?}", sim.me.pos);
    // Un monstre attaque le maître : en défense, le compagnon l'abat.
    let mut mob = Mob::new(0x5000_0020, Pos::new(300.0, 0.0, 300.0), 10, 60, 2);
    mob.aggro.push(42);
    sim.mobs.push(mob);
    sim.run(&mut brain, &cfg, 20.0, 0.5);
    assert!(!sim.mobs[0].alive, "défense du maître ; trace : {:?}", sim.trace);
    assert!(brain.status_line().contains("compagnon de Maitre"));
}

/// Mort et résurrection : un monstre trop fort tue le bot ; il demande sa résurrection après le délai, revit au
/// point d'ancrage et reprend son activité.
#[test]
fn mort_resurrection_et_reprise() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    sim.me.hp_potions = 0;
    let mut boss = Mob::new(0x5000_0030, Pos::new(200.0, 0.0, 0.0), 3, 100_000, 40);
    boss.aggro.push(1);
    sim.mobs.push(boss);
    let cfg = BotConfig { resurrect_delay_secs: 5, flee_hp_pct: 0.0, ..cfg() };
    let mut brain = Brain::new(6);
    sim.run(&mut brain, &cfg, 60.0, 0.5);
    assert!(brain.stats.deaths >= 1, "trace : {:?}", sim.trace);
    assert!(sim.resurrections >= 1, "résurrection demandée et obtenue ; trace : {:?}", sim.trace);
    assert!(!brain.state.is_dead(), "activité reprise : {} ; trace : {:?}", brain.state, sim.trace);
}

/// Fuite : à bout de PV face à plusieurs agresseurs, le bot décroche, se met hors de portée et se repose.
#[test]
fn fuite_devant_le_surnombre() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    sim.me.home = Pos::new(-6000.0, 0.0, 0.0);
    sim.me.hp = 30;
    for i in 0..3u32 {
        let mut m = Mob::new(0x5000_0040 + i, Pos::new(600.0 + 100.0 * i as f32, 0.0, 0.0), 3, 500, 5);
        m.aggro.push(1);
        sim.mobs.push(m);
    }
    let mut brain = Brain::new(8);
    sim.run(&mut brain, &cfg(), 60.0, 0.5);
    assert!(brain.stats.flees >= 1, "trace : {:?}", sim.trace);
    assert!(sim.states.contains(&BotState::Fleeing) && sim.states.contains(&BotState::Recovering), "{:?} ; trace : {:?}", sim.states, sim.trace);
    assert!(!sim.me.dead, "survécu ; trace : {:?}", sim.trace);
}

/// Potions : blessé en combat avec des potions en poche, le bot en boit.
#[test]
fn potions_en_combat() {
    let mut sim = Sim::new(warrior(1, Pos::new(0.0, 0.0, 0.0)));
    sim.me.hp_potions = 3;
    sim.me.hp = 40;
    let mut m = Mob::new(0x5000_0050, Pos::new(200.0, 0.0, 0.0), 3, 400, 8);
    m.aggro.push(1);
    sim.mobs.push(m);
    let mut brain = Brain::new(9);
    sim.run(&mut brain, &cfg(), 30.0, 0.5);
    assert!(sim.potions_drunk >= 1, "trace : {:?}", sim.trace);
}

/// Charge : 100 cerveaux pensent 1000 tours chacun ; mesure indicative du coût du moteur seul (hors serveur).
#[test]
fn cout_de_cent_cerveaux() {
    let cfg = cfg();
    let mut sims: Vec<(Sim, Brain)> = (0..100u32)
        .map(|i| {
            let mut sim = Sim::new(warrior(i + 1, Pos::new(i as f32 * 50.0, 0.0, 0.0)));
            sim.respawn_secs = Some(10.0);
            sim.mobs.push(Mob::new(0x5000_1000 + i, Pos::new(i as f32 * 50.0 + 800.0, 0.0, 300.0), 3, 60, 4));
            (sim, Brain::new(100 + i as u64))
        })
        .collect();
    let started = std::time::Instant::now();
    let mut thoughts = 0u64;
    for _ in 0..1000 {
        for (sim, brain) in &mut sims {
            let actions = {
                let p = sim.perceive();
                brain.think(&p, &cfg)
            };
            sim.apply(&actions);
            sim.step(0.5);
            thoughts += 1;
        }
    }
    let took = started.elapsed();
    let per = took.as_secs_f64() * 1e6 / thoughts as f64;
    println!("moteur : {thoughts} pensées en {took:?}, {per:.2} µs par pensée (débogage, monde simulé compris)");
    assert!(per < 2000.0, "une pensée coûte {per:.0} µs");
    assert!(sims.iter().map(|(_, b)| b.stats.kills as u64).sum::<u64>() > 100);
}
