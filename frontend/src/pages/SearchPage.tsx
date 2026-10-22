import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { Search as SearchIcon } from "lucide-react";
import { FileBrowser } from "@/components/FileBrowser";
import { search as searchApi } from "@/lib/api";
import { useUiStore } from "@/stores/ui";

/**
 * Búsqueda de archivos por nombre y, opcionalmente, por contenido.
 */
export function SearchPage() {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const viewMode = useUiStore((s) => s.viewMode);
  const selection = useUiStore((s) => s.selection);
  const toggleSelected = useUiStore((s) => s.toggleSelected);

  const query = params.get("q") ?? "";
  const [text, setText] = useState(query);
  const includeContent = params.get("content") === "true";

  const listing = useQuery({
    queryKey: ["search", query, includeContent],
    queryFn: () => searchApi.query(query, includeContent),
    enabled: query.trim().length > 0,
  });

  const items = listing.data ?? [];

  function submit(event: React.FormEvent) {
    event.preventDefault();
    const next = new URLSearchParams(params);
    if (text) next.set("q", text);
    else next.delete("q");
    if (includeContent) next.set("content", "true");
    setParams(next);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="border-b border-[var(--color-border)] px-4 py-3">
        <form onSubmit={submit} className="flex items-center gap-2">
          <div className="relative flex-1">
            <SearchIcon
              size={18}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-ink-muted)]"
            />
            <input
              type="search"
              autoFocus
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="Buscar en todos mis archivos"
              aria-label="Buscar en todos mis archivos"
              className="w-full rounded-full border border-[var(--color-border)] py-2 pl-10 pr-3 text-sm outline-none focus:border-[var(--color-accent)]"
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-[var(--color-ink-muted)]">
            <input
              type="checkbox"
              checked={includeContent}
              onChange={(e) => {
                const next = new URLSearchParams(params);
                if (e.target.checked) next.set("content", "true");
                else next.delete("content");
                setParams(next);
              }}
            />
            Buscar en el contenido
          </label>
        </form>
      </header>

      {query.trim() === "" ? (
        <div className="flex flex-1 items-center justify-center text-sm text-[var(--color-ink-muted)]">
          Escribe algo para buscar
        </div>
      ) : listing.isLoading ? (
        <div className="flex flex-1 items-center justify-center text-sm text-[var(--color-ink-muted)]">
          Buscando…
        </div>
      ) : items.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-sm text-[var(--color-ink-muted)]">
          <p>No hay resultados para «{query}»</p>
          <p className="text-xs">Prueba con menos palabras o activa la búsqueda en el contenido.</p>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          <FileBrowser
            items={items}
            viewMode={viewMode}
            selection={selection}
            onToggleSelect={toggleSelected}
            onOpen={(item) => {
              if (item.kind === "folder") {
                navigate(`/drive/${item.id}`);
              } else {
                const link = document.createElement("a");
                link.href = `/api/nodes/${item.id}/download`;
                link.download = item.name;
                document.body.appendChild(link);
                link.click();
                link.remove();
              }
            }}
            onContextMenu={() => undefined}
            onDownload={(item) => {
              const link = document.createElement("a");
              link.href = `/api/nodes/${item.id}/download`;
              link.download = item.name;
              document.body.appendChild(link);
              link.click();
              link.remove();
            }}
            onRename={() => undefined}
            onTrash={() => undefined}
          />
        </div>
      )}
    </div>
  );
}
