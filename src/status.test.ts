import { describe, expect, it } from "vitest";
import { patchStatus } from "./status";

describe("patchStatus", () => {
  it("prioritizes a completed apply", () => {
    expect(patchStatus(true, true, false)).toBe("Patched");
  });

  it("distinguishes current and pending plans", () => {
    expect(patchStatus(false, true, true)).toBe("Up to date");
    expect(patchStatus(false, true, false)).toBe("Patch available");
  });

  it("is ready before a plan exists", () => {
    expect(patchStatus(false, false, false)).toBe("Ready");
  });
});
