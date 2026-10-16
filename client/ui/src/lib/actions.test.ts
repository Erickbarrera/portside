import { describe, expect, it } from "vitest";
import { primaryAction, statusFor } from "./actions";
import { portDetail } from "./mock/fixtures";

describe("primaryAction", () => {
  it("offers install when the manifest has a build for this OS", () => {
    const port = portDetail("example-source-port");
    expect(port && primaryAction(port, "linux", "x86_64")).toEqual({ kind: "install" });
  });

  it("offers install on x86-64 Linux when there's only a Windows build (Proton)", () => {
    const port = portDetail("example-source-port");
    if (!port?.manifest) throw new Error("fixture has no manifest");
    const windowsOnly = {
      ...port,
      manifest: {
        ...port.manifest,
        builds: port.manifest.builds.filter((b) => b.os === "windows"),
      },
    };
    expect(primaryAction(windowsOnly, "linux", "x86_64")).toEqual({ kind: "install" });
    expect(primaryAction(windowsOnly, "linux", "arm64").kind).toBe("view_project");
    expect(primaryAction(windowsOnly, "macos", "arm64").kind).toBe("view_project");
  });

  it("offers play in browser for browser ports", () => {
    const port = portDetail("example-browser-port");
    expect(port && primaryAction(port, "windows", "x86_64")).toEqual({
      kind: "play_in_browser",
      url: "https://example.com/play",
    });
  });

  it("falls back to the project page", () => {
    const port = portDetail("example-decomp");
    expect(port && primaryAction(port, "macos", "arm64")).toEqual({
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
