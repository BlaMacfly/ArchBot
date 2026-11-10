# Installation

ArchBot s'installe **dans** un dépôt ArchCore, comme un module : le dépôt est cloné dans `modules/ArchBot`,
un petit patch ajoute à ArchCore les points d'accroche dont le module a besoin, et la feature Cargo
`archbot` (active par défaut après le patch) compile l'adaptateur dans l'exécutable `archcore`.

## Prérequis

- ArchCore à la révision pour laquelle le patch a été écrit (voir [ARCHCORE-CHANGES.md](ARCHCORE-CHANGES.md) ;
  branche `Core`, commit `6b1bf96` du 9 novembre 2026). Sur une révision plus récente, `git apply` peut
  demander un ajustement à la main : les modifications sont petites et décrites une par une.
- Rust 1.85 ou plus, MariaDB, et les bases d'ArchCore initialisées (`archcore db-init`, `archcore import`),
  comme pour le serveur lui-même.

## Étapes

```bash
cd ArchCore
git clone https://github.com/BlaMacfly/ArchBot modules/ArchBot
git apply --check modules/ArchBot/patches/archcore-archbot.patch   # vérifie que le patch s'applique
git apply modules/ArchBot/patches/archcore-archbot.patch
cp modules/ArchBot/config/archbot.conf.dist config/                 # facultatif : copié au premier démarrage
cargo build --release
```

Ou, sous Windows : `powershell -ExecutionPolicy Bypass -File modules\ArchBot\scripts\install.ps1` depuis la
racine d'ArchCore (et `sh modules/ArchBot/scripts/install.sh` sous Linux), qui enchaînent ces étapes.

Au démarrage du serveur monde, le journal annonce :

```text
INFO  [archbot] v0.1.0 : bots 100 au plus, population initiale 20, niveaux 1..10, pensée toutes les 500 ms (50 bots par tour), autonomes oui, compagnons oui ; 0 personnage(s) de bot en base
INFO  [archbot] personnage créé : Baved (휴먼 여자마법사, niveau 3, compte archbot_baved, cid 11)
INFO  [archbot] Baved entre en jeu (session #4, 4 bot(s))
```

Le `modules/` est ignoré par le `.gitignore` d'ArchCore (ajout du patch) : le module ne pollue pas le dépôt
du serveur. Les bases ne reçoivent qu'une table, `archbot_character` (base des personnages), créée au
premier démarrage.

## Serveur sans bots

- `ArchBot.Enable = 0` dans `config/archbot.conf` : le module est compilé mais ne démarre pas.
- `cargo build --release --no-default-features` : le module n'est pas compilé du tout (le dossier
  `modules/ArchBot` doit rester présent, Cargo lisant son manifeste).

## Mise à jour

```bash
cd ArchCore/modules/ArchBot && git pull
cd ../.. && cargo build --release
```

Si le patch d'ArchCore change entre deux versions d'ArchBot, le journal des versions
([CHANGELOG.md](../CHANGELOG.md)) le dit ; il faut alors rejouer le patch sur un ArchCore propre.

## Désinstallation

```bash
cd ArchCore
git apply -R modules/ArchBot/patches/archcore-archbot.patch
rm -rf modules/ArchBot config/archbot.conf
cargo build --release
```

Les comptes `archbot_*` et leurs personnages restent en base (ils sont inoffensifs) ; pour les retirer :

```sql
UPDATE archcore_characters.characters SET deleted_at = NOW()
 WHERE id IN (SELECT character_id FROM archcore_characters.archbot_character);
DROP TABLE archcore_characters.archbot_character;
DELETE FROM archcore_auth.account WHERE username LIKE 'archbot\_%';
```

## Serveur de test (ARCHCORE_CONF_DIR)

Comme pour `archcore sim`, un second jeu de configuration permet de faire tourner les bots sur des bases de
test : copier `config/*.dist` dans un dossier, y changer ports et noms de bases, y mettre un
`archbot.conf`, puis `ARCHCORE_CONF_DIR=<dossier> archcore all`. Les scénarios `archcore sim bots` lisent
la même variable.
