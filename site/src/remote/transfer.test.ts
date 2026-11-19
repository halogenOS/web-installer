// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  HttpStatus,
  bodyBrokenAfter,
  lostConnection,
  replyInTurn,
  sendRequests,
  servedFile,
  statusReply,
  type Reply,
  type SentRequest,
} from "../browser_doubles/web_requests.ts";
import { TransferEnd, TransferFailureKind, type PauseBeforeRetry, type TransferFailure } from "../wasm/contract.ts";
import { RequestScheduler } from "./request_scheduler.ts";
import {
  fetchRangeWithRetries,
  fetchWithRetries,
  requestStatusWithRetries,
  type BodySink,
  type RemotePorts,
} from "./transfer.ts";

const FILE_HREF = "https://release.example/build/boot.img";
const FILE = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);

interface Network {
  ports: RemotePorts;
  sent: SentRequest[];
  waits: number[];
  failures: TransferFailure[];
}

// The engine decides the pauses. Each test hands the pause it wants for each kind of failure, and a kind without a
// pause ends the transfer.
function network(reply: Reply, pauses: Partial<Record<TransferFailure["kind"], number>>): Network {
  const sent: SentRequest[] = [];
  const waits: number[] = [];
  const failures: TransferFailure[] = [];
  const pauseBeforeRetry: PauseBeforeRetry = (failure) => {
    failures.push(failure);
    return pauses[failure.kind];
  };
  return {
    ports: {
      request: sendRequests(reply, sent),
      pauseBeforeRetry,
      wait: async (milliseconds) => {
        waits.push(milliseconds);
      },
      scheduler: new RequestScheduler(() => 0),
    },
    sent,
    waits,
    failures,
  };
}

interface CollectedBody {
  sink: BodySink<number[]>;
  begun: () => number;
}

function collectBody(wanted = Infinity): CollectedBody {
  let bytes: number[] = [];
  let begun = 0;
  return {
    sink: {
      begin: () => {
        begun += 1;
        bytes = [];
      },
      chunk: (chunk) => {
        bytes.push(...chunk);
        return bytes.length < wanted;
      },
      end: async () => bytes,
    },
    begun: () => begun,
  };
}

function methodsAndModes(sent: SentRequest[]): (string | undefined)[][] {
  return sent.map(({ init }) => [init.method, init.mode]);
}

test("a transfer asks without credentials and referrer, and returns what the sink makes of the whole body", async () => {
  const { ports, sent } = network(servedFile(FILE), {});
  const { sink } = collectBody();
  const result = await fetchWithRetries({ href: FILE_HREF, headers: () => ({}), sink, reportRetry: () => undefined }, ports);
  assert.deepEqual(result, { kind: TransferEnd.Received, value: [...FILE] });
  assert.deepEqual(
    sent.map(({ init }) => [init.method, init.mode, init.credentials, init.referrerPolicy]),
    [["GET", "cors", "omit", "no-referrer"]],
  );
});

test("a lost connection waits the pause given by the engine, reports the retry and asks again with headers read after it", async () => {
  const { ports, sent, waits } = network(replyInTurn(lostConnection, lostConnection, servedFile(FILE)), {
    [TransferFailureKind.Connection]: 30,
  });
  const { sink } = collectBody();
  const retries: number[] = [];
  const result = await fetchWithRetries(
    {
      href: FILE_HREF,
      headers: () => ({ "x-retries": String(retries.length) }),
      sink,
      reportRetry: (failedAttempts) => retries.push(failedAttempts),
    },
    ports,
  );
  assert.deepEqual(result, { kind: TransferEnd.Received, value: [...FILE] });
  assert.deepEqual(waits, [30]);
  assert.deepEqual(retries, [1]);
  assert.deepEqual(
    sent.map(({ init, headers }) => [init.method, headers.get("x-retries")]),
    [
      ["GET", "0"],
      ["HEAD", "0"],
      ["GET", "1"],
    ],
  );
});

test("a body that breaks counts as a lost connection, and the sink begins again with the next attempt", async () => {
  const { ports, failures } = network(replyInTurn(bodyBrokenAfter(FILE.slice(0, 3), HttpStatus.Ok), servedFile(FILE)), {
    [TransferFailureKind.Connection]: 0,
  });
  const body = collectBody();
  const result = await fetchWithRetries(
    { href: FILE_HREF, headers: () => ({}), sink: body.sink, reportRetry: () => undefined },
    ports,
  );
  assert.deepEqual(failures, [{ kind: TransferFailureKind.Connection }]);
  assert.equal(body.begun(), 2);
  assert.deepEqual(result, { kind: TransferEnd.Received, value: [...FILE] });
});

test("a status without a pause from the engine ends the transfer with the status and its retry-after", async () => {
  const { ports } = network(statusReply(HttpStatus.ServiceUnavailable, { "retry-after": "120" }), {});
  const { sink } = collectBody();
  const result = await fetchWithRetries({ href: FILE_HREF, headers: () => ({}), sink, reportRetry: () => undefined }, ports);
  assert.deepEqual(result, {
    kind: TransferEnd.Failed,
    failure: { kind: TransferFailureKind.Status, status: HttpStatus.ServiceUnavailable, retryAfter: "120" },
  });
});

test("a range transfer takes only a partial answer, and a whole file sent for a range is a status failure", async () => {
  const range = (): Record<string, string> => ({ Range: "bytes=2-4" });
  const partial = await fetchRangeWithRetries(
    { href: FILE_HREF, headers: range, sink: collectBody().sink, reportRetry: () => undefined },
    network(servedFile(FILE), {}).ports,
  );
  const whole = await fetchRangeWithRetries(
    { href: FILE_HREF, headers: range, sink: collectBody().sink, reportRetry: () => undefined },
    network(statusReply(HttpStatus.Ok), {}).ports,
  );
  assert.deepEqual(partial, { kind: TransferEnd.Received, value: [3, 4, 5] });
  assert.deepEqual(whole, {
    kind: TransferEnd.Failed,
    failure: { kind: TransferFailureKind.Status, status: HttpStatus.Ok, retryAfter: null },
  });
});

test("a sink that has all it needs ends the transfer and cancels the rest of the body", async () => {
  let cancelled = false;
  const endless: Reply = () =>
    new Response(
      new ReadableStream<Uint8Array<ArrayBuffer>>({
        pull: (controller) => {
          controller.enqueue(new Uint8Array([9, 9]));
        },
        cancel: () => {
          cancelled = true;
        },
      }),
      { status: HttpStatus.Ok },
    );
  const { sink } = collectBody(4);
  const result = await fetchWithRetries(
    { href: FILE_HREF, headers: () => ({}), sink, reportRetry: () => undefined },
    network(endless, {}).ports,
  );
  assert.deepEqual(result, { kind: TransferEnd.Received, value: [9, 9, 9, 9] });
  assert.equal(cancelled, true);
});

test("a GET that fails where a no-cors HEAD reaches the host counts as refused across origins, and runs again alone", async () => {
  const { ports, sent, failures } = network(
    replyInTurn(lostConnection, statusReply(HttpStatus.Ok), lostConnection, servedFile(FILE)),
    {},
  );
  const { sink } = collectBody();
  const result = await fetchWithRetries({ href: FILE_HREF, headers: () => ({}), sink, reportRetry: () => undefined }, ports);
  assert.deepEqual(result, { kind: TransferEnd.Received, value: [...FILE] });
  assert.deepEqual(failures, [{ kind: TransferFailureKind.CrossOriginRefused }]);
  assert.deepEqual(methodsAndModes(sent), [
    ["GET", "cors"],
    ["HEAD", "no-cors"],
    ["GET", "cors"],
    ["GET", "cors"],
  ]);
});

test("a refusal across origins without a pause from the engine ends the transfer as refused, after the run alone", async () => {
  const { ports, sent } = network(
    replyInTurn(
      lostConnection,
      statusReply(HttpStatus.Ok),
      lostConnection,
      lostConnection,
      statusReply(HttpStatus.Ok),
      lostConnection,
    ),
    {},
  );
  const { sink } = collectBody();
  const result = await fetchWithRetries({ href: FILE_HREF, headers: () => ({}), sink, reportRetry: () => undefined }, ports);
  assert.deepEqual(result, { kind: TransferEnd.Failed, failure: { kind: TransferFailureKind.CrossOriginRefused } });
  assert.equal(sent.length, 6);
});

test("a lost connection without a pause from the engine is a fault of the engine", async () => {
  const { ports } = network(lostConnection, {});
  const { sink } = collectBody();
  await assert.rejects(
    fetchWithRetries({ href: FILE_HREF, headers: () => ({}), sink, reportRetry: () => undefined }, ports),
    /no pause for a connection that failed/,
  );
});

test("a status request returns the status of an ok answer and the failure of any other", async () => {
  const ok = await requestStatusWithRetries(
    { href: FILE_HREF, headers: { Range: "bytes=0-0" }, reportRetry: () => undefined },
    network(servedFile(FILE), {}).ports,
  );
  const missing = await requestStatusWithRetries(
    { href: FILE_HREF, headers: {}, reportRetry: () => undefined },
    network(statusReply(HttpStatus.NotFound), {}).ports,
  );
  assert.deepEqual(ok, { kind: TransferEnd.Received, value: HttpStatus.PartialContent });
  assert.deepEqual(missing, {
    kind: TransferEnd.Failed,
    failure: { kind: TransferFailureKind.Status, status: HttpStatus.NotFound, retryAfter: null },
  });
});
