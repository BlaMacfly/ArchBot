# Tests — procédure et résultats (état au 9 novembre 2026, ArchCore `6b1bf96`)

Trois familles, à ne pas confondre : **tests unitaires du moteur** (monde simulé, sans serveur), **tests
d'intégration serveur** (vrai ArchCore, vraies bases, client simulé à travers le vrai protocole), **tests en
jeu** (client ArchLord réel). Un test compilé n'est pas un test réussi ; un test unitaire ne prouve rien sur le
serveur ; un test serveur ne prouve rien sur le rendu dans le client.

## 1. Tests unitaires du moteur (`cargo test` dans ArchBot)

48 tests : 38 unitaires (géométrie, configuration et validation, états, A* et contournement, traversée des
cellules, navigation et blocage, ciblage, survie, classes, formation, commandes, population, métriques,
cerveau : riposte → butin → exploration, mort et résurrection, fuite et repos, compagnon, classe à distance,
cible inaccessible, exploration qui s'éloigne puis campe, progression d'expérience en 64 bits, choix des bots à faire sortir quand la population visée baisse, bot déplacé loin de son point d'ancrage) et 10 scénarios dans un monde simulé
(`tests/scenarios.rs`) :

| Test de la mission | Scénario | Résultat |
|---|---|---|
| 2 — Déplacement | `deplacement_atteint_la_destination`, `deplacement_contourne_les_obstacles` (enceinte avec une porte, aucune case bloquée traversée) | PASS |
| 3 — Combat | `combat_detecte_attaque_tue_et_ramasse`, `riposte_a_une_attaque` | PASS |
| 4 — Autonomie | `autonomie_sur_la_duree` (900 s simulées, monstres qui réapparaissent, aucun état tenu anormalement ; 2,5 milliards d'expérience par victoire : gains comptés au-delà de 2³², montées de niveau) | PASS |
| 5 — Compagnon | `compagnon_suit_et_repond_aux_commandes` (suivi, stay, come, défense du maître) | PASS |
| mort / fuite / potions | `mort_resurrection_et_reprise`, `fuite_devant_le_surnombre`, `potions_en_combat` | PASS |
| 7 — Charge (moteur seul) | `cout_de_cent_cerveaux` : 100 cerveaux × 1000 tours, **1,06 µs par pensée** (débogage, monde simulé compris) | PASS |

## 2. Tests d'intégration serveur (`archcore sim bots`, vrai protocole)

Serveur ArchCore (branche `archbot`, release) sur les bases `archcore_test_*` complètes (12 802 points
d'apparition), `ARCHCORE_CONF_DIR=config_test`, compte MJ `gmtest`. Commande :
`archcore sim bots gmtest gmtest GmTest 3`. **Résultat : 16 PASS, 0 FAIL, 0 BLOCKED, 0 NOT RUN.**

| Test de la mission | Vérification | Résultat |
|---|---|---|
| 1 — Création | `/bot spawn 3` : 3 personnages créés (compte, modèle, niveau, équipement de départ), chargés, entrés | PASS |
| 1 — Visibilité | AGPMCHARACTER op 0 des bots reçus par le client simulé (Braekrian, Baved, Caod) | PASS |
| 2 — Déplacement | 0x40 d'un bot avec destination reçu par le client | PASS |
| 5 — Compagnon | `/bot hire`, invitation de groupe du bot, groupe formé, suivi à 300 unités du MJ, `stay`, `status`, `come` | PASS |
| 3 — Combat | le compagnon frappe la cible du MJ (Green Worm, résultat 5) ; en attitude offensive il engage de lui-même | PASS |
| 6 — Persistance | 9 personnages de bots repris au redémarrage (« 9 personnage(s) de bot en base », réentrée avec niveau, position, objets) | PASS (observé au journal) |
| métriques, départ | `/bot stats`, `/bot dismiss`, `/bot despawn <nom>`, retrait de la vue (op 2/3) | PASS |

Observé dans le journal du serveur pendant ces sessions : monstres tués par les bots autonomes (Dark Spider,
Giant Bat, Green Worm), montées de niveau (1 → 3, 3 → 4), 58 ramassages (or, potions, objets), potions bues,
compétences apprises (Invincible State…), aucun avertissement ni erreur du module ; tours d'IA de 0,1 ms
(dernier) à 14 ms (maximum, au démarrage) pour 5 à 9 bots.

## 3. Test de charge (Test 7) — **PASS** (mesure réelle, serveur de test)

Commande : `archcore sim bots gmtest gmtest GmTest load 1,10,25,50,100` ; serveur release, bases
`archcore_test_*`, `archbot.conf` : `MaxBots = 150`, `SpawnIntervalMs = 500`, `BotsPerTick = 50`,
`AIUpdateIntervalMs = 500`, `FarTickDivisor = 1` (tous les bots pensent, aucun vrai joueur n'étant requis),
`Debug = 1`. À chaque palier : population fixée, attente des entrées, métriques remises à zéro, 60 s de jeu,
relevé `/bot stats`. CPU et mémoire du processus `archcore.exe` relevés toutes les 10 s par `Get-Process`
(machine : Windows 11, 16 processeurs logiques ; le serveur partage la machine avec MariaDB).

| Bots | Tours d'IA en 60 s | Durée d'un tour : moyenne / EMA / maximum | Pensées | Monstres tués | CPU du processus (% des 16 cœurs) | Mémoire (Mo) |
|---|---|---|---|---|---|---|
| 1 | 118 | 0,27 / 0,19 / 3,25 ms | 118 | 0 | 0,03–0,08 | 184,5 |
| 10 | 118 | 2,56 / 2,42 / 15,2 ms | 1180 | 13 | 0,02–0,09 | 184,6 |
| 25 | 117 | 6,98 / 5,17 / **331 ms** | 2925 | 26 | 0,04–0,14 | 184,6 |
| 50 | 118 | 7,36 / 8,70 / 22,3 ms | 5900 | 54 | 0,08–0,16 | 184,6–184,9 |
| 100 | 117 | 9,56 / 14,1 / 22,8 ms | 5850 | 84 | 0,08–0,24 | 185,2 |

Lecture :

- Un tour d'IA dure **moins de 10 ms en moyenne avec 100 bots** (verrou du monde tenu d'autant, toutes les
  500 ms) ; avec `BotsPerTick = 50`, chaque bot pense une fois par seconde à 100 bots (5850 pensées par
  minute), toutes les 500 ms en dessous de 50.
- Le maximum de 331 ms au palier 25 coïncide avec la seule « requête lente » du journal (`UPDATE
  character_items`, 308 ms) : une attente de MariaDB sous le verrou du monde, propre au modèle d'ArchCore
  (les écritures ont lieu sous le verrou), pas un calcul de l'IA. Les autres maxima restent sous 23 ms.
- CPU : le processus entier (serveur monde, 100 bots qui combattent, écritures SQL) reste sous 0,25 % de
  la machine, soit environ 3 à 4 % d'un cœur. Mémoire : +0,7 Mo pour 100 bots (184,5 → 185,2 Mo).
- Aucune erreur du module, aucun bot bloqué dans un état invalide (états observés au palier 100 : 16 idle,
  50 exploring, 24 combat, 1 fleeing, 2 recovering, 7 looting), 227 ramassages et 84 monstres tués en une
  minute. Aucune mort de bot pendant la mesure (niveaux 1 à 5 face à des monstres de niveau proche).

Ce qui n'est **pas** mesuré : l'effet de vrais clients connectés en nombre (le trafic sortant vers les
clients qui voient les bots), un serveur plus modeste qu'un poste de bureau, des bots de haut niveau avec de
nombreuses compétences, plus de 100 bots.

## 3 bis. Essai sur serveur réel du 9 novembre 2026 (ArchCore `6b1bf96`) — **PASS**

ArchCore `6b1bf96` (quêtes, titres, maîtrises et courbe d'expérience de Webzen en 64 bits, Gacha) patché sur
une copie de travail, compilé en release dans son propre `CARGO_TARGET_DIR`. Serveur de test 11102 / 11108,
bases `archcore_test_*` (3 235 quêtes, 780 objets du monde à quêtes, 2 768 titres, 287 maîtrises, `level_exp`
1-121 : 6 298 132 446 points pour passer le niveau 120), sauvegardées avant l'essai
(`Sauvegardes-bases/2026-11-09-avant-archbot`). `archbot.conf` : `InitialPopulation = 12`, `MaxBots = 30`,
`Debug = 1`, le reste par défaut (niveaux 1-10, `FarTickDivisor = 4`). 100 personnages de bots déjà en base
(essais du 2 novembre) : repris, aucun créé. Durée : **17 min** de serveur (10:59 → 11:17), 12 à 20 bots
autonomes et 1 compagnon. CPU et mémoire du processus relevés toutes les 10 s (`Get-Process`, 101 relevés).

| Étape | Commande | Résultat |
|---|---|---|
| Bout en bout | `archcore sim bots gmtest gmtest GmTest 3` | **16 PASS**, 0 FAIL (création, visibilité, déplacement, compagnon, ordres, combat, métriques, départ) |
| Compagnon, 12 min | `archcore sim bots gmtest gmtest GmTest companion test test Testeur 12` | **8 PASS**, 0 FAIL — voir ci-dessous |
| Charge | `archcore sim bots gmtest gmtest GmTest load 5,10,20` | 13, 13 puis 20 bots : tours de 3,6 / 4,5 / 4,3 ms en moyenne (max 17,8 ms), 0 erreur |

Compagnon : `Testeur` (compte `test`, sans droits MJ, mis au niveau 15 le temps de l'essai puis remis au
niveau 1) est posé par le MJ au point de chasse des Vers verts ; `/bot hire` sans nom engage Huzym, le bot
libre le plus proche, qui l'invite ; groupe formé ; Huzym rejoint son maître depuis le village. 6 min en
attitude défensive (le joueur attaque un monstre toutes les 15 s), 6 min en attitude offensive (le joueur se
promène sur un cercle de 1 500 unités) : **52 déplacements, 238 coups sur 6 cibles, 29 victoires, 73
ramassages, 2 niveaux, 0 mort**, écart maximal de 2 179 unités entre sa destination et le maître (pendant les
marches de celui-ci). Puis `/exp 5000000000 "Huzym"` : il passe au niveau 80 et son statut annonce
« 79 niveau(x) et 5000000363 point(s) d'expérience gagnés » — l'expérience en 64 bits est lue et comptée de
bout en bout. `/bot dismiss`, puis niveau rétabli (à celui de l'embauche, 1 : le scénario relit désormais le niveau atteint avant le don).

Sur l'ensemble de l'essai (journal du serveur) : **360 monstres tués par les bots** (Green Worm 216, Yellow
Worm 121, Deer, Dark Spider, Young Barrick, Giant Bat, Rock Spider), 1 104 ramassages, 16 montées de niveau en chasse (niveaux 1 à 5) plus celle du don d'expérience,
8 changements d'équipement, 7 compétences apprises, potions bues, fuites et repos ; **2 morts de bots, suivies
chacune d'une résurrection au point de la région 8 à 11 s plus tard** et d'une reprise de la chasse. Tours
d'IA : 2,6 ms en moyenne à 13 bots, 44,5 ms au plus (une fois). CPU du processus entier : **0,08 % de la
machine en moyenne, 0,20 % au plus** (16 cœurs : 1,3 à 3 % d'un cœur), mémoire stable à 205 Mo. **Aucune
erreur ni aucun avertissement du module**, aucune requête lente ; les seuls avertissements du serveur sont
les reconnexions du compte MJ entre deux scénarios et une connexion de simulateur fermée par le client.
Quêtes, titres et Gacha : rien de déclenché par les bots, rien de cassé (les bots n'en prennent ni n'en
utilisent ; les compteurs de titre d'ArchCore avancent seuls sans objectif accepté).

Ce que cet essai ne prouvait pas, repris dans la seconde passe ci-dessous : l'arrêt propre (serveur arrêté par
`taskkill /F`) et la baisse de population (le palier « 5 » gardait 13 bots). Reste non prouvé : un bot de haut
niveau en chasse réelle (seul le don d'expérience atteint le niveau 80) ; aucun vrai client.

### Seconde passe du 9 novembre 2026 (11:26 → 11:37, puis vérification)

Même serveur de test, exe recompilé avec la sortie des bots en trop. Le serveur est lancé dans sa propre console
(`Start-Process`, sortie redirigée) pour pouvoir recevoir un vrai Ctrl+C.

| Point | Résultat |
|---|---|
| `sim bots … load 5,10,20` | **5, 10 puis 20 bots** : à `/bot population 5`, 7 des 12 bots se déconnectent (« 7 bot(s) en trop se déconnectent (au calme d'abord) ») en moins de 10 s ; puis 10, puis 20 par la réserve. Tours de 4,2 / 4,0 / 4,9 ms en moyenne, 0 erreur. Les bots sortis gardent niveau et expérience (Zeidrirn niveau 5, 352 points ; `sessions` + 1, `last_seen` à l'heure de sortie) et reviennent tels quels aux paliers suivants. |
| `sim bots … companion test test Testeur 6` (corrigé) | **8 PASS**, 0 FAIL. Cydeigia (niveau 6) : 40 déplacements, 50 coups sur 7 cibles, 60 victoires, 107 ramassages, 0 mort ; 5 000 000 000 points comptés ; niveau relu avant le don (6) et rétabli à 6 ; Testeur remis au niveau 1. |
| Arrêt propre (Ctrl+C) | `tokio::signal::ctrl_c` reçu (« arrêt… ») ; `archbot::shutdown` fait sortir les **20 bots** (« 20 bot(s) sorti(s) du monde ») par `remove_player` dans la même seconde, avant la fermeture des bases ; processus terminé en moins d'une seconde. **20 / 20 positions en base identiques** à celles du journal de sortie, `last_seen` mis à jour. Aucune erreur ; les deux seuls avertissements du journal sont une reconnexion du compte MJ et une connexion de simulateur fermée par le client. |

Défaut trouvé pendant cette passe et corrigé : des bots relançaient sans fin des compétences qui exigent une arme
qu'ils ne tiennent pas (Dual Shot, Dual Slash, Flame Core) : **2 840 refus** du serveur en 11 min à 20 bots
(1 834 actions `cast`, tours de 11 ms en moyenne). La perception écarte désormais ces compétences (contrôle
`requirement::met` d'ArchCore) : **0 refus en 14 min sur deux passes (20 à 24 bots), 107 actions cast au lieu de 1 834 sur une durée comparable**.

Second défaut trouvé et corrigé : un bot déplacé loin de son point d'ancrage (amené par `/move` devant le MJ
depuis un autre village, à un million d'unités) restait inactif, ses buts d'exploration n'ayant pas de chemin
(Brunyrn : immobile sur plusieurs sessions). `sim bots … 3` à 20 bots donnait alors **13 PASS, 3 FAIL**
(aucun déplacement des bots amenés ; le compagnon ne frappait pas la cible du MJ, déjà prise par des bots
laissés au point de chasse par les essais précédents). Le moteur recentre désormais l'exploration là où est le
bot au-delà de 36 000 unités (2 × 6 × `ExploreRadius`). Après correction, même serveur, 20 à 24 bots :
**16 PASS, 0 FAIL** (Brunyrn se déplace), puis arrêt par Ctrl+C : 20 bots sortis, **20 / 20 positions
identiques** en base. Avertissements des deux dernières passes : deux requêtes lentes de MariaDB (243 et
346 ms, `character_items`), une reconnexion du compte MJ, deux connexions de simulateur fermées ; aucun du module.

Arrêt d'ArchCore : le seul arrêt propre est Ctrl+C (`server::run` attend `tokio::signal::ctrl_c`, puis
`archbot::shutdown` et la fermeture des bases). Sous Windows sans console, il faut l'envoyer par
`AttachConsole` + `GenerateConsoleCtrlEvent` (script utilisé : un PowerShell qui s'attache à la console du
serveur). `server shutdown` / `/announce_restart` avec `World.ExitOnRestart = 1` finissent par
`std::process::exit(0)` sans passer par cette séquence : les bots ne sont pas sortis proprement et les bases ne
sont pas fermées. Il faudrait qu'ArchCore fasse aboutir le compte à rebours au même chemin que Ctrl+C (par
exemple un `tokio::sync::Notify` attendu avec `ctrl_c()` dans `server::run`).

## 4. Tests en jeu (client ArchLord réel) — **NOT RUN**

Rien n'a été vérifié avec le vrai client : la visibilité des bots, leurs déplacements et leurs coups ont été
vérifiés sur ce que le serveur envoie à un client simulé, qui lit les mêmes paquets que le client.

## 5. Tests d'ArchCore

`cargo test --workspace` dans ArchCore `6b1bf96` patché : 366 tests du serveur (dont 1 de l'adaptateur :
expérience cumulée en 64 bits) et 11 d'`archcore-net` passent, plus les 48 d'ArchBot compilés dans l'espace de
travail.
