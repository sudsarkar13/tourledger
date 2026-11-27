import assert from "node:assert/strict";
import { test } from "node:test";
import { dueNow, momentOf, numberOf, plan, pruned, toSchedule, wording, type Reminder } from "./reminderPlan.ts";

const reminder = (kind: Reminder["kind"], on: string, due = "2026-12-10"): Reminder => ({
  key: `loan:3:${kind}`,
  loan_id: "loan",
  name: "Car loan",
  number: 3,
  due,
  amount: 1340994,
  lent: false,
  on,
  kind,
});
const all = [reminder("before", "2026-12-07"), reminder("due", "2026-12-10"), reminder("missed", "2026-12-11")];

test("amounts appear only when asked for", () => {
  assert.equal(wording(all[1], false).body, "Due today.");
  assert.match(wording(all[1], true).body, /₹13,409\.94 due today\./);
  assert.doesNotMatch(JSON.stringify(wording(all[2], false)), /₹/);
  assert.match(wording({ ...all[1], lent: true }, false).title, /repayment to you due today/);
});

test("only what is still to come is scheduled", () => {
  const now = new Date(2026, 11, 8, 10);
  assert.deepEqual(toSchedule(all, 9, now).map((r) => r.kind), ["due", "missed"]);
  assert.equal(momentOf("2026-12-10", 9).getHours(), 9);
});

test("a passed reminder shows once, and never when it has gone stale", () => {
  const early = new Date(2026, 11, 8, 10);
  assert.deepEqual(dueNow(all, 9, early, {}).map((r) => r.kind), ["before"]);
  // Already shown, or delivered by the system at its time.
  assert.deepEqual(dueNow(all, 9, early, { "loan:3:before": early.getTime() - 1 }), []);
  // Scheduled for later does not count as shown.
  assert.equal(dueNow(all, 9, early, { "loan:3:before": early.getTime() + 1000 }).length, 1);
  // Two days after: "due on 10 Dec" and "due today" are no longer true.
  assert.deepEqual(dueNow(all, 9, new Date(2026, 11, 12, 10), {}).map((r) => r.kind), ["missed"]);
  assert.deepEqual(dueNow(all, 9, new Date(2026, 11, 10, 8), {}).map((r) => r.kind), []);
});

test("numbers are stable and positive, and old records are forgotten", () => {
  assert.equal(numberOf("loan:3:due"), numberOf("loan:3:due"));
  assert.notEqual(numberOf("loan:3:due"), numberOf("loan:4:due"));
  assert.ok(numberOf("01a2197d-036e-75ce-a70d-bc292f2fa299:180:missed") > 0);
  const now = new Date(2026, 11, 12);
  assert.deepEqual(Object.keys(pruned({ old: 0, recent: now.getTime() }, now)), ["recent"]);
});

const kinds = (list: Reminder[]) => list.map((r) => r.kind);
const [before, due, missed] = all.map((r) => numberOf(r.key));

test("what is no longer listed is cancelled", () => {
  const now = new Date(2026, 11, 8, 10);
  const told = { "loan:3:due": momentOf("2026-12-10", 9).getTime() };
  // Marked paid, closed, or reminders switched off: nothing is listed.
  const dropped = plan([], 9, now, told, [due, missed]);
  assert.deepEqual(dropped.cancel, [due, missed]);
  // And it is not remembered as shown, should it come back.
  assert.deepEqual(dropped.told, {});
});

test("a reminder just past its time is left to arrive", () => {
  const nine = momentOf("2026-12-10", 9).getTime();
  const told = { "loan:3:due": nine };
  const soon = plan([all[1], all[2]], 9, new Date(2026, 11, 10, 9, 5), told, [due, missed]);
  assert.deepEqual([soon.cancel, kinds(soon.show), kinds(soon.schedule)], [[missed], [], ["missed"]]);
});

test("one the system dropped is shown when the app is next opened", () => {
  const told = { "loan:3:due": momentOf("2026-12-10", 9).getTime() };
  const later = plan([all[1]], 9, new Date(2026, 11, 10, 12), told, [due]);
  assert.deepEqual([later.cancel, kinds(later.show)], [[due], ["due"]]);
});

test("a delivered reminder is not repeated", () => {
  const told = { "loan:3:before": momentOf("2026-12-07", 9).getTime() };
  const next = plan(all, 9, new Date(2026, 11, 8, 10), told, []);
  assert.deepEqual([kinds(next.show), kinds(next.schedule)], [[], ["due", "missed"]]);
  assert.ok(!next.cancel.includes(before));
});
