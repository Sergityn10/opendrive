import { useEffect, useRef } from "react";
import { Download, Folder as FolderIcon, Pencil, Share2, Trash2 } from "lucide-react";
import { cn } from "@/lib/cn";
import type { NodeSummary } from "@/lib/types";

interface ContextMenuProps {
  /** Posición del menú en coordenadas de pantalla. */
  position: { x: number; y: number };
  /** Elemento sobre el que se ha abierto. */
  item: NodeSummary;
  /** Callback de descarga. */
  onDownload: (item: NodeSummary) => void;
  /** Callback de renombrado. */
  onRename: (item: NodeSummary) => void;
  /** Callback de borrado. */
  onTrash: (item: NodeSummary) => void;
  /** Callback de compartir. */
  onShare: (item: NodeSummary) => void;
  /** Callback para cerrar el menú. */
  onClose: () => void;
}

/**
 * Menú contextual de un elemento del explorador.
 *
 * Se posiciona dentro de la ventana y se cierra al pulsar fuera o Escape.
 */
export function ContextMenu({
  position,
  item,
  onDownload,
  onRename,
  onTrash,
  onShare,
  onClose,
}: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handlePointerDown(event: MouseEvent) {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        onClose();
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  // Mantiene el menú dentro de la ventana.
  const style = {
    left: Math.min(position.x, window.innerWidth - 208),
    top: Math.min(position.y, window.innerHeight - 200),
  };

  return (
    <div
      ref={ref}
      role="menu"
      style={style}
      className={cn(
        "fixed z-50 w-52 rounded-lg border border-[var(--color-border)]",
        "bg-[var(--color-surface)] py-1 shadow-xl",
      )}
    >
      {item.kind === "file" && (
        <MenuButton
          icon={Download}
          label="Descargar"
          onClick={() => {
            onDownload(item);
            onClose();
          }}
        />
      )}
      <MenuButton
        icon={FolderIcon}
        label="Abrir"
        onClick={() => {
          onClose();
        }}
        hidden
      />
      <MenuButton
        icon={Pencil}
        label="Renombrar"
        onClick={() => {
          onRename(item);
          onClose();
        }}
      />
      <MenuButton
        icon={Share2}
        label="Compartir"
        onClick={() => {
          onShare(item);
          onClose();
        }}
      />
      <div className="my-1 border-t border-[var(--color-border)]" />
      <MenuButton
        icon={Trash2}
        label="Mover a la papelera"
        danger
        onClick={() => {
          onTrash(item);
          onClose();
        }}
      />
    </div>
  );
}

interface MenuButtonProps {
  icon: typeof Download;
  label: string;
  onClick: () => void;
  danger?: boolean;
  hidden?: boolean;
}

function MenuButton({ icon: Icon, label, onClick, danger, hidden }: MenuButtonProps) {
  if (hidden) return null;
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm",
        "hover:bg-[var(--color-surface-hover)]",
        danger && "text-[var(--color-danger)]",
      )}
    >
      <Icon size={16} />
      {label}
    </button>
  );
}
