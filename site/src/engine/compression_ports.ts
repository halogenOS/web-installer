// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { ChunkOperationKind } from "../wasm/contract.ts";
import type { ChunkToCompress } from "./compression_messages.ts";

interface CompressionJob {
  kind: ChunkOperationKind;
  bytes: Uint8Array<ArrayBuffer>;
  resolve: (data: Uint8Array) => void;
}

export class CompressionPorts {
  readonly #ports: readonly MessagePort[];
  readonly #waiting: CompressionJob[] = [];
  readonly #running = new Map<MessagePort, CompressionJob>();

  constructor(ports: readonly MessagePort[]) {
    this.#ports = ports;
    for (const port of ports) {
      port.onmessage = (event: MessageEvent<unknown>) => {
        const data = event.data;
        if (!(data instanceof Uint8Array)) {
          throw new Error("A compression worker sent a message that the engine worker cannot read.");
        }
        this.#finish(port, data);
      };
    }
  }

  count(): number {
    return this.#ports.length;
  }

  compress(kind: ChunkOperationKind, bytes: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
    return new Promise((resolve) => {
      this.#waiting.push({ kind, bytes, resolve });
      this.#handOverWaiting();
    });
  }

  #finish(port: MessagePort, data: Uint8Array): void {
    this.#running.get(port)?.resolve(data);
    this.#running.delete(port);
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
