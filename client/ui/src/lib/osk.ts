// Portside's on-screen keyboard (couch mode, docs/ui.md): its keys, and what a key does to
// a text field. Pure, so it can be tested without a page.

/** Rows of character keys; Shift makes letters capitals. */
export const ROWS = ["1234567890", "qwertyuiop", "asdfghjkl@", "zxcvbnm.-_"];

export interface Field {
  value: string;
  start: number;
  end: number;
}

/** Types `text` over the selection. */
export function typeText(f: Field, text: string): Field {
  const value = f.value.slice(0, f.start) + text + f.value.slice(f.end);
  const at = f.start + text.length;
  return { value, start: at, end: at };
}

/** Deletes the selection, else the character before the caret. */
export function backspace(f: Field): Field {
  if (f.start !== f.end) return typeText(f, "");
  if (f.start === 0) return f;
  return typeText({ ...f, start: f.start - 1 }, "");
}
