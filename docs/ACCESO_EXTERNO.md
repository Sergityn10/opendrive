# Guía de Acceso (Red Local, IP Directa y Dominio Externo)

Esta guía explica cómo desplegar y acceder a tu servidor **OpenDrive** desde un portátil con **Linux (solo CLI, sin entorno gráfico)**, permitiendo tanto el acceso directo por IP en tu red local (sin necesidad de DNS) como el acceso con dominio propio y TLS.

---

## 1. Modos de Acceso: ¿DNS es obligatorio?

**No, el DNS NO es obligatorio.** Puedes elegir el modo que mejor te convenga:

| Modo | Cuándo usarlo | Cómo arrancar | Acceso |
|---|---|---|---|
| **Modo Directo por IP (Recomendado para empezar)** | Sin comprar ni configurar dominios. Acceso en casa o por IP pública. | `docker compose up -d` | `http://<IP_DE_TU_PORTATIL>:3000` |
| **Modo Dominio Propio con HTTPS** | Cuando quieras tu propio dominio (`drive.tudominio.com`) con certificado SSL. | `docker compose --profile tls up -d` | `https://drive.tudominio.com` |
| **Modo Contenedores Individuales** | Si prefieres levantar el frontend y el backend en comandos `docker run` separados. | `docker run ...` (ver sección 4) | `http://<IP_DE_TU_PORTATIL>:3000` |

---

## 2. Preparación del Portátil en Linux CLI

En un portátil con Linux sin entorno gráfico (Ubuntu Server, Debian, Fedora, Arch, etc.):

### A. Evitar que Linux se suspenda al cerrar la tapa
Por defecto, `systemd` suspende el equipo cuando cierras la pantalla del portátil. Para que siga funcionando como servidor con la tapa cerrada:

1. Edita el fichero de configuración de logind:
   ```bash
   sudo nano /etc/systemd/logind.conf
   ```
2. Descomenta o añade las siguientes líneas:
   ```ini
   HandleLidSwitch=ignore
   HandleLidSwitchExternalPower=ignore
   HandleLidSwitchDocked=ignore
   ```
3. Reinicia el servicio para aplicar los cambios sin reiniciar el equipo:
   ```bash
   sudo systemctl restart systemd-logind
   ```

### B. Averiguar la IP local de tu portátil
En la consola de tu portátil Linux, ejecuta:
```bash
ip -4 addr show
# O simplemente:
hostname -I
```
Verás una IP del tipo `192.168.0.XX` (por ejemplo `192.168.0.20`). Esa es la IP con la que accederás a la web desde tu móvil u otro ordenador.

---

## 3. Despliegue con Docker Compose (Recomendado)

### Paso 1: Configurar el archivo `.env`
Copia la plantilla y ajústala:
```bash
cp .env.example .env
nano .env
```
Para acceso directo por IP, los valores por defecto ya funcionan:
```dotenv
WEB_PORT=3000
BACKEND_PORT=8080
OPENDRIVE_PUBLIC_URL=http://localhost:3000
POSTGRES_PASSWORD=mi-clave-segura
ADMIN_PASSWORD=mi-clave-admin
```

### Paso 2: Arrancar el servicio
```bash
docker compose up -d --build
```
Esto compilará y levantará:
- `opendrive-frontend` (Nginx + Web UI en el puerto 3000)
- `opendrive-backend` (API Rust + Tus en el puerto 8080)
- `postgres` (Base de datos PostgreSQL 18)

### Paso 3: Abrir la Web
Desde cualquier otro ordenador, tableta o teléfono móvil conectado a la misma red Wi-Fi de tu casa, abre el navegador e introduce:
```
http://192.168.0.20:3000
```
*(Sustituye `192.168.0.20` por la IP que te dio `hostname -I`).*

---

## 4. Despliegue con Contenedores Docker Individuales (Frontend y Backend Separados)

Si deseas gestionar o levantar los contenedores por separado con la CLI de Docker:

### 1. Crear una red compartida para los contenedores
```bash
docker network create opendrive-net
```

### 2. Arrancar la base de datos PostgreSQL
```bash
docker run -d \
  --name opendrive-db \
  --restart unless-stopped \
  --network opendrive-net \
  -e POSTGRES_USER=opendrive \
  -e POSTGRES_PASSWORD=mi-clave-secreta \
  -e POSTGRES_DB=opendrive \
  -v $(pwd)/data/postgres:/var/lib/postgresql/data \
  postgres:18.6-bookworm
```

### 3. Compilar y arrancar el Backend
```bash
# Compilar la imagen del backend
docker build -t opendrive-backend ./backend

# Arrancar el contenedor del backend
docker run -d \
  --name opendrive-backend \
  --restart unless-stopped \
  --network opendrive-net \
  -p 8080:8080 \
  -e DATABASE_URL=postgres://opendrive:mi-clave-secreta@opendrive-db:5432/opendrive \
  -e OPENDRIVE_BIND=0.0.0.0:8080 \
  -e OPENDRIVE_DATA_DIR=/data \
  -e OPENDRIVE_ADMIN_PASSWORD=admin12345 \
  -e OPENDRIVE_PUBLIC_URL=http://192.168.0.20:3000 \
  -v $(pwd)/data:/data \
  opendrive-backend
```

### 4. Compilar y arrancar el Frontend (Web UI)
```bash
# Compilar la imagen del frontend
docker build -t opendrive-frontend ./frontend

# Arrancar el contenedor del frontend (Nginx proxea a opendrive-backend)
docker run -d \
  --name opendrive-frontend \
  --restart unless-stopped \
  --network opendrive-net \
  -p 3000:80 \
  -e BACKEND_HOST=opendrive-backend \
  -e BACKEND_PORT=8080 \
  opendrive-frontend
```

---

## 5. Acceso con Dominio Propio y TLS (Opcional, cuando decidas activarlo)

Si más adelante quieres acceder mediante `https://drive.tudominio.com` con certificados SSL automáticos de Let's Encrypt:

### A. Reenvío de Puertos en el Router (Port Forwarding)
Accede a la administración de tu router (`http://192.168.0.1`) y crea dos reglas hacia la IP de tu portátil:
- Puerto `80` TCP $\rightarrow$ IP del portátil : `80`
- Puerto `443` TCP y UDP $\rightarrow$ IP del portátil : `443`

### B. Registro DNS en tu proveedor de dominio
Crea un registro `A` apuntando a tu IP WAN pública:
- Tipo: `A`
- Host: `drive` (o `@`)
- IP: Tu IP pública actual
- TTL: 300 segundos

### C. Arrancar Caddy con el perfil TLS
En `.env`, define tu dominio y correo:
```dotenv
DOMAIN=drive.tudominio.com
ACME_EMAIL=tu-correo@gmail.com
OPENDRIVE_PUBLIC_URL=https://drive.tudominio.com
```

Y arranca Caddy:
```bash
docker compose --profile tls up -d
```
Caddy se encargará de gestionar el certificado SSL automáticamente.

---

## 6. Mantener la IP Actualizada si es Dinámica (DDNS en Docker)

Como el servicio DDNS de tu router no está activo y las conexiones residenciales pueden cambiar de IP pública de vez en cuando, el proyecto incluye un contenedor automático (`ddns-updater`):

1. Ve a la carpeta `deploy/ddns/`:
   ```bash
   cp deploy/ddns/config.json.example deploy/ddns/config.json
   ```
2. Edita `deploy/ddns/config.json` con los datos de tu proveedor DNS (Cloudflare, DuckDNS, Namecheap, Porkbun, etc.).
3. Para arrancarlo junto con OpenDrive:
   ```bash
   docker compose --profile ddns up -d
   ```
El contenedor comprobará cada 5 minutos si tu IP pública cambia y, si lo hace, actualizará el registro DNS de tu dominio automáticamente.

---

## 7. Comprobación de Conexión

1. **Desde fuera de tu red (Prueba clave):**
   - Desconecta tu teléfono móvil de la red Wi-Fi de tu casa (activa los **datos móviles 4G/5G**).
   - Abre el navegador del móvil e introduce: `https://drive.tudominio.com`.
   - Inicia sesión con tus credenciales de administrador y prueba a subir un archivo.
   - Deberías ver el candado verde/seguro de HTTPS válido.

2. **WebDAV desde el exterior:**
   - URL: `https://drive.tudominio.com/dav`
   - Usuario: tu email de OpenDrive
   - Contraseña: tu contraseña de OpenDrive
   - Funciona con rclone, Windows Explorer, Finder o clientes móviles como Cx File Explorer / Documents by Readdle.

---

## 8. Diagnóstico de Problemas Frecuentes

### A. Funciona desde el móvil con 4G, pero da error desde un ordenador conectado al Wi-Fi de casa
- **Causa:** Muchos routers de operadoras no soportan *NAT Hairpinning* (o *NAT Loopback*), lo que significa que no permiten que un dispositivo interno acceda a la IP WAN pública del propio router.
- **Solución:**
  En los ordenadores que estén dentro de tu casa, añade una línea en tu archivo de hosts (`C:\Windows\System32\drivers\etc\hosts` en Windows o `/etc/hosts` en Linux/Mac):
  ```
  192.168.0.20 drive.tudominio.com
  ```
  O utiliza un servidor DNS local como Pi-hole o AdGuard Home.

### B. Error al emitir el certificado TLS ("challenge failed" o "timeout")
- Comprueba que el registro `A` de tu dominio apunta a la IP WAN correcta (`.\deploy\setup-windows-host.ps1` te indica tu IP actual).
- Comprueba que el puerto 80 y 443 están reenviados en el router a `192.168.0.20`.
- Asegúrate de que no haya bloqueos en el firewall de Windows.

---

## 9. Alternativa: Cloudflare Tunnel (Sin abrir puertos en el router)

Si prefieres **no abrir puertos** en tu router o tu proveedor de internet bloquea los puertos 80/443:

1. Crea una cuenta gratuita en [Cloudflare](https://dash.cloudflare.com/) y traslada los servidores DNS de tu dominio a Cloudflare.
2. En el panel de Cloudflare, ve a **Zero Trust** -> **Networks** -> **Tunnels** -> **Create a Tunnel**.
3. Elige el nombre del túnel y copia el `token` que te proporciona.
4. En tu `.env`:
   ```dotenv
   CLOUDFLARE_TUNNEL_TOKEN=tu_token_aqui
   ```
5. En Cloudflare Tunnel, en la pestaña *Public Hostnames*, añade:
   - Hostname: `drive.tudominio.com`
   - Service: `HTTPS` -> `caddy:443` (en *Additional application settings* -> *TLS*, marca *No TLS Verify*).
6. Levanta OpenDrive con el perfil de túnel:
   ```bash
   docker compose --profile tunnel up -d
   ```
¡Con Cloudflare Tunnel no necesitas abrir puertos, no necesitas DDNS y tu IP de casa permanece completamente oculta!
