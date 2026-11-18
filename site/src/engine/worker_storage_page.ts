// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { canDownload, canReadByRange, fetchRemoteRange, waitFor } from "../remote/fetch.ts";
import { RangedFile } from "../remote/ranged_file.ts";
import { RequestScheduler, monotonicMilliseconds } from "../remote/request_scheduler.ts";
import { FetchedReleaseSite } from "../remote/release_site.ts";
import type { BodySink, ReportRetry } from "../remote/transfer.ts";
import { InstallStore } from "../store/install_store.ts";
import type {
  FileDigest,
  PageFile,
  PageSink,
  PauseBeforeRetry,
  PickedPageFile,
  Progress,
  RangeArrival,
  StoragePage,
  StoredFile,
  TransferResult,
} from "../wasm/contract.ts";
import { WorkerMessageKind, type WorkerMessage } from "./messages.ts";
import { PickedFile } from "./picked_file.ts";

export interface StoragePorts {
  filesPicked: () => Promise<File[]>;
  pauseBeforeRetry: PauseBeforeRetry;
  post: (message: WorkerMessage) => void;
}

export class WorkerStoragePage implements StoragePage {
  protected readonly post: (message: WorkerMessage) => void;
  readonly #ports: StoragePorts;
  readonly #site: FetchedReleaseSite;
  readonly #reportRetry: ReportRetry;
  readonly #store = new InstallStore();
  readonly #scheduler = new RequestScheduler(monotonicMilliseconds);

  constructor(ports: StoragePorts) {
    this.post = ports.post;
    this.#ports = ports;
    this.#reportRetry = (failedAttempts) => {
      ports.post({ kind: WorkerMessageKind.Retrying, failedAttempts });
    };
    this.#site = new FetchedReleaseSite(ports.pauseBeforeRetry, this.#reportRetry, this.#scheduler);
  }

  fetchReleases(): Promise<TransferResult<string>> {
    return this.#site.fetchReleases();
  }

  fetchReleaseManifest(release: bigint): Promise<TransferResult<string>> {
    return this.#site.fetchReleaseManifest(release);
  }

  createFile(name: string, size: number): Promise<void> {
    return this.#store.create(name, size);
  }

  async download(
    address: string,
    name: string,
    offset: number,
    size: number,
    digest: FileDigest,
  ): Promise<TransferResult<number>> {
    const file = this.#store.file(name);
    let received = 0;
    const sink: BodySink<number> = {
      begin: () => {
        received = 0;
        digest.restart();
      },
      chunk: (bytes) => {
        digest.update(bytes);
        file.writeAt(offset + received, bytes.subarray(0, Math.max(0, size - received)));
        received += bytes.length;
        return true;
      },
      end: () => Promise.resolve(received),
    };
    try {
      return await this.#site.fetch(address, sink);
    } finally {
      digest.free();
    }
  }

  async downloadRange(
    address: string,
    name: string,
    offset: number,
    length: number,
    arrival: RangeArrival,
  ): Promise<TransferResult<number>> {
    const file = this.#store.file(name);
    let stored = 0;
    const sink: BodySink<number> = {
      begin: () => {
        stored = 0;
        arrival.restart();
      },
      chunk: (bytes) => {
        const taken = arrival.take(bytes);
        file.writeAt(offset + stored, bytes.subarray(0, taken));
        stored += taken;
        return taken === bytes.length;
      },
      end: () => Promise.resolve(stored),
    };
    try {
      return await fetchRemoteRange(
        address,
        { offset, length },
        sink,
        this.#reportRetry,
        this.#ports.pauseBeforeRetry,
        this.#scheduler,
      );
    } finally {
      arrival.free();
    }
  }

  file(name: string): StoredFile {
    return this.#store.file(name);
  }

  fileFrom(name: string, offset: number): PageSink {
    return this.#store.file(name).sinkFrom(offset);
  }

  canReadByRange(address: string): Promise<boolean> {
    return canReadByRange(address, this.#ports.pauseBeforeRetry, this.#reportRetry, this.#scheduler);
  }

  canDownload(address: string): Promise<boolean> {
    return canDownload(address, this.#ports.pauseBeforeRetry, this.#reportRetry, this.#scheduler);
  }

  remoteFile(address: string, size: number): PageFile {
    return new RangedFile(address, size);
  }

  waitBeforeRetry(failedAttempts: number, milliseconds: number): Promise<void> {
    this.#reportRetry(failedAttempts);
    return waitFor(milliseconds);
  }

  async pickedFiles(): Promise<PickedPageFile[]> {
    const files = await this.#ports.filesPicked();
    return files.map((file) => ({ name: file.name, file: new PickedFile(file) }));
  }

  removeFile(name: string): Promise<void> {
    return this.#store.remove(name);
  }

  removeAllFiles(): Promise<void> {
    return this.#store.removeAll();
  }

  freeStorage(): Promise<number> {
    return this.#store.freeStorage();
  }

  report(progress: Progress): void {
    this.post({ kind: WorkerMessageKind.Progress, progress });
  }
}
