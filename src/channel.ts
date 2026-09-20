export const PATCH_CHANNEL_KEY = "modpack-manager.patch-channel";
export const ROOT_KEY = "modpack-manager.root";
export const DEFAULT_PATCH_CHANNEL = import.meta.env.VITE_DEFAULT_PATCH_CHANNEL ?? "";
export const APP_UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000;

export function patchPollIntervalMs(minutes: number): number {
  const bounded = Math.min(Math.max(minutes || 30, 5), 24 * 60);
  return bounded * 60 * 1000;
}
