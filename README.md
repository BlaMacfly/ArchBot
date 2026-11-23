<h1 align="center">ArchBot</h1>

<p align="center">
  <b>PlayerBots module for ArchCore — Module PlayerBots pour ArchCore</b><br>
  Rust · autonomous and companion bots for the ArchLord server emulator
</p>

<p align="center">
  <a href="#-english">🇬🇧 English</a> ·
  <a href="#-français">🇫🇷 Français</a> ·
  <a href="docs/">📖 Docs</a> ·
  <a href="https://github.com/BlaMacfly/ArchCore">ArchCore</a> ·
  <a href="https://github.com/BlaMacfly/ArchBot/releases">Releases</a>
</p>

---

## 🇬🇧 English

**ArchBot** adds server-driven player characters to [ArchCore](https://github.com/BlaMacfly/ArchCore), the
ArchLord server emulator written in Rust. Bots are **real characters**: they have an account, a class, a
level, an inventory and skills, they are visible to players, they explore, fight monsters, pick up loot,
gain experience, level up, equip better items, learn skills, rest, flee, die and resurrect — all through
ArchCore's own systems. No ArchLord client is needed and no TCP connection is opened per bot.

Two modes:

- **Autonomous bots** (mode A): a configurable population that lives in the world and makes it feel
  populated. They leave their starting village to find hunting grounds, fight monsters of their level,
  loot, equip upgrades, learn skills, rest when hurt, flee when outnumbered, and come back after death.
- **Companion bots** (mode B): a player hires a bot (`/bot hire`), it joins the party, follows in formation,
  defends the player, attacks the player's target and obeys chat orders (`follow`, `stay`, `come`,
  `attack`, `passive`, `defensive`, `aggressive`, `stop`, `status`).

The AI concepts are inspired by [mod-playerbots](https://github.com/mod-playerbots/mod-playerbots) for
AzerothCore (state engines, grinding, follow/stay, flee, loot, population manager, activity throttling);
the code is an independent Rust implementation built on the real mechanics of ArchLord as ArchCore
implements them. See [docs/CORRESPONDANCE-PLAYERBOTS.md](docs/CORRESPONDANCE-PLAYERBOTS.md).

### How it works

```text
ArchCore world state ──► Perception ──► archbot engine (Brain::think) ──► Actions ──► ArchCore functions
   (players, monsters,     (what the bot    state machine, A*, targeting,    (run_to, start_attack,
    loot, skills, map)      can know)        survival, class profile          start_cast, use_item,
                                                                              pick up, resurrect, equip…)
```

- A bot is an ordinary `Player` of `WorldState`, admitted on a **detached session** (no socket): visibility,
  combat, experience, loot, party and saving work exactly as for a human player, and sending packets to the
  bot costs nothing.
- Every tick (500 ms by default) the adapter builds a `Perception` for a slice of the bots, the engine
  decides `Action`s, and the adapter executes them through the same functions a client packet would reach.
  **Bots never bypass a server rule** (range, attack cadence, loot ownership, prices, skill trees).
- The engine is a separate crate with no ArchCore dependency: it is tested in a simulated world, and is
  deterministic for a given seed.

Priorities of a tick: death/resurrection → flee → potions → combat (always fight back; otherwise pick a
target by mode) → loot → progression (equipment, skills) → rest → follow (companion) or explore
(autonomous). Details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

### Architecture of the repository

```text
src/            archbot — the engine, a pure Rust crate with no dependency on ArchCore
                ai/ (brain, states), movement/ (A* on the terrain grid, smoothing, stuck detection),
                combat/ (targeting, survival, classes), party/, commands/, population, diagnostics, config, world
archcore/       the adapter, compiled *inside* the archcore crate (#[path] under the `archbot` feature):
                manager (AI loop, population), lifecycle (accounts, characters, enter/leave), perception,
                actions, chat (/bot, whispers, /p), persistence (table archbot_character), sim_bots (scenarios)
config/         archbot.conf.dist — every ArchBot.* setting, documented
patches/        archcore-archbot.patch — the hooks ArchCore needs (12 files, see docs/ARCHCORE-CHANGES.md)
scripts/        install.ps1 / install.sh
docs/           analysis, mapping, architecture, installation, configuration, commands, AI, tests, limitations
tests/          engine scenarios in a simulated world (movement, obstacles, combat, autonomy, companion, load)
```

### Status

| Feature | Status |
|---|---|
| Bot accounts and characters created like the creation screen (class, starting gear, level range) | ✅ verified on a live ArchCore test server |
| Bots enter the world as real characters, visible to a client (AGPMCHARACTER op 0) | ✅ verified with `archcore sim bots` (real protocol) |
| Exploration with obstacle avoidance (A* on `map_segment`), stuck detection, hunting-ground drift | ✅ engine tests + live server |
| Target selection, melee/ranged positioning, real skills, potions, rest, flee, death, resurrection | ✅ engine tests + live server (kills, level-ups, loot observed) |
| Loot pickup, equipment upgrades, skill learning with real skill points and mastery trees | ✅ live server |
| Companion: hire, party invitation, follow in formation, stay/come, defensive/aggressive, chat orders | ✅ verified with `archcore sim bots` |
| Persistence (characters kept and resumed at next start), rotation, GM commands, metrics | ✅ live server |
| Load: 1 → 100 bots on a live test server | ✅ 100 bots: 9.6 ms per AI tick on average (max 22.8 ms), process under 0.25 % of a 16-core machine, +0.7 MB — see docs/TESTS.md |
| ArchCore `6b1bf96` (Webzen quests, titles, masteries, 64-bit XP curve 1-121, Gacha) | ✅ live test server, 9 November 2026: 17 min, 12 to 20 autonomous bots + 1 companion, 16/16 end-to-end checks, 8/8 companion checks (12 min), 360 kills, 1 104 pickups, 2 deaths and resurrections, 5 000 000 000 XP counted (64 bits), 0 module error, process at 0.08 % CPU on average |
| Quests, titles, Gacha | ⚪ ignored by bots (nothing breaks; bots take no quest, equip no title, never roll the Gacha) |
| Rendering in the real ArchLord client | ⚪ not verified yet (only what the server sends to a simulated client) |
| Shopping at NPCs, quests, PvP, mounts, guilds, healing allies | ❌ not implemented (see docs/LIMITATIONS.md) |

### Requirements

- ArchCore at the revision the patch was written for (branch `Core`, commit `3dd6fe1`, 23 November 2026; the
  same hooks are also on ArchCore's `archbot` branch). On a newer revision the patch may need a manual
  touch-up: the changes are small and listed one by one in [docs/ARCHCORE-CHANGES.md](docs/ARCHCORE-CHANGES.md).
- Rust 1.85+, MariaDB, and ArchCore's databases initialised (`archcore db-init`, `archcore import`).

### Installation

```bash
cd ArchCore
git clone https://github.com/BlaMacfly/ArchBot modules/ArchBot
git apply modules/ArchBot/patches/archcore-archbot.patch      # or: pwsh modules/ArchBot/scripts/install.ps1
cp modules/ArchBot/config/archbot.conf.dist config/           # optional: created at first start
cargo build --release                                         # the `archbot` feature is on by default
```

At world start the log shows `[archbot] v0.1.0 : bots 100 au plus, population initiale 20 …`, then one
line per bot created and entering the world. To run without bots: `ArchBot.Enable = 0`, or build with
`--no-default-features`. Full steps, update and uninstall: [docs/INSTALLATION.md](docs/INSTALLATION.md).

### Configuration (excerpt)

`config/archbot.conf`, same syntax as `world.conf`, every key overridable by `ARCHCORE_<KEY>`; all values
are validated and out-of-range values are clamped with a warning.

```text
ArchBot.Enable = 1                 ArchBot.AutonomousBots = 1      ArchBot.CompanionBots = 1
ArchBot.MaxBots = 100              ArchBot.InitialPopulation = 20  ArchBot.SpawnIntervalMs = 1000
ArchBot.MinLevel = 1               ArchBot.MaxLevel = 10           ArchBot.Classes = ""      # TIDs, empty = all
ArchBot.AllowCombat = 1            ArchBot.AllowLoot = 1           ArchBot.AllowProgression = 1
ArchBot.AllowParty = 1             ArchBot.PersistCharacters = 1   ArchBot.RotationMinutes = 0
ArchBot.AIUpdateIntervalMs = 500   ArchBot.BotsPerTick = 50        ArchBot.FarTickDivisor = 4
ArchBot.ScanRadius = 4000          ArchBot.ExploreRadius = 3000    ArchBot.FleeHpPercent = 20
ArchBot.CommandLevel = 2           ArchBot.Debug = 0
```

All 36 keys with bounds and effects: [docs/CONFIGURATION.md](docs/CONFIGURATION.md).

### Commands

| Command | Who | Effect |
|---|---|---|
| `/bot help`, `/bot list` | everyone | help, bots in game (name, level, mode, state, counters) |
| `/bot hire [name]` | everyone | a free bot becomes your companion and invites you to its party (a new one is created next to you if none is free) |
| `/bot follow [d]`, `stay`, `come`, `attack`, `passive`, `defensive`, `aggressive`, `stop`, `status`, `dismiss` | master | orders to your companions — also by whispering the order to the bot, or `/p order` in the party |
| `/bot stats [reset]`, `/bot spawn <n>`, `/bot despawn <n\|all\|name>`, `/bot population <n>` | GM (`ArchBot.CommandLevel`) | metrics, population control |

Details and replies: [docs/COMMANDES.md](docs/COMMANDES.md).

### Tests

```bash
cargo test                                            # engine: 38 unit tests + 10 simulated-world scenarios
# with ArchCore patched and a test server running (ARCHCORE_CONF_DIR):
archcore sim bots gmtest gmtest GmTest 3              # 16 end-to-end checks through the real protocol
archcore sim bots gmtest gmtest GmTest load 1,10,25,50,100   # load test, 60 s of metrics per step
archcore sim bots gmtest gmtest GmTest companion test test Testeur 12   # long companion run, non-GM player
```

Results, methodology and what each test does **not** prove: [docs/TESTS.md](docs/TESTS.md).

### License and credits

ArchBot is free software under the [GNU GPL v3 or later](LICENSE) (`GPL-3.0-or-later`), like ArchCore.
mod-playerbots (GPL-2.0-or-later) was studied for its concepts only; no code was copied. ArchLord game data
belong to their publisher and are never shipped here.

---

## 🇫🇷 Français

**ArchBot** ajoute à [ArchCore](https://github.com/BlaMacfly/ArchCore), l'émulateur de serveur ArchLord
écrit en Rust, des personnages joueurs pilotés par le serveur. Les bots sont de **vrais personnages** :
compte, classe, niveau, inventaire, compétences ; visibles des joueurs, ils explorent, combattent les
monstres, ramassent le butin, gagnent de l'expérience, montent de niveau, s'équipent, apprennent des
compétences, se reposent, fuient, meurent et ressuscitent — par les systèmes d'ArchCore eux-mêmes. Aucun
client ArchLord n'est nécessaire, aucune connexion TCP par bot.

Deux modes :

- **Bots autonomes** (mode A) : une population réglable qui vit dans le monde. Ils quittent leur village de
  départ pour trouver un terrain de chasse, combattent des monstres de leur niveau, ramassent, s'équipent,
  apprennent, se reposent quand ils sont blessés, fuient le surnombre et reviennent après la mort.
- **Compagnons** (mode B) : un joueur engage un bot (`/bot hire`), qui rejoint son groupe, le suit en
  formation, le défend, attaque sa cible et obéit aux ordres de discussion (`follow`, `stay`, `come`,
  `attack`, `passive`, `defensive`, `aggressive`, `stop`, `status`).

Les concepts d'IA viennent de [mod-playerbots](https://github.com/mod-playerbots/mod-playerbots)
(AzerothCore) ; le code est une implémentation Rust indépendante, fondée sur les mécaniques réelles
d'ArchLord telles qu'ArchCore les implémente ([docs/CORRESPONDANCE-PLAYERBOTS.md](docs/CORRESPONDANCE-PLAYERBOTS.md)).

### Fonctionnement

```text
État du monde ArchCore ──► Perception ──► moteur archbot (Brain::think) ──► Actions ──► fonctions d'ArchCore
  (joueurs, monstres,       (ce que le bot    machine à états, A*, ciblage,    (run_to, start_attack,
   butin, compétences,       peut savoir)      survie, profil de classe          start_cast, use_item,
   carte)                                                                        ramassage, résurrection…)
```

- Un bot est un `Player` ordinaire de `WorldState`, admis sur une **session détachée** (sans connexion) :
  visibilité, combat, expérience, butin, groupe et sauvegarde marchent exactement comme pour un joueur, et
  les paquets qu'on lui envoie ne coûtent rien.
- À chaque tour (500 ms par défaut), l'adaptateur construit une `Perception` pour une tranche des bots, le
  moteur décide des `Action`s, l'adaptateur les exécute par les fonctions qu'atteindrait un paquet du
  client. **Un bot ne contourne aucune règle du serveur** (portée, cadence des coups, propriété du butin,
  prix, arbres de maîtrise).
- Le moteur est une crate à part, sans dépendance à ArchCore : testé dans un monde simulé, déterministe à
  graine égale.

Priorités d'un tour : mort/résurrection → fuite → potions → combat (toujours riposter ; sinon une cible selon
le mode) → butin → progression (équipement, compétences) → repos → suivi (compagnon) ou exploration
(autonome). Détail : [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

### Arborescence

```text
src/            archbot — le moteur, crate Rust pure sans dépendance à ArchCore
                ai/ (cerveau, états), movement/ (A* sur la grille du terrain, lissage, blocage),
                combat/ (ciblage, survie, classes), party/, commands/, population, diagnostics, config, world
archcore/       l'adaptateur, compilé à l'intérieur de la crate archcore (#[path], feature `archbot`) :
                manager (boucle d'IA, population), lifecycle (comptes, personnages, entrée/sortie),
                perception, actions, chat (/bot, chuchotements, /p), persistence (table archbot_character),
                sim_bots (scénarios)
config/         archbot.conf.dist — toutes les clés ArchBot.*, documentées
patches/        archcore-archbot.patch — les points d'accroche d'ArchCore (12 fichiers, docs/ARCHCORE-CHANGES.md)
scripts/        install.ps1 / install.sh
docs/           analyse, correspondance, architecture, installation, configuration, commandes, tests, limites
tests/          scénarios du moteur dans un monde simulé (déplacement, obstacles, combat, autonomie, compagnon, charge)
```

### État

| Fonctionnalité | État |
|---|---|
| Comptes et personnages de bots créés comme à l'écran de création (classe, équipement de départ, niveau) | ✅ vérifié sur un serveur ArchCore de test |
| Entrée dans le monde comme vrais personnages, visibles d'un client (AGPMCHARACTER op 0) | ✅ vérifié par `archcore sim bots` (vrai protocole) |
| Exploration avec contournement des obstacles (A* sur `map_segment`), détection de blocage, terrain de chasse | ✅ tests du moteur + serveur |
| Choix de cible, placement au contact ou à distance, compétences réelles, potions, repos, fuite, mort, résurrection | ✅ tests du moteur + serveur (victoires, montées de niveau, butin observés) |
| Ramassage du butin, équipement des meilleurs objets, apprentissage des compétences (points et arbres réels) | ✅ serveur |
| Compagnon : embauche, invitation de groupe, suivi en formation, stay/come, défensif/offensif, ordres | ✅ vérifié par `archcore sim bots` |
| Persistance (personnages gardés et repris), rotation, commandes MJ, métriques | ✅ serveur |
| Charge : 1 → 100 bots sur un serveur de test | ✅ 100 bots : 9,6 ms par tour d'IA en moyenne (max 22,8 ms), processus sous 0,25 % d'une machine à 16 cœurs, +0,7 Mo — docs/TESTS.md |
| ArchCore `6b1bf96` (quêtes, titres, maîtrises de Webzen, courbe d'expérience 1-121 en 64 bits, Gacha) | ✅ serveur de test, 9 novembre 2026 : 17 min, 12 à 20 bots autonomes + 1 compagnon, 16/16 vérifications de bout en bout, 8/8 pour le compagnon (12 min), 360 victoires, 1 104 ramassages, 2 morts et résurrections, 5 000 000 000 points d'expérience comptés (64 bits), 0 erreur du module, processus à 0,08 % de CPU en moyenne |
| Quêtes, titres, Gacha | ⚪ ignorés par les bots (rien ne casse ; aucun bot ne prend de quête, n'équipe de titre ni ne tire au Gacha) |
| Rendu dans le vrai client ArchLord | ⚪ pas encore vérifié (seulement ce que le serveur envoie à un client simulé) |
| Achats chez les PNJ, quêtes, PvP, montures, guildes, soins des alliés | ❌ non implémentés (docs/LIMITATIONS.md) |

### Prérequis

- ArchCore à la révision du patch (branche `Core`, commit `3dd6fe1`, 23 novembre 2026 ; les mêmes points
  d'accroche sont sur la branche `archbot` d'ArchCore). Sur une révision plus récente, le patch peut demander
  une retouche à la main : les modifications sont petites et listées une par une dans
  [docs/ARCHCORE-CHANGES.md](docs/ARCHCORE-CHANGES.md).
- Rust 1.85 ou plus, MariaDB, les bases d'ArchCore initialisées (`archcore db-init`, `archcore import`).

### Installation

```bash
cd ArchCore
git clone https://github.com/BlaMacfly/ArchBot modules/ArchBot
git apply modules/ArchBot/patches/archcore-archbot.patch      # ou : powershell -File modules\ArchBot\scripts\install.ps1
cp modules/ArchBot/config/archbot.conf.dist config/           # facultatif : copié au premier démarrage
cargo build --release                                         # la feature `archbot` est active par défaut
```

Au démarrage du monde, le journal affiche `[archbot] v0.1.0 : bots 100 au plus, population initiale 20 …`,
puis une ligne par bot créé et entré en jeu. Sans bots : `ArchBot.Enable = 0`, ou compilation avec
`--no-default-features`. Étapes complètes, mise à jour, désinstallation, serveur de test :
[docs/INSTALLATION.md](docs/INSTALLATION.md).

### Configuration (extrait)

`config/archbot.conf`, même syntaxe que `world.conf`, chaque clé remplaçable par `ARCHCORE_<CLÉ>` ; toute
valeur est validée, une valeur hors bornes est ramenée dans les bornes avec un avertissement.

```text
ArchBot.Enable = 1                 ArchBot.AutonomousBots = 1      ArchBot.CompanionBots = 1
ArchBot.MaxBots = 100              ArchBot.InitialPopulation = 20  ArchBot.SpawnIntervalMs = 1000
ArchBot.MinLevel = 1               ArchBot.MaxLevel = 10           ArchBot.Classes = ""      # TID, vide = tous
ArchBot.AllowCombat = 1            ArchBot.AllowLoot = 1           ArchBot.AllowProgression = 1
ArchBot.AllowParty = 1             ArchBot.PersistCharacters = 1   ArchBot.RotationMinutes = 0
ArchBot.AIUpdateIntervalMs = 500   ArchBot.BotsPerTick = 50        ArchBot.FarTickDivisor = 4
ArchBot.ScanRadius = 4000          ArchBot.ExploreRadius = 3000    ArchBot.FleeHpPercent = 20
ArchBot.CommandLevel = 2           ArchBot.Debug = 0
```

Les 36 clés, bornes et effets : [docs/CONFIGURATION.md](docs/CONFIGURATION.md).

### Commandes

| Commande | Qui | Effet |
|---|---|---|
| `/bot help`, `/bot list` | tous | aide, bots en jeu (nom, niveau, mode, état, compteurs) |
| `/bot hire [nom]` | tous | un bot libre devient votre compagnon et vous invite dans son groupe (un nouveau est créé à côté de vous s'il n'y en a aucun de libre) |
| `/bot follow [d]`, `stay`, `come`, `attack`, `passive`, `defensive`, `aggressive`, `stop`, `status`, `dismiss` | le maître | ordres à vos compagnons — aussi en chuchotant l'ordre au bot, ou `/p ordre` dans le groupe |
| `/bot stats [reset]`, `/bot spawn <n>`, `/bot despawn <n\|all\|nom>`, `/bot population <n>` | MJ (`ArchBot.CommandLevel`) | métriques, population |

Détail et réponses des bots : [docs/COMMANDES.md](docs/COMMANDES.md).

### Tests

```bash
cargo test                                            # moteur : 38 tests unitaires + 10 scénarios en monde simulé
# avec ArchCore patché et un serveur de test (ARCHCORE_CONF_DIR) :
archcore sim bots gmtest gmtest GmTest 3              # 16 vérifications de bout en bout, vrai protocole
archcore sim bots gmtest gmtest GmTest load 1,10,25,50,100   # test de charge, 60 s de mesure par palier
archcore sim bots gmtest gmtest GmTest companion test test Testeur 12   # essai long d'un compagnon, joueur sans droits MJ
```

Résultats, méthode et ce que chaque test ne prouve **pas** : [docs/TESTS.md](docs/TESTS.md).

### Documentation

| Page | Contenu |
|---|---|
| [docs/ANALYSE-ARCHCORE.md](docs/ANALYSE-ARCHCORE.md) | Phase 1 : analyse d'ArchCore (architecture, systèmes, mécaniques d'ArchLord, manques) |
| [docs/CORRESPONDANCE-PLAYERBOTS.md](docs/CORRESPONDANCE-PLAYERBOTS.md) | Phase 2 : mod-playerbots et ce qu'ArchBot en reprend |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Architecture du module, fonctionnement de l'IA, navigation, classes, persistance |
| [docs/INSTALLATION.md](docs/INSTALLATION.md) | Installation, mise à jour, désinstallation, serveur de test |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Toutes les clés `ArchBot.*` |
| [docs/COMMANDES.md](docs/COMMANDES.md) | Commandes des joueurs, des MJ et du simulateur |
| [docs/TESTS.md](docs/TESTS.md) | Procédure et résultats (unitaires, serveur, charge, en jeu) |
| [docs/LIMITATIONS.md](docs/LIMITATIONS.md) | Limitations connues, fonctionnalités futures |
| [docs/ARCHCORE-CHANGES.md](docs/ARCHCORE-CHANGES.md) | Modifications apportées à ArchCore (patch) |
| [docs/CONTRIBUER.md](docs/CONTRIBUER.md) | Guide pour les contributeurs |
| [CHANGELOG.md](CHANGELOG.md) | Journal des versions |

### Licence et crédits

ArchBot est un logiciel libre sous [GNU GPL v3 ou ultérieure](LICENSE), comme ArchCore. mod-playerbots
(GPL-2.0-or-later) a été étudié pour ses concepts seulement ; aucun code n'en est repris. Les données du
jeu ArchLord appartiennent à leur éditeur et ne sont jamais fournies ici.
