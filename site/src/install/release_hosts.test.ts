// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { FetchKind, type ReleaseFetch } from "../wasm/contract.ts";
import { hostsOfFetches } from "./release_hosts.ts";

test("each host lists the names of each kind of file once, in the order of the first fetch", () => {
  const fetches: ReleaseFetch[] = [
    { kind: FetchKind.BuildFile, address: "https://release.example/build/system.img" },
    { kind: FetchKind.BuildFile, address: "https://release.example/build/vendor.img" },
    { kind: FetchKind.SourceFiles, repository: "maker/blobs", address: "https://code.example/maker/blobs/1/" },
    { kind: FetchKind.SourceFiles, repository: "maker/blobs", address: "https://code.example/maker/blobs/2/" },
    { kind: FetchKind.StockFirmware, file_name: "stock.7z.001", address: "https://release.example/stock/stock.7z.001" },
    { kind: FetchKind.StockFirmware, file_name: "stock.7z.002", address: "https://release.example/stock/stock.7z.002" },
    { kind: FetchKind.SourceLfsObjects, repository: "maker/blobs", address: "http://localhost:8080/maker/blobs/1/" },
  ];

  assert.deepEqual(hostsOfFetches(fetches), [
    {
      host: "release.example",
      files: new Map([
        [FetchKind.BuildFile, []],
        [FetchKind.StockFirmware, ["stock.7z.001", "stock.7z.002"]],
      ]),
    },
    { host: "code.example", files: new Map([[FetchKind.SourceFiles, ["maker/blobs"]]]) },
    { host: "localhost:8080", files: new Map([[FetchKind.SourceLfsObjects, ["maker/blobs"]]]) },
  ]);
});
