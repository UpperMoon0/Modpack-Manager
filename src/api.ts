import { invoke } from "@tauri-apps/api/core";
import type { ApplyResult, PatchPlan, PatchState, ResolvedPatchChannel } from "./types";

export const patchApi = {
  channel(source: string) {
    return invoke<ResolvedPatchChannel>("resolve_patch_channel", { source });
  },
  plan(manifestSource: string, root: string) {
    return invoke<PatchPlan>("plan_patch", { manifestSource, root });
  },
  apply(manifestSource: string, root: string) {
    return invoke<ApplyResult>("apply_patch", { manifestSource, root });
  },
  state(root: string) {
    return invoke<PatchState | null>("read_patch_state", { root });
  }
};
