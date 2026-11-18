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
