// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { parseArgs } from "node:util";
import { readDistPackages } from "./dist_package_releases.ts";
import { githubReleaseSource } from "./github_releases.ts";
import type { ReleaseSource } from "./release_source.ts";

const { positionals: distPackageFolders } = parseArgs({ options: {}, allowPositionals: true });

export const releaseSource: ReleaseSource =
  distPackageFolders.length === 0 ? githubReleaseSource : await readDistPackages(distPackageFolders);
