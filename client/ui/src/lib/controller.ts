// Moving around the app with a controller or the arrow keys (docs/ui.md, "Controller").
// Controllers are read in Rust (any OS) and arrive as `gamepad-input` events.
import { goto } from "$app/navigation";
import { events, type PadAction } from "./ipc";
import { type Direction, focusables, pickNext, usesArrowKeys } from "./spatial-nav";

/** How far one step of the right stick, or an arrow with nothing to move to, scrolls. */
const SCROLL_STEP = 120;

/** The open modal dialog, which keeps focus inside it, else the whole page. */
function scope(): ParentNode {
  const dialogs = document.querySelectorAll<HTMLDialogElement>("dialog[open]");
  return dialogs[dialogs.length - 1] ?? document;
}

/** The page's main scrolling area. */
function main(): HTMLElement | null {
  return document.getElementById("content");
}

/** Shows focus rings while a controller or the keyboard is in use, hides them on a click. */
function usingPad() {
  document.documentElement.dataset.input = "pad";
}

function focus(el: HTMLElement) {
  el.focus({ preventScroll: true });
  el.scrollIntoView({ block: "nearest", inline: "nearest" });
}

function move(dir: Direction) {
  const root = scope();
  const all = focusables(root);
  const current = document.activeElement;
  if (!(current instanceof HTMLElement) || !all.includes(current)) {
    // Nothing focused yet: start in the page content, past any search field (typing comes
    // second with a controller), else the first thing there is.
    const content = all.filter((el) => main()?.contains(el));
    const start = content.find((el) => !usesArrowKeys(el)) ?? content[0] ?? all[0];
    if (start) focus(start);
    return;
  }
  const others = all.filter((el) => el !== current);
  const next = pickNext(
    current.getBoundingClientRect(),
    others.map((el) => el.getBoundingClientRect()),
    dir,
  );
  if (next >= 0) focus(others[next]);
  else if (root === document && (dir === "up" || dir === "down")) scroll(dir === "up" ? -1 : 1);
}

function scroll(sign: number) {
  main()?.scrollBy({ top: sign * SCROLL_STEP });
}

function back() {
  const root = scope();
  if (root instanceof HTMLDialogElement) {
    // Same as Escape: the dialog's own cancel handling runs.
    if (root.dispatchEvent(new Event("cancel", { cancelable: true }))) root.close();
    return;
  }
  const el = document.activeElement;
  if (el instanceof HTMLElement && usesArrowKeys(el)) {
    el.blur();
    return;
  }
  history.back();
}

function section(sections: string[], step: number) {
  const path = location.pathname;
  const at = sections.findIndex((s) => path === s || path.startsWith(`${s}/`));
  const next = (at + step + sections.length) % sections.length;
  void goto(sections[at < 0 && step < 0 ? sections.length - 1 : next]);
}

/** Runs one controller action. `sections` are the sidebar's pages, in order. */
export function act(action: PadAction, sections: string[]) {
  usingPad();
  switch (action) {
    case "up":
    case "down":
    case "left":
    case "right":
      move(action);
      break;
    case "accept": {
      const el = document.activeElement;
      if (el instanceof HTMLElement && el !== document.body) el.click();
      else move("down");
      break;
    }
    case "back":
      back();
      break;
    case "prev_section":
      section(sections, -1);
      break;
    case "next_section":
      section(sections, 1);
      break;
    case "scroll_up":
      scroll(-1);
      break;
    case "scroll_down":
      scroll(1);
      break;
  }
}

const ARROWS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};

/** Follows controllers and arrow keys until the returned function is called. */
export function startController(sections: string[]): () => void {
  const unlisten = events.gamepadInput.listen(({ payload }) => {
    // Controllers are read even while a game has the screen; only act when we have it.
    if (document.hasFocus()) act(payload.action, sections);
  });
  const onKey = (e: KeyboardEvent) => {
    const dir = ARROWS[e.key];
    if (!dir || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || e.defaultPrevented) return;
    const field = document.activeElement;
    // Fields keep the arrows they use; up and down still leave a one-line text field.
    const oneLine =
      field instanceof HTMLInputElement &&
      ["text", "search", "email", "url", "password", "tel"].includes(field.type) &&
      (dir === "up" || dir === "down");
    if (usesArrowKeys(field) && !oneLine) return;
    e.preventDefault();
    usingPad();
    move(dir);
  };
  const onPointer = () => {
    delete document.documentElement.dataset.input;
  };
  window.addEventListener("keydown", onKey);
  window.addEventListener("pointerdown", onPointer);
  return () => {
    void unlisten.then((fn) => fn());
    window.removeEventListener("keydown", onKey);
    window.removeEventListener("pointerdown", onPointer);
  };
}
