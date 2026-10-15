import { describe, expect, it } from "vitest";
import { subForums, threadHref } from "./boards";

describe("boards", () => {
  it("keeps the thread URI in one path segment", () => {
    const href = threadHref("at://did:plc:a/app.bsky.feed.post/3k");
    expect(href).toBe("/thread/at%3A%2F%2Fdid%3Aplc%3Aa%2Fapp.bsky.feed.post%2F3k");
    expect(decodeURIComponent(href.slice("/thread/".length))).toBe(
      "at://did:plc:a/app.bsky.feed.post/3k",
    );
  });

  it("lists the five sub-forums", () => {
    expect(subForums).toEqual(["general", "help", "bugs", "mods", "multiplayer"]);
  });
});
