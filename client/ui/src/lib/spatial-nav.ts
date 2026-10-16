// Moving focus by direction, for controllers and arrow keys (docs/ui.md, "Controller").
// The geometry is pure so it can be tested without a page.

export type Direction = "up" | "down" | "left" | "right";

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

const OFF_LINE = 1e6;

/**
 * The box to move to from `from` in direction `dir`, as an index into `boxes`, or -1 when
 * nothing lies that way. Boxes in line with `from` (sharing its column, or its row) come
 * before any off to the side, so moving down a grid stays in its column and moving down
 * the page doesn't jump into the sidebar.
 */
export function pickNext(from: Box, boxes: Box[], dir: Direction): number {
  const vertical = dir === "up" || dir === "down";
  const sign = dir === "down" || dir === "right" ? 1 : -1;
  let best = -1;
  let bestScore = Number.POSITIVE_INFINITY;
  boxes.forEach((b, i) => {
    // How far b lies ahead, edge to edge; it must start past from's far edge (a little
    // overlap is allowed for uneven rows).
    const ahead = vertical
      ? sign > 0
        ? b.top - (from.top + from.height)
        : from.top - (b.top + b.height)
      : sign > 0
        ? b.left - (from.left + from.width)
        : from.left - (b.left + b.width);
    if (ahead < -4) return;
    // How far b is off to the side; 0 when they share a column (or row).
    const side = vertical
      ? gap(from.left, from.left + from.width, b.left, b.left + b.width)
      : gap(from.top, from.top + from.height, b.top, b.top + b.height);
    // Anything in line beats anything off to the side, however near.
    const score = (side > 0 ? OFF_LINE : 0) + Math.max(ahead, 0) + side * 3;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}

/** Distance between two ranges on one axis; 0 when they overlap. */
function gap(a0: number, a1: number, b0: number, b1: number): number {
  if (b1 < a0) return a0 - b1;
  if (b0 > a1) return b0 - a1;
  return 0;
}

const FOCUSABLE =
  "a[href], button:not(:disabled), input:not(:disabled):not([type=hidden]), select:not(:disabled), textarea:not(:disabled), summary, [tabindex]:not([tabindex='-1'])";

/** Elements a controller can reach inside `root`: focusable and on screen. */
export function focusables(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => {
    if (el.closest("[inert], [hidden]")) return false;
    const r = el.getBoundingClientRect();
    // Skips hidden things and ones parked off screen (the "Skip to content" link).
    return (
      r.width > 0 && r.height > 0 && r.right > 0 && getComputedStyle(el).visibility !== "hidden"
    );
  });
}

/** True for fields where arrow keys move the caret or change the value. */
export function usesArrowKeys(el: Element | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.isContentEditable || el instanceof HTMLTextAreaElement) return true;
  if (el instanceof HTMLSelectElement) return true;
  if (el instanceof HTMLInputElement) {
    return !["button", "submit", "reset", "checkbox", "image"].includes(el.type);
  }
  return false;
}
