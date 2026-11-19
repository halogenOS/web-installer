// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { Waits } from "./waits.ts";

function settle(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

test("every wait begun before a value comes ends with that value", async () => {
  const waits = new Waits<string>();
  const first = waits.next();
  const second = waits.next();
  waits.resolveAll("pixel");
  assert.deepEqual(await Promise.all([first, second]), ["pixel", "pixel"]);
});

test("a wait begun after a value came ends only with the next value", async () => {
  const waits = new Waits<string>();
  waits.resolveAll("before");
  let ended: string | undefined;
  void waits.next().then((value) => {
    ended = value;
  });
  await settle();
  assert.equal(ended, undefined);
  waits.resolveAll("after");
  await settle();
  assert.equal(ended, "after");
});
