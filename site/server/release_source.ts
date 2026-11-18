// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { Readable } from "node:stream";

export const FileOpening = {
  Opened: "opened",
  HostRefused: "host_refused",
  HostUnreachable: "host_unreachable",
} as const;

export type OpenedReleaseFile =
  | { kind: typeof FileOpening.Opened; body: Readable }
  | { kind: typeof FileOpening.HostRefused; status: number }
  | { kind: typeof FileOpening.HostUnreachable };

export interface SourceFile {
  name: string;
  size: number;
  open(): Promise<OpenedReleaseFile>;
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
