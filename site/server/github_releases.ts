// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { Readable } from "node:stream";
import { defineCachedFunction, useRuntimeConfig } from "nitropack/runtime";
import { PARTIAL_CONTENT, rangeHeader, type ByteRange } from "../src/remote/byte_range.ts";
import { FileOpening, type OpenedReleaseFile, type ReleaseSource } from "./release_source.ts";

const RELEASES_ADDRESS = "https://api.github.com/repos/halogenOS/releases/releases";

const RELEASES_PER_PAGE = 100;

const RELEASE_LIST_SECONDS = 30 * 60;

const API_HEADERS = {
  Accept: "application/vnd.github+json",
  "X-GitHub-Api-Version": "2022-11-28",
} as const;

interface GithubAsset {
  name: string;
  size: number;
  address: string;
}

interface GithubRelease {
  id: number;
  tag: string;
  name: string;
  publishedAt: string;
  assets: GithubAsset[];
}

function readGithubAsset(value: unknown): GithubAsset | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  if (!("name" in value && typeof value.name === "string")) return undefined;
  if (!("size" in value && typeof value.size === "number")) return undefined;
  if (!("browser_download_url" in value && typeof value.browser_download_url === "string")) return undefined;
  return { name: value.name, size: value.size, address: value.browser_download_url };
}

function readGithubRelease(value: unknown): GithubRelease | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  if (!("id" in value && typeof value.id === "number")) return undefined;
  if (!("tag_name" in value && typeof value.tag_name === "string")) return undefined;
  if (!("name" in value && typeof value.name === "string")) return undefined;
  if (!("published_at" in value && typeof value.published_at === "string")) return undefined;
  if (!("assets" in value && Array.isArray(value.assets))) return undefined;
  return {
    id: value.id,
    tag: value.tag_name,
    name: value.name,
    publishedAt: value.published_at,
    assets: value.assets.map(readGithubAsset).filter((asset) => asset !== undefined),
  };
}

function authorizationHeaders(): Record<string, string> {
  const token: unknown = useRuntimeConfig().githubToken;
  return typeof token === "string" && token !== "" ? { Authorization: `Bearer ${token}` } : {};
}

interface ReleasePage {
  releases: GithubRelease[];
  isLast: boolean;
}

async function fetchReleasePage(page: number): Promise<ReleasePage> {
  const address = new URL(RELEASES_ADDRESS);
  address.searchParams.set("per_page", String(RELEASES_PER_PAGE));
  address.searchParams.set("page", String(page));
  const response = await fetch(address, { headers: { ...API_HEADERS, ...authorizationHeaders() } });
  if (!response.ok) throw new Error(`GitHub answered page ${String(page)} of the releases with ${String(response.status)}.`);
  const body: unknown = await response.json();
  if (!Array.isArray(body)) throw new Error(`Page ${String(page)} of the releases on GitHub is not a list.`);
  return {
    releases: body.map(readGithubRelease).filter((release) => release !== undefined),
    isLast: body.length < RELEASES_PER_PAGE,
  };
}

const fetchGithubReleases = defineCachedFunction(
  async (): Promise<GithubRelease[]> => {
    const releases: GithubRelease[] = [];
    for (let page = 1; ; page += 1) {
      const read = await fetchReleasePage(page);
      releases.push(...read.releases);
      if (read.isLast) return releases;
    }
  },
  { name: "github-releases", getKey: () => "all", maxAge: RELEASE_LIST_SECONDS, swr: true },
);

async function openGithubAsset(address: string, range: ByteRange | undefined): Promise<OpenedReleaseFile> {
  let response: Response;
  try {
    response = await fetch(address, { headers: range === undefined ? {} : { Range: rangeHeader(range) } });
  } catch (cause) {
    if (cause instanceof TypeError) return { kind: FileOpening.HostUnreachable };
    throw cause;
  }
  if (!response.ok || response.body === null) {
    await response.body?.cancel();
    return { kind: FileOpening.HostRefused, status: response.status };
  }
  return {
    kind: FileOpening.Opened,
    body: Readable.from(response.body),
    range: response.status === PARTIAL_CONTENT ? range : undefined,
  };
}

export const githubReleaseSource: ReleaseSource = {
  listReleases: async () =>
    (await fetchGithubReleases()).map((release) => ({
      id: release.id,
      tag: release.tag,
      name: release.name,
      publishedAt: release.publishedAt,
      files: release.assets.map((asset) => ({
        name: asset.name,
        size: asset.size,
        open: (range) => openGithubAsset(asset.address, range),
      })),
    })),
};
