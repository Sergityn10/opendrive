import { X } from "lucide-react";
import { formatBytes } from "@/lib/format";
import { useUiStore } from "@/stores/ui";
import type { UploadState } from "@/hooks/useUploads";

interface UploadTrayProps {
  /** Estado de las subidas en curso. */
  uploads: UploadState[];
  /** Callback para añadir más archivos. */
  onPickFiles: () => void;
  /** Callback para cancelar todas las subidas. */
  onCancelAll: () => void;
}

/**
 * Bandeja de subidas: muestra el progreso de los archivos en curso.
 */
export function UploadTray({ uploads, onPickFiles, onCancelAll }: UploadTrayProps) {
  const open = useUiStore((s) => s.uploadTrayOpen);
  const setOpen = useUiStore((s) => s.setUploadTrayOpen);

  if (!open || uploads.length === 0) return null;

  const active = uploads.filter(
    (u) => u.status === "uploading" || u.status === "waiting",
  );
  const finished = uploads.filter(
    (u) => u.status === "success" || u.status === "error",
  );

  return (
    <aside className="flex max-h-72 w-96 shrink-0 flex-col border-l border-[var(--color-border)] bg-[var(--color-surface)]">
      <header className="flex items-center justify-between border-b border-[var(--color-border)] px-4 py-2">
        <h2 className="text-sm font-medium">
          Subidas{active.length > 0 ? ` (${active.length} en curso)` : ""}
        </h2>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onPickFiles}
            className="btn btn-ghost btn-icon"
            title="Añadir archivos"
            aria-label="Añadir archivos"
          >
            +
          </button>
          <button
            type="button"
            onClick={() => {
              onCancelAll();
              setOpen(false);
            }}
            className="btn btn-ghost btn-icon"
            title="Cerrar la bandeja"
            aria-label="Cerrar la bandeja"
          >
            <X size={16} />
          </button>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto">
        {[...active, ...finished].map((upload) => {
          const percent =
            upload.percentage ??
            (upload.size > 0
              ? Math.min(100, Math.round((upload.uploaded / upload.size) * 100))
              : 0);
          return (
            <div
              key={upload.id}
              className="border-b border-[var(--color-border)] px-4 py-2"
            >
              <div className="flex items-baseline justify-between gap-2">
                <p className="truncate text-sm" title={upload.name}>
                  {upload.name}
                </p>
                <span className="shrink-0 text-xs text-[var(--color-ink-muted)]">
                  {upload.status === "success"
                    ? "Completado"
                    : upload.status === "error"
                      ? "Error"
                      : `${percent}%`}
                </span>
              </div>
              <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-[var(--color-surface-hover)]">
                <div
                  className={
                    upload.status === "error"
                      ? "h-full bg-[var(--color-danger)]"
                      : upload.status === "success"
                        ? "h-full bg-[var(--color-success)]"
                        : "h-full bg-[var(--color-accent)]"
                  }
                  style={{
                    width: `${upload.status === "success" ? 100 : percent}%`,
                  }}
                />
              </div>
              {upload.error && (
                <p className="mt-1 text-xs text-[var(--color-danger)]">
                  {upload.error}
                </p>
              )}
              <p className="mt-0.5 text-xs text-[var(--color-ink-muted)]">
                {formatBytes(upload.uploaded)} de {formatBytes(upload.size)}
              </p>
            </div>
          );
        })}
      </div>
    </aside>
  );
}
