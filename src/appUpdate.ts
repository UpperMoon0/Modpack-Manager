export function shouldNotifyAppUpdate(
  nextVersion: string,
  lastNotifiedVersion: string | null
): boolean {
  const version = nextVersion.trim();
  return version.length > 0 && version !== lastNotifiedVersion;
}
