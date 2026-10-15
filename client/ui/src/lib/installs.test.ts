import { describe, expect, it } from "vitest";
import { fraction } from "./installs.svelte";

describe("fraction", () => {
  it("is the downloaded share, capped at 1", () => {
    expect(fraction({ state: "downloading", done: 25, total: 100 })).toBe(0.25);
    expect(fraction({ state: "downloading", done: 120, total: 100 })).toBe(1);
  });
  it("is unknown without a size or outside downloading", () => {
    expect(fraction({ state: "downloading", done: 25, total: null })).toBeNull();
    expect(fraction({ state: "unpacking" })).toBeNull();
  });
});
