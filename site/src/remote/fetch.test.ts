// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  HttpStatus,
  answerRange,
  lostConnection,
  provideFetch,
  provideXmlHttpRequest,
  servedFile,
  statusReply,
} from "../browser_doubles/web_requests.ts";
import { RangeReadFailure, TransferFailureKind, type PauseBeforeRetry } from "../wasm/contract.ts";
import { canDownload, canReadByRange, readRangeBlocking } from "./fetch.ts";
import { RequestScheduler } from "./request_scheduler.ts";

const FILE_HREF = "https://release.example/build/vendor.img";
const FILE = new Uint8Array([10, 11, 12, 13, 14, 15]);

const NO_PAUSE = (): undefined => undefined;

function scheduler(): RequestScheduler {
  return new RequestScheduler(() => 0);
}

test("a host can be read by range when it answers a range of the first byte with a partial answer", async (context) => {
  const sent = provideFetch(context, servedFile(FILE));
  assert.equal(await canReadByRange(FILE_HREF, NO_PAUSE, () => undefined, scheduler()), true);
  assert.deepEqual(
    sent.map(({ href, headers }) => [href, headers.get("range")]),
    [[FILE_HREF, "bytes=0-0"]],
  );
});

test("a host that sends the whole file for a range cannot be read by range", async (context) => {
  provideFetch(context, statusReply(HttpStatus.Ok));
  assert.equal(await canReadByRange(FILE_HREF, NO_PAUSE, () => undefined, scheduler()), false);
});

test("a host that fails the range cannot be read by range", async (context) => {
  provideFetch(context, statusReply(HttpStatus.NotFound));
  assert.equal(await canReadByRange(FILE_HREF, NO_PAUSE, () => undefined, scheduler()), false);
});

test("a file can be downloaded when the host answers with an ok status", async (context) => {
  provideFetch(context, servedFile(FILE));
  assert.equal(await canDownload(FILE_HREF, NO_PAUSE, () => undefined, scheduler()), true);
});

test("a file cannot be downloaded when the host answers with a failed status", async (context) => {
  provideFetch(context, statusReply(HttpStatus.NotFound));
  assert.equal(await canDownload(FILE_HREF, NO_PAUSE, () => undefined, scheduler()), false);
});

test("a check retries a lost connection after the pause given by the engine", async (context) => {
  let attempts = 0;
  provideFetch(context, (request) => {
    attempts += 1;
    return attempts <= 2 ? lostConnection(request) : servedFile(FILE)(request);
  });
  const retries: number[] = [];
  const pauseOnce: PauseBeforeRetry = (_failure, attempt) => (attempt === 1 ? 0 : undefined);
  assert.equal(await canDownload(FILE_HREF, pauseOnce, (failed) => retries.push(failed), scheduler()), true);
  assert.deepEqual(retries, [1]);
});

test("a blocking range read returns the bytes of the range", (context) => {
  const sent = provideXmlHttpRequest(context, (request) => answerRange(FILE, request.headers.get("range")));
  assert.deepEqual(readRangeBlocking(FILE_HREF, 2, 3), new Uint8Array([12, 13, 14]));
  assert.deepEqual(
    sent.map(({ method, href, headers }) => [method, href, headers.get("range")]),
    [["GET", FILE_HREF, "bytes=2-4"]],
  );
});

test("a blocking range read of no bytes sends no request", (context) => {
  const sent = provideXmlHttpRequest(context, (request) => answerRange(FILE, request.headers.get("range")));
  assert.deepEqual(readRangeBlocking(FILE_HREF, 4, 0), new Uint8Array(0));
  assert.equal(sent.length, 0);
});

test("a blocking range read fails with the status and retry-after of an answer that is not partial", (context) => {
  provideXmlHttpRequest(context, () => ({
    status: HttpStatus.ServiceUnavailable,
    body: new Uint8Array(0),
    headers: new Headers({ "retry-after": "5" }),
  }));
  assert.throws(
    () => readRangeBlocking(FILE_HREF, 0, 2),
    (failure) =>
      failure instanceof RangeReadFailure &&
      failure.failure.kind === TransferFailureKind.Status &&
      failure.failure.status === HttpStatus.ServiceUnavailable &&
      failure.failure.retryAfter === "5",
  );
});

test("a blocking range read that loses the connection fails as a lost connection", (context) => {
  provideXmlHttpRequest(context, () => {
    throw new DOMException("The request failed.", "NetworkError");
  });
  assert.throws(
    () => readRangeBlocking(FILE_HREF, 0, 2),
    (failure) => failure instanceof RangeReadFailure && failure.failure.kind === TransferFailureKind.Connection,
  );
});

test("a partial answer with other bytes than the range asked for cannot be read", (context) => {
  provideXmlHttpRequest(context, () => ({
    status: HttpStatus.PartialContent,
    body: FILE.slice(0, 1),
    headers: new Headers(),
  }));
  assert.throws(
    () => readRangeBlocking(FILE_HREF, 0, 2),
    (failure) => failure instanceof DOMException && failure.name === "NotReadableError",
  );
});
