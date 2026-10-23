import { useCallback, useEffect, useMemo, useState } from "react";
import Uppy from "@uppy/core";
import Tus from "@uppy/tus";
import { useQueryClient } from "@tanstack/react-query";
import { useUiStore } from "@/stores/ui";

/**
 * Estado de una subida en curso.
 */
export interface UploadState {
  /** Identificador del archivo dentro de Uppy. */
  id: string;
  /** Nombre del archivo. */
  name: string;
  /** Tamaño total en bytes. */
  size: number;
  /** Bytes enviados hasta ahora. */
  uploaded: number;
  /** Porcentaje enviado, si el servidor lo conoce. */
  percentage: number | null;
  /** Estado de la subida. */
  status: "waiting" | "uploading" | "success" | "error";
  /** Mensaje de error, si lo hay. */
  error?: string;
}

/**
 * Gestiona la instancia de Uppy y el estado de las subidas.
 *
 * El protocolo de subida es tus 1.0: si la conexión se interrumpe, la subida
 * se reanuda desde el último offset confirmado por el servidor, sin volver a
 * enviar los bytes ya recibidos.
 */
export function useUploads(parentId: string | null) {
  const queryClient = useQueryClient();
  const setTrayOpen = useUiStore((s) => s.setUploadTrayOpen);
  const [uploads, setUploads] = useState<UploadState[]>([]);

  const uppy = useMemo(() => {
    const instance = new Uppy({ autoProceed: false });

    instance.use(Tus, {
      endpoint: "/api/uploads",
      // La cookie de sesión viaja con cada petición: el endpoint tus la
      // valida igual que el resto de la API.
      withCredentials: true,
      chunkSize: 8 * 1024 * 1024,
      retryDelays: [0, 1000, 3000, 5000],
    });

    instance.on("error", (error) => {
      console.error("[OpenDrive Uppy Error]:", error);
    });

    instance.on("upload-error", (file, error, response) => {
      console.error("[OpenDrive Upload Error]:", file?.name, error, response);
    });

    return instance;
  }, []);

  // Refleja el estado de Uppy en la bandeja.
  useEffect(() => {
    function sync() {
      const files = uppy.getFiles();
      const next: UploadState[] = files.map((file) => {
        const progress = file.progress;
        const uploaded =
          progress && "bytesUploaded" in progress && progress.bytesUploaded
            ? progress.bytesUploaded
            : 0;
        return {
          id: file.id,
          name: file.name ?? "archivo",
          size: file.size ?? 0,
          uploaded,
          percentage: progress?.percentage ?? null,
          status: progress?.uploadComplete
            ? "success"
            : file.error
              ? "error"
              : progress?.uploadStarted
                ? "uploading"
                : "waiting",
          error: file.error ?? undefined,
        };
      });
      setUploads(next);
      if (next.length > 0) setTrayOpen(true);
    }

    uppy.on("upload-progress", sync);
    uppy.on("upload-success", sync);
    uppy.on("upload-error", sync);
    uppy.on("file-added", sync);
    uppy.on("file-removed", sync);

    return () => {
      uppy.off("upload-progress", sync);
      uppy.off("upload-success", sync);
      uppy.off("upload-error", sync);
      uppy.off("file-added", sync);
      uppy.off("file-removed", sync);
    };
  }, [uppy, setTrayOpen]);

  // Refresca el listado cuando una subida termina.
  useEffect(() => {
    function handleSuccess(file?: { successful?: boolean }) {
      if (file?.successful) {
        queryClient.invalidateQueries({ queryKey: ["nodes"] });
      }
    }
    uppy.on("upload-success", handleSuccess as never);
    return () => {
      uppy.off("upload-success", handleSuccess as never);
    };
  }, [uppy, queryClient]);

  // Libera la instancia al desmontar.
  useEffect(() => () => uppy.destroy(), [uppy]);

  /** Encola una lista de archivos y lanza la subida. */
  const uploadFiles = useCallback(
    (files: File[]) => {
      if (files.length === 0) return;

      // Limpia de la instancia de Uppy cualquier archivo que ya haya completado,
      // fallado o que tenga el mismo nombre para permitir reintentar sin errores.
      const existing = uppy.getFiles();
      const newNames = new Set(files.map((f) => f.name));
      for (const file of existing) {
        if (file.progress?.uploadComplete || file.error || newNames.has(file.name)) {
          try {
            uppy.removeFile(file.id);
          } catch {
            // Ignorar
          }
        }
      }

      try {
        uppy.addFiles(
          files.map((file) => ({
            name: file.name,
            type: file.type,
            data: file,
            // Uppy copia `name` a `filename` y `type` a `filetype` en los
            // metadatos de tus. La carpeta de destino se añade aquí.
            meta: { parent_id: parentId ?? "" },
          })),
        );
        setTrayOpen(true);
        uppy.upload().catch((err) => {
          console.error("Fallo al iniciar uppy.upload():", err);
        });
      } catch (err) {
        console.warn("Aviso al añadir archivos a Uppy:", err);
      }
    },
    [uppy, parentId, setTrayOpen],
  );

  /** Abre el selector de archivos del sistema y encola las subidas. */
  const pickFiles = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.onchange = () => {
      if (!input.files || input.files.length === 0) return;
      uploadFiles(Array.from(input.files));
    };
    input.click();
  }, [uploadFiles]);

  return { uppy, uploads, pickFiles, uploadFiles };
}
