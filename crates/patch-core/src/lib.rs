mod channel;
mod engine;
mod schema;
mod source;

pub use channel::{load_patch_channel, PatchChannel, PatchChannelRelease, ResolvedPatchChannel};
pub use engine::{
    apply_manifest, load_manifest, plan_manifest, read_state, ApplyResult, PatchPlan, PatchProgress,
    PatchProgressPhase, PatchState, PlanItem, ProgressCallback,
};
pub use schema::{Artifact, Operation, PatchManifest, Target};
