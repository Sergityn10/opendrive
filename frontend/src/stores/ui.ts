import { create } from "zustand";
import type { SortField, SortOrder } from "@/lib/types";

/** Modo de visualización del explorador de archivos. */
export type ViewMode = "grid" | "list";

/** Estado de la interfaz que no viene del servidor. */
interface UiState {
  /** Modo de visualización actual. */
  viewMode: ViewMode;
  /** Campo de ordenación. */
  sort: SortField;
  /** Dirección de ordenación. */
  order: SortOrder;
  /** Texto del buscador. */
  searchText: string;
  /** `true` si la búsqueda incluye el contenido de los documentos. */
  searchContent: boolean;
  /** Identificadores de los nodos seleccionados. */
  selection: Set<string>;
  /** Nodo sobre el que se ha abierto el menú contextual. */
  contextTarget: string | null;
  /** Panel de subida visible. */
  uploadTrayOpen: boolean;

  setViewMode: (mode: ViewMode) => void;
  toggleViewMode: () => void;
  setSort: (field: SortField) => void;
  toggleOrder: () => void;
  setSearchText: (text: string) => void;
  setSearchContent: (value: boolean) => void;
  setSelection: (ids: Set<string>) => void;
  toggleSelected: (id: string, exclusive?: boolean) => void;
  clearSelection: () => void;
  setContextTarget: (id: string | null) => void;
  setUploadTrayOpen: (open: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  viewMode: "grid",
  sort: "name",
  order: "asc",
  searchText: "",
  searchContent: false,
  selection: new Set(),
  contextTarget: null,
  uploadTrayOpen: false,

  setViewMode: (viewMode) => set({ viewMode }),
  toggleViewMode: () =>
    set((state) => ({ viewMode: state.viewMode === "grid" ? "list" : "grid" })),

  setSort: (sort) =>
    set((state) => ({
      sort,
      // Al cambiar el campo se alterna la dirección, como en Google Drive.
      order: state.sort === sort ? (state.order === "asc" ? "desc" : "asc") : "asc",
    })),

  toggleOrder: () =>
    set((state) => ({ order: state.order === "asc" ? "desc" : "asc" })),

  setSearchText: (searchText) => set({ searchText }),
  setSearchContent: (searchContent) => set({ searchContent }),

  setSelection: (selection) => set({ selection }),

  toggleSelected: (id, exclusive = false) =>
    set((state) => {
      const selection = new Set(exclusive ? [] : state.selection);
      if (state.selection.has(id) && !exclusive) {
        selection.delete(id);
      } else {
        selection.add(id);
      }
      return { selection };
    }),

  clearSelection: () => set({ selection: new Set() }),
  setContextTarget: (contextTarget) => set({ contextTarget }),
  setUploadTrayOpen: (uploadTrayOpen) => set({ uploadTrayOpen }),
}));
