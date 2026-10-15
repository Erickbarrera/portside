import { commands, setupIpc } from "$lib/ipc";
import type { LayoutLoad } from "./$types";

// SPA inside Tauri: no server rendering, every route falls back to index.html.
export const ssr = false;
export const prerender = false;

export const load: LayoutLoad = async () => {
  await setupIpc();
  return { app: await commands.appInfo() };
};
