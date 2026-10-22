import { useEffect, useState } from "react";
import { Download, X } from "lucide-react";
import { FileIcon } from "@/components/FileIcon";
import { formatBytes } from "@/lib/format";
import { nodeCategory } from "@/lib/format";
import type { NodeSummary } from "@/lib/types";

interface PreviewDialogProps {
  /** Nodo a previsualizar. */
  item: NodeSummary;
  /** Callback para cerrar. */
  onClose: () => void;
  /** Callback para descargar. */
  onDownload: (item: NodeSummary) => void;
}

/**
 * Cuadro de previsualización de un archivo.
 *
 * Las imágenes, el vídeo, el audio y el texto se muestran directamente. Los
 * PDF se renderizan con pdf.js en el navegador, sin pasar los bytes por el
 * servidor más de una vez.
 */
export function PreviewDialog({ item, onClose, onDownload }: PreviewDialogProps) {
  const category = nodeCategory(item.kind, item.mime_type, item.name);

  // Cierra con Escape.
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const downloadUrl = `/api/nodes/${item.id}/download`;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={`Vista previa de ${item.name}`}
    >
      <div
        className="flex max-h-full w-full max-w-4xl flex-col overflow-hidden rounded-xl bg-[var(--color-surface)] shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center gap-3 border-b border-[var(--color-border)] px-4 py-2">
          <FileIcon name={item.name} kind={item.kind} mimeType={item.mime_type} size={20} />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium" title={item.name}>
              {item.name}
            </p>
            <p className="text-xs text-[var(--color-ink-muted)]">
              {item.kind === "file" ? formatBytes(item.size_bytes) : "Carpeta"}
              {item.mime_type ? ` · ${item.mime_type}` : ""}
            </p>
          </div>
          <button
            type="button"
            onClick={() => onDownload(item)}
            className="btn btn-ghost btn-icon"
            title="Descargar"
            aria-label="Descargar"
          >
            <Download size={18} />
          </button>
          <button
            type="button"
            onClick={onClose}
            className="btn btn-ghost btn-icon"
            title="Cerrar"
            aria-label="Cerrar"
          >
            <X size={18} />
          </button>
        </header>

        <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-[var(--color-surface-alt)] p-4">
          <PreviewBody item={item} url={downloadUrl} category={category} />
        </div>
      </div>
    </div>
  );
}

interface PreviewBodyProps {
  item: NodeSummary;
  url: string;
  category: ReturnType<typeof nodeCategory>;
}

function PreviewBody({ item, url, category }: PreviewBodyProps) {
  switch (category) {
    case "image":
      return (
        <img
          src={url}
          alt={item.name}
          className="max-h-[70vh] max-w-full rounded object-contain shadow"
        />
      );

    case "video":
      return (
        <video
          src={url}
          controls
          className="max-h-[70vh] max-w-full rounded bg-black shadow"
        />
      );

    case "audio":
      return <audio src={url} controls className="w-full max-w-lg" />;

    case "pdf":
      return <PdfPreview url={url} title={item.name} />;

    case "document":
    case "file":
      return <TextPreview url={url} />;

    default:
      return (
        <div className="flex flex-col items-center gap-2 py-10 text-[var(--color-ink-muted)]">
          <FileIcon name={item.name} kind={item.kind} mimeType={item.mime_type} size={48} />
          <p className="text-sm">No hay vista previa para este formato</p>
        </div>
      );
  }
}

/**
 * Vista previa de un PDF con pdf.js.
 *
 * El PDF se descarga una sola vez y se renderiza en el navegador.
 */
function PdfPreview({ url, title }: { url: string; title: string }) {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function render() {
      try {
        const pdfjs = await import("pdfjs-dist");
        // El worker se carga desde el mismo origen que la aplicación.
        pdfjs.GlobalWorkerOptions.workerSrc = new URL(
          "pdfjs-dist/build/pdf.worker.min.mjs",
          import.meta.url,
        ).toString();

        const pdf = await pdfjs.getDocument({ url }).promise;
        if (cancelled) return;

        const container = window.document.getElementById("pdf-preview");
        if (!container) return;

        container.innerHTML = "";
        // Solo se renderiza la primera página: es suficiente para
        // identificar el documento y mantiene el coste bajo.
        const page = await pdf.getPage(1);
        const viewport = page.getViewport({ scale: 1.4 });
        const canvas = window.document.createElement("canvas");
        canvas.width = viewport.width;
        canvas.height = viewport.height;
        canvas.className = "rounded shadow bg-white";
        container.appendChild(canvas);
        await page.render({ canvas, viewport }).promise;
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error ? err.message : "No se pudo previsualizar el PDF",
          );
        }
      }
    }

    void render();
    return () => {
      cancelled = true;
    };
  }, [url]);

  if (error) {
    return (
      <div className="flex flex-col items-center gap-2 py-10 text-sm text-[var(--color-ink-muted)]">
        <p>No se pudo previsualizar el PDF</p>
        <p className="text-xs">{error}</p>
      </div>
    );
  }

  return (
    <div id="pdf-preview" className="flex flex-col items-center gap-2" title={title}>
      <p className="text-xs text-[var(--color-ink-muted)]">Cargando PDF…</p>
    </div>
  );
}

/**
 * Vista previa de un archivo de texto.
 */
function TextPreview({ url }: { url: string }) {
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetch(url, { credentials: "same-origin" })
      .then((response) => {
        if (!response.ok) throw new Error(`Error ${response.status}`);
        return response.text();
      })
      .then((text) => {
        if (!cancelled) setContent(text.slice(0, 200_000));
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "No se pudo leer el archivo");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [url]);

  if (error) {
    return (
      <p className="py-10 text-sm text-[var(--color-ink-muted)]">
        No se pudo previsualizar el archivo
      </p>
    );
  }

  if (content === null) {
    return <p className="py-10 text-sm text-[var(--color-ink-muted)]">Cargando…</p>;
  }

  return (
    <pre className="max-h-[70vh] w-full overflow-auto whitespace-pre-wrap rounded bg-[var(--color-surface)] p-4 text-left font-mono text-xs shadow">
      {content}
    </pre>
  );
}
