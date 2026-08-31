import assert from "node:assert/strict";
import test from "node:test";

import { validateReadinessReport } from "./validate-readiness-report.mjs";

function report(status, overrides = {}) {
  return {
    schemaVersion: "norn.readiness.v1",
    status,
    issues: [],
    ...overrides,
  };
}

test("accepts each documented readiness status only with its matching exit code", () => {
  assert.equal(validateReadinessReport(report("ok"), 0), "ok");
  assert.equal(validateReadinessReport(report("warn"), 1), "warn");
  assert.equal(validateReadinessReport(report("fail"), 2), "fail");
});

test("rejects mismatched status, unexpected exits, and malformed reports", () => {
  assert.throws(() => validateReadinessReport(report("fail"), 0), /do not match/);
  assert.throws(() => validateReadinessReport(report("ok"), 124), /unexpected exit/);
  assert.throws(
    () => validateReadinessReport(report("ok", { issues: undefined }), 0),
    /expected schema/,
  );
});
