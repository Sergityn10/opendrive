import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// Configuración de Vite para el frontend de OpenDrive.
//
// La SPA se sirve bajo el mismo origen que la API, así que las rutas
// relativas `/api/...` funcionan tanto en desarrollo como en producción.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    port: 5173,
    // En desarrollo, el proxy evita los problemas de CORS con la API en :8080.
    proxy: {
      "/api": {
        target: "http://localhost:8080",
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
        // Se separan las dependencias pesadas para que el navegador las
        // cachee de forma independiente del código de la aplicación.
        manualChunks(id: string) {
          if (id.includes("@uppy")) return "uppy";
          if (id.includes("react-router")) return "router";
          if (id.includes("@tanstack")) return "query";
          return undefined;
        },
      },
    },
  },
});
