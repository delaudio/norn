// Interval scheduler for automatic refresh. Extracted so it can be tested with
// fake timers: a null interval (off) starts nothing, and the returned stop
// function clears the timer on interval/settings change or unmount.

export function startSyncScheduler(intervalMs: number | null, run: () => void): () => void {
  if (intervalMs === null) {
    return () => {};
  }
  const id = setInterval(run, intervalMs);
  return () => clearInterval(id);
}
