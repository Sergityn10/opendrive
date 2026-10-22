import { useCallback, useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Download, Folder as FolderIcon, MoreVertical, Pencil, Trash2 } from "lucide-react";
import { FileIcon } from "@/components/FileIcon";
import { cn } from "@/lib/cn";
import { formatBytes, formatDate } from "@/lib/format";
import type { NodeSummary } from "@/lib/types";

interface FileBrowserProps {
  /** Elementos a mostrar. */
  items: NodeSummary[];
  /** Modo de visualización. */
  viewMode: "grid" | "list";
  /** Identificadores de los elementos seleccionados. */
  selection: Set<string>;
  /** Callback al seleccionar o deseleccionar un elemento. */
  onToggleSelect: (id: string, exclusive: boolean) => void;
  /** Callback al abrir un elemento. */
  onOpen: (item: NodeSummary) => void;
  /** Callback al mostrar el menú contextual de un elemento. */
  onContextMenu: (item: NodeSummary, position: { x: number; y: number }) => void;
  /** Callback al pedir la descarga de un archivo. */
  onDownload: (item: NodeSummary) => void;
  /** Callback al renombrar un elemento. */
  onRename: (item: NodeSummary) => void;
  /** Callback al enviar un elemento a la papelera. */
  onTrash: (item: NodeSummary) => void;
}

/**
 * Explorador de archivos: cuadrícula o lista.
 */
export function FileBrowser({
  items,
  viewMode,
  selection,
  onToggleSelect,
  onOpen,
  onContextMenu,
  onDownload,
  onRename,
  onTrash,
}: FileBrowserProps) {
  if (items.length === 0) {
    return <EmptyState />;
  }

  return viewMode === "grid" ? (
    <FileGrid
      items={items}
      selection={selection}
      onToggleSelect={onToggleSelect}
      onOpen={onOpen}
      onContextMenu={onContextMenu}
      onDownload={onDownload}
      onRename={onRename}
      onTrash={onTrash}
    />
  ) : (
    <FileList
      items={items}
      selection={selection}
      onToggleSelect={onToggleSelect}
      onOpen={onOpen}
      onContextMenu={onContextMenu}
      onDownload={onDownload}
      onRename={onRename}
      onTrash={onTrash}
    />
  );
}

/** Acciones compartidas por la cuadrícula y la lista. */
interface ItemActions {
  onOpen: (item: NodeSummary) => void;
  onContextMenu: (item: NodeSummary, position: { x: number; y: number }) => void;
  onDownload: (item: NodeSummary) => void;
  onRename: (item: NodeSummary) => void;
  onTrash: (item: NodeSummary) => void;
}

/** `true` si el archivo es una imagen y por tanto tiene miniatura. */
function isImage(item: NodeSummary): boolean {
  return item.mime_type?.startsWith("image/") ?? false;
}

/**
 * Cuadrícula de archivos.
 */
function FileGrid({
  items,
  selection,
  onToggleSelect,
  ...actions
}: Omit<FileBrowserProps, "viewMode">) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-2 p-4">
      {items.map((item) => (
        <GridItem
          key={item.id}
          item={item}
          selected={selection.has(item.id)}
          onToggleSelect={onToggleSelect}
          {...actions}
        />
      ))}
    </div>
  );
}

interface GridItemProps extends ItemActions {
  item: NodeSummary;
  selected: boolean;
  onToggleSelect: (id: string, exclusive: boolean) => void;
}

function GridItem({
  item,
  selected,
  onToggleSelect,
  onOpen,
  onContextMenu,
  onDownload,
  onRename,
  onTrash,
}: GridItemProps) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={(e) => onToggleSelect(item.id, e.ctrlKey || e.metaKey)}
      onDoubleClick={() => onOpen(item)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(item, { x: e.clientX, y: e.clientY });
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") onOpen(item);
        if (e.key === " ") {
          e.preventDefault();
          onToggleSelect(item.id, false);
        }
      }}
      aria-selected={selected}
      className={cn(
        "group relative flex cursor-pointer flex-col items-center rounded-lg border p-3 text-center",
        "transition-colors hover:bg-[var(--color-surface-hover)]",
        selected && "border-[var(--color-accent)] bg-[var(--color-surface-hover)]",
      )}
    >
      <ItemMenu
        item={item}
        onDownload={onDownload}
        onRename={onRename}
        onTrash={onTrash}
      />
      <div className="mb-2 flex h-16 w-full items-center justify-center">
        {item.kind === "file" && isImage(item) ? (
          <img
            src={`/api/nodes/${item.id}/thumbnail?size=256`}
            alt=""
            loading="lazy"
            decoding="async"
            className="max-h-16 max-w-full rounded object-cover"
            onError={(event) => {
              // Si la miniatura no se puede generar, se muestra el icono.
              event.currentTarget.style.display = "none";
            }}
          />
        ) : (
          <FileIcon
            name={item.name}
            kind={item.kind}
            mimeType={item.mime_type}
            size={44}
          />
        )}
      </div>
      <p className="w-full truncate text-sm" title={item.name}>
        {item.name}
      </p>
      {item.kind === "file" && (
        <p className="mt-0.5 text-xs text-[var(--color-ink-muted)]">
          {formatBytes(item.size_bytes)}
        </p>
      )}
    </div>
  );
}

/**
 * Lista de archivos, virtualizada.
 */
function FileList({
  items,
  selection,
  onToggleSelect,
  ...actions
}: Omit<FileBrowserProps, "viewMode">) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // `useVirtualizer` de TanStack devuelve funciones que no se pueden memoizar
  // sin riesgo de estado obsoleto, así que el compilador de React las omite.
  // Es un comportamiento conocido y seguro de esta biblioteca.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 44,
    overscan: 12,
  });

  return (
    <div ref={scrollRef} className="flex-1 overflow-y-auto px-2 pb-4">
      <table className="w-full table-fixed border-collapse text-sm">
        <thead className="sticky top-0 z-10 bg-[var(--color-surface)]">
          <tr className="text-left text-xs uppercase tracking-wide text-[var(--color-ink-muted)]">
            <th className="w-10 px-2 py-2" />
            <th className="py-2 font-medium">Nombre</th>
            <th className="hidden w-40 py-2 font-medium md:table-cell">Modificado</th>
            <th className="hidden w-28 py-2 text-right font-medium sm:table-cell">
              Tamaño
            </th>
            <th className="w-10" />
          </tr>
        </thead>
        <tbody>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const item = items[virtualRow.index];
            return (
              <tr
                key={item.id}
                onClick={(e) => onToggleSelect(item.id, e.ctrlKey || e.metaKey)}
                onDoubleClick={() => actions.onOpen(item)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  actions.onContextMenu(item, { x: e.clientX, y: e.clientY });
                }}
                className={cn(
                  "cursor-pointer border-b border-transparent hover:bg-[var(--color-surface-hover)]",
                  selection.has(item.id) && "bg-[var(--color-surface-hover)]",
                )}
                style={{ height: virtualRow.size }}
              >
                <td className="px-2">
                  <FileIcon
                    name={item.name}
                    kind={item.kind}
                    mimeType={item.mime_type}
                    size={18}
                  />
                </td>
                <td className="truncate py-2 pr-2">
                  <span className="truncate" title={item.name}>
                    {item.name}
                  </span>
                </td>
                <td className="hidden py-2 text-[var(--color-ink-muted)] md:table-cell">
                  {formatDate(item.updated_at)}
                </td>
                <td className="hidden py-2 text-right text-[var(--color-ink-muted)] sm:table-cell">
                  {item.kind === "file" ? formatBytes(item.size_bytes) : "—"}
                </td>
                <td>
                  <ItemMenu
                    item={item}
                    onDownload={actions.onDownload}
                    onRename={actions.onRename}
                    onTrash={actions.onTrash}
                  />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

interface ItemMenuProps {
  item: NodeSummary;
  onDownload: (item: NodeSummary) => void;
  onRename: (item: NodeSummary) => void;
  onTrash: (item: NodeSummary) => void;
}

/**
 * Menú de acciones de un elemento.
 */
function ItemMenu({ item, onDownload, onRename, onTrash }: ItemMenuProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;

    function handlePointerDown(event: MouseEvent) {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handlePointerDown);
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [open]);

  const toggle = useCallback((event: React.MouseEvent) => {
    event.stopPropagation();
    setOpen((value) => !value);
  }, []);

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={toggle}
        className="btn btn-ghost btn-icon opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100"
        aria-label={`Acciones de ${item.name}`}
        title="Acciones"
      >
        <MoreVertical size={16} />
      </button>
      {open && (
        <div
          className={cn(
            "absolute right-0 top-full z-20 mt-0.5 w-44 rounded-lg border",
            "border-[var(--color-border)] bg-[var(--color-surface)] py-1 shadow-lg",
          )}
          onClick={(e) => e.stopPropagation()}
        >
          {item.kind === "file" && (
            <MenuButton
              icon={Download}
              label="Descargar"
              onClick={() => {
                onDownload(item);
                setOpen(false);
              }}
            />
          )}
          <MenuButton
            icon={Pencil}
            label="Renombrar"
            onClick={() => {
              onRename(item);
              setOpen(false);
            }}
          />
          <MenuButton
            icon={Trash2}
            label="Mover a la papelera"
            danger
            onClick={() => {
              onTrash(item);
              setOpen(false);
            }}
          />
        </div>
      )}
    </div>
  );
}

interface MenuButtonProps {
  icon: typeof FolderIcon;
  label: string;
  onClick: () => void;
  danger?: boolean;
}

function MenuButton({ icon: Icon, label, onClick, danger }: MenuButtonProps) {
  return (
    <button
      type="button"
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

/**
 * Estado vacío del explorador.
 */
function EmptyState() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 py-20 text-center">
      <FolderIcon size={44} className="text-[var(--color-border)]" />
      <p className="text-sm text-[var(--color-ink-muted)]">
        Esta carpeta está vacía
      </p>
    </div>
  );
}
