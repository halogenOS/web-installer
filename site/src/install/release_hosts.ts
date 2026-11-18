// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { FetchKind, type ReleaseFetch } from "../wasm/contract.ts";

export interface FetchedHost {
  host: string;
  files: Map<FetchKind, string[]>;
}

function nameOf(fetch: ReleaseFetch): string | undefined {
  switch (fetch.kind) {
    case FetchKind.BuildFile:
      return undefined;
    case FetchKind.SourceFiles:
    case FetchKind.SourceLfsObjects:
      return fetch.repository;
    case FetchKind.StockFirmware:
      return fetch.file_name;
  }
}

export function hostsOfFetches(fetches: readonly ReleaseFetch[]): FetchedHost[] {
  const hosts = new Map<string, Map<FetchKind, string[]>>();
  for (const fetch of fetches) {
    const host = new URL(fetch.address).host;
    const files = hosts.get(host) ?? new Map<FetchKind, string[]>();
    hosts.set(host, files);
    const names = files.get(fetch.kind) ?? [];
    files.set(fetch.kind, names);
    const name = nameOf(fetch);
    if (name !== undefined && !names.includes(name)) names.push(name);
  }
  return [...hosts].map(([host, files]) => ({ host, files }));
}
