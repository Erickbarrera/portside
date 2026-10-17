import type { Os, PlayableStatus, PortType } from "./catalog";

// Labels always accompany status colors (status is never shown by color alone).
export const statusLabel: Record<PlayableStatus, string> = {
  perfect: "Perfect",
  playable: "Playable",
  in_progress: "In progress",
  boots: "Boots",
  not_playable: "Not playable",
};

export const osLabel: Record<Os, string> = {
  windows: "Windows",
  macos: "macOS",
  linux: "Linux",
  android: "Android",
};

export const typeLabel: Record<PortType, string> = {
  "static-recomp": "Static recomp",
  "source-port": "Source port",
  "engine-remake": "Engine remake",
  "browser-port": "Browser port",
  decompilation: "Decompilation",
  disassembly: "Disassembly",
  tool: "Tool",
};
