import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// Configuración de Vite para el frontend de OpenDrive.
//
// La SPA se sirve bajo el mismo origen que la API, así que las rutas
// relativas `/api/...` funcionan tanto en desarrollo como en producción.
//
// En desarrollo, el proxy de Vite redirige `/api` y `/dav` al backend
// (por defecto http://localhost:8080 o la IP de tu servidor configurada en VITE_BACKEND_URL).
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const backendTarget = env.VITE_BACKEND_URL || "http://localhost:8080";

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        "@": path.resolve(import.meta.dirname, "./src"),
      },
    },
    server: {
      port: 5173,
      proxy: {
        "/api": {
          target: backendTarget,
          changeOrigin: true,
        },
        "/dav": {
          target: backendTarget,
          changeOrigin: true,
        },
      },
    },
    build: {
      outDir: "dist",
      sourcemap: false,
      chunkSizeWarningLimit: 900,
      rollupOptions: {
        output: {
          manualChunks(id: string) {
            if (id.includes("@uppy")) return "uppy";
            if (id.includes("react-router")) return "router";
            if (id.includes("@tanstack")) return "query";
            return undefined;
          },
        },
      },
    },
  };
});
