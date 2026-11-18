// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { formatMegabytes, megabytesOf } from "./amounts.ts";

const BLOCK = { done: 802_654_379, total: 1_664_231_699 };

test("a total rounds to the nearest megabyte, not up, so one block reads the same in every text", () => {
  assert.deepEqual(megabytesOf(BLOCK), { done: "803", total: "1,664" });
  assert.equal(formatMegabytes(BLOCK), "803 of 1,664 MB");
});

test("a total under 10 MB keeps one decimal for both values", () => {
  assert.deepEqual(megabytesOf({ done: 1_250_000, total: 4_940_000 }), { done: "1.3", total: "4.9" });
});
