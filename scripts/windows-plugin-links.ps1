# Reversible, workspace-local alternative when Developer Mode is unavailable.
# Run after flutter pub get. No OS setting is changed.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$metadata = Join-Path $projectRoot 'app/.flutter-plugins-dependencies'
if (-not (Test-Path -LiteralPath $metadata)) { throw 'Run flutter pub get in app first.' }
$plugins = Get-Content -LiteralPath $metadata -Raw | ConvertFrom-Json
$linkRoot = Join-Path $projectRoot 'app/windows/flutter/ephemeral/.plugin_symlinks'
New-Item -ItemType Directory -Path $linkRoot -Force | Out-Null
foreach ($plugin in $plugins.plugins.windows) {
    if ($plugin.name -notmatch '^[a-zA-Z0-9_]+$') { throw 'Invalid plugin name.' }
    $destination = Join-Path $linkRoot $plugin.name
    if (-not (Test-Path -LiteralPath $destination)) {
        New-Item -ItemType Junction -Path $destination -Target $plugin.path | Out-Null
    }
}
Write-Host 'Windows plugin junctions ready. Use flutter commands with --no-pub.'
