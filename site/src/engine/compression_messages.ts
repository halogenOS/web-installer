// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { ChunkOperationKind } from "../wasm/contract.ts";
import { isRecord } from "./messages.ts";

export interface ChunkToCompress {
  kind: ChunkOperationKind;
  bytes: Uint8Array;
}

const KINDS: readonly unknown[] = Object.values(ChunkOperationKind);

function isChunkOperationKind(value: unknown): value is ChunkOperationKind {
  return KINDS.includes(value);
}

export function readChunkToCompress(data: unknown): ChunkToCompress | undefined {
  if (!isRecord(data)) return undefined;
  const { kind, bytes } = data;
  if (!isChunkOperationKind(kind) || !(bytes instanceof Uint8Array)) return undefined;
  return { kind, bytes };
}

export const CompressionReplyKind = {
  Compressed: "compressed",
  Failed: "failed",
} as const;

// A failure travels as name and message, since structured clone keeps the name of an error only
// for the error types built into JavaScript.
export type CompressionReply =
  | { kind: typeof CompressionReplyKind.Compressed; bytes: Uint8Array }
  | { kind: typeof CompressionReplyKind.Failed; name: string; message: string };

export function readCompressionReply(data: unknown): CompressionReply | undefined {
  if (!isRecord(data)) return undefined;
  const { kind, bytes, name, message } = data;
  if (kind === CompressionReplyKind.Compressed && bytes instanceof Uint8Array) {
    return { kind, bytes };
  }
  if (kind === CompressionReplyKind.Failed && typeof name === "string" && typeof message === "string") {
    return { kind, name, message };
  }
  return undefined;
}
