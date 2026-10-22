import { cn } from "@/lib/cn";
import { nodeCategory } from "@/lib/format";
import {
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileText,
  FileVideo,
  Folder,
  type LucideIcon,
} from "lucide-react";

/** Icono y color según el tipo de nodo. */
const ICONS: Record<
  ReturnType<typeof nodeCategory>,
  { icon: LucideIcon; color: string }
> = {
  folder: { icon: Folder, color: "text-[#5f6368]" },
  image: { icon: FileImage, color: "text-[#188038]" },
  video: { icon: FileVideo, color: "text-[#d93025]" },
  audio: { icon: FileAudio, color: "text-[#e8710a]" },
  pdf: { icon: FileText, color: "text-[#d93025]" },
  document: { icon: FileText, color: "text-[#1a73e8]" },
  archive: { icon: FileArchive, color: "text-[#e8710a]" },
  file: { icon: FileCode, color: "text-[#5f6368]" },
};

interface FileIconProps {
  /** Nombre del archivo. */
  name: string;
  /** Tipo de nodo. */
  kind: "file" | "folder";
  /** Tipo MIME, si se conoce. */
  mimeType: string | null;
  /** Tamaño en píxeles del icono. */
  size?: number;
  /** Clases CSS adicionales. */
  className?: string;
}

/**
 * Icono de un nodo del árbol de archivos.
 */
export function FileIcon({
  name,
  kind,
  mimeType,
  size = 24,
  className,
}: FileIconProps) {
  const category = nodeCategory(kind, mimeType, name);
  const { icon: Icon, color } = ICONS[category];
  return <Icon size={size} className={cn(color, className)} aria-hidden />;
}
