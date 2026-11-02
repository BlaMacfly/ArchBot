# Journal des versions

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
