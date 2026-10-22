# Guía para Publicar en Repositorio Remoto (GitHub / GitLab)

Esta guía explica paso a paso cómo subir este proyecto a un repositorio remoto (como GitHub o GitLab) de forma 100% segura, garantizando que **no se filtre ningún secreto**, contraseña, archivo de base de datos ni datos personales.

---

## 1. Protección de Secretos y `.gitignore`

El proyecto ya cuenta con un archivo [`.gitignore`](../.gitignore) estricto y configurado según las mejores prácticas de seguridad:

| Elemento | ¿Se sube al repositorio? | Protección |
|---|:---:|---|
| Archivo `.env` (con tus contraseñas reales) | ❌ **NUNCA** | Ignorado por `.gitignore` (`.env`, `.env.*`) |
| Plantillas de ejemplo (`.env.example`, `.env.backend.example`) | ✅ **SÍ** | Solo contienen variables de plantilla sin contraseñas reales |
| Datos de usuarios (`data/blobs`, `data/postgres`, `data/thumbs`) | ❌ **NUNCA** | Ignorados por `data/**` (solo se sube `data/.gitkeep` para crear la carpeta vacía) |
| Certificados SSL / TLS (`deploy/certs/*.crt`, `*.pem`, `*.key`) | ❌ **NUNCA** | Ignorados por comodín `*.crt`, `*.key`, `deploy/certs/*` |
| Credenciales DDNS (`deploy/ddns/config.json`) | ❌ **NUNCA** | Ignorado; solo se sube `config.json.example` |
| Artefactos de compilación (`target/`, `node_modules/`, `dist/`) | ❌ **NUNCA** | Ignorados para no sobrecargar el repositorio |
| Archivos de sistema (`.DS_Store`, `Thumbs.db`, `.vscode/`) | ❌ **NUNCA** | Ignorados automáticamente |

---

## 2. Pasos para Subir el Proyecto a GitHub

### Paso 1: Crear el repositorio en GitHub
1. Entra en tu cuenta de [GitHub](https://github.com).
2. Haz clic en el botón verde **New** (Nuevo repositorio).
3. Dale un nombre (por ejemplo: `Google-Drive-OpenSource` o `opendrive`).
4. **IMPORTANTE:** Déjalo **sin marcar** las casillas de *"Add a README file"*, *"Add .gitignore"* o *"Choose a license"* (ya los tenemos en local).
5. Haz clic en **Create repository**.
6. Copia la URL de tu repositorio (por ejemplo: `https://github.com/<tu-usuario>/Google-Drive-OpenSource.git`).

---

### Paso 2: Verificar el estado en tu PC local

Abre una terminal en la raíz del proyecto (`c:\Users\USUARIO\Desktop\Sergio\Proyectos-programacion\Google-Drive-OpenSource`) y ejecuta:

```powershell
# Comprobar que ningún archivo secreto o con contraseñas está pendiente de seguimiento
git status
```

Verás que `.env`, `data/postgres`, etc., **no** aparecen en la lista.

---

### Paso 3: Añadir los archivos y realizar el primer commit

```bash
# Añade todos los archivos permitidos al stage
git add .

# Crea el commit inicial
git commit -m "feat: inicializar proyecto OpenDrive con backend Rust, frontend React y despliegue Docker"
```

---

### Paso 4: Vincular con tu repositorio remoto y subir

```bash
# Asegurar que la rama principal se llame 'main'
git branch -M main

# Añadir el repositorio remoto (sustituye con la URL de tu GitHub)
git remote add origin https://github.com/<tu-usuario>/Google-Drive-OpenSource.git

# Subir los archivos a GitHub
git push -u origin main
```

*(Si ya tenías un origen previo configurado, puedes cambiarlo con `git remote set-url origin https://github.com/<tu-usuario>/<tu-repo>.git`).*

---

## 3. (Opcional) Configurar GitHub Actions para Compilación Automática

El proyecto incluye dos flujos de trabajo automáticos en `.github/workflows/`:
1. **`ghcr-publish.yml`**: Compila y sube imágenes a **GitHub Container Registry** (`ghcr.io`). **Funciona sin configuración extra**, ya que utiliza el token automático de GitHub (`GITHUB_TOKEN`).
2. **`docker-hub-publish.yml`**: Compila y sube imágenes a **Docker Hub**. Para activarlo:

1. Ve a tu repositorio en GitHub: **Settings** -> **Secrets and variables** -> **Actions**.
2. Haz clic en **New repository secret** y añade:
   - **`DOCKERHUB_USERNAME`**: Tu usuario de Docker Hub.
   - **`DOCKERHUB_TOKEN`**: Tu token de acceso personal de Docker Hub (obtenido en *Docker Hub -> Account Settings -> Personal Access Tokens*).

---

## 4. Flujo de Trabajo para Futuras Actualizaciones

Cada vez que hagas mejoras o corrijas errores:

```bash
# 1. Comprobar los cambios
git status

# 2. Añadir y hacer commit
git add .
git commit -m "refactor: descripcion de las mejoras"

# 3. Subir a GitHub
git push origin main
```
