import { openUrl } from "@tauri-apps/plugin-opener";

/** Opens an https link in the system browser (the webview never navigates away). */
export function openExternal(event: MouseEvent, url: string) {
  event.preventDefault();
  if (!url.startsWith("https://")) return;
  void openUrl(url);
}
