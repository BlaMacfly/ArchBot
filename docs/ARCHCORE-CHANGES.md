# Modifications apportées à ArchCore

Le patch `patches/archcore-archbot.patch` (branche `Core`, commit `a8610ef` du 22 novembre 2026 ; écrit sur `c9a3c6d`, adapté le 3, le 8,
le 9 puis le 22 novembre) touche 12 fichiers pour 121 lignes ajoutées et 9 retirées. Pour `a8610ef`, seul `server.rs` a changé de
forme : ArchCore y fait désormais un arrêt propre (`world::shutdown::run`, qui fait quitter le monde à tous les joueurs) ;
`archbot::shutdown()` passe juste avant, après le second Ctrl+C armé. Ailleurs, seuls des numéros de ligne ont bougé. Aucune logique de jeu n'est changée : ce sont des
points d'accroche, des découpages de fonctions existantes et des exports `pub(crate)`.

| Fichier | Changement | Pourquoi |
|---|---|---|
| `crates/archcore/Cargo.toml` | feature `archbot` (par défaut) et dépendance facultative `archbot = { path = "../../modules/ArchBot" }` | compiler le moteur ; sans le dossier `modules/ArchBot`, Cargo ne résout plus le manifeste : garder le dossier ou retirer ces lignes |
| `crates/archcore/src/main.rs` | `#[cfg(feature = "archbot")] #[path = "../../../modules/ArchBot/archcore/mod.rs"] mod archbot;` | l'adaptateur vit dans le dépôt ArchBot mais se compile dans la crate `archcore` |
| `crates/archcore/src/net.rs` | champ `detached` dans `SessionInner`, `SessionHandle::detached(server)`, `is_detached()`, court-circuit dans `send` et `send_plain` | un `Player` sans connexion TCP, dont les envois ne coûtent rien |
| `crates/archcore/src/world/mod.rs` | `game_enter` découpé : la partie « charger le personnage » devient `pub(crate) async fn admit(ctx, st, s, account, gm_level, row) -> Result<bool>` ; `enter_world` et `remove_player` passent `pub(crate)` | entrer en jeu sans clé d'authentification ; sortir proprement |
| `crates/archcore/src/world/combat.rs` | `on_action` découpé : `pub(crate) fn start_attack(ctx, st, sid, target, forced)` | prendre une cible avec les règles du paquet du client |
| `crates/archcore/src/world/skills.rs` | `request_cast` découpé : `pub(crate) fn start_cast(ctx, st, sid, skill_id, target, forced, ground)` ; `SkillCatalog::all()` | lancer une compétence avec tous les contrôles ; lister les compétences apprenables |
| `crates/archcore/src/world/chat.rs` | hook `archbot::chat::on_chat` au début de `on_chatting` (sous `cfg(feature)`) | commandes `/bot`, chuchotements et `/p` adressés aux bots |
| `crates/archcore/src/server.rs` | `archbot::start(w.clone())` après le démarrage du monde, `archbot::shutdown()` avant `world::shutdown::run` (sortie des joueurs) et la fermeture des bases | boucle d'IA ; sortie des bots à l'arrêt |
| `crates/archcore/src/login.rs` | `give_starting_items` devient une fonction libre `pub(crate)` (la méthode l'appelle) | équipement de départ des personnages créés par le module |
| `crates/archcore/src/tools/sim.rs` | module `bots` (`#[path]`, sous `cfg(feature)`) et sous-commande `sim bots` | scénarios de bout en bout |
| `.gitignore` | `/modules/` | le module cloné n'apparaît pas dans le dépôt du serveur |

## Appliquer à la main sur une révision plus récente

Chaque modification est locale et commentée « ArchBot » dans le patch ; en cas de conflit, rejouer la même
idée au même endroit (les fonctions s'appellent toujours `game_enter`, `on_action`, `request_cast`,
`on_chatting`, `run` de `server.rs`). Le test `cargo test` d'ArchCore doit continuer à passer : avec le patch (6b1bf96),
les 366 + 11 tests du serveur passent (dont 1 de l'adaptateur), plus les 48 d'ArchBot compilés dans l'espace de travail.

## Ce que le patch ne fait pas

- Aucune table, aucune migration d'ArchCore : la table `archbot_character` est créée par le module au
  démarrage (base des personnages).
- Aucune clé dans `world.conf` : la configuration est dans `archbot.conf`.
- Aucun changement des règles de jeu, des paquets ou des schémas.
