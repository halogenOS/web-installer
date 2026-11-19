// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { OriginPrivateFiles, provideOriginPrivateFiles } from "../browser_doubles/origin_private_files.ts";
import type { PhoneId } from "../device/usb.ts";
import {
  AskedFiles,
  BlockEdge,
  CheckVerdict,
  PhoneAnswer,
  PhoneCheckKind,
  Stage,
  StepKind,
  type InstallFailure,
  type InstallPlan,
} from "../wasm/contract.ts";
import { PageMessageKind, WorkerMessageKind, readPageMessage, readWorkerMessage } from "./messages.ts";

const PHONE: PhoneId = { vendorId: 0x18d1, productId: 0x4ee0, serialNumber: "707317ab" };
const FAILURE: InstallFailure = { cause: "The phone left.", nextStep: "Plug the phone in again." };
const PLAN: InstallPlan = {
  device: "guacamole",
  build: "xos-17.0-20261119-guacamole",
  shipped: ["boot"],
  built: ["vendor"],
  sources: [{ repository: "maker/blobs", commit: "0123abcd" }],
  stockFiles: [{ file_name: "stock.zip", address: "https://release.example/stock.zip", sha256: "00", size: 7 }],
  install: [{ step: StepKind.Flash, partition: "boot", logical: false }],
};

test("a progress message is read when it has the fields of its stage", () => {
  const progresses = [
    { stage: Stage.Connecting },
    { stage: Stage.Checking, ...PLAN },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Passed, reading: { check: PhoneCheckKind.Model, product: "OnePlus7Pro" } },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Failed, reading: { check: PhoneCheckKind.Bootloader, unlocked: PhoneAnswer.No } },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Failed, reading: { check: PhoneCheckKind.Slot, currentSlot: undefined } },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Passed, reading: { check: PhoneCheckKind.Partitions, fit: { fitting: 12, count: 12 } } },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Failed, reading: { check: PhoneCheckKind.Partitions, fit: undefined } },
    {
      stage: Stage.ChoosingFiles,
      asked: AskedFiles.StockFirmware,
      files: [{ file: PLAN.stockFiles[0], picked: false }],
      wrongPicks: [FAILURE],
    },
    { stage: Stage.FetchingStockFirmware, downloaded: 3, block: 1, readFrom: BlockEdge.End },
    { stage: Stage.Building, partition: "vendor", done: 1, total: 4 },
    { stage: Stage.Installing, index: 0, count: 1, step: PLAN.install[0] },
    { stage: Stage.ReleaseRead, ...PLAN },
  ];
  for (const progress of progresses) {
    assert.deepEqual(readWorkerMessage({ kind: WorkerMessageKind.Progress, progress }), {
      kind: WorkerMessageKind.Progress,
      progress,
    });
  }
});

test("a progress message that lacks a field of its stage, or names no stage, is not read", () => {
  const progresses = [
    { stage: Stage.Building, done: 1, total: 4 },
    { stage: Stage.Checking, ...PLAN, sources: [{ repository: "maker/blobs" }] },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Passed, reading: { check: PhoneCheckKind.Battery, socOk: "maybe" } },
    { stage: Stage.PhoneChecked, verdict: "unsure", reading: { check: PhoneCheckKind.Model, product: "OnePlus7Pro" } },
    { stage: Stage.PhoneChecked, verdict: CheckVerdict.Failed, reading: { check: PhoneCheckKind.Partitions, fit: { fitting: 11 } } },
    { stage: Stage.FetchingStockFirmware, downloaded: 3, block: 1, readFrom: "middle" },
    { stage: Stage.ChoosingFiles, asked: AskedFiles.ReleaseFiles, files: [], wrongPicks: [{ cause: "lost" }] },
    { stage: "flying" },
  ];
  for (const progress of progresses) {
    assert.equal(readWorkerMessage({ kind: WorkerMessageKind.Progress, progress }), undefined);
  }
});

test("a retry, phone text or failure message is read when it has its field", () => {
  const messages = [
    { kind: WorkerMessageKind.Retrying, failedAttempts: 2 },
    { kind: WorkerMessageKind.PhoneMessage, text: "erasing" },
    { kind: WorkerMessageKind.Failed, failure: FAILURE },
  ];
  for (const message of messages) assert.deepEqual(readWorkerMessage(message), message);
});

test("a worker message with an unknown kind or a field of the wrong type is not read", () => {
  const messages = [
    { kind: WorkerMessageKind.Retrying, failedAttempts: "2" },
    { kind: WorkerMessageKind.PhoneMessage },
    { kind: WorkerMessageKind.Failed, failure: { cause: "lost" } },
    { kind: "shout", text: "hello" },
    "progress",
    null,
  ];
  for (const message of messages) assert.equal(readWorkerMessage(message), undefined);
});

test("an install or a new choice of phone is read with the id of the phone", () => {
  const messages = [
    { kind: PageMessageKind.Install, phone: PHONE, releaseTag: "xos-17.0-20261119" },
    { kind: PageMessageKind.Install, phone: { ...PHONE, serialNumber: undefined }, releaseTag: "xos-17.0-20261119" },
    { kind: PageMessageKind.PhoneChosen, phone: PHONE },
  ];
  for (const message of messages) assert.deepEqual(readPageMessage(message), message);
});

test("an install without a phone id or a release tag is not read", () => {
  const messages = [
    { kind: PageMessageKind.Install, phone: { vendorId: PHONE.vendorId }, releaseTag: "xos-17.0-20261119" },
    { kind: PageMessageKind.Install, phone: PHONE },
    { kind: PageMessageKind.PhoneChosen },
    { kind: "uninstall", phone: PHONE },
  ];
  for (const message of messages) assert.equal(readPageMessage(message), undefined);
});

test("picked files are read only as a list of files", () => {
  const files = [new File([new Uint8Array([1])], "stock.zip")];
  assert.deepEqual(readPageMessage({ kind: PageMessageKind.FilesPicked, files }), {
    kind: PageMessageKind.FilesPicked,
    files,
  });
  assert.equal(readPageMessage({ kind: PageMessageKind.FilesPicked, files: ["stock.zip"] }), undefined);
});

test("a package to save is read with a file handle, a release tag and the ports of the compression workers", async (context) => {
  const files = provideOriginPrivateFiles(context, 1 << 20);
  const file = await files.root.getFileHandle("package.zip", { create: true });
  const channel = new MessageChannel();
  context.after(() => {
    channel.port1.close();
  });
  const message = {
    kind: PageMessageKind.SavePackage,
    file,
    releaseTag: "xos-17.0-20261119",
    compressionPorts: [channel.port1],
  };
  assert.deepEqual(readPageMessage(message), message);
  assert.equal(readPageMessage({ ...message, compressionPorts: [file] }), undefined);
  assert.equal(readPageMessage({ ...message, file: new OriginPrivateFiles(0).root }), undefined);
});
