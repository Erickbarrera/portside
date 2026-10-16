import { describe, expect, it } from "vitest";
import { searchGames, sidebarGames } from "./installed-list";
import type { InstallState, Installed, PortSummary } from "./ipc";

const game = (port_id: string, title: string) => ({ port_id, title }) as Installed;
const downloading: InstallState = { state: "downloading", done: 1, total: 2 };

describe("sidebarGames", () => {
  const installed = {
    b: game("b", "Beta Racer (static recomp)"),
    a: game("a", "Alpha Quest (source port)"),
    c: game("c", "Gamma"),
  };

  it("lists installed games by title, with what each is doing", () => {
    const list = sidebarGames(installed, { c: downloading }, { b: {} }, {});
    expect(list).toEqual([
      { id: "a", title: "Alpha Quest (source port)", name: "Alpha Quest", activity: null },
      { id: "b", title: "Beta Racer (static recomp)", name: "Beta Racer", activity: "playing" },
      { id: "c", title: "Gamma", name: "Gamma", activity: "updating" },
    ]);
  });

  it("includes installs still running, titled from the catalog", () => {
    const summaries = { d: { id: "d", title: "Delta" } as PortSummary };
    const list = sidebarGames({}, { d: downloading, e: downloading }, {}, summaries);
    expect(list).toEqual([
      { id: "d", title: "Delta", name: "Delta", activity: "installing" },
      { id: "e", title: "e", name: "e", activity: "installing" },
    ]);
  });

  it("keeps the note when two ports of one game are installed", () => {
    const list = sidebarGames(
      { ...installed, a2: game("a2", "Alpha Quest (decompilation)") },
      {},
      {},
      {},
    );
    expect(list.map((g) => g.name)).toEqual([
      "Alpha Quest (decompilation)",
      "Alpha Quest (source port)",
      "Beta Racer",
      "Gamma",
    ]);
  });

  it("narrows by the search text, ignoring case", () => {
    const list = sidebarGames(installed, {}, {}, {});
    expect(searchGames(list, "  RACER ").map((g) => g.id)).toEqual(["b"]);
    expect(searchGames(list, "zzz")).toEqual([]);
    expect(searchGames(list, " ")).toBe(list);
  });
});
