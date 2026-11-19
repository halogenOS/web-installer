// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import { ChunkOperationKind } from "../wasm/contract.ts";
import {
  CompressionReplyKind,
  readChunkToCompress,
  type ChunkToCompress,
  type CompressionReply,
} from "./compression_messages.ts";
import { CompressionPorts } from "./compression_ports.ts";

// The worker end of a channel, standing where a compression worker would.
class CompressionWorkerEnd {
  readonly #port: MessagePort;
  readonly #arrived: ChunkToCompress[] = [];
  readonly #waiting: ((chunk: ChunkToCompress) => void)[] = [];
  received = 0;

  constructor(port: MessagePort) {
    this.#port = port;
    port.onmessage = (event: MessageEvent<unknown>) => {
      const chunk = readChunkToCompress(event.data);
      assert.ok(chunk !== undefined);
      this.received += 1;
      const waiting = this.#waiting.shift();
      if (waiting === undefined) this.#arrived.push(chunk);
      else waiting(chunk);
    };
  }

  next(): Promise<ChunkToCompress> {
    const arrived = this.#arrived.shift();
    if (arrived !== undefined) return Promise.resolve(arrived);
    return new Promise((resolve) => {
      this.#waiting.push(resolve);
    });
  }

  reply(reply: CompressionReply): void {
    this.#port.postMessage(reply);
  }
}

function compressionChannels(context: TestContext, count: number): { ports: CompressionPorts; workers: CompressionWorkerEnd[] } {
  const channels = Array.from({ length: count }, () => new MessageChannel());
  context.after(() => {
    for (const channel of channels) {
      channel.port1.close();
      channel.port2.close();
    }
  });
  return {
    ports: new CompressionPorts(channels.map(({ port2 }) => port2)),
    workers: channels.map(({ port1 }) => new CompressionWorkerEnd(port1)),
  };
}

function compressed(bytes: number[]): CompressionReply {
  return { kind: CompressionReplyKind.Compressed, bytes: new Uint8Array(bytes) };
}

test("the ports count one compressor for each port", (context) => {
  assert.equal(compressionChannels(context, 3).ports.count(), 3);
});

test("each worker compresses one chunk at a time, and a chunk waits for the first worker that is free", async (context) => {
  const { ports, workers } = compressionChannels(context, 2);
  const [first, second] = workers;
  assert.ok(first !== undefined && second !== undefined);
  const results = [1, 2, 3].map((byte) => ports.compress(ChunkOperationKind.ReplaceXz, new Uint8Array([byte])));
  assert.deepEqual((await first.next()).bytes, new Uint8Array([1]));
  assert.deepEqual((await second.next()).bytes, new Uint8Array([2]));
  first.reply(compressed([10]));
  const third = await first.next();
  assert.deepEqual(third, { kind: ChunkOperationKind.ReplaceXz, bytes: new Uint8Array([3]) });
  second.reply(compressed([20]));
  first.reply(compressed([30]));
  assert.deepEqual(await Promise.all(results), [new Uint8Array([10]), new Uint8Array([20]), new Uint8Array([30])]);
  assert.deepEqual([first.received, second.received], [2, 1]);
});

test("a chunk moves to its worker without a copy", async (context) => {
  const { ports, workers } = compressionChannels(context, 1);
  const chunk = new Uint8Array([1, 2, 3]);
  const result = ports.compress(ChunkOperationKind.Replace, chunk);
  assert.equal(chunk.byteLength, 0);
  workers[0]?.reply(compressed([1]));
  await result;
});

test("a failed compression rejects its chunk with the name and message sent by the worker, and the next chunk goes on", async (context) => {
  const { ports, workers } = compressionChannels(context, 1);
  const [worker] = workers;
  assert.ok(worker !== undefined);
  const failing = ports.compress(ChunkOperationKind.ReplaceBz, new Uint8Array([1]));
  const next = ports.compress(ChunkOperationKind.ReplaceBz, new Uint8Array([2]));
  await worker.next();
  worker.reply({ kind: CompressionReplyKind.Failed, name: "RangeError", message: "The chunk is too large." });
  await assert.rejects(failing, (failure) => failure instanceof Error && failure.name === "RangeError" && failure.message === "The chunk is too large.");
  assert.deepEqual((await worker.next()).bytes, new Uint8Array([2]));
  worker.reply(compressed([7]));
  assert.deepEqual(await next, new Uint8Array([7]));
});
