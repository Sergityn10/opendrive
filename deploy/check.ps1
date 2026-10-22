<#
.SYNOPSIS
    Compila y comprueba el backend de OpenDrive dentro de un contenedor Linux,
    igual que en el Dockerfile de producción.

.EXAMPLE
    ./deploy/check.ps1                # cargo check
    ./deploy/check.ps1 -Mode clippy   # cargo clippy
    ./deploy/check.ps1 -Mode test     # cargo test
#>
param(
    [ValidateSet('check', 'clippy', 'test', 'fmt', 'build')]
    [string]$Mode = 'check'
)

$root = Split-Path -Parent $PSScriptRoot
$backend = Join-Path $root 'backend'
$image = if ($env:RUST_IMAGE) { $env:RUST_IMAGE } else { 'rust:1.99.1-bookworm' }

$commands = @{
    check  = 'cargo check --workspace --all-targets'
    clippy = 'cargo clippy --workspace --all-targets --all-features -- -D warnings'
    test   = 'cargo test --workspace'
    fmt    = 'cargo fmt --all --check'
    build  = 'cargo build --workspace --release'
}

Write-Host "==> $Mode ($image)" -ForegroundColor Cyan

# La salida se captura a fichero porque PowerShell trata cualquier escritura en
# stderr de un proceso nativo como error terminante con ErrorActionPreference=Stop.
$log = Join-Path $env:TEMP "opendrive-$Mode.log"
$args = @(
    'run', '--rm',
    '-v', "${backend}:/app",
    '-v', 'opendrive-cargo-registry:/usr/local/cargo/registry',
    '-v', 'opendrive-cargo-git:/usr/local/cargo/git',
    '-v', 'opendrive-target:/app/target',
    '-w', '/app',
    '-e', 'SQLX_OFFLINE=true',
    '-e', 'CARGO_TERM_COLOR=never',
    $image,
    'bash', '-c', "set -euo pipefail; $($commands[$Mode])"
)

& docker @args *>&1 | Tee-Object -FilePath $log
$code = $LASTEXITCODE

if ($code -ne 0) {
    Write-Host ""
    Write-Host "FALLO ($Mode). Salida completa:" -ForegroundColor Red
    Get-Content -LiteralPath $log -Tail 120
}

exit $code
