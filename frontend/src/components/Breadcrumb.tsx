import { ChevronRight } from "lucide-react";
import type { NodeSummary } from "@/lib/types";

interface BreadcrumbProps {
  /** Ruta de ancestros, desde la raíz hasta la carpeta actual. */
  path: NodeSummary[];
  /** Callback al navegar a un ancestro. */
  onNavigate: (id: string) => void;
}

/**
 * Ruta de navegación (migas de pan).
 */
export function Breadcrumb({ path, onNavigate }: BreadcrumbProps) {
  if (path.length === 0) return null;

  return (
    <nav
      aria-label="Ruta de navegación"
      className="flex items-center gap-0.5 overflow-x-auto px-4 py-1.5 text-sm"
    >
      {path.map((node, index) => {
        const last = index === path.length - 1;
        return (
          <span key={node.id} className="flex shrink-0 items-center gap-0.5">
            {index > 0 && (
              <ChevronRight size={14} className="text-[var(--color-ink-muted)]" />
            )}
            <button
              type="button"
              onClick={() => onNavigate(node.id)}
              className={
                last
                  ? "rounded px-1.5 py-0.5 font-medium text-[var(--color-ink)]"
                  : "rounded px-1.5 py-0.5 text-[var(--color-accent)] hover:bg-[var(--color-surface-hover)]"
              }
              aria-current={last ? "page" : undefined}
            >
              {node.name}
            </button>
          </span>
        );
      })}
    </nav>
  );
}
