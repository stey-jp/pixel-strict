param([string]$ImagePath, [switch]$Release)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$resolvedImage = if ($ImagePath) { (Resolve-Path -LiteralPath $ImagePath).Path } else { $null }
$priorToolchain = $env:RUSTUP_TOOLCHAIN
try {
    $env:RUSTUP_TOOLCHAIN = '1.97.1'
    Push-Location (Join-Path $projectRoot 'app')
    flutter pub get
    # pub get may finish dependency resolution but fail creating symbolic links.
    & (Join-Path $PSScriptRoot 'windows-plugin-links.ps1')
    $flutterArgs = @('run', '-d', 'windows', '--no-pub')
    if ($Release) { $flutterArgs += '--release' }
    if ($resolvedImage) { $flutterArgs += "--dart-entrypoint-args=$resolvedImage" }
    & flutter @flutterArgs
    if ($LASTEXITCODE -ne 0) { throw 'Flutter run failed.' }
} finally {
    Pop-Location
    $env:RUSTUP_TOOLCHAIN = $priorToolchain
}
