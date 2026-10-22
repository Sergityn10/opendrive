import { useState } from "react";
import { Link } from "react-router";
import {
  Server,
  Layers,
  ShieldCheck,
  Cloud,
  Copy,
  Check,
  Download,
  RefreshCw,
  Terminal,
  Key,
  Folder,
  Globe,
  Sliders,
  ChevronDown,
  ChevronUp,
  Sparkles,
  ArrowLeft,
  CheckCircle2,
} from "lucide-react";
import { cn } from "@/lib/cn";

export type DeployMode = "backend-only" | "full" | "tls" | "tunnel";
export type RegistryType = "dockerhub" | "ghcr" | "local";
export type PostgresVolumeType = "named" | "bind";

function generateSecureSecret(length = 24): string {
  const chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*(-_=+.)";
  const array = new Uint8Array(length);
  crypto.getRandomValues(array);
  return Array.from(array, (byte) => chars[byte % chars.length]).join("");
}

function downloadTextFile(filename: string, content: string) {
  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

interface ComposeGeneratorPageProps {
  isPublic?: boolean;
}

export function ComposeGeneratorPage({ isPublic = false }: ComposeGeneratorPageProps) {
  // Configuración general
  const [mode, setMode] = useState<DeployMode>("backend-only");
  const [registryType, setRegistryType] = useState<RegistryType>("dockerhub");
  const [dockerUser, setDockerUser] = useState("sergityn");
  const [imageTag, setImageTag] = useState("latest");

  // Red y puertos
  const [backendPort, setBackendPort] = useState("8080");
  const [webPort, setWebPort] = useState("3000");
  const [domain, setDomain] = useState("drive.tudominio.com");
  const [acmeEmail, setAcmeEmail] = useState("admin@tudominio.com");
  const [cloudflareToken, setCloudflareToken] = useState("");

  // Almacenamiento
  const [dataPath, setDataPath] = useState("./data");
  const [postgresVolType, setPostgresVolType] = useState<PostgresVolumeType>("named");
  const [postgresBindPath, setPostgresBindPath] = useState("./postgres-data");

  // Credenciales
  const [postgresUser, setPostgresUser] = useState("opendrive");
  const [postgresPassword, setPostgresPassword] = useState(() => generateSecureSecret(20));
  const [postgresDb, setPostgresDb] = useState("opendrive");

  const [adminEmail, setAdminEmail] = useState("admin@opendrive.local");
  const [adminUsername, setAdminUsername] = useState("admin");
  const [adminPassword, setAdminPassword] = useState(() => generateSecureSecret(20));

  // Opciones avanzadas
  const [allowRegistration, setAllowRegistration] = useState(false);
  const [maxUploadSize, setMaxUploadSize] = useState("0");
  const [sessionTtlHours, setSessionTtlHours] = useState("24");
  const [rustLog, setRustLog] = useState("info");
  const [showAdvanced, setShowAdvanced] = useState(false);

  // Estado UI
  const [activeTab, setActiveTab] = useState<"compose" | "env" | "cli">("compose");
  const [copied, setCopied] = useState(false);

  // Regenerar todas las contraseñas
  function regeneratePasswords() {
    setPostgresPassword(generateSecureSecret(20));
    setAdminPassword(generateSecureSecret(20));
  }

  // Nombres de imágenes según registro
  let backendImage = "";
  let frontendImage = "";

  if (registryType === "dockerhub") {
    const user = dockerUser.trim() || "sergityn";
    backendImage = `${user}/opendrive-backend:${imageTag}`;
    frontendImage = `${user}/opendrive-frontend:${imageTag}`;
  } else if (registryType === "ghcr") {
    const user = dockerUser.trim().toLowerCase() || "sergityn";
    backendImage = `ghcr.io/${user}/opendrive-backend:${imageTag}`;
    frontendImage = `ghcr.io/${user}/opendrive-frontend:${imageTag}`;
  } else {
    backendImage = `opendrive-backend:${imageTag}`;
    frontendImage = `opendrive-frontend:${imageTag}`;
  }

  // Generación dinámica de compose.yaml
  function buildComposeYaml(): string {
    const isBackendOnly = mode === "backend-only";
    const isTls = mode === "tls";
    const isTunnel = mode === "tunnel";

    const lines: string[] = [];
    lines.push(`# ==============================================================================`);
    lines.push(`# OpenDrive — Configuración personalizada de Docker Compose`);
    lines.push(`# Generado para modo: ${mode.toUpperCase()}`);
    lines.push(`# ==============================================================================`);
    lines.push(`name: opendrive${isBackendOnly ? "-backend" : ""}`);
    lines.push(``);
    lines.push(`services:`);

    // 1. Backend Service
    lines.push(`  # Backend: API REST Rust + WebDAV RFC 4918 + Subidas resumibles tus`);
    lines.push(`  backend:`);
    lines.push(`    image: ${backendImage}`);
    if (registryType === "local") {
      lines.push(`    build:`);
      lines.push(`      context: ./backend`);
      lines.push(`      dockerfile: Dockerfile`);
    }
    lines.push(`    restart: unless-stopped`);
    lines.push(`    ports:`);
    lines.push(`      - "\${BACKEND_PORT:-${backendPort}}:8080"`);
    lines.push(`    environment:`);
    lines.push(`      DATABASE_URL: postgres://\${POSTGRES_USER:-${postgresUser}}:\${POSTGRES_PASSWORD}@postgres:5432/\${POSTGRES_DB:-${postgresDb}}`);
    lines.push(`      OPENDRIVE_BIND: 0.0.0.0:8080`);
    lines.push(`      OPENDRIVE_DATA_DIR: /data`);
    if (isTls) {
      lines.push(`      OPENDRIVE_PUBLIC_URL: https://${domain}`);
    } else {
      lines.push(`      OPENDRIVE_PUBLIC_URL: \${OPENDRIVE_PUBLIC_URL:-http://localhost:${isBackendOnly ? backendPort : webPort}}`);
    }
    lines.push(`      OPENDRIVE_ADMIN_EMAIL: \${ADMIN_EMAIL:-${adminEmail}}`);
    lines.push(`      OPENDRIVE_ADMIN_PASSWORD: \${ADMIN_PASSWORD}`);
    lines.push(`      OPENDRIVE_ADMIN_USERNAME: \${ADMIN_USERNAME:-${adminUsername}}`);
    lines.push(`      OPENDRIVE_SESSION_TTL_HOURS: \${SESSION_TTL_HOURS:-${sessionTtlHours}}`);
    lines.push(`      OPENDRIVE_MAX_UPLOAD_SIZE: \${MAX_UPLOAD_SIZE:-${maxUploadSize}}`);
    lines.push(`      OPENDRIVE_TUS_EXPIRY_HOURS: \${TUS_EXPIRY_HOURS:-24}`);
    lines.push(`      OPENDRIVE_DEFAULT_QUOTA_BYTES: \${DEFAULT_QUOTA_BYTES:-0}`);
    lines.push(`      OPENDRIVE_ALLOW_REGISTRATION: \${ALLOW_REGISTRATION:-${allowRegistration}}`);
    lines.push(`      RUST_LOG: \${RUST_LOG:-${rustLog}}`);
    lines.push(`      RUST_BACKTRACE: "1"`);
    lines.push(`    volumes:`);
    lines.push(`      # Almacenamiento de archivos (blobs, staging, thumbs)`);
    lines.push(`      - ${dataPath}:/data`);
    lines.push(`    depends_on:`);
    lines.push(`      postgres:`);
    lines.push(`        condition: service_healthy`);
    lines.push(`    networks: [opendrive]`);
    lines.push(``);

    // 2. Postgres Service
    lines.push(`  # Base de datos PostgreSQL`);
    lines.push(`  postgres:`);
    lines.push(`    image: postgres:18.6-bookworm`);
    lines.push(`    restart: unless-stopped`);
    lines.push(`    environment:`);
    lines.push(`      POSTGRES_USER: \${POSTGRES_USER:-${postgresUser}}`);
    lines.push(`      POSTGRES_PASSWORD: \${POSTGRES_PASSWORD}`);
    lines.push(`      POSTGRES_DB: \${POSTGRES_DB:-${postgresDb}}`);
    lines.push(`      PGDATA: /var/lib/postgresql/data/pgdata`);
    lines.push(`    volumes:`);
    if (postgresVolType === "named") {
      lines.push(`      # Volumen gestionado por Docker (evita problemas de permisos UID 999 vs 1000)`);
      lines.push(`      - postgres_data:/var/lib/postgresql/data`);
    } else {
      lines.push(`      # Carpeta dedicada en el host`);
      lines.push(`      - ${postgresBindPath}:/var/lib/postgresql/data`);
    }
    lines.push(`    healthcheck:`);
    lines.push(`      test: ["CMD-SHELL", "pg_isready -U \${POSTGRES_USER:-${postgresUser}} -d \${POSTGRES_DB:-${postgresDb}}"]`);
    lines.push(`      interval: 5s`);
    lines.push(`      timeout: 5s`);
    lines.push(`      retries: 12`);
    lines.push(`      start_period: 10s`);
    lines.push(`    networks: [opendrive]`);
    lines.push(``);

    // 3. Frontend Service (si no es backend-only)
    if (!isBackendOnly) {
      lines.push(`  # Frontend: Interfaz Web React SPA (Nginx)`);
      lines.push(`  frontend:`);
      lines.push(`    image: ${frontendImage}`);
      if (registryType === "local") {
        lines.push(`    build:`);
        lines.push(`      context: ./frontend`);
        lines.push(`      dockerfile: Dockerfile`);
      }
      lines.push(`    restart: unless-stopped`);
      if (!isTls) {
        lines.push(`    ports:`);
        lines.push(`      - "\${WEB_PORT:-${webPort}}:80"`);
      }
      lines.push(`    environment:`);
      lines.push(`      BACKEND_HOST: backend`);
      lines.push(`      BACKEND_PORT: 8080`);
      lines.push(`    depends_on:`);
      lines.push(`      backend:`);
      lines.push(`        condition: service_healthy`);
      lines.push(`    networks: [opendrive]`);
      lines.push(``);
    }

    // 4. Caddy Reverse Proxy con TLS (si es modo TLS)
    if (isTls) {
      lines.push(`  # Caddy: Proxy inverso con certificados SSL/TLS automáticos (Let's Encrypt)`);
      lines.push(`  caddy:`);
      lines.push(`    image: caddy:2.8-alpine`);
      lines.push(`    restart: unless-stopped`);
      lines.push(`    ports:`);
      lines.push(`      - "80:80"`);
      lines.push(`      - "443:443"`);
      lines.push(`      - "443:443/udp"`);
      lines.push(`    environment:`);
      lines.push(`      DOMAIN: \${DOMAIN:-${domain}}`);
      lines.push(`      ACME_EMAIL: \${ACME_EMAIL:-${acmeEmail}}`);
      lines.push(`    volumes:`);
      lines.push(`      - ./deploy/Caddyfile:/etc/caddy/Caddyfile:ro`);
      lines.push(`      - caddy_data:/data`);
      lines.push(`      - caddy_config:/config`);
      lines.push(`    depends_on:`);
      lines.push(`      - backend`);
      if (!isBackendOnly) lines.push(`      - frontend`);
      lines.push(`    networks: [opendrive]`);
      lines.push(``);
    }

    // 5. Cloudflare Tunnel (si es modo tunnel)
    if (isTunnel) {
      lines.push(`  # Cloudflare Tunnel: Conexión segura sin abrir puertos en el Router`);
      lines.push(`  tunnel:`);
      lines.push(`    image: cloudflare/cloudflared:latest`);
      lines.push(`    restart: unless-stopped`);
      lines.push(`    command: tunnel run`);
      lines.push(`    environment:`);
      lines.push(`      TUNNEL_TOKEN: \${CLOUDFLARE_TUNNEL_TOKEN}`);
      lines.push(`    networks: [opendrive]`);
      lines.push(``);
    }

    // Volumes
    const hasNamedVolumes = postgresVolType === "named" || isTls;
    if (hasNamedVolumes) {
      lines.push(`volumes:`);
      if (postgresVolType === "named") {
        lines.push(`  postgres_data:`);
      }
      if (isTls) {
        lines.push(`  caddy_data:`);
        lines.push(`  caddy_config:`);
      }
      lines.push(``);
    }

    // Networks
    lines.push(`networks:`);
    lines.push(`  opendrive:`);
    lines.push(`    driver: bridge`);

    return lines.join("\n");
  }

  // Generación dinámica de .env
  function buildEnvFile(): string {
    const isBackendOnly = mode === "backend-only";
    const isTls = mode === "tls";
    const isTunnel = mode === "tunnel";

    const lines: string[] = [];
    lines.push(`# ---------------------------------------------------------------------------`);
    lines.push(`# OpenDrive — Variables de Entorno`);
    lines.push(`# Modo: ${mode.toUpperCase()}`);
    lines.push(`# ---------------------------------------------------------------------------`);
    lines.push(``);

    if (registryType === "dockerhub") {
      lines.push(`# Docker Hub`);
      lines.push(`DOCKERHUB_USERNAME=${dockerUser}`);
    } else if (registryType === "ghcr") {
      lines.push(`# GitHub Packages`);
      lines.push(`IMAGE_PREFIX=ghcr.io/${dockerUser.toLowerCase()}`);
    }
    lines.push(`IMAGE_TAG=${imageTag}`);
    lines.push(``);

    lines.push(`# Puertos y Acceso`);
    lines.push(`BACKEND_PORT=${backendPort}`);
    if (!isBackendOnly) {
      lines.push(`WEB_PORT=${webPort}`);
    }
    if (isTls) {
      lines.push(`DOMAIN=${domain}`);
      lines.push(`ACME_EMAIL=${acmeEmail}`);
      lines.push(`OPENDRIVE_PUBLIC_URL=https://${domain}`);
    } else {
      lines.push(`OPENDRIVE_PUBLIC_URL=http://localhost:${isBackendOnly ? backendPort : webPort}`);
    }
    if (isTunnel) {
      lines.push(`CLOUDFLARE_TUNNEL_TOKEN=${cloudflareToken || "tu_token_aqui"}`);
    }
    lines.push(``);

    lines.push(`# Credenciales de Base de Datos PostgreSQL`);
    lines.push(`POSTGRES_USER=${postgresUser}`);
    lines.push(`POSTGRES_PASSWORD=${postgresPassword}`);
    lines.push(`POSTGRES_DB=${postgresDb}`);
    lines.push(``);

    lines.push(`# Administrador Inicial de OpenDrive`);
    lines.push(`ADMIN_EMAIL=${adminEmail}`);
    lines.push(`ADMIN_USERNAME=${adminUsername}`);
    lines.push(`ADMIN_PASSWORD=${adminPassword}`);
    lines.push(``);

    lines.push(`# Configuración del Sistema`);
    lines.push(`SESSION_TTL_HOURS=${sessionTtlHours}`);
    lines.push(`MAX_UPLOAD_SIZE=${maxUploadSize}`);
    lines.push(`TUS_EXPIRY_HOURS=24`);
    lines.push(`DEFAULT_QUOTA_BYTES=0`);
    lines.push(`ALLOW_REGISTRATION=${allowRegistration}`);
    lines.push(`RUST_LOG=${rustLog}`);

    return lines.join("\n");
  }

  // Generación de script CLI para Linux
  function buildCliInstructions(): string {
    const isBackendOnly = mode === "backend-only";
    return `# ==============================================================================
# Comandos de despliegue en tu Servidor o Portátil Linux (CLI)
# ==============================================================================

# 1. Crear carpeta de trabajo e ingresar a ella
mkdir -p ~/opendrive && cd ~/opendrive

# 2. Descargar o copiar los archivos 'compose.yaml' y '.env' generados en esta página
# (puedes usar los botones 'Descargar' de arriba y pasarlos por SCP o crearlos con nano)

# 3. Descargar las imágenes desde el registro
docker compose pull

# 4. Iniciar OpenDrive en segundo plano
docker compose up -d

# 5. Comprobar que está en funcionamiento
docker compose ps
docker compose logs -f backend

# ==============================================================================
# Acceso a los servicios:
# - API Health:  http://localhost:${backendPort}/api/health
# - WebDAV:      http://localhost:${backendPort}/dav
${!isBackendOnly ? `# - Web UI:      http://localhost:${webPort}\n` : ""}# ==============================================================================`;
  }

  const composeContent = buildComposeYaml();
  const envContent = buildEnvFile();
  const cliContent = buildCliInstructions();

  const currentTabContent =
    activeTab === "compose"
      ? composeContent
      : activeTab === "env"
        ? envContent
        : cliContent;

  async function handleCopy() {
    await navigator.clipboard.writeText(currentTabContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 2500);
  }

  function handleDownloadCurrent() {
    const filename =
      activeTab === "compose"
        ? "compose.yaml"
        : activeTab === "env"
          ? ".env"
          : "deploy.sh";
    downloadTextFile(filename, currentTabContent);
  }

  function handleDownloadBoth() {
    downloadTextFile("compose.yaml", composeContent);
    setTimeout(() => {
      downloadTextFile(".env", envContent);
    }, 300);
  }

  return (
    <div className="flex-1 overflow-y-auto bg-[var(--color-surface-alt)]">
      {/* Top Header */}
      <header className="sticky top-0 z-10 border-b border-[var(--color-border)] bg-[var(--color-surface)] px-6 py-4 shadow-xs">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <div className="flex items-center gap-3">
            {isPublic ? (
              <Link
                to="/login"
                className="flex items-center gap-1.5 text-xs font-medium text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
              >
                <ArrowLeft size={16} />
                Volver al inicio de sesión
              </Link>
            ) : (
              <Link
                to="/drive"
                className="flex items-center gap-1.5 text-xs font-medium text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
              >
                <ArrowLeft size={16} />
                Volver a Mi Drive
              </Link>
            )}
            <span className="text-[var(--color-border)]">|</span>
            <div className="flex items-center gap-2">
              <span className="inline-flex items-center gap-1 rounded-full bg-blue-50 px-2.5 py-0.5 text-xs font-semibold text-[var(--color-accent)]">
                <Sparkles size={12} />
                Docker Hub & GHCR Ready
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={regeneratePasswords}
              className="btn border border-[var(--color-border)] bg-[var(--color-surface)] text-xs text-[var(--color-ink)] hover:bg-[var(--color-surface-hover)]"
              title="Genera nuevas contraseñas seguras para BD y Administrador"
            >
              <RefreshCw size={14} />
              Regenerar Contraseñas
            </button>
            <button
              type="button"
              onClick={handleDownloadBoth}
              className="btn btn-primary text-xs"
              title="Descarga compose.yaml y .env listos para subir a tu servidor"
            >
              <Download size={14} />
              Descargar compose.yaml + .env
            </button>
          </div>
        </div>
      </header>

      {/* Main Container */}
      <div className="mx-auto max-w-7xl p-6">
        <div className="mb-6">
          <h1 className="text-2xl font-semibold text-[var(--color-ink)]">
            Generador de Docker Compose
          </h1>
          <p className="mt-1 text-sm text-[var(--color-ink-muted)]">
            Personaliza y genera los archivos de despliegue para tu servidor portátil o VPS, igual que en Immich o Nextcloud.
          </p>
        </div>

        {/* Presets Grid */}
        <div className="mb-6 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <PresetCard
            title="Solo Backend"
            badge="Recomendado"
            badgeColor="blue"
            description="Motor ultraligero: API Rust, WebDAV RFC 4918 y Postgres. Ideal para portátil CLI y rclone."
            icon={Terminal}
            active={mode === "backend-only"}
            onClick={() => setMode("backend-only")}
          />
          <PresetCard
            title="Completo"
            badge="Web + API"
            badgeColor="green"
            description="Frontend Web en :3000, Backend en :8080 y PostgreSQL. Todo el ecosistema listo."
            icon={Layers}
            active={mode === "full"}
            onClick={() => setMode("full")}
          />
          <PresetCard
            title="Dominio + HTTPS"
            badge="Let's Encrypt"
            badgeColor="purple"
            description="Proxy Caddy con certificados TLS automáticos para tu dominio propio (puerto 443)."
            icon={ShieldCheck}
            active={mode === "tls"}
            onClick={() => setMode("tls")}
          />
          <PresetCard
            title="Cloudflare Tunnel"
            badge="Sin abrir puertos"
            badgeColor="orange"
            description="Acceso externo cifrado a través de túnel Cloudflare sin tocar la configuración del router."
            icon={Cloud}
            active={mode === "tunnel"}
            onClick={() => setMode("tunnel")}
          />
        </div>

        {/* 2-Column Layout: Options (Left) | Code Preview (Right) */}
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-12">
          {/* Controls Column */}
          <div className="space-y-5 lg:col-span-5">
            {/* 1. Registro e Imágenes */}
            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 shadow-2xs">
              <div className="mb-3 flex items-center gap-2 text-sm font-medium text-[var(--color-ink)]">
                <Server size={16} className="text-[var(--color-accent)]" />
                <span>Origen de Imágenes Docker</span>
              </div>

              <div className="space-y-3 text-xs">
                <div>
                  <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                    Registro de Contenedores
                  </label>
                  <div className="grid grid-cols-3 gap-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-alt)] p-1">
                    <button
                      type="button"
                      onClick={() => setRegistryType("dockerhub")}
                      className={cn(
                        "rounded-md py-1.5 font-medium transition-colors",
                        registryType === "dockerhub"
                          ? "bg-[var(--color-surface)] text-[var(--color-accent)] shadow-2xs"
                          : "text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
                      )}
                    >
                      Docker Hub
                    </button>
                    <button
                      type="button"
                      onClick={() => setRegistryType("ghcr")}
                      className={cn(
                        "rounded-md py-1.5 font-medium transition-colors",
                        registryType === "ghcr"
                          ? "bg-[var(--color-surface)] text-[var(--color-accent)] shadow-2xs"
                          : "text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
                      )}
                    >
                      GitHub (GHCR)
                    </button>
                    <button
                      type="button"
                      onClick={() => setRegistryType("local")}
                      className={cn(
                        "rounded-md py-1.5 font-medium transition-colors",
                        registryType === "local"
                          ? "bg-[var(--color-surface)] text-[var(--color-accent)] shadow-2xs"
                          : "text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
                      )}
                    >
                      Local (Build)
                    </button>
                  </div>
                </div>

                {registryType !== "local" && (
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                        {registryType === "dockerhub" ? "Usuario Docker Hub" : "Usuario GitHub"}
                      </label>
                      <input
                        type="text"
                        value={dockerUser}
                        onChange={(e) => setDockerUser(e.target.value)}
                        placeholder="sergityn"
                        className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                        Etiqueta (Tag)
                      </label>
                      <input
                        type="text"
                        value={imageTag}
                        onChange={(e) => setImageTag(e.target.value)}
                        placeholder="latest"
                        className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* 2. Red y Puertos */}
            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 shadow-2xs">
              <div className="mb-3 flex items-center gap-2 text-sm font-medium text-[var(--color-ink)]">
                <Globe size={16} className="text-[var(--color-accent)]" />
                <span>Puertos y Enrutamiento</span>
              </div>

              <div className="space-y-3 text-xs">
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Puerto Backend (API & WebDAV)
                    </label>
                    <input
                      type="number"
                      value={backendPort}
                      onChange={(e) => setBackendPort(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>

                  {mode !== "backend-only" && mode !== "tls" && (
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                        Puerto Web UI (Frontend)
                      </label>
                      <input
                        type="number"
                        value={webPort}
                        onChange={(e) => setWebPort(e.target.value)}
                        className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                  )}
                </div>

                {mode === "tls" && (
                  <div className="space-y-2 rounded-lg bg-blue-50/60 p-2.5 border border-blue-100">
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink)]">
                        Dominio Público (FQDN)
                      </label>
                      <input
                        type="text"
                        value={domain}
                        onChange={(e) => setDomain(e.target.value)}
                        placeholder="drive.tudominio.com"
                        className="w-full rounded-md border border-blue-200 bg-white px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink)]">
                        Email Let's Encrypt (para certificados)
                      </label>
                      <input
                        type="email"
                        value={acmeEmail}
                        onChange={(e) => setAcmeEmail(e.target.value)}
                        placeholder="admin@tudominio.com"
                        className="w-full rounded-md border border-blue-200 bg-white px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                  </div>
                )}

                {mode === "tunnel" && (
                  <div className="rounded-lg bg-amber-50/70 p-2.5 border border-amber-200">
                    <label className="mb-1 block font-medium text-[var(--color-ink)]">
                      Token del Túnel de Cloudflare
                    </label>
                    <input
                      type="text"
                      value={cloudflareToken}
                      onChange={(e) => setCloudflareToken(e.target.value)}
                      placeholder="eyJh..."
                      className="w-full rounded-md border border-amber-300 bg-white px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                )}
              </div>
            </div>

            {/* 3. Almacenamiento y Volúmenes */}
            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 shadow-2xs">
              <div className="mb-3 flex items-center gap-2 text-sm font-medium text-[var(--color-ink)]">
                <Folder size={16} className="text-[var(--color-accent)]" />
                <span>Almacenamiento y Persistencia</span>
              </div>

              <div className="space-y-3 text-xs">
                <div>
                  <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                    Carpeta de Archivos en el Host (Blobs, staging, thumbs)
                  </label>
                  <input
                    type="text"
                    value={dataPath}
                    onChange={(e) => setDataPath(e.target.value)}
                    placeholder="./data"
                    className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                  />
                </div>

                <div>
                  <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                    Volumen de Base de Datos PostgreSQL
                  </label>
                  <div className="grid grid-cols-2 gap-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-alt)] p-1">
                    <button
                      type="button"
                      onClick={() => setPostgresVolType("named")}
                      className={cn(
                        "rounded-md py-1.5 px-2 font-medium transition-colors text-center",
                        postgresVolType === "named"
                          ? "bg-[var(--color-surface)] text-[var(--color-accent)] shadow-2xs"
                          : "text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
                      )}
                    >
                      Volumen Docker (Seguro)
                    </button>
                    <button
                      type="button"
                      onClick={() => setPostgresVolType("bind")}
                      className={cn(
                        "rounded-md py-1.5 px-2 font-medium transition-colors text-center",
                        postgresVolType === "bind"
                          ? "bg-[var(--color-surface)] text-[var(--color-accent)] shadow-2xs"
                          : "text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
                      )}
                    >
                      Carpeta Host
                    </button>
                  </div>
                  <p className="mt-1 text-[11px] text-[var(--color-ink-muted)]">
                    {postgresVolType === "named"
                      ? "✓ Recomendado: Docker gestiona los permisos del UID 999 sin conflictos con el backend."
                      : "Directorio dedicado en el host (fuera de ./data para no interferir con permisos)."}
                  </p>
                </div>

                {postgresVolType === "bind" && (
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Ruta Host para Postgres
                    </label>
                    <input
                      type="text"
                      value={postgresBindPath}
                      onChange={(e) => setPostgresBindPath(e.target.value)}
                      placeholder="./postgres-data"
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                )}
              </div>
            </div>

            {/* 4. Credenciales y Cuentas */}
            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 shadow-2xs">
              <div className="mb-3 flex items-center justify-between">
                <div className="flex items-center gap-2 text-sm font-medium text-[var(--color-ink)]">
                  <Key size={16} className="text-[var(--color-accent)]" />
                  <span>Credenciales y Seguridad</span>
                </div>
                <button
                  type="button"
                  onClick={regeneratePasswords}
                  className="inline-flex items-center gap-1 text-[11px] font-medium text-[var(--color-accent)] hover:underline"
                >
                  <RefreshCw size={11} />
                  Generar
                </button>
              </div>

              <div className="space-y-3 text-xs">
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Usuario PostgreSQL
                    </label>
                    <input
                      type="text"
                      value={postgresUser}
                      onChange={(e) => setPostgresUser(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Nombre de Base de Datos
                    </label>
                    <input
                      type="text"
                      value={postgresDb}
                      onChange={(e) => setPostgresDb(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                </div>

                <div>
                  <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                    Contraseña de Base de Datos PostgreSQL
                  </label>
                  <input
                    type="text"
                    value={postgresPassword}
                    onChange={(e) => setPostgresPassword(e.target.value)}
                    className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 font-mono text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                  />
                </div>

                <div className="grid grid-cols-3 gap-2">
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Usuario Admin
                    </label>
                    <input
                      type="text"
                      value={adminUsername}
                      onChange={(e) => setAdminUsername(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Email Admin
                    </label>
                    <input
                      type="email"
                      value={adminEmail}
                      onChange={(e) => setAdminEmail(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Contraseña Admin
                    </label>
                    <input
                      type="text"
                      value={adminPassword}
                      onChange={(e) => setAdminPassword(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 font-mono text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* 5. Opciones Avanzadas */}
            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 shadow-2xs">
              <button
                type="button"
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="flex w-full items-center justify-between text-sm font-medium text-[var(--color-ink)]"
              >
                <div className="flex items-center gap-2">
                  <Sliders size={16} className="text-[var(--color-ink-muted)]" />
                  <span>Ajustes Avanzados del Servidor</span>
                </div>
                {showAdvanced ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
              </button>

              {showAdvanced && (
                <div className="mt-3 space-y-3 border-t border-[var(--color-border)] pt-3 text-xs">
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={allowRegistration}
                      onChange={(e) => setAllowRegistration(e.target.checked)}
                      className="rounded text-[var(--color-accent)] focus:ring-[var(--color-accent)]"
                    />
                    <span className="font-medium text-[var(--color-ink)]">
                      Permitir registro público de usuarios
                    </span>
                  </label>

                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                        Límite por Archivo (0 = ilimitado)
                      </label>
                      <input
                        type="text"
                        value={maxUploadSize}
                        onChange={(e) => setMaxUploadSize(e.target.value)}
                        placeholder="0"
                        className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                        Duración de Sesión (horas)
                      </label>
                      <input
                        type="number"
                        value={sessionTtlHours}
                        onChange={(e) => setSessionTtlHours(e.target.value)}
                        placeholder="24"
                        className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="mb-1 block font-medium text-[var(--color-ink-muted)]">
                      Nivel de Log
                    </label>
                    <select
                      value={rustLog}
                      onChange={(e) => setRustLog(e.target.value)}
                      className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-xs text-[var(--color-ink)] outline-none focus:border-[var(--color-accent)]"
                    >
                      <option value="info">info (recomendado)</option>
                      <option value="debug">debug (desarrollo)</option>
                      <option value="warn">warn (solo advertencias)</option>
                      <option value="error">error (mínimo)</option>
                    </select>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Code Preview & Actions Column */}
          <div className="flex flex-col lg:col-span-7">
            <div className="flex flex-1 flex-col overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] shadow-sm">
              {/* Tabs Bar */}
              <div className="flex items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface-alt)] px-4 py-2">
                <div className="flex items-center gap-1">
                  <TabButton
                    label="compose.yaml"
                    icon={Server}
                    active={activeTab === "compose"}
                    onClick={() => setActiveTab("compose")}
                  />
                  <TabButton
                    label=".env"
                    icon={Key}
                    active={activeTab === "env"}
                    onClick={() => setActiveTab("env")}
                  />
                  <TabButton
                    label="Comandos CLI (Linux)"
                    icon={Terminal}
                    active={activeTab === "cli"}
                    onClick={() => setActiveTab("cli")}
                  />
                </div>

                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    onClick={handleCopy}
                    className="btn border border-[var(--color-border)] bg-[var(--color-surface)] text-xs text-[var(--color-ink)] hover:bg-[var(--color-surface-hover)]"
                  >
                    {copied ? (
                      <>
                        <Check size={14} className="text-green-600" />
                        <span className="text-green-600 font-medium">¡Copiado!</span>
                      </>
                    ) : (
                      <>
                        <Copy size={14} />
                        Copiar
                      </>
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={handleDownloadCurrent}
                    className="btn border border-[var(--color-border)] bg-[var(--color-surface)] text-xs text-[var(--color-ink)] hover:bg-[var(--color-surface-hover)]"
                  >
                    <Download size={14} />
                    Descargar
                  </button>
                </div>
              </div>

              {/* Code Display Area */}
              <div className="relative flex-1 overflow-auto bg-[#1e1e1e] p-4 font-mono text-xs text-gray-200">
                <pre className="whitespace-pre">
                  <code>{currentTabContent}</code>
                </pre>
              </div>

              {/* Quick Guide Footer */}
              <div className="border-t border-[var(--color-border)] bg-[var(--color-surface-alt)] p-4 text-xs">
                <div className="flex items-start gap-2 text-[var(--color-ink-muted)]">
                  <CheckCircle2 size={16} className="mt-0.5 shrink-0 text-green-600" />
                  <div>
                    <span className="font-semibold text-[var(--color-ink)]">
                      Instalación lista en 3 pasos:
                    </span>
                    <ol className="mt-1 list-decimal space-y-0.5 pl-4">
                      <li>
                        Crea una carpeta en tu servidor o portátil con:{" "}
                        <code className="rounded bg-gray-200 px-1 py-0.5 font-mono text-gray-800">
                          mkdir -p ~/opendrive && cd ~/opendrive
                        </code>
                      </li>
                      <li>Descarga o pega ambos archivos (<strong>compose.yaml</strong> y <strong>.env</strong>) dentro de esa carpeta.</li>
                      <li>
                        Arranca con{" "}
                        <code className="rounded bg-gray-200 px-1 py-0.5 font-mono text-gray-800">
                          docker compose pull && docker compose up -d
                        </code>
                      </li>
                    </ol>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

interface PresetCardProps {
  title: string;
  badge: string;
  badgeColor: "blue" | "green" | "purple" | "orange";
  description: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  active: boolean;
  onClick: () => void;
}

function PresetCard({
  title,
  badge,
  badgeColor,
  description,
  icon: Icon,
  active,
  onClick,
}: PresetCardProps) {
  const badgeClasses = {
    blue: "bg-blue-100 text-blue-800",
    green: "bg-green-100 text-green-800",
    purple: "bg-purple-100 text-purple-800",
    orange: "bg-amber-100 text-amber-800",
  }[badgeColor];

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex flex-col rounded-xl border p-3.5 text-left transition-all",
        active
          ? "border-[var(--color-accent)] bg-blue-50/50 shadow-xs ring-1 ring-[var(--color-accent)]"
          : "border-[var(--color-border)] bg-[var(--color-surface)] hover:border-gray-400 hover:bg-gray-50/50"
      )}
    >
      <div className="flex items-center justify-between">
        <div
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg",
            active ? "bg-[var(--color-accent)] text-white" : "bg-gray-100 text-gray-600"
          )}
        >
          <Icon size={18} />
        </div>
        <span className={cn("rounded-full px-2 py-0.5 text-[10px] font-semibold", badgeClasses)}>
          {badge}
        </span>
      </div>
      <p className="mt-2.5 text-sm font-semibold text-[var(--color-ink)]">{title}</p>
      <p className="mt-1 text-xs leading-relaxed text-[var(--color-ink-muted)]">
        {description}
      </p>
    </button>
  );
}

interface TabButtonProps {
  label: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  active: boolean;
  onClick: () => void;
}

function TabButton({ label, icon: Icon, active, onClick }: TabButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
        active
          ? "bg-[var(--color-surface)] text-[var(--color-accent)] shadow-2xs"
          : "text-[var(--color-ink-muted)] hover:text-[var(--color-ink)]"
      )}
    >
      <Icon size={14} />
      {label}
    </button>
  );
}
