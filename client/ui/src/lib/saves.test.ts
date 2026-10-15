import { describe, expect, it } from "vitest";
import { describeFiles } from "./saves.svelte";

describe("describeFiles", () => {
  it("counts files", () => {
    expect(describeFiles(1, "2 KB")).toBe("1 file · 2 KB");
    expect(describeFiles(3, "1.2 MB")).toBe("3 files · 1.2 MB");
  });
});
