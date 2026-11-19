// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import { provideOriginPrivateFiles } from "../browser_doubles/origin_private_files.ts";
import {
  HttpStatus,
  bodyBrokenAfter,
  provideFetch,
  replyInTurn,
  servedFile,
  statusReply,
  type Reply,
  type SentRequest,
} from "../browser_doubles/web_requests.ts";
import {
  TransferEnd,
  TransferFailureKind,
  type FileDigest,
  type PauseBeforeRetry,
  type RangeArrival,
} from "../wasm/contract.ts";
import { WorkerMessageKind, type WorkerMessage } from "./messages.ts";
import { WorkerStoragePage } from "./worker_storage_page.ts";

const IMAGE_HREF = "https://release.example/build/super.img";
const IMAGE = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);

class DigestDouble implements FileDigest {
  fed: number[] = [];
  frees = 0;

  restart(): void {
    this.fed = [];
  }

  update(piece: Uint8Array): void {
    this.fed.push(...piece);
  }

  free(): void {
    this.frees += 1;
  }
}

// The engine wants `wanted` bytes of a range and takes no byte past them.
class ArrivalDouble implements RangeArrival {
  readonly #wanted: number;
  #taken = 0;
  frees = 0;

  constructor(wanted: number) {
    this.#wanted = wanted;
  }

  restart(): void {
    this.#taken = 0;
  }

  take(piece: Uint8Array): number {
    const taken = Math.min(piece.length, this.#wanted - this.#taken);
    this.#taken += taken;
    return taken;
  }

  free(): void {
    this.frees += 1;
  }
}

const PAUSE_ON_LOST_CONNECTION: PauseBeforeRetry = (failure) =>
  failure.kind === TransferFailureKind.Connection ? 0 : undefined;

interface StoragePageOnHost {
  page: WorkerStoragePage;
  sent: SentRequest[];
  posted: WorkerMessage[];
}

function pageOnHost(context: TestContext, reply: Reply): StoragePageOnHost {
  provideOriginPrivateFiles(context, 1 << 20);
  const sent = provideFetch(context, reply);
  const posted: WorkerMessage[] = [];
  const page = new WorkerStoragePage({
    filesPicked: () => Promise.resolve([]),
    pauseBeforeRetry: PAUSE_ON_LOST_CONNECTION,
    post: (message) => posted.push(message),
  });
  return { page, sent, posted };
}

test("a download stores the body at its offset in the stored file and feeds every byte to the digest once", async (context) => {
  const { page } = pageOnHost(context, servedFile(IMAGE));
  const digest = new DigestDouble();
  await page.createFile("super.img", 12);
  assert.deepEqual(await page.download(IMAGE_HREF, "super.img", 2, IMAGE.length, digest), {
    kind: TransferEnd.Received,
    value: IMAGE.length,
  });
  assert.deepEqual(page.file("super.img").read(0, 12), new Uint8Array([0, 0, ...IMAGE, 0, 0]));
  assert.deepEqual(digest.fed, [...IMAGE]);
  assert.equal(digest.frees, 1);
});

test("a download broken in the middle asks for the missing bytes and goes on with the digest and the file", async (context) => {
  const { page, sent, posted } = pageOnHost(
    context,
    replyInTurn(bodyBrokenAfter(IMAGE.slice(0, 3), HttpStatus.Ok), servedFile(IMAGE)),
  );
  const digest = new DigestDouble();
  await page.createFile("super.img", IMAGE.length);
  assert.deepEqual(await page.download(IMAGE_HREF, "super.img", 0, IMAGE.length, digest), {
    kind: TransferEnd.Received,
    value: IMAGE.length,
  });
  assert.deepEqual(
    sent.map(({ headers }) => headers.get("range")),
    [null, "bytes=3-7"],
  );
  assert.deepEqual(page.file("super.img").read(0, IMAGE.length), IMAGE);
  assert.deepEqual(digest.fed, [...IMAGE]);
  assert.deepEqual(posted, [{ kind: WorkerMessageKind.Retrying, failedAttempts: 1 }]);
});

test("a host that answers the missing bytes with the whole file starts the digest and the file again", async (context) => {
  const whole: Reply = () => new Response(IMAGE, { status: HttpStatus.Ok });
  const { page } = pageOnHost(context, replyInTurn(bodyBrokenAfter(IMAGE.slice(0, 3), HttpStatus.Ok), whole));
  const digest = new DigestDouble();
  await page.createFile("super.img", IMAGE.length);
  assert.deepEqual(await page.download(IMAGE_HREF, "super.img", 0, IMAGE.length, digest), {
    kind: TransferEnd.Received,
    value: IMAGE.length,
  });
  assert.deepEqual(page.file("super.img").read(0, IMAGE.length), IMAGE);
  assert.deepEqual(digest.fed, [...IMAGE]);
});

test("a body longer than the download stores only its size and counts every byte it received", async (context) => {
  const { page } = pageOnHost(context, servedFile(IMAGE));
  await page.createFile("boot.img", 5);
  assert.deepEqual(await page.download(IMAGE_HREF, "boot.img", 0, 5, new DigestDouble()), {
    kind: TransferEnd.Received,
    value: IMAGE.length,
  });
  assert.equal(page.file("boot.img").size(), 5);
  assert.deepEqual(page.file("boot.img").read(0, 5), IMAGE.slice(0, 5));
});

test("a failed download frees the digest and returns the failure", async (context) => {
  const { page } = pageOnHost(context, statusReply(HttpStatus.NotFound));
  const digest = new DigestDouble();
  await page.createFile("boot.img", IMAGE.length);
  assert.deepEqual(await page.download(IMAGE_HREF, "boot.img", 0, IMAGE.length, digest), {
    kind: TransferEnd.Failed,
    failure: { kind: TransferFailureKind.Status, status: HttpStatus.NotFound, retryAfter: null },
  });
  assert.equal(digest.frees, 1);
});

test("a range download asks for its range and stores the bytes the engine takes at its offset", async (context) => {
  const { page, sent } = pageOnHost(context, servedFile(IMAGE));
  const arrival = new ArrivalDouble(3);
  await page.createFile("stock.7z", 6);
  assert.deepEqual(await page.downloadRange(IMAGE_HREF, "stock.7z", 2, 4, arrival), {
    kind: TransferEnd.Received,
    value: 3,
  });
  assert.deepEqual(
    sent.map(({ headers }) => headers.get("range")),
    ["bytes=2-5"],
  );
  assert.deepEqual(page.file("stock.7z").read(0, 6), new Uint8Array([0, 0, 3, 4, 5, 0]));
  assert.equal(arrival.frees, 1);
});

test("a range download broken in the middle starts the range and the arrival again", async (context) => {
  const { page } = pageOnHost(
    context,
    replyInTurn(bodyBrokenAfter(IMAGE.slice(2, 3), HttpStatus.PartialContent), servedFile(IMAGE)),
  );
  const arrival = new ArrivalDouble(4);
  await page.createFile("stock.7z", 4);
  assert.deepEqual(await page.downloadRange(IMAGE_HREF, "stock.7z", 0, 4, arrival), {
    kind: TransferEnd.Received,
    value: 4,
  });
  assert.deepEqual(page.file("stock.7z").read(0, 4), IMAGE.slice(0, 4));
});
