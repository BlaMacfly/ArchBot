//! Métriques du module : durée des tours d'IA, bots traités, actions, erreurs, répartition des états.

use crate::ai::state::BotState;
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    pub ticks: u64,
    /// Bots qui ont pensé, cumulés sur tous les tours.
    pub thoughts: u64,
    /// Actions exécutées par l'hôte, par nature.
    pub actions: Vec<(&'static str, u64)>,
    pub errors: u64,
    pub last_tick: Duration,
    pub max_tick: Duration,
    /// Moyenne mobile exponentielle de la durée d'un tour (facteur 0,1).
    pub ema_tick_us: f64,
    pub total_tick: Duration,
    /// Bots par état au dernier tour.
    pub states: [usize; 9],
    pub bots: usize,
    pub spawned: u64,
    pub despawned: u64,
    pub kills: u64,
    pub deaths: u64,
    /// Niveaux et expérience gagnés par les bots (64 bits, comme l'expérience d'ArchCore).
    pub level_ups: u64,
    pub exp_gained: u64,
}

impl Metrics {
    pub fn record_tick(&mut self, took: Duration, thought: usize) {
        self.ticks += 1;
        self.thoughts += thought as u64;
        self.last_tick = took;
        self.total_tick += took;
        if took > self.max_tick {
            self.max_tick = took;
        }
        let us = took.as_secs_f64() * 1e6;
        self.ema_tick_us = if self.ticks == 1 { us } else { self.ema_tick_us * 0.9 + us * 0.1 };
    }

    pub fn record_action(&mut self, kind: &'static str) {
        match self.actions.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => self.actions.push((kind, 1)),
        }
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    pub fn set_states(&mut self, states: impl Iterator<Item = BotState>) {
        self.states = [0; 9];
        self.bots = 0;
        for s in states {
            self.states[s.index()] += 1;
            self.bots += 1;
        }
    }

    pub fn average_tick(&self) -> Duration {
        if self.ticks == 0 {
            Duration::ZERO
        } else {
            self.total_tick / self.ticks as u32
        }
    }

    /// Rapport sur plusieurs lignes (commande `/bot stats`).
    pub fn report(&self) -> String {
        let mut lines = vec![format!(
            "ArchBot : {} bot(s), {} tour(s) d'IA, dernier {:.2} ms, moyenne {:.2} ms (EMA {:.2} ms), maximum {:.2} ms",
            self.bots,
            self.ticks,
            self.last_tick.as_secs_f64() * 1e3,
            self.average_tick().as_secs_f64() * 1e3,
            self.ema_tick_us / 1e3,
            self.max_tick.as_secs_f64() * 1e3
        )];
        let states: Vec<String> = BotState::ALL.iter().filter(|s| self.states[s.index()] > 0).map(|s| format!("{} {}", s.name(), self.states[s.index()])).collect();
        lines.push(format!("États : {}", if states.is_empty() { "aucun bot".to_string() } else { states.join(", ") }));
        let mut actions: Vec<String> = self.actions.iter().map(|(k, n)| format!("{k} {n}")).collect();
        actions.sort();
        lines.push(format!("Actions : {}", if actions.is_empty() { "aucune".to_string() } else { actions.join(", ") }));
        lines.push(format!(
            "Entrées {}, sorties {}, monstres tués {}, morts {}, niveaux gagnés {}, expérience gagnée {}, pensées {}, erreurs {}",
            self.spawned, self.despawned, self.kills, self.deaths, self.level_ups, self.exp_gained, self.thoughts, self.errors
        ));
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesures() {
        let mut m = Metrics::default();
        m.record_tick(Duration::from_millis(2), 3);
        m.record_tick(Duration::from_millis(4), 5);
        assert_eq!((m.ticks, m.thoughts, m.max_tick), (2, 8, Duration::from_millis(4)));
        assert_eq!(m.average_tick(), Duration::from_millis(3));
        assert!((m.ema_tick_us - 2200.0).abs() < 1.0);
        m.record_action("move");
        m.record_action("move");
        m.record_action("attack");
        m.set_states([BotState::Combat, BotState::Combat, BotState::Idle].into_iter());
        let r = m.report();
        assert!(r.contains("combat 2") && r.contains("idle 1") && r.contains("move 2") && r.contains("attack 1"), "{r}");
    }
}
