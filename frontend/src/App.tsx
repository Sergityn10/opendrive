import { Navigate, Route, Routes } from "react-router";
import { useAuth } from "@/hooks/useAuth";
import { LoginPage } from "@/pages/LoginPage";
import { DrivePage } from "@/pages/DrivePage";
import { TrashPage } from "@/pages/TrashPage";
import { SearchPage } from "@/pages/SearchPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { AppShell } from "@/components/AppShell";

/**
 * Raíz de la aplicación: rutas y control de acceso.
 */
export function App() {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-ink-muted)]">
        Cargando…
      </div>
    );
  }

  if (!user) {
    return (
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="*" element={<Navigate to="/login" replace />} />
      </Routes>
    );
  }

  return (
    <Routes>
      <Route path="/login" element={<Navigate to="/drive" replace />} />
      <Route element={<AppShell />}>
        <Route path="/drive" element={<DrivePage />} />
        <Route path="/drive/:folderId" element={<DrivePage />} />
        <Route path="/trash" element={<TrashPage />} />
        <Route path="/search" element={<SearchPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/drive" replace />} />
    </Routes>
  );
}
