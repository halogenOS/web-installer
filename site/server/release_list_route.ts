// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { defineEventHandler, getRequestURL } from "h3";
import { releaseFilePath } from "../src/remote/release_paths.ts";
import { releaseSource } from "./command_line.ts";

interface ListedAsset {
  name: string;
  url: string;
}

interface ListedRelease {
  id: number;
  tag: string;
  name: string;
  publishedAt: string;
  assets: ListedAsset[];
}

interface ReleaseList {
  releases: ListedRelease[];
}

export default defineEventHandler(async (event): Promise<ReleaseList> => {
  const page = getRequestURL(event, { xForwardedHost: true, xForwardedProto: true });
  const releases = await releaseSource.listReleases();
  return {
    releases: releases.map((release) => ({
      id: release.id,
      tag: release.tag,
      name: release.name,
      publishedAt: release.publishedAt,
      assets: release.files.map((file) => ({
        name: file.name,
        url: new URL(releaseFilePath(release.id, file.name), page).href,
      })),
    })),
  };
});
