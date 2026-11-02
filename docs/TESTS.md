# Tests — procédure et résultats (état au 2 novembre 2026)

Trois familles, à ne pas confondre : **tests unitaires du moteur** (monde simulé, sans serveur), **tests
d'intégration serveur** (vrai ArchCore, vraies bases, client simulé à travers le vrai protocole), **tests en
jeu** (client ArchLord réel). Un test compilé n'est pas un test réussi ; un test unitaire ne prouve rien sur le
serveur ; un test serveur ne prouve rien sur le rendu dans le client.

## 1. Tests unitaires du moteur (`cargo test` dans ArchBot)

45 tests : 35 unitaires (géométrie, configuration et validation, états, A* et contournement, traversée des
cellules, navigation et blocage, ciblage, survie, classes, formation, commandes, population, métriques,
cerveau : riposte → butin → exploration, mort et résurrection, fuite et repos, compagnon, classe à distance,
cible inaccessible, exploration qui s'éloigne puis campe) et 10 scénarios dans un monde simulé
(`tests/scenarios.rs`) :

| Test de la mission | Scénario | Résultat |
|---|---|---|
| 2 — Déplacement | `deplacement_atteint_la_destination`, `deplacement_contourne_les_obstacles` (enceinte avec une porte, aucune case bloquée traversée) | PASS |
| 3 — Combat | `combat_detecte_attaque_tue_et_ramasse`, `riposte_a_une_attaque` | PASS |
| 4 — Autonomie | `autonomie_sur_la_duree` (900 s simulées, monstres qui réapparaissent, aucun état tenu anormalement) | PASS |
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

## 4. Tests en jeu (client ArchLord réel) — **NOT RUN**

Rien n'a été vérifié avec le vrai client : la visibilité des bots, leurs déplacements et leurs coups ont été
vérifiés sur ce que le serveur envoie à un client simulé, qui lit les mêmes paquets que le client.

## 5. Tests d'ArchCore

`cargo test` dans ArchCore patché : 299 + 11 tests du serveur passent, plus les 45 d'ArchBot compilés dans
l'espace de travail.
