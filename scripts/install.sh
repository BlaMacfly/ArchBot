#!/bin/sh
# Installe ArchBot dans un dépôt ArchCore (à lancer depuis la racine d'ArchCore) :
#   sh modules/ArchBot/scripts/install.sh [--no-build]
# Étapes : vérifie la racine, clone (ou garde) modules/ArchBot, applique le patch, copie archbot.conf, compile.
set -e
REPO="${ARCHBOT_REPO:-https://github.com/BlaMacfly/ArchBot}"
if [ ! -f Cargo.toml ] || [ ! -d crates/archcore ]; then
  echo "À lancer depuis la racine du dépôt ArchCore (Cargo.toml et crates/archcore attendus)." >&2
  exit 1
fi
if [ ! -f modules/ArchBot/Cargo.toml ]; then
  mkdir -p modules
  git clone "$REPO" modules/ArchBot
else
  echo "modules/ArchBot déjà présent : laissé tel quel (git pull pour mettre à jour)."
fi
PATCH=modules/ArchBot/patches/archcore-archbot.patch
if git apply --check "$PATCH" 2>/dev/null; then
  git apply "$PATCH"
  echo "Patch ArchCore appliqué."
elif git apply --check -R "$PATCH" 2>/dev/null; then
  echo "Patch ArchCore déjà appliqué."
else
  echo "Le patch ne s'applique pas sur cette révision d'ArchCore : voir modules/ArchBot/docs/ARCHCORE-CHANGES.md." >&2
  exit 1
fi
CONF="${ARCHCORE_CONF_DIR:-config}"
if [ ! -f "$CONF/archbot.conf" ]; then
  cp modules/ArchBot/config/archbot.conf.dist "$CONF/archbot.conf"
  echo "$CONF/archbot.conf créé (à adapter)."
fi
if [ "$1" != "--no-build" ]; then
  cargo build --release
  echo "ArchBot installé : lancer target/release/archcore (le journal annonce [archbot] au démarrage du monde)."
fi
