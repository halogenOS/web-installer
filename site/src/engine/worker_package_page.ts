// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { ChunkOperationKind, PackagePage } from "../wasm/contract.ts";
import type { CompressionPorts } from "./compression_ports.ts";
import { WorkerStoragePage, type StoragePorts } from "./worker_storage_page.ts";

export class WorkerPackagePage extends WorkerStoragePage implements PackagePage {
  readonly #file: FileSystemWritableFileStream;
  readonly #fileInPageStorage: boolean;
  readonly #compressors: CompressionPorts;

  constructor(
    ports: StoragePorts,
    file: FileSystemWritableFileStream,
    fileInPageStorage: boolean,
    compressors: CompressionPorts,
  ) {
    super(ports);
    this.#file = file;
    this.#fileInPageStorage = fileInPageStorage;
    this.#compressors = compressors;
  }

  writePackage(bytes: Uint8Array<ArrayBuffer>): Promise<void> {
    return this.#file.write(bytes);
  }

  closePackage(): Promise<void> {
    return this.#file.close();
  }

  packageInPageStorage(): boolean {
    return this.#fileInPageStorage;
  }

  chunkCompressors(): number {
    return this.#compressors.count();
  }

  compressChunk(kind: ChunkOperationKind, chunk: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
    return this.#compressors.compress(kind, chunk);
  }

  abortPackage(): Promise<void> {
    return this.#file.abort();
  }
}
