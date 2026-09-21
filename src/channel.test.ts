import { describe, expect, it } from "vitest";
import { patchPollIntervalMs } from "./channel";

describe("patchPollIntervalMs", () => {
  it("uses the channel interval", () => {
    expect(patchPollIntervalMs(30)).toBe(30 * 60 * 1000);
  });

  it("bounds unreasonable remote values", () => {
    expect(patchPollIntervalMs(1)).toBe(5 * 60 * 1000);
    expect(patchPollIntervalMs(99999)).toBe(24 * 60 * 60 * 1000);
  });
});
