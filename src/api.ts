import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyResult,
  PatchPlan,
  PatchState,
  ResolvedPatchChannel,
  TfgPlanResponse,
  FreshInstallPlan,
  FreshInstallResult
} from "./types";

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

export const tfgApi = {
  openFolder(root: string) {
    return invoke<void>("open_folder", { root });
  },
  installPlan(destination: string) {
    return invoke<FreshInstallPlan>("plan_tfg_install", { destination });
  },
  install(destination: string, expectedPatchVersion: string) {
    return invoke<FreshInstallResult>("install_tfg", { destination, expectedPatchVersion });
  },
  plan(root: string) {
    return invoke<TfgPlanResponse>("plan_tfg_patch", { root });
  },
  apply(root: string, expectedPatchVersion: string) {
    return invoke<ApplyResult>("apply_tfg_patch", { root, expectedPatchVersion });
  },
  state(root: string) {
    return patchApi.state(root);
  }
};
