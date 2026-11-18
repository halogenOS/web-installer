// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import init, { compressChunk } from "../wasm/web_installer_wasm.js";
import { readChunkToCompress } from "./compression_messages.ts";

let engine: Promise<WebAssembly.Exports> | undefined;

function compressChunksFrom(port: MessagePort): void {
  port.onmessage = (event: MessageEvent<unknown>) => {
    const chunk = readChunkToCompress(event.data);
    if (chunk === undefined) {
      throw new Error("The engine worker sent a message that the compression worker cannot read.");
    }
    engine ??= init();
    engine
      .then(() => {
        const data = compressChunk(chunk.kind, chunk.bytes);
        port.postMessage(data, [data.buffer]);
      })
      .catch(reportError);
  };
}

self.onmessage = (event: MessageEvent<unknown>) => {
  const port = event.data;
  if (!(port instanceof MessagePort)) {
    throw new Error("The page sent a message that the compression worker cannot read.");
  }
  compressChunksFrom(port);
};
