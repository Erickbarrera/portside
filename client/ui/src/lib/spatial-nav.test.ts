import { describe, expect, it } from "vitest";
import { type Box, pickNext } from "./spatial-nav";

const box = (left: number, top: number, width = 100, height = 40): Box => ({
  left,
  top,
  width,
  height,
});

describe("pickNext", () => {
  // A 3×2 grid of tiles, 100 wide and 40 tall, with 20 between them.
  const grid = [box(0, 0), box(120, 0), box(240, 0), box(0, 60), box(120, 60), box(240, 60)];

  it("moves along a row and down a column", () => {
    expect(pickNext(grid[0], grid, "right")).toBe(1);
    expect(pickNext(grid[1], grid, "left")).toBe(0);
    expect(pickNext(grid[1], grid, "down")).toBe(4);
    expect(pickNext(grid[5], grid, "up")).toBe(2);
  });

  it("finds nothing past the edge", () => {
    expect(pickNext(grid[2], grid, "right")).toBe(-1);
    expect(pickNext(grid[0], grid, "up")).toBe(-1);
  });

  it("prefers the box in line over a nearer one to the side", () => {
    const from = box(0, 0);
    const nearSide = box(300, 50); // closer vertically, far to the right
    const inLine = box(0, 200);
    expect(pickNext(from, [nearSide, inLine], "down")).toBe(1);
  });

  it("stays in the page rather than jumping into a sidebar", () => {
    const back = box(288, 20, 60, 20);
    const sidebarLink = box(10, 80, 230, 40); // just below, off to the left
    const play = box(300, 320); // far below, in line
    expect(pickNext(back, [sidebarLink, play], "down")).toBe(1);
  });

  it("reaches a box off to the side when nothing is in line", () => {
    const sidebar = box(0, 0, 200, 40);
    const content = box(260, 300);
    expect(pickNext(sidebar, [content], "right")).toBe(0);
  });

  it("skips boxes behind", () => {
    expect(pickNext(grid[4], [grid[0], grid[1]], "down")).toBe(-1);
  });
});
