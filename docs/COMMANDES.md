# Commandes

Les commandes passent par la discussion ordinaire du client (préfixe `/bot`), par un **chuchotement au bot**,
ou par la discussion de groupe (`/p ordre`). Les réponses des bots arrivent en chuchotement ; celles du module
en message système.

## Joueurs

| Commande | Effet |
|---|---|
| `/bot help` | Aide |
| `/bot list` | Bots en jeu (nom, niveau, temps en jeu, mode, état, cible, compteurs dont niveaux et expérience gagnés ; 20 lignes au plus) |
| `/bot hire [nom]` | Le bot libre le plus proche (à moins de 20 000 unités), ou le bot nommé, devient votre compagnon. Personne de libre : un nouveau bot est créé à côté de vous si `ArchBot.MaxBots` le permet. Le compagnon vous invite dans son groupe (acceptez l'invitation). |
| `/bot dismiss [nom]` | Vos compagnons (ou celui nommé) redeviennent autonomes et quittent le groupe |
| `/bot follow [distance]` | Vous suivre, à cette distance (100 à 3000) |
| `/bot stay` | Rester sur place |
| `/bot come` | Vous rejoindre et reprendre le suivi |
| `/bot attack` | Attaquer votre cible, même en attitude passive |
| `/bot passive` | N'attaquer jamais de lui-même (il riposte seulement s'il est frappé) |
| `/bot defensive` | Défendre son maître (qui attaque le maître, puis la cible du maître) — attitude par défaut |
| `/bot aggressive` | En plus, engager les monstres à moins de 1500 unités du maître |
| `/bot stop` | Cesser le combat et revenir au suivi |
| `/bot status` | État du compagnon |

Les mêmes ordres se donnent par chuchotement (`/w NomDuBot stay`), auquel le bot répond, ou à tous vos
compagnons du groupe par `/p stay`. Un chuchotement « hire » engage le bot, « leave » le libère ; tout
autre texte reçoit l'aide. Seul le maître d'un compagnon peut lui donner des ordres.

## Maîtres du jeu (`ArchBot.CommandLevel`, 2 par défaut)

| Commande | Effet |
|---|---|
| `/bot stats` | Métriques : bots, tours d'IA (dernier, moyenne, EMA, maximum), bots par état, actions exécutées par nature, entrées, sorties, monstres tués, morts, niveaux gagnés, expérience gagnée (64 bits), pensées, erreurs |
| `/bot stats reset` | Remet les métriques à zéro (tests de charge) |
| `/bot spawn <n>` | Fait entrer n bots tout de suite (réserve, sinon création) ; la population visée suit |
| `/bot despawn <n>` / `all` / `<nom>` | Sort n bots autonomes (les plus anciens), tous, ou celui nommé |
| `/bot population <n>` | Population de bots autonomes visée : le gestionnaire en fait entrer (un par `SpawnIntervalMs`) ou fait se déconnecter les bots en trop, comme des joueurs (au calme d'abord, deux par tour ; en combat ou morts après 30 s) |

Toutes les commandes `/bot` sont consommées par le module : elles n'atteignent ni les commandes MJ
d'ArchCore ni la discussion.

## Commandes du simulateur (`archcore sim`)

| Commande | Effet |
|---|---|
| `archcore sim bots [compte] [mdp] [perso] [n]` | 16 vérifications de bout en bout à travers le vrai protocole (compte MJ) |
| `archcore sim bots load [paliers] [compte] [mdp] [perso]` | Test de charge : population par paliers, 60 s de mesure chacun |
| `archcore sim bots gmtest gmtest GmTest companion test test Testeur [minutes]` | Essai long d'un compagnon engagé par un joueur sans droits MJ : `/bot hire`, groupe, chasse en attitude défensive puis offensive, `/bot stats` chaque minute, 5 000 000 000 points d'expérience donnés au compagnon (`/exp`) et comptés par son statut, `/bot dismiss` ; niveaux rétablis à la fin |
| `archcore sim client gmtest gmtest GmTest "/bot list"` | Toute commande `/bot …` depuis un client simulé |
