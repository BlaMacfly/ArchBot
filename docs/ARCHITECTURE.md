# Architecture et fonctionnement de l'IA

## Deux couches

```text
┌──────────────────────────────── archcore (exécutable) ───────────────────────────────┐
│  net.rs ─ world/ (WorldState, Player, combat, presence, loot, skills, party, …)       │
│     ▲ fonctions existantes                      │ état du monde                         │
│     │                                           ▼                                       │
│  ┌──┴──────────────── archcore/ (adaptateur ArchBot, compilé par #[path]) ───────────┐ │
│  │ manager.rs  boucle d'IA (tâche tokio), population, tranches de bots, métriques     │ │
│  │ lifecycle   comptes + personnages (create_bot), admit/enter_world, remove_player   │ │
│  │ perception  WorldState → Perception       actions  Action → fonctions d'ArchCore  │ │
│  │ chat.rs     /bot, chuchotements, /p        persistence  table archbot_character    │ │
│  └────────────────────────┬──────────────────────────────▲───────────────────────────┘ │
└───────────────────────────┼──────────────────────────────┼─────────────────────────────┘
                            │ Perception                   │ Vec<Action>
                  ┌─────────▼──────────────────────────────┴─────────┐
                  │  archbot (crate pure, sans dépendance à ArchCore) │
                  │  ai::Brain::think — machine à états              │
                  │  movement (A*, lissage, blocage)  combat (cible, │
                  │  survie, classes)  party  commands  population   │
                  │  diagnostics::Metrics  config::BotConfig          │
                  └──────────────────────────────────────────────────┘
```

- La crate `archbot` ne connaît ni paquets, ni base, ni `WorldState`. Elle est synchrone, déterministe à
  graine égale, et testée dans un monde simulé (`tests/`).
- L'adaptateur est le seul à toucher ArchCore. Il est compilé **dans** la crate `archcore` (déclaration
  `#[path = "../../../modules/ArchBot/archcore/mod.rs"] mod archbot;` dans `main.rs`, sous la feature
  `archbot`) : il a accès aux fonctions `pub(crate)` du serveur sans qu'ArchCore devienne une bibliothèque.

## Un bot est un joueur

Un bot est un `Player` ordinaire de `WorldState`, inséré par `world::admit` (la partie de `GAME_ENTER` qui
charge le personnage) puis `world::enter_world`, sur une `SessionHandle::detached` : une session sans
connexion TCP, dont les envois sont sans effet. Conséquences :

- **visibilité** : `presence::refresh` le montre aux joueurs des secteurs voisins comme n'importe qui ;
- **combat, expérience, butin, groupe, sauvegarde** : les fonctions du serveur ne font aucune différence ;
- **coût** : aucun paquet n'est construit pour lui (court-circuit dans `send`/`send_plain`), aucun
  chiffrement, aucune tâche réseau.

## Un tour d'IA (`manager::Manager::tick`)

1. Verrou du monde (comme la boucle de combat d'ArchCore), puis verrou du gestionnaire.
2. Bots disparus du monde oubliés ; population : une entrée au plus par `SpawnIntervalMs` tant que la
   population visée n'est pas atteinte ; rotation (un départ par tour au plus) ; invitations de groupe
   reçues par des bots acceptées.
3. Les bots sont répartis en tranches de `BotsPerTick` : chaque tour, une tranche pense. Un bot sans vrai
   joueur à `FarFromPlayersUnits` ne pense qu'un tour sur `FarTickDivisor`.
4. Pour chaque bot de la tranche : `perception::build` (copie des données utiles : soi, monstres à
   `ScanRadius` avec leurs agresseurs réels, joueurs, objets au sol, compétences connues avec leurs valeurs
   réelles, maître), `Brain::think`, puis `actions::execute` pour chaque action.
5. Métriques : durée du tour (dernier, moyenne, EMA, maximum), bots par état, actions par nature.

Les actions passent par les mêmes chemins que les paquets du client : `presence::run_to`,
`combat::start_attack` (portée, cadence anti-triche), `skills::start_cast` (tous les contrôles),
`recovery::use_item`, `loot::on_item_packet`, `party::on_management` avec un paquet reconstruit,
`combat::resurrect`, `equipment::equip`, `skills::learn/upgrade`. **Un bot ne contourne aucune règle.**

## Le cerveau (`archbot::ai::brain`)

Priorités d'un tour, de la plus haute à la plus basse :

| Priorité | Décision |
|---|---|
| mort | `Dead` → après `ResurrectDelaySeconds`, `Resurrecting` → `Action::Resurrect` (relancée toutes les 5 s) |
| fuite | PV < `FleeHpPercent`, attaqué, et sans potion / en surnombre / face à plus fort : `Fleeing` vers le refuge (ancrage à moins de 8000) ou à l'opposé des menaces par bonds de 3500 ; 3 s sans agresseur → `Recovering` |
| potions | en combat (ou mana) sous les seuils, 2,5 s entre deux |
| combat | riposte à qui frappe (le plus proche, puis le plus faible) ; sinon selon le mode : autonome → cible éligible (fenêtre de niveau, pas déjà engagée par un autre joueur hors groupe, la plus proche avec un coût par écart de niveau) ; compagnon → défensif (qui attaque le maître, puis sa cible), offensif (+ monstres à 1500 du maître), passif (rien, sauf ordre `attack`). En combat : soin de soi, compétence offensive réelle (prête, payable, à portée), placement au bout de la portée pour les classes à distance, `Attack` |
| cible morte | victoire comptée, terrain de chasse recentré ici |
| butin | avant d'engager un nouveau monstre : objets dont le bot a le droit (propriété 30 s), à `PickupRadius`, sinon marche jusqu'à eux ; 15 s au plus, puis pause de 20 s |
| progression | hors combat : `EquipUpgrades` toutes les 30 s, `LearnSkills` toutes les 60 s s'il a des points |
| repos | PV < `RestHpPercent` sans agresseur : immobile jusqu'à `RestUntilPercent` (régénération du serveur) |
| mode | compagnon : suivi en formation (arc derrière le maître), `Stay` ; autonome : exploration |

Une cible dont les PV ne baissent pas en 25 s est écartée 90 s (cible inaccessible).

## Navigation (`archbot::movement`)

ArchCore n'a pas de NavMesh et fait marcher les personnages en ligne droite ; le client contourne lui-même
les obstacles. Un bot n'a pas de client : il planifie sur la grille des segments de 400 unités
(`map.is_blocked`) avec un **A\*** à 8 voisins (sans coupe de coin), borné à 3000 nœuds, puis lisse le
chemin par visée directe (traversée exacte des cellules, Amanatides–Woo). Le serveur avance le bot jusqu'à
chaque point de passage. Une position qui n'avance plus pendant 3 s avec une destination est un
**blocage** : replanification (3 au plus), puis abandon du but. Les buts d'exploration sont tirés dans un
disque autour du point d'ancrage, dont le rayon grandit (×1,5 par exploration sans proie, ×6 au plus) jusqu'à
ce qu'un monstre soit tué : le lieu devient le **terrain de chasse**, centre des explorations suivantes.

## Classes (`archbot::combat::classes`)

D'après le champ `Class` du modèle : combattants (1, 3) au contact, archers (2) et lanceurs (4) à 90 % de
leur portée d'attaque (les lanceurs gardent 10 % de PM en réserve). Les compétences viennent uniquement de
celles que le bot a **réellement apprises**, avec les valeurs de SKILL_CONST (portée, coût, incantation,
relance) : aucune compétence n'est inventée. L'apprentissage (`LearnSkills`) suit les arbres de maîtrise,
les points, le prix et le niveau, par `skills::learn` et `skills::upgrade` d'ArchCore.

Expérience : ArchCore la tient en 64 bits (`characters.exp` BIGINT, courbe `level_exp` de Webzen, niveaux 1 à
121, plus de 2³² points par niveau à partir du niveau 113, 10¹¹ points cumulés au niveau 120). La perception donne au moteur l'expérience du
niveau (`exp`), celle du niveau suivant (`exp_next`) et le cumul depuis le niveau 1 (`exp_total`, calculé par
l'adaptateur à partir de `level_exp`) ; le cerveau en déduit d'un tour à l'autre niveaux gagnés, expérience
gagnée et perdue (pénalité de mort), reportés dans les métriques (`u64`).

## Persistance

Les personnages des bots sont des lignes ordinaires de `characters` (compte `archbot_<nom>`), avec leurs
objets, compétences, argent, position, écrits par ArchCore au fil du jeu (expérience à chaque gain,
positions toutes les 60 s et au départ). La table `archbot_character` les marque comme bots (point
d'ancrage, compteurs). Au démarrage, la réserve est relue et les bots reprennent là où ils étaient.

## Extensions prévues par l'architecture

- Commandes conversationnelles : `commands::parse` est le seul point d'entrée du texte ; une couche de
  compréhension (locale ou distante) peut produire des `Command` sans toucher au cerveau. Les bots ne
  dépendent d'aucune API d'IA externe.
- Nouveaux comportements : un état ou une priorité de plus dans `Brain::think`, une action de plus dans
  `Action` et son exécution dans `archcore/actions.rs`.
- Autre hôte : tout ce que le moteur demande est dans `world::Perception` / `Terrain` ; un autre serveur
  pourrait fournir sa propre perception.
