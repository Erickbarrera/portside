import { describe, expect, it } from "vitest";
import { personName, preview, shortTime } from "./chat.svelte";

describe("chat helpers", () => {
  it("names people by display name, else handle", () => {
    expect(personName({ display_name: "Example A", handle: "a.test" })).toBe("Example A");
    expect(personName({ display_name: "  ", handle: "a.test" })).toBe("a.test");
    expect(personName({ display_name: null, handle: "a.test" })).toBe("a.test");
  });

  it("previews invites and deleted messages", () => {
    const base = { id: "1", rev: "1", sender: "did:plc:a", sent_at: "", deleted: false };
    expect(preview(null)).toBe("No messages yet");
    expect(preview({ ...base, text: "hi", invite: null })).toBe("hi");
    expect(preview({ ...base, text: "x", invite: { port_id: "p" } })).toBe("🎮 Game invite");
    expect(preview({ ...base, text: "", deleted: true, invite: null })).toBe("Message deleted");
  });

  it("shows yesterday and dates", () => {
    const now = new Date("2026-10-14T12:00:00");
    expect(shortTime("2026-10-13T09:00:00", now)).toBe("Yesterday");
    expect(shortTime("not a date", now)).toBe("");
    expect(shortTime("2026-10-01T09:00:00", now)).toMatch(/1/);
  });
});
