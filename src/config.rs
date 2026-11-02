//! Réglages du module (`config/archbot.conf`, clés `ArchBot.*`), au format `Clé = valeur` d'ArchCore.
//!
//! La lecture du fichier appartient à l'hôte (ArchCore sait déjà lire ses `.conf` et les variables
//! `ARCHCORE_*`) ; cette crate reçoit les paires `(clé, valeur)` et en fait un [`BotConfig`] **validé** : une
//! valeur illisible ou hors bornes est remplacée par la valeur par défaut (ou bornée) et signalée. Chaque
//! réglage est réellement lu par le moteur ou par l'adaptateur.

use std::fmt::Write as _;

/// Réglages du module.
#[derive(Clone, Debug, PartialEq)]
pub struct BotConfig {
    /// `ArchBot.Enable` : module actif.
    pub enabled: bool,
    /// `ArchBot.AutonomousBots` : une population de bots autonomes est entretenue.
    pub autonomous: bool,
    /// `ArchBot.CompanionBots` : les joueurs peuvent engager un compagnon (`/bot hire`).
    pub companions: bool,
    /// `ArchBot.MaxBots` : bots en jeu au plus (autonomes et compagnons).
    pub max_bots: usize,
    /// `ArchBot.InitialPopulation` : bots autonomes visés au démarrage.
    pub initial_population: usize,
    /// `ArchBot.SpawnIntervalMs` : délai entre deux entrées en jeu (une par intervalle).
    pub spawn_interval_ms: u64,
    /// `ArchBot.MinLevel`, `ArchBot.MaxLevel` : niveau tiré à la création d'un bot autonome (0 : niveau du modèle).
    pub min_level: i32,
    pub max_level: i32,
    /// `ArchBot.Classes` : TID des modèles jouables permis (liste « , ») ; vide : tous les modèles de l'écran de création.
    pub classes: Vec<u32>,
    pub allow_combat: bool,
    pub allow_loot: bool,
    pub allow_progression: bool,
    pub allow_party: bool,
    /// `ArchBot.PersistCharacters` : les personnages des bots restent en base et sont repris au démarrage
    /// suivant ; sinon ils sont marqués supprimés quand le bot quitte le monde.
    pub persist_characters: bool,
    /// `ArchBot.AIUpdateIntervalMs` : cadence de la pensée.
    pub ai_interval_ms: u64,
    /// `ArchBot.BotsPerTick` : bots qui pensent au plus par tour (les autres attendent le tour suivant).
    pub bots_per_tick: usize,
    /// `ArchBot.ScanRadius` : rayon de perception (monstres, joueurs, objets).
    pub scan_radius: f64,
    /// `ArchBot.ExploreRadius` : rayon d'exploration autour du point d'ancrage.
    pub explore_radius: f64,
    /// `ArchBot.PickupRadius` : distance de ramassage (le serveur refuse au-delà de 2000).
    pub pickup_radius: f64,
    /// `ArchBot.TargetLevelBelow`, `ArchBot.TargetLevelAbove` : fenêtre de niveau des cibles.
    pub target_level_below: i32,
    pub target_level_above: i32,
    /// Seuils de PV en pour cent : fuite, repos (début, fin), potion, engagement.
    pub flee_hp_pct: f64,
    pub rest_hp_pct: f64,
    pub rest_until_pct: f64,
    pub potion_hp_pct: f64,
    pub potion_mp_pct: f64,
    pub engage_hp_pct: f64,
    /// `ArchBot.ResurrectDelaySeconds` : attente avant la résurrection au village.
    pub resurrect_delay_secs: u64,
    /// `ArchBot.FarFromPlayersUnits`, `ArchBot.FarTickDivisor` : un bot sans vrai joueur à cette distance ne
    /// pense qu'un tour sur n.
    pub far_from_players_units: f64,
    pub far_tick_divisor: u32,
    /// `ArchBot.CommandLevel` : niveau MJ des commandes d'administration `/bot spawn|despawn|…`.
    pub command_level: i64,
    /// `ArchBot.AccountPrefix`, `ArchBot.NamePrefix` : préfixes des comptes et des noms des bots.
    pub account_prefix: String,
    pub name_prefix: String,
    /// `ArchBot.RotationMinutes` : durée de session d'un bot autonome avant qu'il cède sa place (0 : jamais).
    pub rotation_minutes: u64,
    /// `ArchBot.FollowDistance` : distance de suivi par défaut d'un compagnon.
    pub follow_distance: f64,
    /// `ArchBot.Chatter` : les bots répondent aux commandes par des messages.
    pub chatter: bool,
    /// `ArchBot.Debug` : traces détaillées.
    pub debug: bool,
}

impl Default for BotConfig {
    fn default() -> Self {
        BotConfig {
            enabled: true,
            autonomous: true,
            companions: true,
            max_bots: 100,
            initial_population: 20,
            spawn_interval_ms: 1000,
            min_level: 1,
            max_level: 10,
            classes: Vec::new(),
            allow_combat: true,
            allow_loot: true,
            allow_progression: true,
            allow_party: true,
            persist_characters: true,
            ai_interval_ms: 500,
            bots_per_tick: 50,
            scan_radius: 4000.0,
            explore_radius: 3000.0,
            pickup_radius: 1500.0,
            target_level_below: 6,
            target_level_above: 2,
            flee_hp_pct: 20.0,
            rest_hp_pct: 45.0,
            rest_until_pct: 85.0,
            potion_hp_pct: 50.0,
            potion_mp_pct: 20.0,
            engage_hp_pct: 60.0,
            resurrect_delay_secs: 8,
            far_from_players_units: 30_000.0,
            far_tick_divisor: 4,
            command_level: 2,
            account_prefix: "archbot_".into(),
            name_prefix: String::new(),
            rotation_minutes: 0,
            follow_distance: 300.0,
            chatter: true,
            debug: false,
        }
    }
}

/// Clés connues, dans l'ordre du fichier modèle.
pub const KEYS: &[&str] = &[
    "ArchBot.Enable",
    "ArchBot.AutonomousBots",
    "ArchBot.CompanionBots",
    "ArchBot.MaxBots",
    "ArchBot.InitialPopulation",
    "ArchBot.SpawnIntervalMs",
    "ArchBot.MinLevel",
    "ArchBot.MaxLevel",
    "ArchBot.Classes",
    "ArchBot.AllowCombat",
    "ArchBot.AllowLoot",
    "ArchBot.AllowProgression",
    "ArchBot.AllowParty",
    "ArchBot.PersistCharacters",
    "ArchBot.AIUpdateIntervalMs",
    "ArchBot.BotsPerTick",
    "ArchBot.ScanRadius",
    "ArchBot.ExploreRadius",
    "ArchBot.PickupRadius",
    "ArchBot.TargetLevelBelow",
    "ArchBot.TargetLevelAbove",
    "ArchBot.FleeHpPercent",
    "ArchBot.RestHpPercent",
    "ArchBot.RestUntilPercent",
    "ArchBot.PotionHpPercent",
    "ArchBot.PotionMpPercent",
    "ArchBot.EngageHpPercent",
    "ArchBot.ResurrectDelaySeconds",
    "ArchBot.FarFromPlayersUnits",
    "ArchBot.FarTickDivisor",
    "ArchBot.CommandLevel",
    "ArchBot.AccountPrefix",
    "ArchBot.NamePrefix",
    "ArchBot.RotationMinutes",
    "ArchBot.FollowDistance",
    "ArchBot.Chatter",
    "ArchBot.Debug",
];

/// Résultat de la lecture : la configuration, les avertissements (valeurs remplacées ou bornées) et les clés
/// `ArchBot.*` inconnues.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub config: BotConfig,
    pub warnings: Vec<String>,
    pub unknown: Vec<String>,
}

struct Reader<'a> {
    pairs: &'a [(String, String)],
    warnings: Vec<String>,
}

impl Reader<'_> {
    fn raw(&self, key: &str) -> Option<&str> {
        // La dernière occurrence l'emporte, comme dans les `.conf` d'ArchCore.
        self.pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.trim())
    }

    fn flag(&mut self, key: &str, default: bool) -> bool {
        self.num::<i64>(key, default as i64) != 0
    }

    fn num<T: std::str::FromStr + std::fmt::Display + Copy>(&mut self, key: &str, default: T) -> T {
        match self.raw(key) {
            None => default,
            Some(v) => match v.parse::<T>() {
                Ok(x) => x,
                Err(_) => {
                    self.warnings.push(format!("{key} = « {v} » illisible, valeur par défaut {default}"));
                    default
                }
            },
        }
    }

    /// Nombre borné : une valeur hors de `[lo, hi]` est ramenée dans l'intervalle, avec un avertissement.
    fn bounded<T: std::str::FromStr + std::fmt::Display + Copy + PartialOrd>(&mut self, key: &str, default: T, lo: T, hi: T) -> T {
        let v = self.num(key, default);
        if v < lo {
            self.warnings.push(format!("{key} = {v} trop petit, ramené à {lo}"));
            lo
        } else if v > hi {
            self.warnings.push(format!("{key} = {v} trop grand, ramené à {hi}"));
            hi
        } else {
            v
        }
    }

    fn text(&self, key: &str, default: &str) -> String {
        self.raw(key).map(str::to_string).unwrap_or_else(|| default.to_string())
    }

    fn list(&mut self, key: &str) -> Vec<u32> {
        let Some(v) = self.raw(key).map(str::to_string) else { return Vec::new() };
        let mut out = Vec::new();
        for part in v.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.parse::<u32>() {
                Ok(n) => out.push(n),
                Err(_) => self.warnings.push(format!("{key} : « {part} » n'est pas un TID, ignoré")),
            }
        }
        out
    }
}

impl BotConfig {
    /// Lit et valide les réglages à partir de paires `(clé, valeur)` (sortie de `parse_conf` d'ArchCore).
    pub fn from_pairs(pairs: &[(String, String)]) -> Parsed {
        let d = BotConfig::default();
        let mut r = Reader { pairs, warnings: Vec::new() };
        let min_level = r.bounded("ArchBot.MinLevel", d.min_level, 0, 120);
        let mut max_level = r.bounded("ArchBot.MaxLevel", d.max_level, 0, 120);
        if max_level < min_level {
            r.warnings.push(format!("ArchBot.MaxLevel = {max_level} sous ArchBot.MinLevel = {min_level} : aligné"));
            max_level = min_level;
        }
        let rest_hp_pct = r.bounded("ArchBot.RestHpPercent", d.rest_hp_pct, 0.0, 100.0);
        let mut rest_until_pct = r.bounded("ArchBot.RestUntilPercent", d.rest_until_pct, 0.0, 100.0);
        if rest_until_pct < rest_hp_pct {
            r.warnings.push("ArchBot.RestUntilPercent sous ArchBot.RestHpPercent : aligné".into());
            rest_until_pct = rest_hp_pct;
        }
        let max_bots = r.bounded("ArchBot.MaxBots", d.max_bots, 0, 5000);
        let mut initial_population = r.bounded("ArchBot.InitialPopulation", d.initial_population, 0, 5000);
        if initial_population > max_bots {
            r.warnings.push(format!("ArchBot.InitialPopulation = {initial_population} au-delà de ArchBot.MaxBots = {max_bots} : ramené"));
            initial_population = max_bots;
        }
        let config = BotConfig {
            enabled: r.flag("ArchBot.Enable", d.enabled),
            autonomous: r.flag("ArchBot.AutonomousBots", d.autonomous),
            companions: r.flag("ArchBot.CompanionBots", d.companions),
            max_bots,
            initial_population,
            spawn_interval_ms: r.bounded("ArchBot.SpawnIntervalMs", d.spawn_interval_ms, 100, 600_000),
            min_level,
            max_level,
            classes: r.list("ArchBot.Classes"),
            allow_combat: r.flag("ArchBot.AllowCombat", d.allow_combat),
            allow_loot: r.flag("ArchBot.AllowLoot", d.allow_loot),
            allow_progression: r.flag("ArchBot.AllowProgression", d.allow_progression),
            allow_party: r.flag("ArchBot.AllowParty", d.allow_party),
            persist_characters: r.flag("ArchBot.PersistCharacters", d.persist_characters),
            ai_interval_ms: r.bounded("ArchBot.AIUpdateIntervalMs", d.ai_interval_ms, 100, 10_000),
            bots_per_tick: r.bounded("ArchBot.BotsPerTick", d.bots_per_tick, 1, 5000),
            scan_radius: r.bounded("ArchBot.ScanRadius", d.scan_radius, 500.0, 20_000.0),
            explore_radius: r.bounded("ArchBot.ExploreRadius", d.explore_radius, 0.0, 50_000.0),
            pickup_radius: r.bounded("ArchBot.PickupRadius", d.pickup_radius, 100.0, 2000.0),
            target_level_below: r.bounded("ArchBot.TargetLevelBelow", d.target_level_below, 0, 120),
            target_level_above: r.bounded("ArchBot.TargetLevelAbove", d.target_level_above, 0, 120),
            flee_hp_pct: r.bounded("ArchBot.FleeHpPercent", d.flee_hp_pct, 0.0, 100.0),
            rest_hp_pct,
            rest_until_pct,
            potion_hp_pct: r.bounded("ArchBot.PotionHpPercent", d.potion_hp_pct, 0.0, 100.0),
            potion_mp_pct: r.bounded("ArchBot.PotionMpPercent", d.potion_mp_pct, 0.0, 100.0),
            engage_hp_pct: r.bounded("ArchBot.EngageHpPercent", d.engage_hp_pct, 0.0, 100.0),
            resurrect_delay_secs: r.bounded("ArchBot.ResurrectDelaySeconds", d.resurrect_delay_secs, 0, 3600),
            far_from_players_units: r.bounded("ArchBot.FarFromPlayersUnits", d.far_from_players_units, 0.0, 10_000_000.0),
            far_tick_divisor: r.bounded("ArchBot.FarTickDivisor", d.far_tick_divisor, 1, 100),
            command_level: r.bounded("ArchBot.CommandLevel", d.command_level, 0, 9),
            account_prefix: r.text("ArchBot.AccountPrefix", &d.account_prefix),
            name_prefix: r.text("ArchBot.NamePrefix", &d.name_prefix),
            rotation_minutes: r.bounded("ArchBot.RotationMinutes", d.rotation_minutes, 0, 100_000),
            follow_distance: r.bounded("ArchBot.FollowDistance", d.follow_distance, 100.0, 3000.0),
            chatter: r.flag("ArchBot.Chatter", d.chatter),
            debug: r.flag("ArchBot.Debug", d.debug),
        };
        let unknown = pairs.iter().map(|(k, _)| k.clone()).filter(|k| k.starts_with("ArchBot.") && !KEYS.contains(&k.as_str())).collect();
        Parsed { config, warnings: r.warnings, unknown }
    }

    /// Lit un texte au format `.conf` (lignes `Clé = valeur`, commentaires `#`, en-têtes `[…]` ignorés).
    pub fn from_conf_text(text: &str) -> Parsed {
        Self::from_pairs(&parse_conf(text))
    }

    /// Cadence de la pensée.
    pub fn ai_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.ai_interval_ms)
    }

    /// Résumé lisible pour le journal du serveur.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "bots {} au plus, population initiale {}, niveaux {}..{}, pensée toutes les {} ms ({} bots par tour), autonomes {}, compagnons {}",
            self.max_bots,
            self.initial_population,
            self.min_level,
            self.max_level,
            self.ai_interval_ms,
            self.bots_per_tick,
            if self.autonomous { "oui" } else { "non" },
            if self.companions { "oui" } else { "non" }
        );
        s
    }
}

/// Lignes `Clé = valeur` (même règle que `config::parse_conf` d'ArchCore : commentaires `#`, en-têtes `[…]`
/// ignorés, guillemets ôtés).
pub fn parse_conf(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(v);
            Some((k.trim().to_string(), v.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defauts_et_lecture() {
        let p = BotConfig::from_conf_text("[archbot]\n# commentaire\nArchBot.Enable = 1\nArchBot.MaxBots = 40\nArchBot.Classes = \"96, 1, 6\"\nArchBot.AccountPrefix = \"bot_\"\n");
        assert!(p.warnings.is_empty() && p.unknown.is_empty(), "{p:?}");
        assert_eq!((p.config.max_bots, p.config.initial_population), (40, 20));
        assert_eq!(p.config.classes, vec![96, 1, 6]);
        assert_eq!(p.config.account_prefix, "bot_");
        assert_eq!(BotConfig::from_pairs(&[]).config, BotConfig::default(), "sans clé : défauts");
    }

    #[test]
    fn validation() {
        let p = BotConfig::from_conf_text(
            "ArchBot.MaxBots = 10\nArchBot.InitialPopulation = 50\nArchBot.AIUpdateIntervalMs = 5\nArchBot.MinLevel = 30\nArchBot.MaxLevel = 20\n\
             ArchBot.FleeHpPercent = abc\nArchBot.Classes = 96,x\nArchBot.Typo = 1\nArchBot.RestHpPercent = 90\nArchBot.RestUntilPercent = 50\n",
        );
        let c = &p.config;
        assert_eq!((c.max_bots, c.initial_population), (10, 10), "population ramenée au maximum");
        assert_eq!(c.ai_interval_ms, 100, "cadence bornée");
        assert_eq!((c.min_level, c.max_level), (30, 30), "niveaux alignés");
        assert_eq!(c.flee_hp_pct, 20.0, "illisible : défaut");
        assert_eq!(c.classes, vec![96]);
        assert_eq!((c.rest_hp_pct, c.rest_until_pct), (90.0, 90.0));
        assert_eq!(p.unknown, vec!["ArchBot.Typo".to_string()]);
        assert_eq!(p.warnings.len(), 6, "{:?}", p.warnings);
    }

    #[test]
    fn toutes_les_cles_sont_lues() {
        // Chaque clé connue, mise à une valeur valide différente du défaut, doit changer la configuration ou être un texte.
        let text: String = KEYS.iter().map(|k| format!("{k} = 3\n")).collect();
        let p = BotConfig::from_conf_text(&text);
        assert!(p.unknown.is_empty());
        assert_eq!(p.config.max_bots, 3);
        assert_eq!(p.config.far_tick_divisor, 3);
        assert!(p.config.debug && p.config.chatter);
    }
}
