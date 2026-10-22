# Guía de Despliegue con Docker Hub y `docker compose pull`

Esta guía describe el flujo de trabajo recomendado para compilar OpenDrive en tu equipo de desarrollo, subir las imágenes a **Docker Hub**, y desplegar en tu portátil Linux **sin necesidad de compilar código fuente en el portátil**.

---

## 1. ¿Por qué este método es el más recomendado?

```
[ Tu PC de Desarrollo / GitHub Actions ]
      │  (Compila Rust y Node una sola vez)
      ▼
[ Docker Hub (En la nube) ]
  ├── tu_usuario/opendrive-backend:latest
  └── tu_usuario/opendrive-frontend:latest
      │
      ▼  docker compose pull (descarga directa en 15 segundos)
[ Portátil Linux Headless ]
  Solo necesita: compose.yaml y .env
  (Cero uso intensivo de CPU, sin compilar nada)
```

1. **El portátil no sufre compilando Rust**: Compilar axum, sqlx y el frontend en un portátil puede tardar 15 minutos y saturar la CPU y la RAM. Con Docker Hub, el portátil solo descarga las imágenes ya compiladas en segundos.
2. **No necesitas clonar el repositorio completo en el portátil**: Solo necesitas dos ficheros (`compose.yaml` y `.env`).
3. **Actualizaciones instantáneas**: Cuando hagas cambios y subas una nueva versión a Docker Hub, en el portátil solo ejecutas:
   ```bash
   docker compose pull && docker compose up -d
   ```

---

## 2. Paso 1: Subir las imágenes a Docker Hub (Desde tu PC de desarrollo)

### Método A: Mediante los scripts incluidos (Recomendado)

En tu equipo de desarrollo, inicia sesión en Docker Hub una vez:
```bash
docker login
```

Y ejecuta el script de subida indicando tu usuario de Docker Hub:

- **En Windows (PowerShell):**
  ```powershell
  # Subir Backend + Frontend:
  .\deploy\push-dockerhub.ps1 -Username "tu_usuario"

  # Subir ÚNICAMENTE el Backend:
  .\deploy\push-dockerhub.ps1 -Username "tu_usuario" -BackendOnly
  ```

- **En Linux / macOS (Bash):**
  ```bash
  # Subir Backend + Frontend:
  ./deploy/push-dockerhub.sh tu_usuario

  # Subir ÚNICAMENTE el Backend:
  ./deploy/push-dockerhub.sh tu_usuario latest --backend-only
  ```

El script compilará lo que le indiques, lo etiquetará y lo subirá automáticamente a Docker Hub:
- `tu_usuario/opendrive-backend:latest`
- `tu_usuario/opendrive-frontend:latest` (si no usas `--backend-only`)

### Método B: Automático con GitHub Actions
Si tu proyecto está en GitHub:
1. En GitHub, ve a **Settings** -> **Secrets and variables** -> **Actions**.
2. Añade dos secretos:
   - `DOCKERHUB_USERNAME`: tu usuario de Docker Hub.
   - `DOCKERHUB_TOKEN`: tu Access Token generado en Docker Hub (*Account Settings -> Security -> New Access Token*).
3. Cada vez que crees una etiqueta (tag) como `v1.0.0` o vayas a la pestaña **Actions** -> **Publicar en Docker Hub** -> **Run workflow**, GitHub compilará y subirá las imágenes por ti.

---

## 3. Paso 2: Desplegar en el Portátil Linux

En tu portátil Linux (CLI sin entorno gráfico):

### Opción 1: Sin clonar el código fuente (La más limpia en VPS / Portátil)
Solo necesitas crear una carpeta y descargar los 2 ficheros necesarios:

#### A. Para desplegar ÚNICAMENTE el Backend (API REST + WebDAV + Postgres):
```bash
# 1. Crear carpeta y entrar
mkdir -p ~/opendrive-backend && cd ~/opendrive-backend

# 2. Descargar el compose exclusivo de backend y su plantilla de variables
curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/compose.backend.yaml -o compose.yaml
curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/.env.backend.example -o .env

# 3. Configurar usuario de Docker Hub y contraseñas
nano .env

# 4. Descargar la imagen precompilada y arrancar
docker compose pull
docker compose up -d
```
El backend estará escuchando en `http://<IP_DEL_SERVIDOR>:8080` (WebDAV en `/dav`).

#### B. Para desplegar COMPLETO (Frontend Web UI + Backend + Postgres):
```bash
# 1. Crear carpeta y entrar
mkdir -p ~/opendrive && cd ~/opendrive

# 2. Descargar compose completo y plantilla de variables
curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/compose.yaml -o compose.yaml
curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/.env.example -o .env

# 3. Configurar variables
nano .env

# 4. Descargar imágenes y arrancar
docker compose pull
docker compose up -d
```
La interfaz web estará en `http://<IP_DEL_SERVIDOR>:3000`.

### Opción 2: Si clonaste el repositorio en el portátil
Si tienes el repositorio clonado en el portátil, ejecuta el asistente con la opción deseada:
```bash
chmod +x ./deploy/deploy-laptop.sh

# Para arrancar todo:
./deploy/deploy-laptop.sh

# Para arrancar ÚNICAMENTE el backend (sin frontend):
./deploy/deploy-laptop.sh --backend-only
```

---

## 4. Acceder al Servidor

Una vez arrancado con `docker compose up -d`, abre cualquier navegador en tu red local:
```
http://<IP_DE_TU_PORTATIL>:3000
```
*(Para saber la IP del portátil, ejecuta `hostname -I` en su consola).*

---

## 5. Cómo Actualizar en el Futuro

Cuando hagas cambios en el código:
1. En tu PC: ejecuta `./deploy/push-dockerhub.sh tu_usuario` para subir la nueva versión a Docker Hub.
2. En tu portátil Linux: ejecuta simplemente:
   ```bash
   docker compose pull && docker compose up -d
   ```
¡Docker descargará únicamente las capas modificadas y reiniciará los contenedores en 5 segundos sin perder datos!
