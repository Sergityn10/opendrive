<#
.SYNOPSIS
    Compila y sube las imagenes de OpenDrive a GitHub Container Registry (ghcr.io).

.DESCRIPTION
    Este script compila el frontend, el backend o ambos en tu maquina de desarrollo y
    los publica en ghcr.io/<usuario> con la etiqueta indicada.

.EXAMPLE
    .\deploy\push-ghcr.ps1 -Username "miusuario"
    .\deploy\push-ghcr.ps1 -Username "miusuario" -BackendOnly
    .\deploy\push-ghcr.ps1 -Username "miusuario" -Tag "1.0.0" -BackendOnly
#>
[CmdletBinding()]
param(
    [Parameter(Position=0)]
    [string]$Username,
    
    [Parameter(Position=1)]
    [string]$Tag = "latest",

    [switch]$BackendOnly,
    [switch]$FrontendOnly
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot

if (-not $Username) {
    $envPath = Join-Path $root ".env"
    if (Test-Path $envPath) {
        $lines = Get-Content $envPath
        foreach ($line in $lines) {
            if ($line -match '^IMAGE_PREFIX=ghcr\.io/(.+)$') {
                $Username = $matches[1].Trim()
                break
            } elseif ($line -match '^DOCKERHUB_USERNAME=(.+)$' -and -not $Username) {
                $Username = $matches[1].Trim()
            }
        }
    }
}

if (-not $Username -or $Username -eq "tu_usuario_dockerhub" -or $Username -eq "opendrive") {
    Write-Host "Error: Debes especificar tu usuario de GitHub." -ForegroundColor Red
    Write-Host "Uso: .\deploy\push-ghcr.ps1 -Username <tu_usuario_github> [-Tag <tag>] [-BackendOnly]" -ForegroundColor Yellow
    exit 1
}

# GHCR exige nombres de imagen en minúsculas
$Username = $Username.ToLower()
$registry = "ghcr.io/$Username"

$doBackend = (-not $FrontendOnly)
$doFrontend = (-not $BackendOnly)

Write-Host "========================================================" -ForegroundColor Cyan
Write-Host " Subiendo OpenDrive a GitHub Container Registry (ghcr.io)" -ForegroundColor Cyan
Write-Host "   Repositorio: $registry" -ForegroundColor White
Write-Host "   Tag        : $Tag" -ForegroundColor White
if ($BackendOnly) {
    Write-Host "   Modo       : SOLO BACKEND" -ForegroundColor Yellow
} elseif ($FrontendOnly) {
    Write-Host "   Modo       : SOLO FRONTEND" -ForegroundColor Yellow
} else {
    Write-Host "   Modo       : COMPLETO (Backend + Frontend)" -ForegroundColor White
}
Write-Host "========================================================" -ForegroundColor Cyan

# 1. Backend
if ($doBackend) {
    Write-Host "`n==> Compilando Backend..." -ForegroundColor Yellow
    docker build -t "$registry/opendrive-backend:$Tag" "$root\backend"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al compilar backend" }

    Write-Host "==> Subiendo Backend a ghcr.io..." -ForegroundColor Yellow
    docker push "$registry/opendrive-backend:$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al subir backend a ghcr.io" }
    Write-Host " [OK] Backend subido: $registry/opendrive-backend:$Tag" -ForegroundColor Green
}

# 2. Frontend
if ($doFrontend) {
    Write-Host "`n==> Compilando Frontend..." -ForegroundColor Yellow
    docker build -t "$registry/opendrive-frontend:$Tag" "$root\frontend"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al compilar frontend" }

    Write-Host "==> Subiendo Frontend a ghcr.io..." -ForegroundColor Yellow
    docker push "$registry/opendrive-frontend:$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al subir frontend a ghcr.io" }
    Write-Host " [OK] Frontend subido: $registry/opendrive-frontend:$Tag" -ForegroundColor Green
}

Write-Host "`n========================================================" -ForegroundColor Green
Write-Host " [OK] Proceso completado exitosamente." -ForegroundColor Green
Write-Host " En tu servidor o portatil Linux configura en .env:" -ForegroundColor Cyan
Write-Host "   IMAGE_PREFIX=$registry" -ForegroundColor White
Write-Host " Y descarga con:" -ForegroundColor Cyan
if ($BackendOnly) {
    Write-Host "   docker compose pull backend && docker compose up -d backend" -ForegroundColor White
} else {
    Write-Host "   docker compose pull && docker compose up -d" -ForegroundColor White
}
Write-Host "========================================================" -ForegroundColor Green
