# Limitations connues et fonctionnalités futures

## Limitations (v0.1)

- **Pas de vérification avec le client ArchLord réel** : seulement avec le client simulé d'ArchCore.
- **Pas de test de charge mené à terme** (voir TESTS.md § 3) : aucun nombre de bots n'est garanti.
- **Pas d'achat chez les PNJ** : les bots ne se réapprovisionnent pas en potions (seulement celles de
  l'équipement de départ et du butin) et ne vendent rien.
- **Pas de quêtes, pas de PvP, pas de montures, pas de guildes, pas de téléportation volontaire** pour les
  bots. Les 3 235 quêtes de Webzen (PNJ et objets du monde, ramassage op 11) sont ignorées : un bot n'en prend
  aucune, les objets du monde à quêtes ne sont pas des monstres et n'entrent pas dans sa perception.
- **Titres et Gacha** : un bot ne choisit, n'équipe ni ne poursuit aucun titre (les compteurs d'ArchCore
  avancent seuls, sans objectif accepté rien n'est enregistré) et ne tire jamais au Gacha ; les tickets ou
  objets de Gacha ramassés restent dans son sac. Seules les potions (`potion` du catalogue) sont bues.
- **Soins des alliés** : seul le soin de soi est utilisé ; les compétences « sur un ami » ne le sont pas.
- **Compétences de zone, invocations, états spéciaux** : lancées comme des compétences offensives
  ordinaires, sans tactique propre.
- **Point d'ancrage** : le village de la race ; les bots s'en éloignent progressivement pour chasser, mais ne
  connaissent pas les zones par niveau. Un bot de haut niveau reste près de son village. Un bot déplacé à plus
  de 36 000 unités (MJ, portail) chasse là où il est ; ce nouveau centre n'est pas enregistré en base.
- **Un personnage au modèle 1724 (Scion)** est signalé par `archcore db-check` comme « modèle inconnu »
  (table `character_template`) : limitation du contrôle d'ArchCore, le modèle est bien jouable.
- **Patch d'ArchCore** : écrit pour le commit `6b1bf96` de la branche `Core` ; à rejouer à la main si le
  serveur évolue.
- **Arrêt du serveur** : les bots ne sont sortis proprement (`archbot::shutdown`) que par Ctrl+C. Le compte à
  rebours de `server shutdown` / `/announce_restart` (`World.ExitOnRestart = 1`) termine ArchCore par
  `std::process::exit(0)` sans cette étape : positions et compteurs des bots depuis leur dernière écriture
  sont perdus. À corriger côté ArchCore (même chemin d'arrêt que Ctrl+C).
- **Baisse de population** : les bots en trop partent au calme d'abord ; un bot sans cesse en combat peut
  rester jusqu'à 30 s de plus.
- Les bots parlent français (réponses aux ordres) ; pas de localisation.

## Fonctionnalités futures

- Réapprovisionnement chez les marchands, vente du butin inutile, réparation.
- Zones de chasse par niveau (régions avec plage de niveaux dans `Regions::level_range`) et portails.
- Soins et renforcements des membres du groupe, rôles tank / dégâts / soutien actifs.
- Quêtes simples (chasse) et dialogues avec les PNJ.
- Commandes conversationnelles et pont vers un modèle d'IA local ou distant (facultatif).
- Rotation jour/nuit de la population, horaires.
- Mesures de charge intégrées (CPU du processus, mémoire) dans `/bot stats`.
