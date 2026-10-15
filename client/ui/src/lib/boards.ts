// Labels and links for discussion boards (docs/specs/boards.md).
import type { SiteBoard, SubForum } from "./ipc";

export const subForumLabel: Record<SubForum, string> = {
  general: "General",
  help: "Help",
  bugs: "Bugs",
  mods: "Mods",
  multiplayer: "Multiplayer",
};

export const subForums = Object.keys(subForumLabel) as SubForum[];

/** App route of a thread; the at:// URI is encoded into one path segment. */
export function threadHref(uri: string): string {
  return `/thread/${encodeURIComponent(uri)}`;
}

export const siteBoardLabel: Record<SiteBoard, string> = {
  help: "Portside help",
  features: "Feature requests",
};

export const siteBoards = Object.keys(siteBoardLabel) as SiteBoard[];
