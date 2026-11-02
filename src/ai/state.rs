//! États d'un bot.

/// État comportemental. Les transitions sont décidées par [`super::brain::Brain::think`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BotState {
    #[default]
    Idle,
    Exploring,
    Following,
    Combat,
    Fleeing,
    Recovering,
    Looting,
    Dead,
    Resurrecting,
}

impl BotState {
    pub const ALL: [BotState; 9] = [
        BotState::Idle,
        BotState::Exploring,
        BotState::Following,
        BotState::Combat,
        BotState::Fleeing,
        BotState::Recovering,
        BotState::Looting,
        BotState::Dead,
        BotState::Resurrecting,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BotState::Idle => "idle",
            BotState::Exploring => "exploring",
            BotState::Following => "following",
            BotState::Combat => "combat",
            BotState::Fleeing => "fleeing",
            BotState::Recovering => "recovering",
            BotState::Looting => "looting",
            BotState::Dead => "dead",
            BotState::Resurrecting => "resurrecting",
        }
    }

    /// Le bot est mort (ou attend sa résurrection).
    pub fn is_dead(self) -> bool {
        matches!(self, BotState::Dead | BotState::Resurrecting)
    }

    pub fn index(self) -> usize {
        BotState::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

impl std::fmt::Display for BotState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
