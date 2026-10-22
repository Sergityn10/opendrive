import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Combina clases de Tailwind resolviendo los conflictos.
 *
 * `clsx` se encarga de las clases condicionales y `tailwind-merge` de que la
 * última regla gana cuando dos utilidades compiten (por ejemplo, `p-2` y
 * `p-4`).
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
