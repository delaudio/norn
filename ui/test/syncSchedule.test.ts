import { expect, test } from "bun:test";
import { syncIntervalMs } from "../src/app/syncSchedule";

test("maps retained sync intervals to milliseconds", () => {
  expect(syncIntervalMs(null)).toBeNull();
  expect(syncIntervalMs(undefined)).toBeNull();
  expect(syncIntervalMs(0)).toBeNull();
  expect(syncIntervalMs(45)).toBeNull();
  expect(syncIntervalMs(30)).toBe(30_000);
  expect(syncIntervalMs(60)).toBe(60_000);
  expect(syncIntervalMs(300)).toBe(300_000);
  expect(syncIntervalMs(600)).toBe(600_000);
});
