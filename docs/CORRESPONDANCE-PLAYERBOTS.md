# Phase 2 — mod-playerbots (AzerothCore) et ce qu'ArchBot en reprend

*Référence : `mod-playerbots/mod-playerbots` (GPL-2.0-or-later), lu pour ses **concepts**. Aucun code n'en est
repris : ArchBot est écrit en Rust à partir des mécaniques réelles d'ArchLord telles qu'ArchCore les implémente.
La licence de mod-playerbots n'impose donc rien à ArchBot, qui est sous GPL-3.0-or-later comme ArchCore.*

## Architecture de mod-playerbots en bref

- **Moteur de décision** (`src/Bot/Engine/`) : objets nommés (Strategy, Trigger, Action, Value), file de
  priorités par *relevance*, une action réussie par tour, prérequis / alternatives / continuations,
  multiplicateurs ; quatre moteurs par bot (combat, hors combat, mort, réflexes).
- **PlayerbotAI** : boucle par bot sur le thread de la map, délai de réaction dynamique (×1 avec un maître,
  ×5 en combat, ×10 à ×200 au repos), commandes de discussion → triggers externes, **fausse `WorldSession`
  sans socket** (le bot est un vrai `Player` du cœur ; les paquets serveur → bot deviennent des triggers,
  les paquets bot → serveur sont réinjectés dans la table d'opcodes).
- **RandomPlayerbotMgr** : population cible, connexions par lots, événements temporisés (randomize,
  teleport, revive, logout), tables SQL propres (`playerbots_*`).
- **PlayerbotFactory** : niveau, sorts, talents, équipement pondéré, consommables.
- **Comportements** : grind, follow/stay/guard/flee (formations), loot (pile de corps, filtre), soins,
  repos, mort (retour au corps, esprit guérisseur), voyage (graphe de nœuds, A*), RPG pondéré, récolte.
- **Classes** : une stratégie par spécialisation, rotations à relevances décroissantes, vérifications de
  sort (`CanCastSpell`).
- **Performance** : `BotActiveAlone` (pourcentage de bots actifs à la fois), délais étalés par GUID,
  valeurs en cache, pas de SQL synchrone sur les threads de map, `PerfMonitor`.

## Correspondance

| Concept de mod-playerbots | Catégorie | Dans ArchBot |
|---|---|---|
| Bot = vrai `Player` du cœur sur une session sans socket | **directement adaptable** | `Player` ordinaire d'ArchCore sur `SessionHandle::detached` ; les envois vers cette session sont sans effet |
| Paquets serveur → bot transformés en événements | adaptable avec modifications | inutile : le moteur lit l'état du monde (`WorldState`) directement dans la `Perception`, plus simple et moins coûteux qu'un décodage de paquets |
| Paquets bot → serveur réinjectés dans les gestionnaires | directement adaptable | `Packet` reconstruit et passé au gestionnaire (potions, ramassage, groupe) ou fonction interne extraite (`start_attack`, `start_cast`) |
| Engine : triggers, actions, relevance, une action par tour | nouvelle implémentation | machine à états hybride (`Brain::think`) : priorités fixes (mort › fuite › potions › combat › butin › progression › repos › mode), plusieurs actions par tour quand elles sont indépendantes. ArchLord n'a pas la richesse de sorts qui justifie un moteur générique |
| Moteurs combat / non-combat / mort | directement adaptable | états `Combat`, `Idle/Exploring/Following/Looting/Recovering`, `Dead/Resurrecting` |
| Délai de réaction dynamique, décalage par GUID | adaptable avec modifications | `AIUpdateIntervalMs`, répartition des bots par tranche de tour (`BotsPerTick`), ralentissement loin des vrais joueurs (`FarTickDivisor`) |
| `BotActiveAlone` (pourcentage de bots actifs) | adaptable | `FarFromPlayersUnits` / `FarTickDivisor` : un bot sans vrai joueur à portée pense moins souvent, sans cesser d'exister |
| RandomPlayerbotMgr : population cible, connexions par lots | directement adaptable | `Manager` : `InitialPopulation`, `MaxBots`, `SpawnIntervalMs`, `/bot population`, rotation (`RotationMinutes`) |
| Tables `playerbots_*` | adaptable avec modifications | une seule table `archbot_character` ; tout le reste vit dans les tables ordinaires d'ArchCore |
| PlayerbotFactory (équipement pondéré, talents) | nouvelle implémentation (réduite) | création par les règles de l'écran de création d'ArchCore (modèle, équipement de départ, niveau tiré) ; progression ensuite réelle (butin, `EquipUpgrades`, `LearnSkills`). Pas d'équipement généré : ArchLord n'a pas d'« item level » comparable, ArchCore n'a pas de table de pondération |
| Grind (`GrindTargetValue`) | directement adaptable | `targeting::pick_autonomous` : fenêtre de niveau, pas de vol de cible, préférence aux monstres qui attaquent le groupe |
| Follow / stay / formations | directement adaptable | `Order::Follow/Stay`, `party::follow_point` (arc derrière le maître) |
| Flee (`FleeManager`) | adaptable | `survival::flee_point` : refuge si proche, sinon à l'opposé du barycentre des menaces ; règles d'aggro d'ArchLord (5000 de poursuite) |
| Loot (`LootObjectStack`, filtres) | adaptable | ramassage de ce qui appartient au bot (propriété 30 s), ArchLord déposant le butin au sol |
| Soins (`PartyMemberToHeal`) | adaptable (réduit) | soin de soi par compétence de soin réelle ; le soin des alliés viendra avec les compétences « sur un ami » (`on_friend`) |
| Repos (food/drink) | adaptable | ArchLord n'a ni nourriture ni boisson : repos immobile (régénération du serveur) et potions |
| Mort (retour au corps) | nouvelle implémentation | ArchLord ressuscite au village/point de région : `Action::Resurrect` après `ResurrectDelaySeconds` |
| Voyage (TravelNode, A*) | nouvelle implémentation | A* sur la grille des segments du terrain (`movement::pathfinding`), lissage, détection de blocage ; exploration qui s'éloigne du village jusqu'à trouver un terrain de chasse (`camp`) |
| Stratégies de classe et rotations | nouvelle implémentation (réduite) | `combat::classes` : distance d'engagement par nature de classe, compétence offensive réellement apprise, prête, payable et à portée ; aucune compétence fictive |
| Commandes de discussion | directement adaptable | `commands::parse` : follow, stay, come, attack, passive/defensive/aggressive, stop, status, help, hire, leave ; par chuchotement, `/p` ou `/bot` |
| Sécurité des commandes (`PlayerbotSecurity`) | adaptable (réduit) | seul le maître commande son compagnon ; commandes d'administration sous `ArchBot.CommandLevel` |
| PerfMonitor | adaptable | `diagnostics::Metrics` : durée des tours, actions, états, `/bot stats` |
| Message obligatoire « This server runs with mod-playerbots » | sans objet | ArchBot n'est pas une œuvre dérivée de mod-playerbots ; il crédite le projet comme source d'inspiration |

## Ce qu'ArchBot ne reprend pas (et pourquoi)

- **Paquets du maître relayés aux bots** (imiter le taxi, les quêtes acceptées) : pas de besoin équivalent
  identifié pour ArchLord v0.1.
- **Stratégies personnalisées en SQL**, textes de bavardage, guildes de bots, donjons, champs de bataille,
  arènes : hors périmètre.
- **Graphe de voyage précalculé** : la grille de segments d'ArchCore suffit à l'échelle d'un terrain de
  chasse ; un graphe entre zones serait une évolution.
