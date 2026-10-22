import { createContext } from "react";
import type { UserProfile } from "@/lib/types";

/**
 * Forma del valor que proporciona el contexto de autenticación.
 */
export interface AuthContextValue {
  /** Perfil del usuario autenticado, o `null` si no hay sesión. */
  user: UserProfile | null;
  /** `true` mientras se comprueba la sesión inicial. */
  loading: boolean;
  /** Inicia sesión con correo o nombre de usuario. */
  login: (identifier: string, password: string) => Promise<void>;
  /** Registra un usuario nuevo e inicia sesión. */
  register: (email: string, username: string, password: string) => Promise<void>;
  /** Cierra la sesión. */
  logout: () => Promise<void>;
}

/**
 * Contexto de autenticación.
 *
 * Vive en su propio módulo para que el proveedor y el hook puedan estar en
 * archivos separados: así la regla de fast refresh de ESLint no se queja de
 * que un archivo exporte a la vez un componente y una función.
 */
export const AuthContext = createContext<AuthContextValue | null>(null);
