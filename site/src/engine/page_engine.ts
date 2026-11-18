// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PhoneProblem } from "../device/usb.ts";
import { FetchedReleaseSite } from "../remote/release_site.ts";
import { RequestScheduler, monotonicMilliseconds } from "../remote/request_scheduler.ts";
import type { ReportRetry } from "../remote/transfer.ts";
import type { InstallFailure, ReleasesToChoose } from "../wasm/contract.ts";
import init, {
  listReleases,
  pauseBeforeRetry,
  phoneProblemFailure,
  saveDialogFailure,
} from "../wasm/web_installer_wasm.js";
import { isInstallFailure } from "./install_failure.ts";

let engine: Promise<WebAssembly.Exports> | undefined;

function loadEngine(): Promise<WebAssembly.Exports> {
  engine ??= init();
  return engine;
}

export const ListingEnd = { Listed: "listed", Failed: "failed" } as const;

export type Listing =
  | ({ kind: typeof ListingEnd.Listed } & ReleasesToChoose)
  | { kind: typeof ListingEnd.Failed; failure: InstallFailure };

export async function listPublishedReleases(
  linkedTag: string | undefined,
  reportRetry: ReportRetry,
): Promise<Listing> {
  await loadEngine();
  const site = new FetchedReleaseSite(
    pauseBeforeRetry,
    reportRetry,
    new RequestScheduler(monotonicMilliseconds),
  );
  try {
    return { kind: ListingEnd.Listed, ...(await listReleases(site, linkedTag)) };
  } catch (cause) {
    if (isInstallFailure(cause)) return { kind: ListingEnd.Failed, failure: cause };
    throw cause;
  }
}

export async function failureOfPhoneProblem(problem: PhoneProblem): Promise<InstallFailure> {
  await loadEngine();
  return phoneProblemFailure(problem);
}

export async function failureOfSaveDialog(): Promise<InstallFailure> {
  await loadEngine();
  return saveDialogFailure();
}
