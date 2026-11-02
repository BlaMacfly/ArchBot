# Configuration

Fichier `config/archbot.conf` (créé depuis `archbot.conf.dist` au premier démarrage, dans le dossier de
configuration d'ArchCore ou `ARCHCORE_CONF_DIR`). Même syntaxe que `world.conf` : `Clé = valeur`,
commentaires `#`, textes entre guillemets. Chaque clé peut être remplacée par une variable d'environnement
`ARCHCORE_<CLÉ>` (`ArchBot.MaxBots` → `ARCHCORE_ARCH_BOT_MAX_BOTS`).

Toute valeur est **validée** au chargement (`archbot::config::BotConfig::from_pairs`) : une valeur illisible
reprend le défaut, une valeur hors bornes est ramenée dans les bornes, une clé `ArchBot.*` inconnue est
signalée ; tout est écrit dans le journal du serveur. Chaque clé est lue par le code.

| Clé | Défaut | Bornes | Effet |
|---|---|---|---|
| `ArchBot.Enable` | 1 | 0/1 | Module actif |
| `ArchBot.AutonomousBots` | 1 | 0/1 | Population de bots autonomes entretenue (mode A) |
| `ArchBot.CompanionBots` | 1 | 0/1 | `/bot hire` permis (mode B) |
| `ArchBot.MaxBots` | 100 | 0–5000 | Bots en jeu au plus, tous modes |
| `ArchBot.InitialPopulation` | 20 | 0–MaxBots | Bots autonomes visés au démarrage |
| `ArchBot.SpawnIntervalMs` | 1000 | 100–600000 | Une entrée en jeu au plus par intervalle |
| `ArchBot.MinLevel` / `MaxLevel` | 1 / 10 | 0–120 | Niveau tiré à la création (0 : niveau du modèle) ; MaxLevel aligné sur MinLevel s'il est plus bas |
| `ArchBot.Classes` | vide | TID | Modèles jouables permis (vide : tous ceux de l'écran de création) |
| `ArchBot.AllowCombat` | 1 | 0/1 | Combat contre les monstres (la riposte comprise) |
| `ArchBot.AllowLoot` | 1 | 0/1 | Ramassage du butin |
| `ArchBot.AllowProgression` | 1 | 0/1 | Équipement des meilleurs objets, apprentissage des compétences |
| `ArchBot.AllowParty` | 1 | 0/1 | Invitations acceptées, compagnon invite son maître |
| `ArchBot.PersistCharacters` | 1 | 0/1 | Personnages gardés en base et repris ; sinon marqués supprimés au départ |
| `ArchBot.AIUpdateIntervalMs` | 500 | 100–10000 | Cadence de la boucle d'IA |
| `ArchBot.BotsPerTick` | 50 | 1–5000 | Bots qui pensent par tour ; au-delà, répartis sur plusieurs tours |
| `ArchBot.ScanRadius` | 4000 | 500–20000 | Rayon de perception |
| `ArchBot.ExploreRadius` | 3000 | 0–50000 | Rayon d'exploration autour du point d'ancrage (élargi jusqu'à ×6 tant qu'aucune proie n'est trouvée, recentré sur le terrain de chasse ensuite) |
| `ArchBot.PickupRadius` | 1500 | 100–2000 | Distance de ramassage |
| `ArchBot.TargetLevelBelow` / `Above` | 6 / 2 | 0–120 | Fenêtre de niveau des cibles choisies |
| `ArchBot.FleeHpPercent` | 20 | 0–100 | Fuite sous ce % de PV (sans potion, en surnombre ou face à plus fort) ; 0 : jamais |
| `ArchBot.RestHpPercent` / `RestUntilPercent` | 45 / 85 | 0–100 | Repos hors combat entre ces deux seuils |
| `ArchBot.PotionHpPercent` / `PotionMpPercent` | 50 / 20 | 0–100 | Potion de vie / de mana sous ces seuils (potions de l'inventaire seulement) |
| `ArchBot.EngageHpPercent` | 60 | 0–100 | Un bot autonome n'engage qu'au-dessus de ce % de PV |
| `ArchBot.ResurrectDelaySeconds` | 8 | 0–3600 | Attente avant la résurrection au village |
| `ArchBot.FarFromPlayersUnits` | 30000 | 0–10⁷ | Distance au plus proche vrai joueur au-delà de laquelle un bot est « loin » |
| `ArchBot.FarTickDivisor` | 4 | 1–100 | Un bot loin ne pense qu'un tour sur n |
| `ArchBot.CommandLevel` | 2 | 0–9 | Niveau MJ de `/bot stats`, `spawn`, `despawn`, `population` (0 : désactivées) |
| `ArchBot.AccountPrefix` | `archbot_` | texte | Préfixe des comptes des bots |
| `ArchBot.NamePrefix` | vide | texte | Préfixe des noms des personnages |
| `ArchBot.RotationMinutes` | 0 | 0–100000 | Durée de session d'un bot autonome (±50 %) avant qu'un autre prenne sa place ; 0 : jamais |
| `ArchBot.FollowDistance` | 300 | 100–3000 | Distance de suivi par défaut d'un compagnon |
| `ArchBot.Chatter` | 1 | 0/1 | Les bots répondent aux ordres et aux chuchotements |
| `ArchBot.Debug` | 0 | 0/1 | Chaque action dans le journal (`ARCHCORE_TRACE=1` pour le niveau DEBUG), rapport de métriques tous les 20 tours |

## Exemples

Serveur peu peuplé, bots discrets :

```text
ArchBot.InitialPopulation = 10
ArchBot.MaxBots = 30
ArchBot.MinLevel = 1
ArchBot.MaxLevel = 15
ArchBot.RotationMinutes = 90
```

Compagnons seulement :

```text
ArchBot.AutonomousBots = 0
ArchBot.CompanionBots = 1
ArchBot.MaxBots = 20
```

Serveur de test chargé (tous les bots pensent à chaque tour, traces) :

```text
ArchBot.InitialPopulation = 100
ArchBot.MaxBots = 150
ArchBot.SpawnIntervalMs = 500
ArchBot.FarTickDivisor = 1
ArchBot.Debug = 1
```
