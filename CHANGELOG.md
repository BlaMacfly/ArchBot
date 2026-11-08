# Journal des versions

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
