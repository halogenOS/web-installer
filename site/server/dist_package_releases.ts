// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { createHash } from "node:crypto";
import { createReadStream, type Stats } from "node:fs";
import { readFile, readdir, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { lastByteOf } from "../src/remote/byte_range.ts";
import { MANIFEST } from "../src/remote/release_paths.ts";
import { FileOpening, type ReleaseSource, type SourceRelease } from "./release_source.ts";

interface ManifestNames {
  device: string;
  build: string;
}

function isManifestNames(value: unknown): value is ManifestNames {
  return (
    typeof value === "object" &&
    value !== null &&
    "device" in value &&
    typeof value.device === "string" &&
    "build" in value &&
    typeof value.build === "string"
  );
}

function idOfWrittenFile(path: string, info: Stats): number {
  return createHash("sha256")
    .update(JSON.stringify([path, info.size, info.mtimeMs]))
    .digest()
    .readUIntBE(0, 6);
}

async function readDistPackage(folder: string): Promise<SourceRelease> {
  const manifestPath = join(folder, MANIFEST);
  const manifestInfo = await stat(manifestPath);
  const manifest: unknown = JSON.parse(await readFile(manifestPath, "utf8"));
  if (!isManifestNames(manifest)) throw new Error(`${manifestPath} does not contain a device and a build.`);
  const entries = await Promise.all(
    (await readdir(folder)).sort().map(async (name) => {
      const path = join(folder, name);
      return { name, path, info: await stat(path) };
    }),
  );
  return {
    id: idOfWrittenFile(manifestPath, manifestInfo),
    tag: manifest.build,
    name: `${manifest.build} for ${manifest.device}`,
    publishedAt: manifestInfo.mtime.toISOString(),
    files: entries
      .filter((entry) => entry.info.isFile())
      .map((entry) => ({
        name: entry.name,
        size: entry.info.size,
        open: (range) =>
          Promise.resolve({
            kind: FileOpening.Opened,
            body: createReadStream(
              entry.path,
              range === undefined ? {} : { start: range.offset, end: lastByteOf(range) },
            ),
            range,
          }),
      })),
  };
}

export async function readDistPackages(folders: readonly string[]): Promise<ReleaseSource> {
  const releases = await Promise.all(folders.map((folder) => readDistPackage(resolve(folder))));
  if (new Set(releases.map((release) => release.id)).size !== releases.length) {
    throw new Error("A dist package folder is given more than once.");
  }
  if (new Set(releases.map((release) => release.tag)).size !== releases.length) {
    throw new Error("Two dist package folders contain the same build, so the tag in a link matches both.");
  }
  for (const release of releases) console.log(`Serving ${release.name}`);
  return { listReleases: () => Promise.resolve(releases) };
}
