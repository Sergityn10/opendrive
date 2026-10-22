import { NavLink, Outlet, useNavigate } from "react-router";
import {
  HardDrive,
  LogOut,
  Search,
  Server,
  Settings,
  Trash2,
} from "lucide-react";
import { useAuth } from "@/hooks/useAuth";

/**
 * Estructura de la aplicación: barra lateral y contenido.
 */
export function AppShell() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();

  async function handleLogout() {
    await logout();
    navigate("/login", { replace: true });
  }

  const items = [
    { to: "/drive", label: "Mi Drive", icon: HardDrive, end: true },
    { to: "/trash", label: "Papelera", icon: Trash2, end: false },
    { to: "/settings", label: "Ajustes", icon: Settings, end: false },
    { to: "/deploy", label: "Despliegue", icon: Server, end: false },
  ];

  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col border-r border-[var(--color-border)] bg-[var(--color-surface-alt)] p-3">
        <div className="mb-4 flex items-center gap-2 px-2 pt-1">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-[var(--color-accent)] font-semibold text-white">
            D
          </div>
          <span className="text-base font-medium">OpenDrive</span>
        </div>

        <nav className="flex flex-col gap-0.5">
          {items.map(({ to, label, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={end}
              className={({ isActive }) =>
                [
                  "flex items-center gap-3 rounded-lg px-3 py-2 text-sm",
                  isActive
                    ? "bg-[var(--color-accent)] text-white"
                    : "text-[var(--color-ink)] hover:bg-[var(--color-surface-hover)]",
                ].join(" ")
              }
            >
              <Icon size={18} />
              {label}
            </NavLink>
          ))}

          <NavLink
            to="/search"
            className={({ isActive }) =>
              [
                "flex items-center gap-3 rounded-lg px-3 py-2 text-sm",
                isActive
                  ? "bg-[var(--color-accent)] text-white"
                  : "text-[var(--color-ink)] hover:bg-[var(--color-surface-hover)]",
              ].join(" ")
            }
          >
            <Search size={18} />
            Buscar
          </NavLink>
        </nav>

        <div className="mt-auto border-t border-[var(--color-border)] pt-3">
          <div className="px-3 pb-2">
            <p className="truncate text-sm font-medium">{user?.username}</p>
            <p className="truncate text-xs text-[var(--color-ink-muted)]">
              {user?.email}
            </p>
          </div>
          <button
            type="button"
            onClick={handleLogout}
            className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-[var(--color-ink)] hover:bg-[var(--color-surface-hover)]"
          >
            <LogOut size={18} />
            Cerrar sesión
          </button>
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <Outlet />
      </main>
    </div>
  );
}
