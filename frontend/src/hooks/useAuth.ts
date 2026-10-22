import { useContext } from "react";
import { AuthContext, type AuthContextValue } from "@/auth/authContext";

/**
 * Devuelve el contexto de autenticación.
 *
 * @throws Si se usa fuera de `<AuthProvider>`.
 */
export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth debe usarse dentro de <AuthProvider>");
  }
  return context;
}
