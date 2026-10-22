<#
.SYNOPSIS
    Compila y sube las imagenes de OpenDrive a Docker Hub.

.DESCRIPTION
    Este script compila el frontend, el backend o ambos en tu maquina de desarrollo y
    los publica en tu cuenta de Docker Hub con la etiqueta indicada.

.EXAMPLE
    .\deploy\push-dockerhub.ps1 -Username "miusuario"
    .\deploy\push-dockerhub.ps1 -BackendOnly
    .\deploy\push-dockerhub.ps1 -Username "miusuario" -Tag "1.0.0" -BackendOnly
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
            if ($line -match '^DOCKERHUB_USERNAME=(.+)$') {
                $Username = $matches[1].Trim()
                break
            }
        }
    }
}

if (-not $Username -or $Username -eq "tu_usuario_dockerhub" -or $Username -eq "opendrive") {
    Write-Host "Error: Debes especificar tu usuario de Docker Hub." -ForegroundColor Red
    Write-Host "Uso: .\deploy\push-dockerhub.ps1 -Username <tu_usuario> [-Tag <tag>] [-BackendOnly]" -ForegroundColor Yellow
    exit 1
}

$doBackend = (-not $FrontendOnly)
$doFrontend = (-not $BackendOnly)

Write-Host "========================================================" -ForegroundColor Cyan
Write-Host " Subiendo OpenDrive a Docker Hub" -ForegroundColor Cyan
Write-Host "   Usuario : $Username" -ForegroundColor White
Write-Host "   Tag     : $Tag" -ForegroundColor White
if ($BackendOnly) {
    Write-Host "   Modo    : SOLO BACKEND" -ForegroundColor Yellow
} elseif ($FrontendOnly) {
    Write-Host "   Modo    : SOLO FRONTEND" -ForegroundColor Yellow
} else {
    Write-Host "   Modo    : COMPLETO (Backend + Frontend)" -ForegroundColor White
}
Write-Host "========================================================" -ForegroundColor Cyan

# 1. Backend
if ($doBackend) {
    Write-Host "`n==> Compilando Backend..." -ForegroundColor Yellow
    docker build -t "$Username/opendrive-backend:$Tag" "$root\backend"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al compilar backend" }

    Write-Host "==> Subiendo Backend a Docker Hub..." -ForegroundColor Yellow
    docker push "$Username/opendrive-backend:$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al subir backend a Docker Hub" }
    Write-Host " [OK] Backend subido: $Username/opendrive-backend:$Tag" -ForegroundColor Green
}

# 2. Frontend
if ($doFrontend) {
    Write-Host "`n==> Compilando Frontend..." -ForegroundColor Yellow
    docker build -t "$Username/opendrive-frontend:$Tag" "$root\frontend"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al compilar frontend" }

    Write-Host "==> Subiendo Frontend a Docker Hub..." -ForegroundColor Yellow
    docker push "$Username/opendrive-frontend:$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Fallo al subir frontend a Docker Hub" }
    Write-Host " [OK] Frontend subido: $Username/opendrive-frontend:$Tag" -ForegroundColor Green
}

Write-Host "`n========================================================" -ForegroundColor Green
Write-Host " [OK] Proceso completado exitosamente." -ForegroundColor Green
Write-Host " En tu servidor o portatil Linux puedes descargar con:" -ForegroundColor Cyan
if ($BackendOnly) {
    Write-Host "   docker compose pull backend && docker compose up -d backend" -ForegroundColor White
} else {
    Write-Host "   docker compose pull && docker compose up -d" -ForegroundColor White
}
Write-Host "========================================================" -ForegroundColor Green
