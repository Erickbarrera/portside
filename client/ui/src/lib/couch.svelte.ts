// Couch mode (docs/ui.md): a bigger, full-screen layout for the Steam Deck and TVs, with
// Portside's own on-screen keyboard. "Auto" turns it on in Steam's Game Mode or Big Picture.
import { inTauri } from "./ipc";

export type CouchSetting = "auto" | "on" | "off";

const KEY = "portside.couch";

function saved(): CouchSetting {
  try {
    const v = localStorage.getItem(KEY);
    return v === "on" || v === "off" ? v : "auto";
  } catch {
    return "auto";
  }
}

let setting = $state<CouchSetting>(saved());
let bigScreen = $state(false);

/** Whether couch mode is on for this setting and screen. */
export function couchOn(choice: CouchSetting, onBigScreen: boolean): boolean {
  return choice === "on" || (choice === "auto" && onBigScreen);
}

export const couch = {
  get setting() {
    return setting;
  },
  get on() {
    return couchOn(setting, bigScreen);
  },
  /** Called once with what Rust knows about the screen. */
  detect(onBigScreen: boolean) {
    bigScreen = onBigScreen;
  },
  set(choice: CouchSetting) {
    setting = choice;
    try {
      if (choice === "auto") localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, choice);
    } catch {
      // Not saved; it still applies until Portside closes.
    }
  },
};

/** Applies couch mode to the page and the window. */
export async function applyCouch(on: boolean) {
  if (on) document.documentElement.dataset.couch = "";
  else delete document.documentElement.dataset.couch;
  if (!inTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow()
    .setFullscreen(on)
    .catch(() => {
      // Some window managers refuse; the bigger layout still applies.
    });
}
