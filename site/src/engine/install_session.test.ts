// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import { provideGlobal } from "../browser_doubles/globals.ts";
import { provideOriginPrivateFiles } from "../browser_doubles/origin_private_files.ts";
import type { PhoneId } from "../device/usb.ts";
import { InstallSession, type InstallEvents } from "./install_session.ts";
import { PageMessageKind, WorkerMessageKind, readPageMessage, type PageMessage, type WorkerMessage } from "./messages.ts";

const PHONE: PhoneId = { vendorId: 0x18d1, productId: 0x4ee0, serialNumber: "707317ab" };
const RELEASE_TAG = "xos-17.0-20261119";

// What a worker gets from the page: the engine worker gets messages of the page, and a compression worker gets the
// port on which it compresses.
type WorkerInput = PageMessage | MessagePort;

class WorkerDouble {
  readonly received: WorkerInput[] = [];
  onmessage: ((event: MessageEvent<object>) => void) | null = null;
  onerror: ((event: { message: string }) => void) | null = null;
  terminated = false;

  postMessage(message: unknown): void {
    const input = message instanceof MessagePort ? message : readPageMessage(message);
    assert.ok(input !== undefined, "The page sent a worker a message that no worker reads.");
    this.received.push(input);
  }

  terminate(): void {
    this.terminated = true;
  }

  sendToPage(data: object): void {
    this.onmessage?.(new MessageEvent("message", { data }));
  }

  fail(message: string): void {
    this.onerror?.({ message });
  }
}

interface StartedWorkers {
  all: WorkerDouble[];
  engine: () => WorkerDouble;
  compression: () => WorkerDouble[];
}

function provideWorkers(context: TestContext): StartedWorkers {
  const all: WorkerDouble[] = [];
  provideGlobal(
    context,
    globalThis,
    "Worker",
    class extends WorkerDouble {
      constructor() {
        super();
        all.push(this);
      }
    },
  );
  const isEngine = (worker: WorkerDouble): boolean => !(worker.received[0] instanceof MessagePort);
  context.after(() => {
    for (const worker of all) {
      for (const input of worker.received) {
        const ports = input instanceof MessagePort ? [input] : input.kind === PageMessageKind.SavePackage ? input.compressionPorts : [];
        for (const port of ports) port.close();
      }
    }
  });
  return {
    all,
    engine: () => {
      const engines = all.filter(isEngine);
      assert.equal(engines.length, 1);
      const [engine] = engines;
      assert.ok(engine !== undefined);
      return engine;
    },
    compression: () => all.filter((worker) => !isEngine(worker)),
  };
}

interface RecordedEvents {
  events: InstallEvents;
  messages: WorkerMessage[];
  crashes: string[];
}

function recordEvents(): RecordedEvents {
  const messages: WorkerMessage[] = [];
  const crashes: string[] = [];
  return {
    events: { onMessage: (message) => messages.push(message), onCrash: (report) => crashes.push(report) },
    messages,
    crashes,
  };
}

test("an install starts the engine worker with the phone and the release, and no compression worker", (context) => {
  const workers = provideWorkers(context);
  InstallSession.install(PHONE, RELEASE_TAG, recordEvents().events);
  assert.equal(workers.all.length, 1);
  assert.deepEqual(workers.engine().received, [{ kind: PageMessageKind.Install, phone: PHONE, releaseTag: RELEASE_TAG }]);
});

test("a message of the engine worker that the page reads goes to the page, and any other is a crash", (context) => {
  const workers = provideWorkers(context);
  const recorded = recordEvents();
  InstallSession.install(PHONE, RELEASE_TAG, recorded.events);
  const retrying = { kind: WorkerMessageKind.Retrying, failedAttempts: 1 };
  workers.engine().sendToPage(retrying);
  workers.engine().sendToPage({ kind: WorkerMessageKind.Retrying });
  assert.deepEqual(recorded.messages, [retrying]);
  assert.equal(recorded.crashes.length, 1);
});

test("an error in a worker is a crash with the message of the error", async (context) => {
  const files = provideOriginPrivateFiles(context, 1 << 20);
  provideGlobal(context, navigator, "hardwareConcurrency", 1);
  const workers = provideWorkers(context);
  const recorded = recordEvents();
  const file = await files.root.getFileHandle("package.zip", { create: true });
  InstallSession.savePackage(file, RELEASE_TAG, recorded.events);
  workers.engine().fail("The engine ran out of memory.");
  workers.compression()[0]?.fail("A compressor ran out of memory.");
  assert.deepEqual(recorded.crashes, ["The engine ran out of memory.", "A compressor ran out of memory."]);
});

test("saving a package starts one compression worker for each hardware thread, each linked to the engine worker by a channel", async (context) => {
  const files = provideOriginPrivateFiles(context, 1 << 20);
  provideGlobal(context, navigator, "hardwareConcurrency", 3);
  const workers = provideWorkers(context);
  const file = await files.root.getFileHandle("package.zip", { create: true });
  InstallSession.savePackage(file, RELEASE_TAG, recordEvents().events);
  const [start] = workers.engine().received;
  assert.ok(start !== undefined && !(start instanceof MessagePort) && start.kind === PageMessageKind.SavePackage);
  assert.equal(start.file, file);
  assert.equal(start.releaseTag, RELEASE_TAG);
  const compressionPorts = workers.compression().map((worker) => worker.received);
  assert.equal(compressionPorts.length, 3);
  assert.equal(start.compressionPorts.length, 3);
  for (const [index, enginePort] of start.compressionPorts.entries()) {
    const [workerPort] = compressionPorts[index] ?? [];
    assert.ok(workerPort instanceof MessagePort);
    const arrived = new Promise<number | undefined>((resolve) => {
      workerPort.onmessage = (event: MessageEvent<unknown>) => {
        resolve(typeof event.data === "number" ? event.data : undefined);
      };
    });
    enginePort.postMessage(index);
    assert.equal(await arrived, index);
  }
});

test("a new choice of phone and picked files go to the engine worker", (context) => {
  const workers = provideWorkers(context);
  const session = InstallSession.install(PHONE, RELEASE_TAG, recordEvents().events);
  const files = [new File([new Uint8Array([1])], "stock.zip")];
  session.choosePhoneAgain(PHONE);
  session.pickFiles(files);
  assert.deepEqual(workers.engine().received.slice(1), [
    { kind: PageMessageKind.PhoneChosen, phone: PHONE },
    { kind: PageMessageKind.FilesPicked, files },
  ]);
});

test("ending a session stops the engine worker and every compression worker", async (context) => {
  const files = provideOriginPrivateFiles(context, 1 << 20);
  provideGlobal(context, navigator, "hardwareConcurrency", 2);
  const workers = provideWorkers(context);
  const session = InstallSession.savePackage(
    await files.root.getFileHandle("package.zip", { create: true }),
    RELEASE_TAG,
    recordEvents().events,
  );
  session.end();
  assert.equal(workers.all.length, 3);
  assert.ok(workers.all.every((worker) => worker.terminated));
});
