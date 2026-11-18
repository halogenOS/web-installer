// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { Readable } from "node:stream";
import type { ByteRange } from "../src/remote/byte_range.ts";

export const FileOpening = {
  Opened: "opened",
  HostRefused: "host_refused",
  HostUnreachable: "host_unreachable",
} as const;

export type OpenedReleaseFile =
  // The range is absent when the body holds the whole file.
  | { kind: typeof FileOpening.Opened; body: Readable; range: ByteRange | undefined }
  | { kind: typeof FileOpening.HostRefused; status: number }
  | { kind: typeof FileOpening.HostUnreachable };

export interface SourceFile {
  name: string;
  size: number;
  open(range: ByteRange | undefined): Promise<OpenedReleaseFile>;
}

export interface SourceRelease {
  id: number;
  tag: string;
  name: string;
  publishedAt: string;
  files: readonly SourceFile[];
}

export interface ReleaseSource {
  listReleases(): Promise<readonly SourceRelease[]>;
}
