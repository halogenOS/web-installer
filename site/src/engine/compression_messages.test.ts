// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { ChunkOperationKind } from "../wasm/contract.ts";
import { CompressionReplyKind, readChunkToCompress, readCompressionReply } from "./compression_messages.ts";

test("a chunk to compress is read with a kind of chunk operation and its bytes", () => {
  const bytes = new Uint8Array([1, 2, 3]);
  for (const kind of Object.values(ChunkOperationKind)) {
    assert.deepEqual(readChunkToCompress({ kind, bytes }), { kind, bytes });
  }
});

test("a chunk with an unknown kind or without bytes is not read", () => {
  const chunks = [
    { kind: "replace_zstd", bytes: new Uint8Array([1]) },
    { kind: ChunkOperationKind.ReplaceXz, bytes: [1] },
    { kind: ChunkOperationKind.Replace },
    null,
  ];
  for (const chunk of chunks) assert.equal(readChunkToCompress(chunk), undefined);
});

test("a compression reply is read as compressed bytes or as a failure with name and message", () => {
  const replies = [
    { kind: CompressionReplyKind.Compressed, bytes: new Uint8Array([9]) },
    { kind: CompressionReplyKind.Failed, name: "RangeError", message: "The chunk is too large." },
  ];
  for (const reply of replies) assert.deepEqual(readCompressionReply(reply), reply);
});

test("a compression reply without the fields of its kind is not read", () => {
  const replies = [
    { kind: CompressionReplyKind.Compressed },
    { kind: CompressionReplyKind.Failed, name: "RangeError" },
    { kind: "half_done", bytes: new Uint8Array([9]) },
    "compressed",
  ];
  for (const reply of replies) assert.equal(readCompressionReply(reply), undefined);
});
