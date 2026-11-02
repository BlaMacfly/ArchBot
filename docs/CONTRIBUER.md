# Guide pour les contributeurs

## Règles

- **Rien d'inventé** : une mécanique d'ArchLord vient d'ArchCore (code, tables importées) ou d'une source
  vérifiable ; une donnée absente est dite absente (voir `docs/LIMITATIONS.md`). Pas de compétence, d'objet
  ou de règle fictive.
- **Le serveur décide** : le moteur demande, ArchCore applique ses règles (portée, cadence, propriété du
  butin, prix…). Un bot ne doit jamais contourner un contrôle que subit un joueur.
- **Pas de système parallèle** : combat, inventaire, déplacement, groupe, sauvegarde passent par les
  fonctions d'ArchCore. Si une fonction manque, l'exposer (`pub(crate)`) ou la découper dans le patch, pas la
  réécrire.
- **Le moteur reste pur** : la crate `archbot` ne dépend pas d'ArchCore ; tout ce qu'elle sait vient de
  `world::Perception`. Les règles d'ArchLord utilisées par le moteur sont commentées avec leur source.
- Code et commentaires en français, comme ArchCore ; `README` bilingue.
- Pas de code copié de mod-playerbots (GPL-2) : concepts seulement, cités dans
  `docs/CORRESPONDANCE-PLAYERBOTS.md`.

## Développement

```bash
# moteur seul
cargo test
cargo clippy --all-targets

# avec ArchCore (depuis la racine d'ArchCore, module installé)
cargo build --release
cargo test -p archcore
ARCHCORE_CONF_DIR=config_test target/release/archcore all      # serveur de test
ARCHCORE_CONF_DIR=config_test target/release/archcore sim bots gmtest gmtest GmTest 3
```

Un serveur de test : copier `config/*.dist`, changer ports (11102/11108) et bases (`archcore_test_*`),
`archcore db-init`, `archcore import`, un compte MJ (`db-init --account gmtest gmtest 3`).

## Où ajouter quoi

| Besoin | Où |
|---|---|
| Nouvel état ou priorité de décision | `src/ai/brain.rs` (`think`), `src/ai/state.rs` |
| Nouvelle action possible | `src/world.rs` (`Action`), exécution dans `archcore/actions.rs` |
| Nouvelle information perçue | `src/world.rs` (`Perception`, vues), construction dans `archcore/perception.rs` |
| Règle de ciblage, de survie, de classe | `src/combat/` |
| Commande de discussion | `src/commands/mod.rs` (analyse), `archcore/chat.rs` (droits, réponses) |
| Réglage | `src/config.rs` (clé, borne, défaut, test), `config/archbot.conf.dist`, `docs/CONFIGURATION.md` |
| Vérification en jeu | `archcore/sim_bots.rs` (verdict PASS / FAIL / BLOCKED / NOT RUN) |

Chaque changement de comportement vient avec un test du moteur (`src/…` ou `tests/scenarios.rs`) et, s'il
touche le serveur, un point dans `sim bots`. Les résultats observés vont dans `docs/TESTS.md`, en séparant ce
qui est unitaire, serveur et en jeu.

## Versions

`CHANGELOG.md` ; la version de `Cargo.toml` est celle du tag. Un changement du patch d'ArchCore est
toujours noté dans le journal des versions.
