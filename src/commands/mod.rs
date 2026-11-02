//! Commandes qu'un joueur adresse à un compagnon (chuchotement ou discussion de groupe) et commandes
//! `/bot …` de la discussion. L'analyse est ici ; l'aiguillage (qui parle, à qui, droits) est dans l'hôte.

use crate::ai::brain::Stance;

/// Commande d'un joueur à un bot.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Devenir le compagnon de l'auteur.
    Hire,
    /// Suivre l'auteur, à cette distance si elle est donnée.
    Follow(Option<f64>),
    /// Rester sur place.
    Stay,
    /// Rejoindre l'auteur (puis le suivre).
    Come,
    /// Attaquer la cible de l'auteur.
    Attack,
    Stance(Stance),
    /// Cesser le combat et revenir.
    Stop,
    /// Quitter l'auteur (redevenir autonome).
    Dismiss,
    Status,
    Help,
}

/// Lit une commande ; `None` si le texte n'en est pas une (une phrase ordinaire).
pub fn parse(text: &str) -> Option<Command> {
    let lower = text.trim().trim_start_matches('/').to_ascii_lowercase();
    let mut words = lower.split_whitespace();
    let head = words.next()?;
    let arg = words.next();
    Some(match head {
        "hire" | "hi" | "hello" | "join" | "engage" => Command::Hire,
        "follow" | "suis" | "suivre" => Command::Follow(arg.and_then(|a| a.parse::<f64>().ok()).filter(|d| *d > 0.0)),
        "stay" | "wait" | "reste" | "attends" => Command::Stay,
        "come" | "here" | "viens" => Command::Come,
        "attack" | "kill" | "attaque" => Command::Attack,
        "passive" | "passif" => Command::Stance(Stance::Passive),
        "defensive" | "defend" | "guard" | "defensif" => Command::Stance(Stance::Defensive),
        "aggressive" | "agressif" | "grind" => Command::Stance(Stance::Aggressive),
        "stop" | "flee" | "halt" | "arrete" => Command::Stop,
        "leave" | "dismiss" | "bye" | "release" | "pars" => Command::Dismiss,
        "status" | "stats" | "?" | "etat" => Command::Status,
        "help" | "aide" | "commands" => Command::Help,
        _ => return None,
    })
}

/// Aide envoyée par un bot.
pub const HELP: &str = "Commandes : hire, follow [distance], stay, come, attack, passive, defensive, aggressive, stop, status, leave, help";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyse() {
        assert_eq!(parse("follow"), Some(Command::Follow(None)));
        assert_eq!(parse("  Follow 500 "), Some(Command::Follow(Some(500.0))));
        assert_eq!(parse("follow abc"), Some(Command::Follow(None)));
        assert_eq!(parse("stay"), Some(Command::Stay));
        assert_eq!(parse("aggressive"), Some(Command::Stance(Stance::Aggressive)));
        assert_eq!(parse("/leave"), Some(Command::Dismiss));
        assert_eq!(parse("bonjour à tous"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("?"), Some(Command::Status));
    }
}
