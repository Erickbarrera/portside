// Generated cover art for ports without a picture, the same as the desktop app's
// (../client/ui/src/lib/cover.ts; keep them in step): a gradient
// picked from the game's key, so every port of one game shares a look on both.

/** Small stable hash (FNV-1a) of a string. */
export function hash(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** CSS background for a generated cover. */
export function coverBackground(key: string): string {
  const h = hash(key);
  const hue = h % 360;
  const second = (hue + 40 + ((h >>> 9) % 80)) % 360;
  const angle = 120 + ((h >>> 17) % 90);
  return `linear-gradient(${angle}deg, hsl(${hue} 55% 32%), hsl(${second} 60% 18%))`;
}

/** The title without a trailing "(…)" note such as "(source port)". */
export function coverTitle(title: string): string {
  return title.replace(/\s*\([^)]*\)\s*$/, "") || title;
}
