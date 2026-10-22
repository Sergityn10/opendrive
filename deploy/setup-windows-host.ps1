<#
.SYNOPSIS
    Diagnostico y preparacion del host Windows para acceder a OpenDrive desde el exterior.

.DESCRIPTION
    Este script analiza la red local, detecta la IP WAN publica, comprueba Docker y
    las reglas del Firewall de Windows para los puertos 80 (HTTP) y 443 (HTTPS/HTTP3).

.EXAMPLE
    .\deploy\setup-windows-host.ps1
    .\deploy\setup-windows-host.ps1 -AddFirewallRules
#>
[CmdletBinding()]
param(
    [switch]$AddFirewallRules
)

$ErrorActionPreference = 'Continue'

function Write-Step {
    param([string]$Title)
    Write-Host "`n=== $Title ===" -ForegroundColor Cyan
}

function Write-Info {
    param([string]$Label, [string]$Value)
    Write-Host "  ${Label}: " -NoNewline -ForegroundColor White
    Write-Host $Value -ForegroundColor Yellow
}

function Write-Success {
    param([string]$Message)
    Write-Host "  [OK] $Message" -ForegroundColor Green
}

function Write-WarningMsg {
    param([string]$Message)
    Write-Host "  [AVISO] $Message" -ForegroundColor Yellow
}

function Is-Admin {
    $currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

Write-Host "--------------------------------------------------------" -ForegroundColor Cyan
Write-Host " OpenDrive - Asistente de configuracion de acceso externo" -ForegroundColor Cyan
Write-Host "--------------------------------------------------------" -ForegroundColor Cyan

# 1. Diagnostico de Red Local
Write-Step "1. Configuracion de Red Local (LAN)"

$activeIpv4 = $null
$activeGateway = $null

try {
    $ipconfig = ipconfig | Out-String
    $lines = $ipconfig -split "`r?`n"
    foreach ($line in $lines) {
        if ($line -match 'IPv4.*:\s*([0-9\.]+)') {
            $ip = $matches[1]
            if ($ip -notmatch '^172\.' -and $ip -notmatch '^127\.' -and $ip -notmatch '^169\.254') {
                if (-not $activeIpv4) { $activeIpv4 = $ip }
            }
        }
        if ($line -match 'Puerta de enlace predeterminada.*:\s*([0-9\.]+)') {
            $gw = $matches[1]
            if ($gw -and -not $activeGateway) { $activeGateway = $gw }
        }
    }
} catch {
    Write-WarningMsg "Error al consultar la red local: $_"
}

if ($activeIpv4) {
    Write-Info "Direccion IP local (LAN) de este portatil" $activeIpv4
} else {
    $activeIpv4 = "192.168.0.20"
    Write-WarningMsg "No se pudo detectar automaticamente la IP local. Usando referencia: $activeIpv4"
}

if ($activeGateway) {
    Write-Info "Puerta de enlace (IP de tu Router)" $activeGateway
} else {
    $activeGateway = "192.168.0.1"
    Write-Info "Puerta de enlace asumida (Router)" $activeGateway
}

# 2. Diagnostico de IP Publica (WAN)
Write-Step "2. Direccion IP Publica (WAN)"
$wanIp = $null
$providers = @(
    "https://api.ipify.org",
    "https://icanhazip.com",
    "https://ifconfig.me/ip"
)

foreach ($url in $providers) {
    try {
        $resp = (Invoke-RestMethod -Uri $url -TimeoutSec 3 -ErrorAction Stop).Trim()
        if ($resp -match '^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$') {
            $wanIp = $resp
            break
        }
    } catch {}
}

if ($wanIp) {
    Write-Info "Tu IP publica actual de Internet (WAN)" $wanIp
    Write-Success "Tu conexion a Internet esta lista para enlazarla a tu dominio propio."
} else {
    Write-WarningMsg "No se pudo consultar la IP publica desde este proceso. Consulta https://whatismyip.com"
}

# 3. Estado de Docker y OpenDrive
Write-Step "3. Estado de Docker y Servicios"
try {
    $dockerPs = docker ps --format "{{.Names}} - {{.Status}}" 2>&1
    if ($LASTEXITCODE -eq 0 -and $dockerPs) {
        Write-Success "Docker esta en ejecucion."
        $opendriveContainers = $dockerPs | Select-String "opendrive"
        if ($opendriveContainers) {
            Write-Host "  Contenedores de OpenDrive activos:" -ForegroundColor Gray
            $opendriveContainers | ForEach-Object { Write-Host "    - $_" -ForegroundColor Green }
        } else {
            Write-WarningMsg "Los contenedores de OpenDrive no estan corriendo. Inicialos con: docker compose up -d"
        }
    } else {
        Write-WarningMsg "Docker no parece responder o no esta iniciado."
    }
} catch {
    Write-WarningMsg "No se pudo contactar con Docker: $_"
}

# 4. Comprobacion y Configuracion del Firewall de Windows
Write-Step "4. Firewall de Windows (Puertos 80 y 443)"
$isAdmin = Is-Admin

$fwRules = @()
try {
    $fwRules = Get-NetFirewallRule -DisplayName "*OpenDrive*" -ErrorAction SilentlyContinue
} catch {}

if ($fwRules) {
    Write-Success "Reglas del firewall de Windows para OpenDrive ya existen:"
    $fwRules | ForEach-Object { Write-Host "    - $($_.DisplayName) [Habilitada: $($_.Enabled)]" -ForegroundColor Gray }
} else {
    Write-WarningMsg "No se han detectado reglas de entrada especificas en el Firewall de Windows para OpenDrive."
    
    if ($AddFirewallRules -or $isAdmin) {
        Write-Host "  Creando reglas de Firewall de Windows (como Administrador)..." -ForegroundColor Cyan
        try {
            New-NetFirewallRule -DisplayName "OpenDrive HTTP (Puerto 80)" -Direction Inbound -LocalPort 80 -Protocol TCP -Action Allow -Profile Any -ErrorAction Stop | Out-Null
            New-NetFirewallRule -DisplayName "OpenDrive HTTPS (Puerto 443 TCP)" -Direction Inbound -LocalPort 443 -Protocol TCP -Action Allow -Profile Any -ErrorAction Stop | Out-Null
            New-NetFirewallRule -DisplayName "OpenDrive HTTP3 (Puerto 443 UDP)" -Direction Inbound -LocalPort 443 -Protocol UDP -Action Allow -Profile Any -ErrorAction Stop | Out-Null
            Write-Success "Reglas de Firewall anadidas correctamente."
        } catch {
            Write-WarningMsg "No se pudieron crear automaticamente las reglas: $_"
        }
    } else {
        Write-Host "  Para permitir el trafico entrante desde tu router sin bloqueos de Windows," -ForegroundColor Yellow
        Write-Host "  abre PowerShell como Administrador y ejecuta:" -ForegroundColor Yellow
        Write-Host "    New-NetFirewallRule -DisplayName 'OpenDrive HTTP' -Direction Inbound -LocalPort 80 -Protocol TCP -Action Allow" -ForegroundColor White
        Write-Host "    New-NetFirewallRule -DisplayName 'OpenDrive HTTPS' -Direction Inbound -LocalPort 443 -Protocol TCP -Action Allow" -ForegroundColor White
        Write-Host "    New-NetFirewallRule -DisplayName 'OpenDrive HTTP3' -Direction Inbound -LocalPort 443 -Protocol UDP -Action Allow" -ForegroundColor White
    }
}

# 5. Recomendacion de Energia en Portatil
Write-Step "5. Configuracion de Energia del Portatil (Recomendacion)"
Write-Host "  Dado que vas a usar el portatil como servidor:" -ForegroundColor Gray
Write-Host "  1. Configuracion de Windows -> Sistema -> Inicio/apagado y suspension:" -ForegroundColor White
Write-Host "     - Suspender tras: 'Nunca' (cuando este conectado a la corriente)." -ForegroundColor White
Write-Host "  2. Panel de control -> Opciones de energia -> Elegir la accion del cierre de la tapa:" -ForegroundColor White
Write-Host "     - Al cerrar la tapa (con corriente): 'No hacer nada'." -ForegroundColor White

# 6. Resumen de configuracion externa
Write-Step "6. Resumen de Pasos a Configurar en tu Router y Dominio"

$displayWan = if ($wanIp) { $wanIp } else { "Tu_IP_WAN" }

Write-Host "A. EN EL PANEL DE TU ROUTER (Accede a http://$activeGateway):" -ForegroundColor Cyan
Write-Host "   1. Reserva de IP Local (DHCP Static Lease):" -ForegroundColor White
Write-Host "      Fija la IP $activeIpv4 para la tarjeta de red de este portatil." -ForegroundColor Gray
Write-Host "   2. Reenvio de Puertos (Port Forwarding / Virtual Server):" -ForegroundColor White
Write-Host "      - Regla 1: Puerto Externo 80  -> IP Interna $activeIpv4 : Puerto 80  (Protocolo: TCP)" -ForegroundColor Gray
Write-Host "      - Regla 2: Puerto Externo 443 -> IP Interna $activeIpv4 : Puerto 443 (Protocolo: TCP y UDP)" -ForegroundColor Gray
Write-Host ""
Write-Host "B. EN TU PROVEEDOR DE DOMINIO:" -ForegroundColor Cyan
Write-Host "   Crea un registro DNS de tipo 'A':" -ForegroundColor White
Write-Host "      - Tipo: A" -ForegroundColor Gray
Write-Host "      - Nombre/Host: drive  (o '@' para el dominio raiz)" -ForegroundColor Gray
Write-Host "      - Valor/Destino: $displayWan" -ForegroundColor Gray
Write-Host "      - TTL: 300 segundos (o el valor minimo permitido)" -ForegroundColor Gray
Write-Host ""
Write-Host "C. EN EL FICHERO .env DE OPENDRIVE:" -ForegroundColor Cyan
Write-Host "   Edita tu fichero .env con:" -ForegroundColor White
Write-Host "      DOMAIN=drive.tudominio.com" -ForegroundColor Gray
Write-Host "      ACME_EMAIL=tu-correo@gmail.com" -ForegroundColor Gray
Write-Host ""
Write-Host "D. SI TU IP WAN ES DINAMICA (Y el router no tiene DDNS activo):" -ForegroundColor Cyan
Write-Host "   Activa el servicio de actualizacion de DNS en Docker:" -ForegroundColor White
Write-Host "      1. Copia deploy/ddns/config.json.example a deploy/ddns/config.json y configuralo." -ForegroundColor Gray
Write-Host "      2. Ejecuta: docker compose --profile ddns up -d" -ForegroundColor Gray
Write-Host ""
Write-Host "--------------------------------------------------------" -ForegroundColor Cyan
Write-Host " Todo listo! Consulta docs/ACCESO_EXTERNO.md para mas detalles." -ForegroundColor Green
Write-Host "--------------------------------------------------------" -ForegroundColor Cyan
