import { Link } from "react-router";
import { Server, ExternalLink } from "lucide-react";
import { useAuth } from "@/hooks/useAuth";
import { formatBytes } from "@/lib/format";

/**
 * Ajustes de la cuenta.
 */
export function SettingsPage() {
  const { user } = useAuth();

  if (!user) return null;

  const quota = user.quota_bytes;
  const used = user.used_bytes;
  const percent = quota > 0 ? Math.min(100, (used / quota) * 100) : 0;

  return (
    <div className="flex-1 overflow-y-auto">
      <header className="border-b border-[var(--color-border)] px-4 py-2">
        <h1 className="text-base">Ajustes</h1>
      </header>

      <div className="max-w-2xl space-y-6 p-6">
        <section>
          <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--color-ink-muted)]">
            Cuenta
          </h2>
          <dl className="rounded-lg border border-[var(--color-border)] p-4 text-sm">
            <Row label="Nombre de usuario" value={user.username} />
            <Row label="Correo electrónico" value={user.email} />
            <Row
              label="Rol"
              value={user.role === "admin" ? "Administrador" : "Usuario"}
            />
          </dl>
        </section>

        <section>
          <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--color-ink-muted)]">
            Almacenamiento
          </h2>
          <div className="rounded-lg border border-[var(--color-border)] p-4 text-sm">
            {quota > 0 ? (
              <>
                <p className="mb-2">
                  {formatBytes(used)} de {formatBytes(quota)} utilizados
                </p>
                <div className="h-2 overflow-hidden rounded-full bg-[var(--color-surface-hover)]">
                  <div
                    className="h-full bg-[var(--color-accent)]"
                    style={{ width: `${percent}%` }}
                  />
                </div>
              </>
            ) : (
              <p>
                {formatBytes(used)} utilizados. No hay cuota configurada para esta
                instancia.
              </p>
            )}
          </div>
        </section>

        <section>
          <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--color-ink-muted)]">
            Sincronización
          </h2>
          <div className="rounded-lg border border-[var(--color-border)] p-4 text-sm">
            <p className="mb-2">
              Esta instancia expone un endpoint <code>WebDAV</code> y otro{" "}
              <code>tus</code> para subir archivos. Puedes conectarte con
              <code> rclone</code> o con un cliente de escritorio.
            </p>
            <p className="text-[var(--color-ink-muted)]">
              El cliente de escritorio llega en una fase posterior del proyecto.
            </p>
          </div>
        </section>

        <section>
          <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--color-ink-muted)]">
            Despliegue y Servidores
          </h2>
          <div className="flex items-center justify-between rounded-lg border border-[var(--color-border)] p-4 text-sm">
            <div>
              <div className="flex items-center gap-2">
                <Server size={18} className="text-[var(--color-accent)]" />
                <p className="font-medium text-[var(--color-ink)]">
                  Generador de Docker Compose
                </p>
              </div>
              <p className="mt-1 text-xs text-[var(--color-ink-muted)]">
                Configura y descarga archivos <code>compose.yaml</code> y <code>.env</code> para tu portátil o servidor headless.
              </p>
            </div>
            <Link
              to="/deploy"
              className="btn btn-primary text-xs"
            >
              Abrir Generador
              <ExternalLink size={12} />
            </Link>
          </div>
        </section>
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between border-b border-[var(--color-border)] py-1.5 last:border-0">
      <dt className="text-[var(--color-ink-muted)]">{label}</dt>
      <dd className="font-medium">{value}</dd>
    </div>
  );
}
