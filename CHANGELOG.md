# Journal des versions

## Non publié — 2026-11-23

ArchCore `3dd6fe1` (World.CreatureSpeed pour la vitesse des animaux ; plus de fausse annulation d'échange à la sortie
d'un joueur).

- Patch ArchCore régénéré pour `3dd6fe1` : mêmes 12 fichiers, mêmes lignes ajoutées et retirées (121 / 9). Seul le
  contexte change : `remove_player` appelle `trade::cancel_if_trading` (le patch la rend toujours `pub(crate)`) ;
  ArchBot n'appelle pas `trade::cancel` lui-même, un bot qui sort passe par `remove_player`.
- Vérifié : ArchBot 48 tests ; ArchCore patché, 385 tests (feature `archbot`) ; arrêt par le launcher ArchHome sur le serveur de test (15 bots puis le joueur sortis, plus d'annulation d'échange affichée, « arrêt terminé »).

EN — Patch updated for ArchCore `3dd6fe1` (World.CreatureSpeed, no more false trade cancellation on logout): same
12 files, context only. 48 ArchBot tests and 385 patched ArchCore tests passed; clean stop through the ArchHome launcher (15 bots then the player out, no trade cancellation shown, "arrêt terminé").

## Non publié — 2026-11-22

ArchCore `a8610ef` (arrêt propre du serveur : plus d'entrée en jeu, annonce, sortie de chaque joueur avec sa position).

- Patch ArchCore régénéré pour `a8610ef` : mêmes 12 fichiers, mêmes lignes ajoutées et retirées (121 / 9). Dans
  `server.rs`, `archbot::shutdown()` passe après le second Ctrl+C armé et avant `world::shutdown::run`, qui fait
  ensuite sortir les joueurs ; ailleurs, seuls des numéros de ligne changent.
- `archbot::shutdown()` met la population visée à 0 avant de faire sortir les bots : la boucle d'IA tourne encore
  pendant la seconde d'annonce d'ArchCore et faisait rentrer un bot, sorti ensuite comme un joueur.
- Vérifié : ArchBot 48 tests ; ArchCore patché, 384 tests (feature `archbot`) ; arrêt par le launcher ArchHome 0.3.8 sur le serveur
  de test (bots et joueur sortis, aucun bot rentré pendant l'arrêt, « arrêt terminé »).

EN — Patch updated for ArchCore `a8610ef` (clean server shutdown): `archbot::shutdown()` runs before
`world::shutdown::run`; same 12 files. On shutdown the target population drops to 0 first, so no bot re-enters
during ArchCore's one-second notice. 384 patched ArchCore tests passed; clean stop through the ArchHome 0.3.8
launcher on the test server; 48 ArchBot tests.

## Non publié — 2026-11-09

ArchCore `6b1bf96` (code de guilde 46, source Webzen : 3 235 quêtes, objets du monde à quêtes et ramassage op 11,
2 768 titres, 287 maîtrises, courbe d'expérience 1-121 en 64 bits ; Gacha).

- Patch ArchCore régénéré pour `6b1bf96` : mêmes 12 fichiers, mêmes lignes ajoutées et retirées (121 / 9) ; seuls
  les numéros de ligne de `tools/sim.rs` et l'index de `world/combat.rs` changent.
- Expérience en 64 bits : la perception donne au moteur l'expérience du niveau, celle du niveau suivant et le cumul
  depuis le niveau 1 (`u64`, d'après `level_exp`) ; le cerveau compte niveaux gagnés, expérience gagnée et perdue
  (pénalité de mort) ; `/bot stats` et le statut d'un bot les affichent (« niveaux gagnés », « expérience gagnée »).
- Quêtes, titres et Gacha : ignorés par les bots, sans rien casser (voir `docs/LIMITATIONS.md`).
- Nouveau scénario `archcore sim bots … companion <compte> <mdp> <perso> [minutes]` : essai long d'un compagnon
  engagé par un joueur sans droits MJ, avec don de 5 000 000 000 points d'expérience vérifié par le statut.
- `/bot population n` fait aussi sortir les bots autonomes en trop quand la cible baisse : déconnexion comme un
  joueur (`remove_player` : groupe quitté, position enregistrée ; compteurs dans `archbot_character`, personnage rendu
  à la réserve), au calme d'abord, deux par tour au plus ; un bot en combat ou mort ne part qu'après 30 s sans bot au
  calme. Règle dans le moteur (`population::leavers`, testée). `sim bots load` attend le compte exact à chaque palier.
- Compétences exigeant une arme ou un bouclier que le bot ne tient pas (Dual Slash, Dual Shot, Flame Core…) : plus
  proposées au moteur (le serveur refusait chaque lancement : 2 840 refus en 11 min à 20 bots).
- Bot déplacé à plus de 36 000 unités de son centre d'exploration (MJ, portail, résurrection ailleurs) : il chasse
  là où il est au lieu de rester inactif (un but tiré autour d'un village lointain n'avait pas de chemin).
- Vérifié à part : arrêt propre par Ctrl+C (`archbot::shutdown`) ; 20 bots sortis, positions enregistrées.
- Vérifié : ArchBot 48 tests ; ArchCore patché 366 + 11 tests ; essai de 17 min sur le serveur de test 11102 / 11108
  (12 à 20 bots autonomes, 1 compagnon) : 16/16, 8/8, 360 victoires, 2 morts et résurrections, 0 erreur du
  module, 0,08 % de CPU en moyenne (`docs/TESTS.md` § 3 bis).

EN — Patch and adapter updated for ArchCore `6b1bf96`; 64-bit experience in the perception, brain statistics and
metrics; quests, titles and Gacha ignored by bots; `/bot population` now also logs surplus bots out cleanly; skills
needing a weapon the bot does not hold are no longer tried; clean Ctrl+C shutdown verified; new long companion scenario; 48 engine tests, 366 + 11 ArchCore
tests and a 17-minute live test-server run passed (see `docs/TESTS.md` § 3 bis).

## Non publié — 2026-11-08

ArchCore `6912f16` (Gacha EP8, retour de bug n° 2).

- Patch ArchCore régénéré : `start_cast` reçoit aussi la position au sol (`ground`, compétences de zone lancées
  au sol, nouvelle dans `request_cast`) ; un bot passe `None`. Contexte de `sim.rs` mis à jour (`sim bug2`).
- Intégré à HomeRPG ArchLord 0.3.1.

## Non publié — 2026-11-03

ArchCore `f2ffef3` (valeurs de l'épisode 8).

- Patch ArchCore régénéré pour `f2ffef3` : mêmes 12 fichiers et mêmes points d'accroche ; seul le contexte de
  `main.rs` changeait (ligne `USAGE`, nouvelles commandes `export-client-tables` et `migrate-ep8`).
- Adaptateur : `party::on_management` prend maintenant le contexte du monde (`WorldCtx`) ; il est passé par
  `actions::accept_party_invitation`, `chat::direct` et `chat::hire_named`. Objets au sol : `owner` est
  devenu `Option<u32>` (`None` : or jeté par un joueur, à tous) ; la perception suit la règle de ramassage
  d'ArchCore.
- Vérifié : ArchCore 343 tests et ArchBot 45 tests avec le patch ; intégré à HomeRPG ArchLord 0.3.0.

## 0.1.0 — 2026-11-02

Première version.

- Moteur `archbot` (crate pure) : machine à états (Idle, Exploring, Following, Combat, Fleeing, Recovering,
  Looting, Dead, Resurrecting), A* sur la grille du terrain avec lissage et détection de blocage, ciblage,
  survie (fuite, potions, repos), profils de classe, formations de groupe, commandes de discussion,
  population, métriques, configuration validée. 45 tests.
- Adaptateur ArchCore : bots = vrais `Player` sur session détachée ; création des comptes et personnages ;
  perception depuis `WorldState` ; actions par les fonctions d'ArchCore ; `/bot …`, chuchotements, `/p` ;
  table `archbot_character` ; scénarios `archcore sim bots` et `sim bots load`.
- Patch ArchCore (12 fichiers, points d'accroche seulement), `archbot.conf.dist`, documentation.
- Vérifié : 16/16 points de bout en bout à travers le vrai protocole sur un serveur de test ; test de charge
  et client ArchLord réel : voir `docs/TESTS.md`.
