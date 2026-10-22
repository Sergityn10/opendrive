import { useEffect, useRef, useState } from "react";
import { ChevronRight, FolderPlus, Grid2x2, List, MoreVertical, Search, Upload } from "lucide-react";
import { useUiStore } from "@/stores/ui";
import { cn } from "@/lib/cn";

interface ToolbarProps {
  /** Título de la vista actual. */
  title: string;
  /** Si hay una carpeta padre a la que se puede volver. */
  canGoUp: boolean;
  /** Callback para volver a la carpeta padre. */
  onGoUp: () => void;
  /** Callback para crear una carpeta. */
  onCreateFolder: () => void;
  /** Callback para abrir el selector de archivos y subir. */
  onUpload: () => void;
  /** Callback al cambiar el texto de búsqueda. */
  onSearch: (text: string) => void;
  /** Texto de búsqueda actual. */
  searchText: string;
}

/**
 * Barra de herramientas del explorador de archivos.
 */
export function Toolbar({
  title,
  canGoUp,
  onGoUp,
  onCreateFolder,
  onUpload,
  onSearch,
  searchText,
}: ToolbarProps) {
  const viewMode = useUiStore((s) => s.viewMode);
  const toggleViewMode = useUiStore((s) => s.toggleViewMode);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // Cierra el menú al pulsar fuera o al pulsar Escape.
  useEffect(() => {
    if (!menuOpen) return;

    function handlePointerDown(event: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setMenuOpen(false);
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [menuOpen]);

  // Atajo de teclado: "/" enfoca el buscador.
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      const typing =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable;
      if (event.key === "/" && !typing) {
        event.preventDefault();
        searchRef.current?.focus();
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <header className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2">
      <div className="flex min-w-0 flex-1 items-center gap-1">
        {canGoUp && (
          <button
            type="button"
            onClick={onGoUp}
            className="btn btn-ghost btn-icon"
            title="Subir un nivel"
            aria-label="Subir un nivel"
          >
            <ChevronRight size={18} className="rotate-180" />
          </button>
        )}
        <h1 className="truncate text-base">{title}</h1>
      </div>

      <div className="relative">
        <Search
          size={16}
          className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--color-ink-muted)]"
        />
        <input
          ref={searchRef}
          type="search"
          value={searchText}
          onChange={(e) => onSearch(e.target.value)}
          placeholder="Buscar en esta carpeta"
          aria-label="Buscar en esta carpeta"
          className="w-56 rounded-full border border-transparent bg-[var(--color-surface-hover)] py-1.5 pl-8 pr-3 text-sm outline-none focus:border-[var(--color-accent)] focus:bg-[var(--color-surface)]"
        />
      </div>

      <button
        type="button"
        onClick={onCreateFolder}
        className="btn btn-ghost"
        title="Nueva carpeta"
      >
        <FolderPlus size={18} />
        <span className="hidden sm:inline">Nueva carpeta</span>
      </button>

      <button type="button" onClick={onUpload} className="btn btn-ghost" title="Subir archivos">
        <Upload size={18} />
        <span className="hidden sm:inline">Subir</span>
      </button>

      <button
        type="button"
        onClick={toggleViewMode}
        className="btn btn-ghost btn-icon"
        title={viewMode === "grid" ? "Ver en lista" : "Ver en cuadrícula"}
        aria-label={viewMode === "grid" ? "Ver en lista" : "Ver en cuadrícula"}
      >
        {viewMode === "grid" ? <List size={18} /> : <Grid2x2 size={18} />}
      </button>

      <div className="relative" ref={menuRef}>
        <button
          type="button"
          onClick={() => setMenuOpen((open) => !open)}
          className="btn btn-ghost btn-icon"
          title="Más opciones"
          aria-label="Más opciones"
        >
          <MoreVertical size={18} />
        </button>
        {menuOpen && (
          <div
            className={cn(
              "absolute right-0 top-full z-20 mt-1 w-56 rounded-lg border",
              "border-[var(--color-border)] bg-[var(--color-surface)] py-1 shadow-lg",
            )}
          >
            <SortMenuItem />
          </div>
        )}
      </div>
    </header>
  );
}

/** Entrada de menú con las opciones de ordenación. */
function SortMenuItem() {
  const sort = useUiStore((s) => s.sort);
  const order = useUiStore((s) => s.order);
  const setSort = useUiStore((s) => s.setSort);
  const toggleOrder = useUiStore((s) => s.toggleOrder);

  const options = [
    { value: "name", label: "Nombre" },
    { value: "modified", label: "Fecha de modificación" },
    { value: "size", label: "Tamaño" },
    { value: "kind", label: "Tipo" },
  ] as const;

  return (
    <div className="py-1">
      <p className="px-3 pb-1 text-xs font-medium uppercase tracking-wide text-[var(--color-ink-muted)]">
        Ordenar por
      </p>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          onClick={() => setSort(option.value)}
          className={cn(
            "flex w-full items-center justify-between px-3 py-1.5 text-left text-sm",
            "hover:bg-[var(--color-surface-hover)]",
            sort === option.value && "font-medium text-[var(--color-accent)]",
          )}
        >
          {option.label}
          {sort === option.value && (
            <span className="text-xs">{order === "asc" ? "↑" : "↓"}</span>
          )}
        </button>
      ))}
      <button
        type="button"
        onClick={toggleOrder}
        className="mt-1 w-full border-t border-[var(--color-border)] px-3 py-1.5 text-left text-sm hover:bg-[var(--color-surface-hover)]"
      >
        {order === "asc" ? "Orden ascendente" : "Orden descendente"}
      </button>
    </div>
  );
}
