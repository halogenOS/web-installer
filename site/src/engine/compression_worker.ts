// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import init, { compressChunk } from "../wasm/web_installer_wasm.js";
import {
  CompressionReplyKind,
  readChunkToCompress,
  type CompressionReply,
} from "./compression_messages.ts";

let engine: Promise<WebAssembly.Exports> | undefined;

async function compress(message: unknown): Promise<Uint8Array> {
  const chunk = readChunkToCompress(message);
  if (chunk === undefined) {
    throw new Error("The engine worker sent a message that the compression worker cannot read.");
  }
  await (engine ??= init());
  return compressChunk(chunk.kind, chunk.bytes);
}

function compressChunksFrom(port: MessagePort): void {
  port.onmessage = (event: MessageEvent<unknown>) => {
    compress(event.data).then(
      (bytes) => {
        const reply: CompressionReply = { kind: CompressionReplyKind.Compressed, bytes };
        port.postMessage(reply, [bytes.buffer]);
      },
      (cause: unknown) => {
        const failure = cause instanceof Error ? cause : new Error(String(cause));
        const reply: CompressionReply = {
          kind: CompressionReplyKind.Failed,
          name: failure.name,
          message: failure.message,
        };
        port.postMessage(reply);
      },
    );
  };
}

self.onmessage = (event: MessageEvent<unknown>) => {
  const port = event.data;
  if (!(port instanceof MessagePort)) {
    throw new Error("The page sent a message that the compression worker cannot read.");
  }
  compressChunksFrom(port);
};
