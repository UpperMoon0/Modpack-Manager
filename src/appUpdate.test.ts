import { describe, expect, it } from "vitest";
import { shouldNotifyAppUpdate } from "./appUpdate";

describe("shouldNotifyAppUpdate", () => {
  it("notifies once for a newly discovered version", () => {
    expect(shouldNotifyAppUpdate("1.0.3", null)).toBe(true);
    expect(shouldNotifyAppUpdate("1.0.3", "1.0.2")).toBe(true);
  });

  it("does not repeatedly notify for the same version", () => {
    expect(shouldNotifyAppUpdate("1.0.3", "1.0.3")).toBe(false);
  });

  it("ignores an empty updater version", () => {
    expect(shouldNotifyAppUpdate("   ", null)).toBe(false);
  });
});
