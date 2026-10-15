// The only way the UI talks to Rust. Types and commands are generated (bindings.ts);
// this file adds the browser-only mock and small helpers.
import type { CommandError } from "./bindings";

export * from "./bindings";

export type Outcome<T> = { status: "ok"; data: T } | { status: "error"; error: CommandError };

/** True inside the Tauri webview. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

let ready: Promise<void> | null = null;

/**
 * In a plain browser (`pnpm --filter @portside/ui dev`), answer commands from synthetic
 * fixtures. Never bundled into release builds.
 */
export function setupIpc(): Promise<void> {
  if (!ready) {
    ready =
      import.meta.env.DEV && !inTauri()
        ? import("../mock/ipc").then((m) => m.installMockIpc())
        : Promise.resolve();
  }
  return ready;
}
