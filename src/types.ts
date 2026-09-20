export interface PlanItem {
  kind: string;
  path: string;
  detail: string;
}

export interface PatchPlan {
  manifestId: string;
  manifestName: string;
  manifestVersion: string;
  target: "client" | "server";
  root: string;
  alreadyApplied: boolean;
  items: PlanItem[];
  warnings: string[];
}

export interface PatchState {
  manifestId: string;
  manifestVersion: string;
  target: "client" | "server";
  manifestSource: string;
  appliedAt: string;
}

export interface ApplyResult {
  state: PatchState;
  backupPath: string | null;
  changedPaths: number;
}

export interface PatchProgress {
  phase: "loading" | "downloading" | "backingUp" | "applying" | "rollback" | "done";
  message: string;
  current: number;
  total: number;
}

export interface ResolvedPatchChannel {
  id: string;
  name: string;
  checkIntervalMinutes: number;
  version: string;
  manifestSource: string;
  notes: string;
  publishedAt: string | null;
}
