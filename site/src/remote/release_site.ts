// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PauseBeforeRetry, ReleaseSite, TransferResult } from "../wasm/contract.ts";
import { fetchRemote } from "./fetch.ts";
import { RELEASES_PATH, releaseManifestPath } from "./release_paths.ts";
import type { AttemptScheduler } from "./request_scheduler.ts";
import type { BodySink, ReportRetry } from "./transfer.ts";

function textSink(): BodySink<string> {
  let chunks: Uint8Array<ArrayBuffer>[] = [];
  return {
    begin: () => {
      chunks = [];
    },
    chunk: (bytes) => {
      chunks.push(bytes);
      return true;
    },
    end: () => new Blob(chunks).text(),
  };
}

export class FetchedReleaseSite implements ReleaseSite {
  readonly #pauseBeforeRetry: PauseBeforeRetry;
  readonly #reportRetry: ReportRetry;
  readonly #scheduler: AttemptScheduler;

  constructor(
    pauseBeforeRetry: PauseBeforeRetry,
    reportRetry: ReportRetry,
    scheduler: AttemptScheduler,
  ) {
    this.#pauseBeforeRetry = pauseBeforeRetry;
    this.#reportRetry = reportRetry;
    this.#scheduler = scheduler;
  }

  fetchReleases(): Promise<TransferResult<string>> {
    return this.fetch(new URL(RELEASES_PATH, location.origin).href, textSink());
  }

  fetchReleaseManifest(release: bigint): Promise<TransferResult<string>> {
    return this.fetch(new URL(releaseManifestPath(release), location.origin).href, textSink());
  }

  fetch<T>(address: string, sink: BodySink<T>): Promise<TransferResult<T>> {
    return fetchRemote(
      { href: address, headers: {}, sink, reportRetry: this.#reportRetry },
      this.#pauseBeforeRetry,
      this.#scheduler,
    );
  }
}
