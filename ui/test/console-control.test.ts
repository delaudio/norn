import { expect, test } from "bun:test";
import { createConsoleControl } from "../src/app/consoleControl";

test("UI console protection reasserts after suspension and releases exactly once", () => {
  const calls: (boolean | string)[] = [];
  const control = createConsoleControl(
    (ignored) => calls.push(ignored),
    () => calls.push("released"),
  );
  control.protect();
  control.protect();
  expect(calls).toEqual([true, true]);
  control.close();
  control.close();
  expect(calls).toEqual([true, true, false, "released"]);
  expect(() => control.protect()).toThrow("closed");
});

test("native cleanup failure still releases the UI binding and cannot repeat", () => {
  let releases = 0;
  const control = createConsoleControl(
    (ignored) => {
      if (!ignored) throw new Error("owned native failure");
    },
    () => releases++,
  );
  control.protect();
  expect(() => control.close()).toThrow("owned native failure");
  expect(releases).toBe(1);
  control.close();
  expect(releases).toBe(1);
});
