// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DeviceStep,
  UsbDeviceDouble,
  UsbSystemDouble,
  fastbootConfiguration,
  type DeviceIds,
} from "../browser_doubles/web_usb.ts";
import {
  PhoneNotReleased,
  PhoneProblem,
  UsbErrorName,
  browserUsb,
  type PhoneId,
  type UsbSystem,
} from "../device/usb.ts";
import { OpeningKind, type PageLink, type PhoneOpening } from "../wasm/contract.ts";
import { WorkerMessageKind, type WorkerMessage } from "./messages.ts";
import { WorkerInstallPage } from "./worker_install_page.ts";

const PHONE: DeviceIds = { vendorId: 0x18d1, productId: 0x4ee0, serialNumber: "707317ab" };
const PHONE_ID: PhoneId = { vendorId: PHONE.vendorId, productId: PHONE.productId, serialNumber: "707317ab" };
const OTHER_PHONE: DeviceIds = { ...PHONE, serialNumber: "1b2c3d4e" };
const OTHER_PHONE_ID: PhoneId = { ...PHONE_ID, serialNumber: "1b2c3d4e" };

function fastbootPhone(ids: DeviceIds = PHONE): UsbDeviceDouble {
  return new UsbDeviceDouble(ids, { configurations: [fastbootConfiguration(1, 0)], configurationValue: 1 });
}

function settle(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

interface PageOnUsb {
  page: WorkerInstallPage;
  posted: WorkerMessage[];
  choosePhone: (phone: PhoneId) => void;
}

function pageOnUsb(findUsb: () => UsbSystem): PageOnUsb {
  const posted: WorkerMessage[] = [];
  let choosePhone: (phone: PhoneId) => void = () => undefined;
  const chosen = new Promise<PhoneId>((resolve) => {
    choosePhone = resolve;
  });
  const page = new WorkerInstallPage({
    findUsb,
    phone: PHONE_ID,
    phoneChosenAgain: () => chosen,
    filesPicked: () => Promise.resolve([]),
    pauseBeforeRetry: () => undefined,
    post: (message) => posted.push(message),
  });
  return { page, posted, choosePhone };
}

function linkOf(opening: PhoneOpening): PageLink {
  assert.ok(OpeningKind.Opened in opening);
  return opening[OpeningKind.Opened];
}

test("opening a plugged phone opens a link to it", async () => {
  const phone = fastbootPhone();
  const { page } = pageOnUsb(() => new UsbSystemDouble([phone]));
  await linkOf(await page.openPhone()).send(new Uint8Array([1]));
  assert.deepEqual(phone.sent.map(({ bytes }) => [...bytes]), [[1]]);
});

test("a phone that cannot be opened is unavailable with its problem", async () => {
  const { page: missing } = pageOnUsb(() => new UsbSystemDouble([]));
  const { page: withoutUsb } = pageOnUsb(() => browserUsb({ navigator: {} }));
  assert.deepEqual(await missing.openPhone(), { [OpeningKind.Unavailable]: PhoneProblem.NotFound });
  assert.deepEqual(await withoutUsb.openPhone(), { [OpeningKind.Unavailable]: PhoneProblem.NoWebUsb });
});

test("a failure other than an unavailable phone goes on to the engine", async () => {
  const phone = fastbootPhone();
  phone.refuse(DeviceStep.ClaimInterface, UsbErrorName.Network);
  phone.refuse(DeviceStep.Close, UsbErrorName.Network);
  const { page } = pageOnUsb(() => new UsbSystemDouble([phone]));
  await assert.rejects(page.openPhone(), PhoneNotReleased);
});

test("a phone cannot be reopened before it was opened", async () => {
  const { page } = pageOnUsb(() => new UsbSystemDouble([fastbootPhone()]));
  await assert.rejects(page.reopenPhone(), /reopened before it was opened/);
});

test("reopening closes the link, waits for the phone to come back and opens it again under one watch", async () => {
  const phone = fastbootPhone();
  const usb = new UsbSystemDouble([phone]);
  const { page } = pageOnUsb(() => usb);
  linkOf(await page.openPhone());
  const watching = usb.listenerCount;
  const reopened = page.reopenPhone();
  await settle();
  assert.equal(phone.opened, false);
  usb.unplug(phone);
  const returned = fastbootPhone();
  usb.plug(returned);
  await linkOf(await reopened).send(new Uint8Array([2]));
  assert.deepEqual(returned.sent.map(({ bytes }) => [...bytes]), [[2]]);
  assert.equal(usb.listenerCount, watching);
});

test("a phone chosen again while the page waits is the phone that opens", async () => {
  const phone = fastbootPhone();
  const other = fastbootPhone(OTHER_PHONE);
  const usb = new UsbSystemDouble([phone, other]);
  const { page, choosePhone } = pageOnUsb(() => usb);
  linkOf(await page.openPhone());
  const reopened = page.reopenPhone();
  choosePhone(OTHER_PHONE_ID);
  await linkOf(await reopened).send(new Uint8Array([3]));
  assert.deepEqual(other.sent.map(({ bytes }) => [...bytes]), [[3]]);
  assert.equal(phone.opened, false);
});

test("a message of the phone goes to the page", () => {
  const { page, posted } = pageOnUsb(() => new UsbSystemDouble([]));
  page.showPhoneMessage("erasing userdata");
  assert.deepEqual(posted, [{ kind: WorkerMessageKind.PhoneMessage, text: "erasing userdata" }]);
});
