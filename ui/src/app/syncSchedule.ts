// Automatic refresh scheduling. The desktop settings model exposes a single
// `automaticSyncIntervalSeconds`; the retained intervals are Off plus 30s, 60s,
// 5m and 10m. Refresh is a bounded read-only metadata update, never a Git
// fetch/pull or an agent review.

export const SYNC_INTERVAL_SECONDS = [30, 60, 300, 600] as const;

/// Milliseconds between refreshes, or null when refresh is off or the value is
/// not one of the retained intervals.
export function syncIntervalMs(seconds: number | null | undefined): number | null {
  if (seconds === null || seconds === undefined) {
    return null;
  }
  return (SYNC_INTERVAL_SECONDS as readonly number[]).includes(seconds) ? seconds * 1000 : null;
}
