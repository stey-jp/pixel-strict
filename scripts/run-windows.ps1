param([string]$ImagePath, [switch]$Release)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$resolvedImage = if ($ImagePath) { (Resolve-Path -LiteralPath $ImagePath).Path } else { $null }
$priorToolchain = $env:RUSTUP_TOOLCHAIN
try {
    $env:RUSTUP_TOOLCHAIN = '1.97.1'
    Push-Location (Join-Path $projectRoot 'app')
    Get-Command flutter -ErrorAction Stop | Out-Null
    $pubResult = & {
        # Windows PowerShell turns redirected native stderr into error records.
        # Inspect the exit code ourselves, including the known symlink failure.
        $ErrorActionPreference = 'Continue'
        $log = & flutter pub get 2>&1
        [pscustomobject]@{ Output = $log; ExitCode = $LASTEXITCODE }
    }
    $pubOutput = $pubResult.Output
    $pubExitCode = $pubResult.ExitCode
    $pubOutput | ForEach-Object { Write-Host $_ }
    $pubText = $pubOutput | Out-String
    # Allow only Flutter's known symlink-privilege failure after resolution.
    # Other failures must not launch with stale dependency metadata.
    if ($pubExitCode -ne 0 -and -not (
        $pubText.Contains('Got dependencies!') -and
        $pubText.Contains('Building with plugins requires symlink support.') -and
        (Test-Path -LiteralPath '.dart_tool/package_config.json') -and
        (Test-Path -LiteralPath '.flutter-plugins-dependencies')
    )) {
        throw "Flutter pub get failed (exit $pubExitCode)."
    }
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
