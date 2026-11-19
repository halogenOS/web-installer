// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { readExactly, writeUntilRefused } from "./sync_transfer.ts";

// An access handle may move fewer bytes than asked for in one call, so this handle moves at most `piece` bytes per
// call. It refuses to grow the file past `room` bytes with QuotaExceededError, as the File System standard
// (https://fs.spec.whatwg.org/, read 2026-11-19) asks of write().
class PieceHandle implements FileSystemSyncAccessHandle {
  bytes: Uint8Array;
  readonly #piece: number;
  readonly #room: number;
  #failure: Error | undefined;

  constructor(bytes: Uint8Array, piece: number, room = Infinity) {
    this.bytes = bytes;
    this.#piece = piece;
    this.#room = room;
  }

  failWith(failure: Error): void {
    this.#failure = failure;
  }

  write(buffer: Uint8Array, options?: { at: number }): number {
    if (this.#failure !== undefined) throw this.#failure;
    const at = options?.at ?? 0;
    const count = Math.min(buffer.length, this.#piece);
    if (at + count > this.#room) throw new DOMException("The storage is full.", "QuotaExceededError");
    if (at + count > this.bytes.length) {
      const grown = new Uint8Array(at + count);
      grown.set(this.bytes);
      this.bytes = grown;
    }
    this.bytes.set(buffer.subarray(0, count), at);
    return count;
  }

  read(buffer: Uint8Array, options?: { at: number }): number {
    const at = options?.at ?? 0;
    const found = this.bytes.subarray(at, at + Math.min(buffer.length, this.#piece));
    buffer.set(found);
    return found.length;
  }

  truncate(size: number): void {
    this.bytes = this.bytes.slice(0, size);
  }

  getSize(): number {
    return this.bytes.length;
  }

  flush(): void {}

  close(): void {}
}

test("a write goes on through short writes until every byte is stored", () => {
  const handle = new PieceHandle(new Uint8Array(2), 3);
  assert.equal(writeUntilRefused(handle, new Uint8Array([1, 2, 3, 4, 5, 6, 7]), 2), 7);
  assert.deepEqual(handle.bytes, new Uint8Array([0, 0, 1, 2, 3, 4, 5, 6, 7]));
});

test("a write that the storage refuses as full returns the bytes stored before the refusal", () => {
  const handle = new PieceHandle(new Uint8Array(0), 2, 5);
  assert.equal(writeUntilRefused(handle, new Uint8Array([1, 2, 3, 4, 5, 6]), 0), 4);
  assert.deepEqual(handle.bytes, new Uint8Array([1, 2, 3, 4]));
});

test("a write fails with any refusal other than a full storage", () => {
  const handle = new PieceHandle(new Uint8Array(0), 2);
  const closed = new DOMException("The access handle is closed.", "InvalidStateError");
  handle.failWith(closed);
  assert.throws(() => writeUntilRefused(handle, new Uint8Array([1]), 0), closed);
});

test("a read goes on through short reads until the target is full", () => {
  const handle = new PieceHandle(new Uint8Array([9, 8, 7, 6, 5, 4, 3]), 2);
  const target = new Uint8Array(5);
  readExactly(handle, target, 1);
  assert.deepEqual(target, new Uint8Array([8, 7, 6, 5, 4]));
});

test("a read fails as not readable when the file ends before the target is full", () => {
  const handle = new PieceHandle(new Uint8Array([9, 8, 7]), 2);
  assert.throws(
    () => readExactly(handle, new Uint8Array(4), 0),
    (failure) => failure instanceof DOMException && failure.name === "NotReadableError",
  );
});
