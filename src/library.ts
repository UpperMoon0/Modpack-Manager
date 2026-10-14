import type { PatchState } from "./types";
import { ROOT_KEY } from "./channel";

export const PACKS_KEY = "modpack-manager.packs";
export const ACTIVITY_KEY = "modpack-manager.activity";
export interface SavedPack { root: string; version: string | null; appliedAt: string | null }
export interface ActivityEntry {
  id: string; root: string; title: string; status: "success" | "error";
  at: string; detail: string; backupPath?: string | null;
}
export function rootKey(root: string) {
  const normalized = root.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  return /^[a-z]:\//i.test(normalized) || normalized.startsWith("//")
    ? normalized.toLowerCase() : normalized;
}
export function readLibrary(storage: Pick<Storage, "getItem">): SavedPack[] {
  let packs: SavedPack[] = [];
  try {
    const value: unknown = JSON.parse(storage.getItem(PACKS_KEY) ?? "[]");
    if (Array.isArray(value)) {
      for (const item of value) {
        if (item && typeof item.root === "string" && item.root.trim()
          && !packs.some((pack) => rootKey(pack.root) === rootKey(item.root))) {
          packs.push({ root: item.root, version: typeof item.version === "string" ? item.version : null,
            appliedAt: typeof item.appliedAt === "string" ? item.appliedAt : null });
        }
      }
    }
  } catch { /* Preserve the old selected folder when migrating corrupt metadata. */ }
  const previous = storage.getItem(ROOT_KEY);
  if (previous?.trim() && !packs.some((pack) => rootKey(pack.root) === rootKey(previous))) {
    packs = [...packs, { root: previous, version: null, appliedAt: null }];
  }
  return packs;
}
export function rememberPack(packs: SavedPack[], root: string, state: PatchState | null, replaceRoot?: string) {
  const existing = packs.find((pack) => rootKey(pack.root) === rootKey(root));
  const next = { root, version: state?.manifestVersion ?? null,
    appliedAt: state?.appliedAt ?? null };
  const retained = packs.filter((pack) => !replaceRoot || rootKey(pack.root) !== rootKey(replaceRoot) || rootKey(pack.root) === rootKey(root));
  return existing ? retained.map((pack) => rootKey(pack.root) === rootKey(root) ? next : pack) : [...retained, next];
}
export function readActivity(storage: Pick<Storage, "getItem">): ActivityEntry[] {
  try {
    const value: unknown = JSON.parse(storage.getItem(ACTIVITY_KEY) ?? "[]");
    if (Array.isArray(value)) return value.filter((item): item is ActivityEntry =>
      item && typeof item.id === "string" && typeof item.root === "string"
      && typeof item.title === "string" && typeof item.detail === "string"
      && typeof item.at === "string" && ["success", "error"].includes(item.status)
      && (item.backupPath == null || typeof item.backupPath === "string")).slice(0, 200);
  } catch { /* A malformed log should not prevent opening the manager. */ }
  return [];
}
