import { invoke, isTauri } from "@tauri-apps/api/core";
export const desktop = isTauri();
export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!desktop)
    throw new Error(
      "Open the desktop app with pnpm tauri dev to use this feature.",
    );
  return invoke<T>(name, args);
}
export function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
