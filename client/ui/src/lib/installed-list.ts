// The sidebar's list of installed games (Steam-style): installed games and installs still
// running, in title order.
import { coverTitle } from "./cover";
import type { InstallState, Installed, PortSummary } from "./ipc";

export type SidebarGame = {
  id: string;
  title: string;
  /** The title without its "(…)" note, unless another listed port of the game needs it. */
  name: string;
  /** What the row says next to the title; null for a plain installed game. */
  activity: "installing" | "updating" | "playing" | null;
};

export function sidebarGames(
  installed: Record<string, Installed>,
  active: Record<string, InstallState>,
  running: Record<string, unknown>,
  summaries: Record<string, PortSummary>,
): SidebarGame[] {
  const ids = new Set([...Object.keys(installed), ...Object.keys(active)]);
  const list = [...ids].map((id): SidebarGame => {
    const title = installed[id]?.title ?? summaries[id]?.title ?? id;
    let activity: SidebarGame["activity"] = null;
    if (!(id in installed)) activity = "installing";
    else if (id in active) activity = "updating";
    else if (id in running) activity = "playing";
    return { id, title, name: coverTitle(title), activity };
  });
  const seen = new Map<string, number>();
  for (const g of list) seen.set(g.name, (seen.get(g.name) ?? 0) + 1);
  for (const g of list) if ((seen.get(g.name) ?? 0) > 1) g.name = g.title;
  return list.sort((a, b) => a.name.localeCompare(b.name) || a.title.localeCompare(b.title));
}

/** The games whose title contains the search text, ignoring case. */
export function searchGames(list: SidebarGame[], query: string): SidebarGame[] {
  const q = query.trim().toLowerCase();
  return q ? list.filter((g) => g.title.toLowerCase().includes(q)) : list;
}
