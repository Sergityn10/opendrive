# Guía Paso a Paso: Subir y Desplegar el Backend en tu Servidor

Esta guía contiene las instrucciones exactas y resumidas para compilar el backend de OpenDrive en tu ordenador, subirlo a un registro en la nube (**Docker Hub** o **GitHub Container Registry - GHCR**) y descargarlo en tu servidor (portátil Linux o VPS) de forma rápida y sin complicaciones.

---

## Resumen del Proceso

```
[ Tu PC de Desarrollo ] ──(1. Subir imagen)──> [ Registro: Docker Hub / GHCR ]
                                                            │
                                                     (2. docker compose pull)
                                                            ▼
                                                [ Tu Portátil o VPS ]
                                                (Descarga y arranca en 10s)
```

---

## FASE 1: Subir el Backend a la Nube (Desde tu PC de Trabajo)

Elige si prefieres usar **Docker Hub** o **GitHub Packages (GHCR)**:

### Opción A: Usando Docker Hub
1. Abre la terminal en la carpeta del proyecto en tu PC e inicia sesión:
   ```bash
   docker login
   ```
2. Ejecuta el script para compilar y subir **únicamente el Backend**:
   - **En Windows (PowerShell):**
     ```powershell
     .\deploy\push-dockerhub.ps1 -Username "tu_usuario_dockerhub" -BackendOnly
     ```
   - **En Linux / macOS (Bash):**
     ```bash
     ./deploy/push-dockerhub.sh tu_usuario_dockerhub latest --backend-only
     ```

---

### Opción B: Usando GitHub Container Registry (ghcr.io)
1. Inicia sesión en `ghcr.io` con tu usuario de GitHub y un Personal Access Token (PAT) con permiso `write:packages`:
   ```bash
   echo TU_GITHUB_TOKEN | docker login ghcr.io -u tu_usuario_github --password-stdin
   ```
2. Ejecuta el script para compilar y subir:
   - **En Windows (PowerShell):**
     ```powershell
     .\deploy\push-ghcr.ps1 -Username "tu_usuario_github" -BackendOnly
     ```
   - **En Linux / macOS (Bash):**
     ```bash
     ./deploy/push-ghcr.sh tu_usuario_github latest --backend-only
     ```

> **Alternativa 100% en la nube (sin compilar en tu PC):**
> Si tu código está en GitHub, ve a la pestaña **Actions** en tu repositorio $\rightarrow$ **Publicar en Docker Hub** o **Publicar en ghcr.io** $\rightarrow$ pulsa **Run workflow** seleccionando `backend`. GitHub compilará y subirá la imagen por ti de forma gratuita.

---

## FASE 2: Descargar y Arrancar en tu Servidor / Portátil Linux

En tu portátil Linux o VPS (conectado por SSH o directamente en su terminal CLI):

### Paso 1: Crear la carpeta de trabajo
```bash
mkdir -p ~/opendrive && cd ~/opendrive
```

### Paso 2: Descargar los archivos mínimos de configuración
Ejecuta estos dos comandos para descargar el docker-compose y la plantilla de variables:

```bash
# Descarga el docker-compose exclusivo para el backend
curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/compose.backend.yaml -o compose.yaml

# Descarga la plantilla de variables
curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/.env.backend.example -o .env
```
*(Si no tienes el repositorio público en GitHub, puedes copiar ambos archivos desde tu PC usando SCP: `scp compose.backend.yaml .env.backend.example usuario@ip_servidor:~/opendrive/` y renombrarlos a `compose.yaml` y `.env`).*

### Paso 3: Configurar el archivo `.env`
Abre el archivo con un editor de texto:
```bash
nano .env
```

Configura únicamente estas líneas clave:
```dotenv
# Si usas Docker Hub:
DOCKERHUB_USERNAME=tu_usuario_dockerhub

# O si usas GitHub GHCR:
# IMAGE_PREFIX=ghcr.io/tu_usuario_github

# Contraseñas seguras:
POSTGRES_PASSWORD=una_contrasena_fuerte_db
ADMIN_PASSWORD=una_contrasena_fuerte_admin
```
Guarda y sal con `Ctrl+O`, `Enter`, y luego `Ctrl+X`.

### Paso 4: Descargar la imagen precompilada y arrancar
Ejecuta:
```bash
docker compose pull
docker compose up -d
```

En unos **10 a 15 segundos**, Docker descargará la imagen ya compilada desde la nube y levantará tanto la base de datos PostgreSQL como el Backend de OpenDrive.

---

## FASE 3: Comprobación y Uso

1. **Averigua la IP de tu servidor:**
   ```bash
   hostname -I
   # Ejemplo: 192.168.0.20
   ```

2. **Comprueba que responde:**
   - Desde la consola del servidor:
     ```bash
     curl http://localhost:8080/api/health
     # Debe responder: {"status":"ok",...}
     ```
   - O abriendo en el navegador de cualquier equipo de tu casa:
     ```
     http://192.168.0.20:8080/api/health
     ```

3. **Conectar vía WebDAV (Guardar archivos en el servidor):**
   - **URL WebDAV:** `http://192.168.0.20:8080/dav`
   - **Usuario:** `admin@opendrive.local` (o el que pusiste en `.env`)
   - **Contraseña:** la que definiste en `ADMIN_PASSWORD`
   - Puedes usar **rclone**, el explorador de archivos de Windows (*Conectar a unidad de red*), Finder en macOS o apps móviles como Cx File Explorer.

---

## FASE 4: Actualizar en el Futuro (Cuando hagas cambios)

1. En tu PC de desarrollo: haces cambios en el código y vuelves a ejecutar el script de subida (`.\deploy\push-dockerhub.ps1 -BackendOnly` o `push-ghcr.ps1`).
2. En tu servidor Linux:
   ```bash
   cd ~/opendrive
   docker compose pull
   docker compose up -d
   ```
El servidor descargará únicamente los cambios y se reiniciará en 5 segundos sin perder ningún archivo ni base de datos.

---

## Preguntas Frecuentes y Solución de Problemas

### Error: `Permission denied (os error 13)` al iniciar backend
Si en los logs de Docker (`docker compose logs backend`) ves:
```text
Error: error interno: error de E/S: Permission denied (os error 13)
```
**Causa:** En Linux, cuando Docker crea la carpeta `./data` por primera vez para montar el volumen, le asigna propietario `root`. Sin embargo, el contenedor de OpenDrive se ejecuta con el usuario no privilegiado `opendrive` (UID 1000).

**Solución rápida en el servidor:**
```bash
sudo chown -R 1000:1000 ./data
docker compose up -d
```
*(Las versiones nuevas de la imagen Docker ya incluyen auto-reparación de permisos al iniciar).*
