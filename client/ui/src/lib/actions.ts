import type { Os, PortDetail_Serialize, PortSummary } from "./ipc";

/** What the game page offers, by docs/specs/catalog.md "Actions shown by type". */
export type PrimaryAction =
  | { kind: "install" }
  | { kind: "play_in_browser"; url: string }
  | { kind: "view_project"; url: string | null };

export function primaryAction(port: PortDetail_Serialize, os: Os): PrimaryAction {
  if (port.manifest?.builds.some((b) => b.os === os)) return { kind: "install" };
  if (port.type === "browser-port" && port.play_url) {
    return { kind: "play_in_browser", url: port.play_url };
  }
  return { kind: "view_project", url: port.repo ?? port.homepage };
}

/** The status shown for this OS: its own row if there is one, else the overall status. */
export function statusFor(port: PortSummary, os: Os) {
  return port.os_status.find((s) => s.os === os)?.status ?? null;
}
