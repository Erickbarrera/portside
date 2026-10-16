// Saved catalog summaries (screenshots, game keys) for games in the library, shared by the
// sidebar and the Library page. Comes from the local catalog, so it works offline.
import { type PortSummary, commands } from "./ipc";

const byId = $state<Record<string, PortSummary>>({});
const asked = new Set<string>();

export const summaries = {
  get byId() {
    return byId;
  },
  /** Loads the summaries not asked for yet. */
  want(ids: string[]) {
    const missing = ids.filter((id) => !asked.has(id));
    if (missing.length === 0) return;
    for (const id of missing) asked.add(id);
    void commands.catalogSummaries(missing).then((result) => {
      if (result.status !== "ok") {
        for (const id of missing) asked.delete(id);
        return;
      }
      for (const s of result.data) byId[s.id] = s;
    });
  },
};
