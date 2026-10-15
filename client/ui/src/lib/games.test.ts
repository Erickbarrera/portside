import { describe, expect, it } from "vitest";
import { crashed, formatPlaytime } from "./games.svelte";

const stats = { port_id: "x", total_secs: 0, last_played: null, sessions: 0 };

describe("formatPlaytime", () => {
  it("is null when never played", () => {
    expect(formatPlaytime(0)).toBeNull();
    expect(formatPlaytime(undefined)).toBeNull();
  });
  it("uses minutes under an hour and hours after", () => {
    expect(formatPlaytime(20)).toBe("Less than a minute");
    expect(formatPlaytime(60)).toBe("1 minute");
    expect(formatPlaytime(12 * 60)).toBe("12 minutes");
    expect(formatPlaytime(3600)).toBe("1 hour");
    expect(formatPlaytime(3.46 * 3600)).toBe("3.5 hours");
    expect(formatPlaytime(41.6 * 3600)).toBe("42 hours");
  });
});

describe("crashed", () => {
  it("is a non-zero exit the user didn't ask for", () => {
    expect(crashed({ state: "exited", code: 1, stopped: false, stats })).toBe(true);
    expect(crashed({ state: "exited", code: 0, stopped: false, stats })).toBe(false);
    expect(crashed({ state: "exited", code: null, stopped: true, stats })).toBe(false);
    expect(crashed(undefined)).toBe(false);
  });
});
