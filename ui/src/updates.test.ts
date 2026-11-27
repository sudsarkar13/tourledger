import assert from "node:assert/strict";
import { test } from "node:test";
import { describe, downloadsByItself, percent, type UpdateStatus } from "./updateSteps.ts";

const status = (patch: Partial<UpdateStatus>): UpdateStatus => ({
  supported: true,
  current: "0.1.2",
  phase: "idle",
  version: "0.1.3",
  notes: null,
  received: 0,
  total: null,
  error: null,
  checked_ms: null,
  metered: false,
  ...patch,
});

test("a found update downloads by itself only when allowed and free to", () => {
  const found = status({ phase: "available" });
  assert.equal(downloadsByItself(found, true), true);
  assert.equal(downloadsByItself(found, false), false);
  assert.equal(downloadsByItself({ ...found, metered: true }, true), false);
  assert.equal(downloadsByItself(status({ phase: "ready" }), true), false);
});

test("progress stays within 0 and 100, and admits an unknown size", () => {
  assert.equal(percent(status({ received: 5 })), null);
  assert.equal(percent(status({ received: 1, total: 3 })), 33);
  assert.equal(percent(status({ received: 9, total: 3 })), 100);
});

test("a failure shows its own reason", () => {
  assert.equal(describe(status({ phase: "failed", error: "No network." })), "No network.");
  assert.match(describe(status({ phase: "ready" })), /0\.1\.3 is ready.*entries stay/);
  assert.match(describe(status({ phase: "ready", error: "Read-only disk." })), /could not be installed: Read-only disk\. You/);
  assert.match(describe(status({ phase: "ready", error: "older than 1002" })), /older than 1002\. You/);
});
