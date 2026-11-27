import assert from "node:assert/strict";
import { test } from "node:test";
import { money, moneyInput, parseDecimal, parseMoney } from "./format.ts";
import { fitScale } from "./pictures.ts";

test("typed rupees become exact paise", () => {
  assert.equal(parseMoney("1,234.5"), 123450);
  assert.equal(parseMoney("₹ 0.29"), 29);
  assert.equal(parseMoney("-1"), null);
  assert.equal(parseMoney("1.234"), null);
  assert.equal(parseMoney(""), null);
  assert.equal(parseMoney("20"), 2000);
  assert.equal(parseMoney("Rs 20/-"), 2000);
  assert.equal(parseMoney("10 rupees"), 1000);
});

test("rates keep their precision", () => {
  assert.equal(parseDecimal("10.55", 2), 1055);
  assert.equal(parseDecimal("88.4567", 6), 88456700);
  assert.equal(parseDecimal("x", 2), null);
});

test("amounts round-trip through the input", () => {
  for (const paise of [5, 100, 123450, 99999999]) {
    assert.equal(parseMoney(moneyInput(paise)), paise);
  }
});

test("money uses Indian grouping and two decimals when needed", () => {
  assert.equal(money(12345600), "₹1,23,456");
  assert.equal(money(123450), "₹1,234.50");
  assert.equal(money(-5000), "-₹50");
});

test("pictures shrink without making long receipts unreadable", () => {
  assert.equal(fitScale(800, 600), 1);
  assert.equal(fitScale(3000, 4000), 0.4);
  // A tall receipt keeps its width rather than being squeezed to a strip.
  assert.equal(fitScale(1000, 3500), 1);
  assert.equal(fitScale(1000, 8000), 0.5);
});
