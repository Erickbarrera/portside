import { describe, expect, it } from "vitest";
import { primaryAction, statusFor } from "./actions";
import { portDetail } from "./mock/fixtures";

describe("primaryAction", () => {
  it("offers install when the manifest has a build for this OS", () => {
    const port = portDetail("example-source-port");
    expect(port && primaryAction(port, "linux")).toEqual({ kind: "install" });
  });

  it("offers play in browser for browser ports", () => {
    const port = portDetail("example-browser-port");
    expect(port && primaryAction(port, "windows")).toEqual({
      kind: "play_in_browser",
      url: "https://example.com/play",
    });
  });

  it("falls back to the project page", () => {
    const port = portDetail("example-decomp");
    expect(port && primaryAction(port, "macos")).toEqual({
      kind: "view_project",
      url: "https://github.com/example/example-decomp",
    });
  });
});

describe("statusFor", () => {
  it("returns the OS row or null", () => {
    const port = portDetail("example-static-recomp");
    expect(port && statusFor(port, "macos")).toBe("in_progress");
    expect(port && statusFor(port, "android")).toBeNull();
  });
});

describe("formatAgo", () => {
  it("rounds to minutes and hours", async () => {
    const { formatAgo } = await import("./labels");
    const now = 1_800_000_000_000;
    expect(formatAgo(now / 1000 - 10, now)).toBe("just now");
    expect(formatAgo(now / 1000 - 60, now)).toBe("1 minute ago");
    expect(formatAgo(now / 1000 - 3 * 3600, now)).toBe("3 hours ago");
  });
});
