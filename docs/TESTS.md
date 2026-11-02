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

## 3. Test de charge (Test 7) — **NOT RUN**

Le scénario `archcore sim bots gmtest gmtest GmTest load 1,10,25,50,100` et l'échantillonneur CPU/mémoire
(`Get-Process archcore`) sont écrits mais le relevé n'a pas été mené à terme (lancement avec les arguments
dans le mauvais ordre, puis arrêt de la session). Aucun chiffre de charge n'est donc avancé au-delà de la mesure
du moteur seul (§ 1). À faire : lancer la commande ci-dessus avec un `archbot.conf` à `MaxBots = 150`,
`FarTickDivisor = 1`, et relever `/bot stats` (moyenne, maximum) à chaque palier avec le CPU et la mémoire du
processus.

## 4. Tests en jeu (client ArchLord réel) — **NOT RUN**

Rien n'a été vérifié avec le vrai client : la visibilité des bots, leurs déplacements et leurs coups ont été
vérifiés sur ce que le serveur envoie à un client simulé, qui lit les mêmes paquets que le client.

## 5. Tests d'ArchCore

`cargo test` dans ArchCore patché : 299 + 11 tests du serveur passent, plus les 45 d'ArchBot compilés dans
l'espace de travail.
