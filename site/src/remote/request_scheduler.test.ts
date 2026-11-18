// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { AttemptLoad, RequestScheduler, type ScheduledWork } from "./request_scheduler.ts";

const RELEASE_HOST = "https://release.example";
const BLOB_HOST = "https://blobs.example";

function settle(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

function testClock(): { now: () => number; set: (milliseconds: number) => void } {
  let time = 0;
  return {
    now: () => time,
    set: (milliseconds) => {
      time = milliseconds;
    },
  };
}

function heldAttempt(load: AttemptLoad): {
  work: ScheduledWork<void>;
  started: () => boolean;
  endRequest: () => Promise<void>;
  finish: () => Promise<void>;
} {
  let started = false;
  let endRequest = (): void => undefined;
  let finish = (): void => undefined;
  const ended = new Promise<void>((resolve) => {
    finish = resolve;
  });
  return {
    work: async (end) => {
      started = true;
      endRequest = end;
      await ended;
      return { value: undefined, load };
    },
    started: () => started,
    endRequest: async () => {
      endRequest();
      await settle();
    },
    finish: async () => {
      finish();
      await settle();
    },
  };
}

test("a host starts with one attempt, and a full window grows when its request did not wait", async () => {
  const clock = testClock();
  const scheduler = new RequestScheduler(clock.now);
  const attempts = [1, 2, 3].map(() => heldAttempt(AttemptLoad.Delivered));
  for (const attempt of attempts) void scheduler.run(RELEASE_HOST, attempt.work);
  await settle();
  assert.deepEqual(attempts.map((attempt) => attempt.started()), [true, false, false]);
  clock.set(5);
  await attempts[0]?.finish();
  assert.deepEqual(attempts.map((attempt) => attempt.started()), [true, true, true]);
});

test("an attempt that runs alone waits for the others, and no other attempt starts before its request ends", async () => {
  const scheduler = new RequestScheduler(testClock().now);
  const running = heldAttempt(AttemptLoad.Delivered);
  const alone = heldAttempt(AttemptLoad.Delivered);
  const later = heldAttempt(AttemptLoad.Delivered);
  void scheduler.run(RELEASE_HOST, running.work);
  void scheduler.runAlone(BLOB_HOST, alone.work);
  void scheduler.run(RELEASE_HOST, later.work);
  await settle();
  assert.equal(alone.started(), false);
  await running.finish();
  assert.equal(alone.started(), true);
  assert.equal(later.started(), false);
  await alone.endRequest();
  assert.equal(later.started(), true);
});

test("a round whose requests waited shrinks the window by one", async () => {
  const clock = testClock();
  const scheduler = new RequestScheduler(clock.now);
  const attempts = [1, 2, 3, 4, 5].map(() => heldAttempt(AttemptLoad.Delivered));
  for (const attempt of attempts) void scheduler.run(RELEASE_HOST, attempt.work);
  await settle();
  clock.set(5);
  await attempts[0]?.finish();
  clock.set(20);
  await attempts[1]?.finish();
  assert.equal(attempts[3]?.started(), true);
  await attempts[2]?.finish();
  assert.equal(attempts[4]?.started(), false);
  await attempts[3]?.finish();
  assert.equal(attempts[4]?.started(), true);
});

test("failed requests that ran together halve the window once", async () => {
  const clock = testClock();
  const scheduler = new RequestScheduler(clock.now);
  const growing = [1, 2, 3, 4].map(() => heldAttempt(AttemptLoad.Delivered));
  for (const attempt of growing) void scheduler.run(RELEASE_HOST, attempt.work);
  await settle();
  clock.set(5);
  await growing[0]?.finish();
  clock.set(10);
  for (const attempt of growing) await attempt.finish();
  const failing = [1, 2, 3].map(() => heldAttempt(AttemptLoad.RequestFailed));
  const waiting = [1, 2, 3].map(() => heldAttempt(AttemptLoad.Delivered));
  for (const attempt of [...failing, ...waiting]) void scheduler.run(RELEASE_HOST, attempt.work);
  await settle();
  assert.deepEqual(failing.map((attempt) => attempt.started()), [true, true, true]);
  for (const attempt of failing) await attempt.finish();
  assert.deepEqual(waiting.map((attempt) => attempt.started()), [true, true, false]);
});

async function runAgainstHost(capacity: number, requestTimes: number[]): Promise<number[]> {
  const clock = testClock();
  const scheduler = new RequestScheduler(clock.now);
  const queued: (() => void)[] = [];
  const answering: { at: number; answer: () => void }[] = [];
  const answerQueued = (): void => {
    for (let next = queued.shift(); next !== undefined; next = queued.shift()) {
      if (answering.length === capacity) {
        queued.unshift(next);
        return;
      }
      next();
    }
  };
  const runningAtStarts: number[] = [];
  let running = 0;
  for (const requestTime of requestTimes) {
    void scheduler.run(RELEASE_HOST, async (endRequest) => {
      running += 1;
      runningAtStarts.push(running);
      await new Promise<void>((answered) => {
        queued.push(() => answering.push({ at: clock.now() + requestTime, answer: answered }));
        answerQueued();
      });
      endRequest();
      running -= 1;
      return { value: undefined, load: AttemptLoad.Delivered };
    });
  }
  await settle();
  for (let next = answering.shift(); next !== undefined; next = answering.shift()) {
    clock.set(next.at);
    next.answer();
    answerQueued();
    answering.sort((first, second) => first.at - second.at);
    await settle();
  }
  return runningAtStarts;
}

test("the window of a host settles at the number of requests that the host answers at a time", async () => {
  const runningAtStarts = await runAgainstHost(6, Array.from({ length: 400 }, () => 10));
  const settled = runningAtStarts.slice(200);
  assert.ok(Math.min(...settled) >= 6, `${String(Math.min(...settled))} attempts ran`);
  assert.ok(Math.max(...settled) <= 8, `${String(Math.max(...settled))} attempts ran`);
});

test("one fast first request does not keep the window of a host small", async () => {
  const runningAtStarts = await runAgainstHost(6, [1, ...Array.from({ length: 400 }, () => 30)]);
  assert.ok(Math.max(...runningAtStarts.slice(200)) >= 6);
});
