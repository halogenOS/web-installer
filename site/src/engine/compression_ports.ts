// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { ChunkOperationKind } from "../wasm/contract.ts";
import {
  CompressionReplyKind,
  readCompressionReply,
  type ChunkToCompress,
  type CompressionReply,
} from "./compression_messages.ts";

interface CompressionJob {
  kind: ChunkOperationKind;
  bytes: Uint8Array<ArrayBuffer>;
  resolve: (data: Uint8Array) => void;
  reject: (failure: Error) => void;
}

function failureOf(reply: Extract<CompressionReply, { kind: typeof CompressionReplyKind.Failed }>): Error {
  const failure = new Error(reply.message);
  failure.name = reply.name;
  return failure;
}

export class CompressionPorts {
  readonly #ports: readonly MessagePort[];
  readonly #waiting: CompressionJob[] = [];
  readonly #running = new Map<MessagePort, CompressionJob>();

  constructor(ports: readonly MessagePort[]) {
    this.#ports = ports;
    for (const port of ports) {
      port.onmessage = (event: MessageEvent<unknown>) => {
        const reply = readCompressionReply(event.data);
        if (reply === undefined) {
          throw new Error("A compression worker sent a message that the engine worker cannot read.");
        }
        this.#finish(port, reply);
      };
    }
  }

  count(): number {
    return this.#ports.length;
  }

  compress(kind: ChunkOperationKind, bytes: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      this.#waiting.push({ kind, bytes, resolve, reject });
      this.#handOverWaiting();
    });
  }

  #finish(port: MessagePort, reply: CompressionReply): void {
    const job = this.#running.get(port);
    this.#running.delete(port);
    if (reply.kind === CompressionReplyKind.Compressed) job?.resolve(reply.bytes);
    else job?.reject(failureOf(reply));
    this.#handOverWaiting();
  }

  #handOverWaiting(): void {
    for (const port of this.#ports) {
      if (this.#running.has(port)) continue;
      const job = this.#waiting.shift();
      if (job === undefined) return;
      this.#running.set(port, job);
      const chunk: ChunkToCompress = { kind: job.kind, bytes: job.bytes };
      port.postMessage(chunk, [job.bytes.buffer]);
    }
  }
}
