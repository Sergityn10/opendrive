import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Breadcrumb } from "@/components/Breadcrumb";
import { ContextMenu } from "@/components/ContextMenu";
import { FileBrowser } from "@/components/FileBrowser";
import { PreviewDialog } from "@/components/PreviewDialog";
import { Toolbar } from "@/components/Toolbar";
import { UploadTray } from "@/components/UploadTray";
import { useUploads } from "@/hooks/useUploads";
import { nodes as nodesApi } from "@/lib/api";
import { isPreviewable } from "@/lib/format";
import { useUiStore } from "@/stores/ui";
import type { NodeSummary } from "@/lib/types";

/**
 * Explorador de archivos: la vista principal de OpenDrive.
 */
export function DrivePage() {
  const { folderId } = useParams<{ folderId: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const viewMode = useUiStore((s) => s.viewMode);
  const sort = useUiStore((s) => s.sort);
  const order = useUiStore((s) => s.order);
  const searchText = useUiStore((s) => s.searchText);
  const setSearchText = useUiStore((s) => s.setSearchText);
  const selection = useUiStore((s) => s.selection);
  const toggleSelected = useUiStore((s) => s.toggleSelected);

  const [contextMenu, setContextMenu] = useState<{
    item: NodeSummary;
    position: { x: number; y: number };
  } | null>(null);
  const [preview, setPreview] = useState<NodeSummary | null>(null);

  const parentId = folderId ?? null;
  const { uppy, uploads, pickFiles } = useUploads(parentId);

  const listing = useQuery({
    queryKey: ["nodes", parentId, sort, order, searchText],
    queryFn: () =>
      nodesApi.list({
        parentId,
        sort,
        order,
        q: searchText || undefined,
        limit: 500,
      }),
  });

  const breadcrumb = useQuery({
    queryKey: ["breadcrumb", parentId],
    queryFn: () => nodesApi.breadcrumb(parentId!),
    enabled: Boolean(parentId),
  });

  // Si la carpeta no existe, se vuelve a la raíz.
  useEffect(() => {
    if (listing.isError) {
      navigate("/drive", { replace: true });
    }
  }, [listing.isError, navigate]);

  const invalidate = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["nodes"] });
  }, [queryClient]);

  const renameMutation = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      nodesApi.rename(id, name),
    onSuccess: invalidate,
  });

  const trashMutation = useMutation({
    mutationFn: (id: string) => nodesApi.trash(id),
    onSuccess: invalidate,
  });

  const createFolderMutation = useMutation({
    mutationFn: (name: string) => nodesApi.createFolder(parentId, name),
    onSuccess: invalidate,
  });

  const items = listing.data?.items ?? [];

  const currentName = useMemo(() => {
    if (!parentId) return "Mi Drive";
    const last = breadcrumb.data?.at(-1);
    return last?.name ?? "Cargando…";
  }, [parentId, breadcrumb.data]);

  function handleOpen(item: NodeSummary) {
    if (item.kind === "folder") {
      navigate(`/drive/${item.id}`);
      return;
    }
    // Los archivos con previsualización se abren en el cuadro de vista
    // previa; el resto se descargan directamente.
    if (isPreviewable(item)) {
      setPreview(item);
      return;
    }
    download(item);
  }

  /** Descarga un archivo con el nombre original. */
  function download(item: NodeSummary) {
    const link = document.createElement("a");
    link.href = nodesApi.downloadUrl(item.id);
    link.download = item.name;
    document.body.appendChild(link);
    link.click();
    link.remove();
  }

  function handleRename(item: NodeSummary) {
    const next = window.prompt("Nuevo nombre", item.name);
    if (next && next !== item.name) {
      renameMutation.mutate({ id: item.id, name: next });
    }
  }

  function handleTrash(item: NodeSummary) {
    trashMutation.mutate(item.id);
  }

  function handleCreateFolder() {
    const name = window.prompt("Nombre de la carpeta", "Nueva carpeta");
    if (name) {
      createFolderMutation.mutate(name);
    }
  }

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col">
        <Toolbar
          title={currentName}
          canGoUp={Boolean(parentId)}
          onGoUp={() => {
            const parent = breadcrumb.data?.at(-2);
            navigate(parent ? `/drive/${parent.id}` : "/drive");
          }}
          onCreateFolder={handleCreateFolder}
          onUpload={pickFiles}
          onSearch={setSearchText}
          searchText={searchText}
        />

        {parentId && (
          <Breadcrumb
            path={breadcrumb.data ?? []}
            onNavigate={(id) => navigate(`/drive/${id}`)}
          />
        )}

        {listing.isLoading ? (
          <div className="flex flex-1 items-center justify-center text-sm text-[var(--color-ink-muted)]">
            Cargando…
          </div>
        ) : listing.isError ? (
          <div className="flex flex-1 items-center justify-center text-sm text-[var(--color-danger)]">
            No se pudo cargar la carpeta
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto">
            <FileBrowser
              items={items}
              viewMode={viewMode}
              selection={selection}
              onToggleSelect={toggleSelected}
              onOpen={handleOpen}
              onContextMenu={(item, position) => setContextMenu({ item, position })}
              onDownload={download}
              onRename={handleRename}
              onTrash={handleTrash}
            />
          </div>
        )}
      </div>

      <UploadTray
        uploads={uploads}
        onPickFiles={pickFiles}
        onCancelAll={() => void uppy.cancelAll()}
      />

      {preview && (
        <PreviewDialog
          item={preview}
          onClose={() => setPreview(null)}
          onDownload={download}
        />
      )}

      {contextMenu && (
        <ContextMenu
          position={contextMenu.position}
          item={contextMenu.item}
          onDownload={handleOpen}
          onRename={handleRename}
          onTrash={handleTrash}
          onShare={() => {
            // El compartido por enlace llega en la Fase 7.
            window.alert("El compartir por enlace llega en una fase posterior.");
          }}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
