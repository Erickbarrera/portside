import { describe, expect, it } from "vitest";
import { coverBackground, coverTitle, hash } from "./cover";

describe("coverBackground", () => {
  it("is stable for a key and differs between keys", () => {
    expect(coverBackground("example-racer")).toBe(coverBackground("example-racer"));
    expect(coverBackground("example-racer")).not.toBe(coverBackground("example-platformer"));
    expect(hash("")).toBe(0x811c9dc5);
  });
});

describe("coverTitle", () => {
  it("drops a trailing note", () => {
    expect(coverTitle("Example Racer (static recomp)")).toBe("Example Racer");
    expect(coverTitle("Example (Deluxe) Kart")).toBe("Example (Deluxe) Kart");
    expect(coverTitle("(only a note)")).toBe("(only a note)");
  });
});
