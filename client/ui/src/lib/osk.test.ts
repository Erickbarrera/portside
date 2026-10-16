import { describe, expect, it } from "vitest";
import { couchOn } from "./couch.svelte";
import { backspace, typeText } from "./osk";

describe("on-screen keyboard", () => {
  it("types at the caret and over a selection", () => {
    expect(typeText({ value: "ab", start: 1, end: 1 }, "x")).toEqual({
      value: "axb",
      start: 2,
      end: 2,
    });
    expect(typeText({ value: "abcd", start: 1, end: 3 }, "x")).toEqual({
      value: "axd",
      start: 2,
      end: 2,
    });
  });

  it("deletes the selection, else the character before the caret", () => {
    expect(backspace({ value: "abc", start: 3, end: 3 })).toEqual({
      value: "ab",
      start: 2,
      end: 2,
    });
    expect(backspace({ value: "abcd", start: 1, end: 3 })).toEqual({
      value: "ad",
      start: 1,
      end: 1,
    });
    expect(backspace({ value: "abc", start: 0, end: 0 })).toEqual({
      value: "abc",
      start: 0,
      end: 0,
    });
  });
});

describe("couchOn", () => {
  it("follows the screen on auto and the choice otherwise", () => {
    expect(couchOn("auto", true)).toBe(true);
    expect(couchOn("auto", false)).toBe(false);
    expect(couchOn("on", false)).toBe(true);
    expect(couchOn("off", true)).toBe(false);
  });
});
