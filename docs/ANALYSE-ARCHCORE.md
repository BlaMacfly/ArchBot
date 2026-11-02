# Phase 1 — Analyse d'ArchCore pour ArchBot

*État analysé : `BlaMacfly/ArchCore`, branche `Core`, commit `c9a3c6d` (2 novembre 2026). Les chemins sont
relatifs à `crates/archcore/src/`. Cette analyse ne décrit que ce qui est dans le code ; les manques sont
dits manquants.*

## 1. Architecture générale

ArchCore est un espace de travail Cargo de deux crates qui donnent un exécutable, `archcore` :

| Crate | Rôle |
|---|---|
| `archcore-net` | protocole du client 64 bits : paquets `AuPacket` pilotés par schémas (`schema.rs`, 2000 lignes de relevés), Blowfish, trames, RC4, texte Windows-1252. Aucune dépendance au serveur. |
| `archcore` | serveur de connexion (11002) et serveur monde (11008), outils (`db-init`, `import`, `sim`…). ~57 000 lignes. |

Le serveur repose sur `tokio` et `mysql_async` (MariaDB). **Il n'y a pas de boucle principale unique** :
chaque connexion est une tâche (`net.rs`), et l'état du monde est un `World { ctx: WorldCtx, state:
tokio::sync::Mutex<WorldState> }` (`world/mod.rs`). Tout gestionnaire de paquet prend le verrou, traite,
puis le rend ; les tâches périodiques (`combat::spawn_ticker` toutes les 100 ms, `presence::spawn_autosave`
toutes les 60 s, `reconnect`, `nature`) font de même. Le verrou est tenu **à travers les `await`** des
requêtes SQL : les paquets de tous les joueurs sont donc traités un à un, comme le dit la documentation.

## 2. Connexions, sessions, authentification

- `net.rs` : `serve()` accepte, `run_session()` lit le flux, découpe les trames, déchiffre, décode selon le
  schéma, applique un seau de jetons (anti-inondation) et appelle `Connection::on_packet`. Une
  `SessionHandle` (`Arc<SessionInner>`) porte l'identifiant de session, le chiffrement et un canal d'envoi
  vers la tâche d'écriture. Avant ArchBot, elle ne pouvait exister que derrière une connexion TCP.
- `login.rs` : comptes (`account`, SHA-256), liste des mondes, personnages (liste, création, suppression),
  entrée dans le monde (clé `auth_token` remise au client).
- `world/mod.rs` : `GAME_ENTER` (`game_enter`) vérifie la clé, charge la ligne `characters`, l'inventaire,
  la banque, les quêtes, les compétences, etc., construit `Player` et l'insère dans `WorldState.players`
  (par identifiant de session) et `by_cid` ; `ENTER_WORLD` (`enter_world`) envoie les paquets d'entrée et
  appelle `presence::enter` (visibilité).

## 3. Structures des joueurs

`Player` (`world/mod.rs:192`) : `sess: SessionHandle`, `account`, `gm_level`, `ch: CharacterRow`
(`character.rs` : identité, niveau, expérience, position, argent…), `factors: Factors` (tableaux
`status`, `point`, `point_max`, `attack`, `damage_*`, `defense_*`… exactement comme AgpmFactors du
client), ensembles `seen_*` (vue), `dest` (marche en cours), `attack: Option<combat::Attack>`, `dead`,
`invincible_until`, `timers` (potions, combat, régénération), `skills: PlayerSkills`, `inventory`, groupe
via `WorldState.parties`, etc. Indices utiles des facteurs : `status[7]` niveau, `status[8]/[9]` marche et
course (dixièmes d'unité par seconde), `point[0]/[1]` PV/PM, `point_max[0]/[1]`, `attack[0]` portée,
`attack[2]` vitesse d'attaque (coups par minute), `ftype` = [race, sexe, classe], `dirt[5]` points de
compétence.

## 4. Déplacement et coordonnées

`world/presence.rs`. Le client n'envoie que sa destination (0x40) ; **le serveur avance lui-même le
personnage** à chaque tour (`move_step`, vitesse `status[8]` ou `status[9]`), l'arrête à destination, et
recalcule la vue au changement de secteur. `run_to(st, sid, dest, now)` et `stop()` font courir ou arrêter
un joueur **sur ordre du serveur** (déjà utilisés par les portails, l'approche de combat…). Positions :
`Vec3 { x, y, z }`, X/Z au sol ; `combat::dist2d` mesure en X/Z. Le sol infranchissable **n'arrête pas** la
marche du serveur (le client gère ses obstacles) — point décisif pour les bots, qui n'ont pas de client.

## 5. Paquets réseau

`net.rs` → `schema::main_by_id` → `WorldConn::on_packet` (`world/mod.rs:487`), un `match` par type de
paquet vers les modules. Les gestionnaires ont la signature `(ctx, &mut st, sid, &Packet)`. Pour un bot,
le plus sûr est d'appeler les **mêmes fonctions** avec un `Packet` reconstruit (`build_packet` puis
`decode_packet`), ou les fonctions internes quand elles existent.

## 6. Visibilité

`world/sector.rs` : secteurs de 6400 unités ; ce qui entre dans les 3 × 3 secteurs apparaît, ce qui sort
des 5 × 5 disparaît. `presence::refresh` (joueurs, symétrique, membres d'un groupe toujours visibles),
`send_nearby_npcs` et `sync_monsters` (PNJ, monstres), `loot::sync_ground` (objets). Tout joueur présent
dans `st.players` avec `in_game` est vu par les autres : **un bot `Player` est visible sans rien d'autre**.

## 7. Monstres et IA existante

`world/monsters.rs` : `MonsterRegistry` (grille de recherche `near(pos, rayon)`), `Monster { cid, tid,
level, pos, hp, life, home, aggro: Vec<Aggro>, target, roam… }`, `stats_of(tid)` (CharacterDataTable :
portée, vitesse d'attaque, dégâts, type attaquable). `world/ai.rs` : un monstre **n'attaque que ceux qui
l'ont frappé** (sauf les « préventifs », `preemptive_step`), vise toutes les 500 ms l'agresseur qui lui a
fait le plus de dégâts à moins de 5000 unités, poursuit, frappe à sa vitesse d'attaque, oublie un agresseur
30 s après son dernier coup, rentre chez lui, et est ramené d'un coup au-delà de 10 000 unités ; patrouille
aléatoire au repos. `world/monster_skills.rs` : compétences des IA de monstres.

## 8. Combat

`world/combat.rs` : `on_action` (0x41) prend un monstre (ou un joueur permis) pour cible ; `attack_step`
(toutes les 100 ms) approche si hors de portée (`approach` : jusqu'à `hit_range/2` de la cible, en
course), puis frappe au rythme de `attack[2]` : `strike` (chance de toucher `hit_chance`, dégâts par
classe `damage_range`, défense), coups élémentaires, critiques et effets (`effects.rs`), diffusion aux
spectateurs, `ai::add_aggro`. `kill` : expérience (`reward`, partage de groupe `party::share_exp`),
butin (`loot::on_kill`), quêtes, titres. `player_death`, `resurrect` (village de la race ou point de la
région), `revive`, `set_level`. Portée effective : `attack[0]` du joueur + `hit_range` du monstre (+ 50).

## 9. Compétences

`world/skills.rs` (2400 lignes) : catalogue SKILL_SPEC/SKILL_CONST (`SkillTemplate`, `SkillLevel` :
portée, coûts, incantation, relance, soin, états…), compétences connues (`PlayerSkills`, `Owned { id de
session, tid, level, ready_at }`), lancement (`request_cast` → approche → `begin` → `complete`), effets
durables, maîtrises et apprentissage (`learn`, `upgrade`, points par niveau).

## 10. Inventaire, équipement, butin, économie

`world/items.rs` (`ItemCatalog`, `Inventory` : `add`, `remove`, `equip`, `free_count`…),
`world/equipment.rs` (`check` niveau/race/classe/sexe, `equip(ctx, st, sid, iid, quiet)`, facteurs
recalculés), `world/loot.rs` (tirage par tables, objets au sol `Ground`, propriété 30 s au tueur, ramassage
op 11 à moins de 2000 unités, `GOLD_TID`), `world/recovery.rs` (potions `use_item`, régénération hors
combat 1,5 % + taux + 5 toutes les 10 s), boutiques (`npc_trade.rs`), banque, échange, maison des ventes.

## 11. Groupes

`world/party.rs` : `Parties` (5 membres, chef en tête, invitations par CID ou nom, `on_management`
INVITE/ACCEPT/REJECT/LEAVE/BANISH/DELEGATION, partage de l'expérience par dégâts ou par niveau, rappel).
Les membres d'un groupe se voient à toute distance.

## 12. Quêtes, progression

`world/quests.rs` (catalogue, journal, objectifs de chasse comptés dans `combat::kill`),
`combat::gain_exp` (table `level_exp`, niveau max 120), `equipment::recompute_factors` à la montée de
niveau, `skills::refresh_points`.

## 13. Base de données

`db.rs` : un pool `mysql_async` par base (`auth`, `characters`, `world`, `logs`), `rows/first/exec/atomic`.
Schéma de référence `sql/00x.sql` + migrations numérotées embarquées (`migrations.rs`, le serveur refuse
une base en retard ou en avance). Tables clés : `account`, `characters`, `character_items`,
`character_skills`, `monster_spawn`, `character_data_table`, `map_segment`, `map_height`, `level_exp`.

## 14. Configuration

`config.rs` : `config/auth.conf`, `config/world.conf` (`Clé = valeur`, modèles `.dist`, variables
`ARCHCORE_<CLÉ>`, clés inconnues signalées, `Settings`), `ARCHCORE_CONF_DIR`. `StartPosition.<race>`,
`World.ViewRadius`, taux, anti-triche (`AntiCheat.*` : vitesse des positions **envoyées par le client**,
cadence des coups), SOAP.

## 15. Terrain

`world/map.rs` : `map_segment` (800 × 800 secteurs de 16 × 16 segments de 400 unités, bit « sol
infranchissable », région) et `map_height` (hauteur interpolée). `is_blocked(x, z)`, `height(x, z)`,
`region(x, z)` ; `regions.rs` : régions sûres, points de résurrection. **Pas de NavMesh ni de recherche
de chemin** côté serveur.

## 16. Mécaniques d'ArchLord relevées (données réelles)

| Mécanique | Source dans ArchCore |
|---|---|
| Classes jouables : 10 modèles de l'écran de création (Knight 96, Archer 1, Mage 6, Berserker 4, Hunter 8, Sorcerer 3, Swashbuckler 377, Ranger 460, Elementalist 9, Scion 1724 ; évolutions Slayer/Orbiter/Summoner) | `character::BASE_TEMPLATE_NAMES`, `Templates::load` |
| Classe 1 combattant, 2 archer, 3 Scion, 4 lanceur ; dégâts : force / dextérité / intelligence | `combat::Attacker::damage_range` |
| Vitesses en dixièmes d'unité/s (4000 marche, 12 000 course par défaut) | `ai.rs`, `presence.rs` |
| Portée d'attaque `attack[0]` (50 au moins) + rayon du corps du monstre (100 par défaut) | `combat.rs`, `monsters.rs` |
| Aggro : seulement qui frappe (sauf préventifs), 5000 de poursuite, 30 s de mémoire, laisse 10 000 | `ai.rs` |
| Potions : délai propre (« Use Interval », 2 s à défaut), PV/PM | `recovery::Potion` |
| Résurrection : village de la race ou point de la région, 10 s d'invincibilité, pénalité d'expérience configurable | `combat::resurrect`, `consumables::town_penalty` |
| Butin : propriété 30 s, expiration 45 s, ramassage à 2000 | `loot.rs` |
| Groupe : 5 membres, partage par dégâts ou par niveau | `party.rs` |
| Villages de départ par race | `StartPosition.*` de `world.conf` |

## 17. Fonctionnalités opérationnelles, partielles, absentes (du point de vue d'un bot)

| Besoin d'ArchBot | État dans ArchCore | Conséquence |
|---|---|---|
| Joueur piloté par le serveur | **absent** : `SessionHandle` exige une connexion | ajout de `SessionHandle::detached` (patch) |
| Entrée en jeu sans clé d'authentification | partiel : `game_enter` mêle authentification et chargement | découpage `admit` / `enter_world` (patch) |
| Ordonner une marche | **opérationnel** : `presence::run_to`, `stop` | réutilisé tel quel |
| Prendre une cible | opérationnel via paquet | `combat::start_attack` extrait de `on_action` (patch) |
| Lancer une compétence | opérationnel via paquet | `skills::start_cast` extrait de `request_cast` (patch) |
| Potions, ramassage, groupe | opérationnels via paquets | paquets reconstruits, sans patch |
| Résurrection, équipement, apprentissage, montée de niveau | opérationnels (`combat::resurrect`, `equipment::equip`, `skills::learn/upgrade`, `combat::reward`) | réutilisés |
| Évitement des obstacles | **absent** côté serveur | A* sur la grille des segments dans ArchBot |
| Achat chez un PNJ (réapprovisionnement) | opérationnel côté joueur (`npc_trade`) | non utilisé par ArchBot v0.1 (limitation documentée) |
| Quêtes par un bot | opérationnel côté joueur (dialogues) | non utilisé par ArchBot v0.1 |
