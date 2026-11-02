<h1 align="center">ArchBot</h1>

<p align="center">
  <b>PlayerBots module for ArchCore — Module PlayerBots pour ArchCore</b><br>
  Rust · autonomous and companion bots for the ArchLord server emulator
</p>

<p align="center">
  <a href="#-english">🇬🇧 English</a> ·
  <a href="#-français">🇫🇷 Français</a> ·
  <a href="docs/">📖 Docs</a> ·
  <a href="https://github.com/BlaMacfly/ArchCore">ArchCore</a>
</p>

---

## 🇬🇧 English

**ArchBot** adds server-driven player characters to [ArchCore](https://github.com/BlaMacfly/ArchCore), the
ArchLord server emulator written in Rust. Bots are **real characters**: they have an account, a class, a
level, an inventory and skills, they are visible to players, they explore, fight monsters, pick up loot,
gain experience, level up, equip better items, learn skills, rest, flee, die and resurrect — all through
ArchCore's own systems. No ArchLord client is needed, no TCP connection is opened per bot.

Two modes:

- **Autonomous bots** (mode A): a configurable population that lives in the world and makes it feel
  populated.
- **Companion bots** (mode B): a player hires a bot (`/bot hire`), it joins the party, follows, defends,
  attacks the player's target and obeys chat orders (`follow`, `stay`, `come`, `attack`, `passive`,
  `defensive`, `aggressive`, `stop`, `status`).

The AI concepts are inspired by [mod-playerbots](https://github.com/mod-playerbots/mod-playerbots) for
AzerothCore (state engines, grinding, follow/stay, flee, loot, population manager, activity throttling);
the code is an independent Rust implementation built on the real mechanics of ArchLord as ArchCore
implements them. See [docs/CORRESPONDANCE-PLAYERBOTS.md](docs/CORRESPONDANCE-PLAYERBOTS.md).

### Architecture

```text
src/            archbot — the engine, a pure Rust crate with no dependency on ArchCore
                Perception in → Action out: state machine (Idle, Exploring, Following, Combat, Fleeing,
                Recovering, Looting, Dead, Resurrecting), A* pathfinding on the terrain grid, targeting,
                survival, class profiles, party formations, chat commands, metrics, validated config
archcore/       the adapter, compiled *inside* the archcore crate (via #[path] under the `archbot` feature)
                bots are ordinary Players on a detached session; perception is built from WorldState and
                actions call presence/combat/skills/loot/party/equipment functions of ArchCore
config/         archbot.conf.dist — every ArchBot.* setting, documented
patches/        the small set of hooks ArchCore needs (see docs/ARCHCORE-CHANGES.md)
docs/           analysis, mapping, installation, configuration, commands, AI, tests, limitations
tests/          engine scenarios in a simulated world (movement, obstacles, combat, autonomy, companion…)
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
| Persistence (characters kept, resumed at next start), rotation, GM commands, metrics | ✅ live server |
| Load: 1 → 100 bots on a live test server | ✅ 100 bots: 9.6 ms per AI tick on average (max 22.8 ms), process under 0.25 % of a 16-core machine, +0.7 MB — see docs/TESTS.md |
| Rendering in the real ArchLord client | ⚪ not verified yet (only what the server sends to a simulated client) |
| Shopping at NPCs, quests, PvP, mounts, guilds for bots | ❌ not implemented (see docs/LIMITATIONS.md) |

### Installation

```bash
cd ArchCore
git clone https://github.com/BlaMacfly/ArchBot modules/ArchBot
git apply modules/ArchBot/patches/archcore-archbot.patch      # or: pwsh modules/ArchBot/scripts/install.ps1
cp modules/ArchBot/config/archbot.conf.dist config/           # optional: created at first start
cargo build --release                                         # the `archbot` feature is on by default
```

Full steps, prerequisites and troubleshooting: [docs/INSTALLATION.md](docs/INSTALLATION.md).

### Quick use

- `/bot help`, `/bot list`
- `/bot hire [name]` → a free bot becomes your companion and invites you to its party
- `/bot follow [distance]`, `/bot stay`, `/bot come`, `/bot attack`, `/bot passive|defensive|aggressive`, `/bot stop`, `/bot status`, `/bot dismiss` — or whisper the order to the bot, or say `/p order` in the party
- GM (`ArchBot.CommandLevel`): `/bot stats [reset]`, `/bot spawn <n>`, `/bot despawn <n|all|name>`, `/bot population <n>`

### Tests

```bash
cargo test                                   # engine: 35 unit tests + 10 simulated-world scenarios
# with ArchCore and a test server running (ARCHCORE_CONF_DIR):
archcore sim bots gmtest gmtest GmTest 3     # 16 end-to-end checks through the real protocol
archcore sim bots load 1,10,25,50,100        # load test with metrics
```

Results, methodology and what each test does **not** prove: [docs/TESTS.md](docs/TESTS.md).

### License

ArchBot is free software under the [GNU GPL v3 or later](LICENSE) (`GPL-3.0-or-later`), like ArchCore.
mod-playerbots (GPL-2.0-or-later) was studied for its concepts only; no code was copied.

---

## 🇫🇷 Français

**ArchBot** ajoute à [ArchCore](https://github.com/BlaMacfly/ArchCore) des personnages joueurs pilotés par
le serveur. Les bots sont de **vrais personnages** (compte, classe, niveau, inventaire, compétences),
visibles des joueurs ; ils explorent, combattent, ramassent le butin, gagnent de l'expérience, montent de
niveau, s'équipent, apprennent des compétences, se reposent, fuient, meurent et ressuscitent — par les
systèmes d'ArchCore eux-mêmes. Aucun client ArchLord n'est nécessaire, aucune connexion TCP par bot.

- **Bots autonomes** (mode A) : une population réglable qui vit dans le monde.
- **Compagnons** (mode B) : un joueur engage un bot (`/bot hire`), qui rejoint son groupe, le suit, le
  défend, attaque sa cible et obéit aux ordres de discussion (`follow`, `stay`, `come`, `attack`,
  `passive`, `defensive`, `aggressive`, `stop`, `status`).

Les concepts d'IA viennent de mod-playerbots (AzerothCore) ; le code est une implémentation Rust
indépendante, fondée sur les mécaniques réelles d'ArchLord telles qu'ArchCore les implémente.

### Documentation

| Page | Contenu |
|---|---|
| [docs/ANALYSE-ARCHCORE.md](docs/ANALYSE-ARCHCORE.md) | Phase 1 : analyse d'ArchCore (architecture, systèmes, mécaniques d'ArchLord, manques) |
| [docs/CORRESPONDANCE-PLAYERBOTS.md](docs/CORRESPONDANCE-PLAYERBOTS.md) | Phase 2 : mod-playerbots et ce qu'ArchBot en reprend |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Architecture du module, fonctionnement de l'IA |
| [docs/INSTALLATION.md](docs/INSTALLATION.md) | Installation, mise à jour, désinstallation |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Toutes les clés `ArchBot.*` |
| [docs/COMMANDES.md](docs/COMMANDES.md) | Commandes des joueurs et des MJ |
| [docs/TESTS.md](docs/TESTS.md) | Procédure et résultats des tests (unitaires, serveur, en jeu, charge) |
| [docs/LIMITATIONS.md](docs/LIMITATIONS.md) | Limitations connues, fonctionnalités futures |
| [docs/ARCHCORE-CHANGES.md](docs/ARCHCORE-CHANGES.md) | Modifications apportées à ArchCore (patch) |
| [docs/CONTRIBUER.md](docs/CONTRIBUER.md) | Guide pour les contributeurs |

### Licence

ArchBot est un logiciel libre sous [GNU GPL v3 ou ultérieure](LICENSE), comme ArchCore.
