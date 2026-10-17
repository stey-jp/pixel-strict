# Reversible, workspace-local alternative when Developer Mode is unavailable.
# Run after flutter pub get. No OS setting is changed.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$metadata = Join-Path $projectRoot 'app/.flutter-plugins-dependencies'
if (-not (Test-Path -LiteralPath $metadata)) { throw 'Run flutter pub get in app first.' }
$plugins = Get-Content -LiteralPath $metadata -Raw | ConvertFrom-Json
function Get-NormalizedDirectory([string]$DirectoryPath) {
    $fullPath = [IO.Path]::GetFullPath($DirectoryPath)
    if ($fullPath -eq [IO.Path]::GetPathRoot($fullPath)) { return $fullPath }
    return $fullPath.TrimEnd([char[]]'\/')
}
$linkRoot = Join-Path $projectRoot 'app/windows/flutter/ephemeral/.plugin_symlinks'
$linkRoot = [IO.Path]::GetFullPath($linkRoot)
New-Item -ItemType Directory -Path $linkRoot -Force | Out-Null
foreach ($plugin in $plugins.plugins.windows) {
    if ($plugin.name -notmatch '^[a-zA-Z0-9_]+$') { throw 'Invalid plugin name.' }
    $destination = Join-Path $linkRoot $plugin.name
    $target = Get-NormalizedDirectory $plugin.path
    if (-not (Test-Path -LiteralPath $target -PathType Container)) { throw "Missing plugin: $target" }
    $existing = Get-Item -LiteralPath $destination -Force -ErrorAction SilentlyContinue
    if ($null -ne $existing) {
        if ($existing.LinkType -notin @('Junction', 'SymbolicLink')) {
            throw "Plugin destination is not a link: $destination"
        }
        $oldTarget = [string]($existing.Target | Select-Object -First 1)
        if (-not [IO.Path]::IsPathRooted($oldTarget)) { $oldTarget = Join-Path $linkRoot $oldTarget }
        $oldTarget = Get-NormalizedDirectory $oldTarget
        if ($oldTarget.Equals($target, [StringComparison]::OrdinalIgnoreCase)) { continue }
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($destination)) -ne $linkRoot) {
            throw 'Plugin link must stay inside the project plugin directory.'
        }
        # Remove only the link itself. The old plugin directory is never removed.
        $existing.Delete()
    }
    New-Item -ItemType Junction -Path $destination -Target $target | Out-Null
}
Write-Host 'Windows plugin junctions ready. Use flutter commands with --no-pub.'
