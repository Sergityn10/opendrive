import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError, auth } from "@/lib/api";
import { AuthContext, type AuthContextValue } from "@/auth/authContext";
import type { UserProfile } from "@/lib/types";

/**
 * Proveedor de autenticación.
 *
 * Al arrancar comprueba si hay una sesión válida llamando a `/auth/me`. Si el
 * servidor responde 401, se considera que no hay sesión.
 */
export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<UserProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const queryClient = useQueryClient();

  useEffect(() => {
    let cancelled = false;

    auth
      .me()
      .then((profile) => {
        if (!cancelled) setUser(profile);
      })
      .catch((error: unknown) => {
        if (!cancelled && !(error instanceof ApiError && error.isUnauthenticated)) {
          console.error("No se pudo comprobar la sesión", error);
        }
        if (!cancelled) setUser(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const value = useMemo<AuthContextValue>(
    () => ({
      user,
      loading,
      async login(identifier, password) {
        const result = await auth.login(identifier, password);
        setUser(result.user);
        await queryClient.invalidateQueries();
      },
      async register(email, username, password) {
        const result = await auth.register(email, username, password);
        setUser(result.user);
        await queryClient.invalidateQueries();
      },
      async logout() {
        await auth.logout();
        setUser(null);
        queryClient.clear();
      },
    }),
    [user, loading, queryClient],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
