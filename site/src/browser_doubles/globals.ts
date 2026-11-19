// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import type { TestContext } from "node:test";

// Node lacks most browser APIs used by the page, so a test defines each one it needs for its own run. The test
// restores the property as it found it once the test ends, absent or not.
export function provideGlobal<T>(context: TestContext, owner: object, name: string, value: T): void {
  const before = Object.getOwnPropertyDescriptor(owner, name);
  Object.defineProperty(owner, name, { value, configurable: true, writable: true });
  context.after(() => {
    if (before === undefined) assert.ok(Reflect.deleteProperty(owner, name));
    else Object.defineProperty(owner, name, before);
  });
}
