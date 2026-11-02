# Installe ArchBot dans un dépôt ArchCore (à lancer depuis la racine d'ArchCore, Windows PowerShell 5.1 ou pwsh) :
#   powershell -ExecutionPolicy Bypass -File modules\ArchBot\scripts\install.ps1
# Étapes : vérifie la racine, clone (ou met à jour) modules\ArchBot, applique le patch, copie archbot.conf, compile.
param(
    [string]$Repo = "https://github.com/BlaMacfly/ArchBot",
    [switch]$NoBuild
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path "Cargo.toml") -or -not (Test-Path "crates\archcore")) {
    Write-Error "À lancer depuis la racine du dépôt ArchCore (Cargo.toml et crates\archcore attendus)."
}
if (-not (Test-Path "modules\ArchBot\Cargo.toml")) {
    New-Item -ItemType Directory -Force "modules" | Out-Null
    git clone $Repo "modules\ArchBot"
    if ($LASTEXITCODE -ne 0) { Write-Error "clone impossible" }
} else {
    Write-Host "modules\ArchBot déjà présent : laissé tel quel (git pull pour mettre à jour)."
}
$patch = "modules\ArchBot\patches\archcore-archbot.patch"
git apply --check $patch 2>$null
if ($LASTEXITCODE -eq 0) {
    git apply $patch
    Write-Host "Patch ArchCore appliqué."
} else {
    git apply --check -R $patch 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Patch ArchCore déjà appliqué."
    } else {
        Write-Error "Le patch ne s'applique pas sur cette révision d'ArchCore : voir modules\ArchBot\docs\ARCHCORE-CHANGES.md pour l'appliquer à la main."
    }
}
$conf = if ($env:ARCHCORE_CONF_DIR) { $env:ARCHCORE_CONF_DIR } else { "config" }
if (-not (Test-Path (Join-Path $conf "archbot.conf"))) {
    Copy-Item "modules\ArchBot\config\archbot.conf.dist" (Join-Path $conf "archbot.conf")
    Write-Host "config\archbot.conf créé (à adapter)."
}
if (-not $NoBuild) {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Error "compilation échouée" }
    Write-Host "ArchBot installé : lancer target\release\archcore.exe (le journal annonce [archbot] au démarrage du monde)."
}
