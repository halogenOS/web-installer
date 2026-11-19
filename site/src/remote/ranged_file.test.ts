// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { answerRange, provideXmlHttpRequest } from "../browser_doubles/web_requests.ts";
import { RangedFile } from "./ranged_file.ts";

const FILE_HREF = "https://release.example/stock/stock.7z.001";
const FILE = new Uint8Array([20, 21, 22, 23, 24, 25, 26, 27]);

test("a remote file has the size it was given and reads each range from the host", (context) => {
  const sent = provideXmlHttpRequest(context, (request) => answerRange(FILE, request.headers.get("range")));
  const file = new RangedFile(FILE_HREF, FILE.length);
  assert.equal(file.size(), FILE.length);
  assert.deepEqual(file.read(5, 3), new Uint8Array([25, 26, 27]));
  assert.deepEqual(file.read(0, 2), new Uint8Array([20, 21]));
  assert.deepEqual(
    sent.map(({ href, headers }) => [href, headers.get("range")]),
    [
      [FILE_HREF, "bytes=5-7"],
      [FILE_HREF, "bytes=0-1"],
    ],
  );
});
