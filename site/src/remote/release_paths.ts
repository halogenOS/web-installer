// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export const RELEASES_PATH = "/api/installer/releases";

export const MANIFEST = "manifest.json";

export const ReleaseFileParameter = {
  Release: "release",
  File: "file",
} as const;

function joinReleaseFilePath(release: string, file: string): string {
  return [RELEASES_PATH, release, file].join("/");
}

export const RELEASE_FILE_ROUTE = joinReleaseFilePath(
  `:${ReleaseFileParameter.Release}`,
  `:${ReleaseFileParameter.File}`,
);

export function releaseFilePath(release: bigint | number, file: string): string {
  return joinReleaseFilePath(String(release), encodeURIComponent(file));
}

export function releaseManifestPath(release: bigint | number): string {
  return releaseFilePath(release, MANIFEST);
}
