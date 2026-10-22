import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw, Trash2 } from "lucide-react";
import { FileBrowser } from "@/components/FileBrowser";
import { nodes as nodesApi, trash as trashApi } from "@/lib/api";
import { useUiStore } from "@/stores/ui";

/**
 * Vista de la papelera.
 */
export function TrashPage() {
  const queryClient = useQueryClient();
  const viewMode = useUiStore((s) => s.viewMode);
  const selection = useUiStore((s) => s.selection);
  const toggleSelected = useUiStore((s) => s.toggleSelected);

  const listing = useQuery({
    queryKey: ["trash"],
    queryFn: trashApi.list,
  });

  const restoreMutation = useMutation({
    mutationFn: (id: string) => nodesApi.restore(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["trash"] }),
  });

  const emptyMutation = useMutation({
    mutationFn: trashApi.empty,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["trash"] }),
  });

  const items = listing.data ?? [];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2">
        <h1 className="flex-1 text-base">Papelera</h1>
        <button
          type="button"
          onClick={() => emptyMutation.mutate()}
          disabled={items.length === 0}
          className="btn btn-ghost disabled:opacity-50"
        >
          <Trash2 size={18} />
          Vaciar la papelera
        </button>
      </header>

      {listing.isLoading ? (
        <div className="flex flex-1 items-center justify-center text-sm text-[var(--color-ink-muted)]">
          Cargando…
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          <FileBrowser
            items={items}
            viewMode={viewMode}
            selection={selection}
            onToggleSelect={toggleSelected}
            onOpen={(item) => restoreMutation.mutate(item.id)}
            onContextMenu={(item, position) => {
              void position;
              restoreMutation.mutate(item.id);
            }}
            onDownload={(item) => restoreMutation.mutate(item.id)}
            onRename={(item) => restoreMutation.mutate(item.id)}
            onTrash={(item) => restoreMutation.mutate(item.id)}
          />
        </div>
      )}
    </div>
  );
}

/**
 * Icono de restaurar, exportado para reutilización.
 */
export { RotateCcw };
