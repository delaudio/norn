import { expect, jest, test } from "bun:test";
import { startSyncScheduler } from "../src/app/syncScheduler";

test("off starts no timer", () => {
  jest.useFakeTimers();
  try {
    let calls = 0;
    const stop = startSyncScheduler(null, () => {
      calls += 1;
    });
    jest.advanceTimersByTime(600_000);
    expect(calls).toBe(0);
    stop();
  } finally {
    jest.useRealTimers();
  }
});

test("each interval fires and stop clears it", () => {
  jest.useFakeTimers();
  try {
    let calls = 0;
    const stop = startSyncScheduler(30_000, () => {
      calls += 1;
    });
    jest.advanceTimersByTime(30_000);
    expect(calls).toBe(1);
    jest.advanceTimersByTime(60_000);
    expect(calls).toBe(3);
    stop();
    jest.advanceTimersByTime(300_000);
    expect(calls).toBe(3);
  } finally {
    jest.useRealTimers();
  }
});
