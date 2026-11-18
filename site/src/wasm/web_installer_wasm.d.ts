// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PhoneProblem } from "../device/usb.ts";
import type {
  ChunkOperationKind,
  InstallFailure,
  InstallPage,
  ReleasesToChoose,
  PackagePage,
  ReleaseSite,
  TransferFailure,
} from "./contract.ts";

export default function init(): Promise<WebAssembly.Exports>;

export function listReleases(site: ReleaseSite, linkedTag: string | undefined): Promise<ReleasesToChoose>;

export function install(page: InstallPage, releaseTag: string): Promise<void>;

export function savePackage(page: PackagePage, releaseTag: string): Promise<void>;

export function compressChunk(kind: ChunkOperationKind, chunk: Uint8Array): Uint8Array<ArrayBuffer>;

export function phoneProblemFailure(problem: PhoneProblem): InstallFailure;

export function saveDialogFailure(): InstallFailure;

export function pauseBeforeRetry(failure: TransferFailure, attempt: number): number | undefined;
