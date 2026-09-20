export function patchStatus(
  hasResult: boolean,
  hasPlan: boolean,
  alreadyApplied: boolean
): "Patched" | "Up to date" | "Patch available" | "Ready" {
  if (hasResult) return "Patched";
  if (hasPlan && alreadyApplied) return "Up to date";
  if (hasPlan) return "Patch available";
  return "Ready";
}
