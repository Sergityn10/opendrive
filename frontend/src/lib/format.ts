/** Utilidades de formato para la interfaz. */

/**
 * Formatea un tamaño en bytes de forma legible.
 *
 * Usa unidades binarias (KiB, MiB) y una precisión adaptativa.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes === 0) return "0 B";

  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** exponent;

  const decimals = exponent === 0 ? 0 : value < 10 ? 1 : 0;
  return `${value.toFixed(decimals)} ${units[exponent]}`;
}

/**
 * Formatea una fecha ISO en una cadena corta relativa al ahora.
 *
 * Hoy muestra la hora; esta semana, el día de la semana; si no, la fecha.
 */
export function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";

  const now = new Date();
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();

  if (sameDay) {
    return date.toLocaleTimeString("es", {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  const daysAgo = Math.floor((now.getTime() - date.getTime()) / 86_400_000);
  if (daysAgo >= 0 && daysAgo < 7) {
    return date.toLocaleDateString("es", { weekday: "short" });
  }

  return date.toLocaleDateString("es", {
    day: "2-digit",
    month: "short",
    year: date.getFullYear() === now.getFullYear() ? undefined : "2-digit",
  });
}

/**
 * Devuelve la extensión en minúsculas de un nombre de archivo, sin el punto.
 */
export function fileExtension(name: string): string {
  const index = name.lastIndexOf(".");
  if (index <= 0 || index === name.length - 1) return "";
  return name.slice(index + 1).toLowerCase();
}

/**
 * `true` si el archivo tiene una vista previa en el navegador.
 */
export function isPreviewable(node: {
  kind: "file" | "folder";
  mime_type: string | null;
  name: string;
}): boolean {
  if (node.kind === "folder") return false;
  return (
    nodeCategory(node.kind, node.mime_type, node.name) !== "file" &&
    nodeCategory(node.kind, node.mime_type, node.name) !== "archive"
  );
}
/**
 * Clasifica un nodo para elegir el icono y el color de la miniatura.
 */
export function nodeCategory(
  kind: "file" | "folder",
  mimeType: string | null,
  name: string,
): "folder" | "image" | "video" | "audio" | "pdf" | "document" | "archive" | "file" {
  if (kind === "folder") return "folder";

  const mime = mimeType ?? "";
  if (mime.startsWith("image/")) return "image";
  if (mime.startsWith("video/")) return "video";
  if (mime.startsWith("audio/")) return "audio";
  if (mime === "application/pdf") return "pdf";

  const ext = fileExtension(name);
  if (
    [
      "doc", "docx", "odt", "rtf", "txt", "md", "pages",
      "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp",
    ].includes(ext)
  ) {
    return "document";
  }
  if (["zip", "tar", "gz", "bz2", "xz", "7z", "rar"].includes(ext)) {
    return "archive";
  }
  return "file";
}
