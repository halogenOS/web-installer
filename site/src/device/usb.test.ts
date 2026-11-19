// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DeviceStep,
  FASTBOOT_FILTER,
  UsbDeviceDouble,
  UsbSystemDouble,
  adbConfiguration,
  fastbootConfiguration,
  type DeviceIds,
} from "../browser_doubles/web_usb.ts";
import {
  PhoneLink,
  PhoneNotReleased,
  PhoneProblem,
  PhoneUnavailable,
  PhoneWatch,
  TransferRefused,
  UNNAMED_PHONE_LABEL,
  UsbErrorName,
  UsbTransferStatus,
  browserUsb,
  isPhoneId,
  requestPhone,
  usbAvailable,
  windowUsb,
  type PhoneId,
  type UsbSystem,
} from "./usb.ts";

const PHONE: DeviceIds = { vendorId: 0x18d1, productId: 0x4ee0, serialNumber: "707317ab", productName: "Pixel" };
const PHONE_ID: PhoneId = { vendorId: PHONE.vendorId, productId: PHONE.productId, serialNumber: "707317ab" };
const OTHER_PHONE: DeviceIds = { ...PHONE, serialNumber: "1b2c3d4e" };
const FASTBOOT_INTERFACE = 1;

function fastbootPhone(ids: DeviceIds = PHONE): UsbDeviceDouble {
  return new UsbDeviceDouble(ids, { configurations: [fastbootConfiguration(1, FASTBOOT_INTERFACE)], configurationValue: 1 });
}

function isUnavailableAs(problem: PhoneProblem): (failure: unknown) => failure is PhoneUnavailable {
  return (failure): failure is PhoneUnavailable => failure instanceof PhoneUnavailable && failure.problem === problem;
}

function settle(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

test("a scope has USB when its navigator has a USB system, and only a window can ask for a device", () => {
  const usb = new UsbSystemDouble([]);
  const workerUsb: UsbSystem = {
    getDevices: () => usb.getDevices(),
    addEventListener: (type, listener) => {
      usb.addEventListener(type, listener);
    },
    removeEventListener: (type, listener) => {
      usb.removeEventListener(type, listener);
    },
  };
  assert.equal(usbAvailable({ navigator: {} }), false);
  assert.equal(usbAvailable({ navigator: { usb } }), true);
  assert.equal(browserUsb({ navigator: { usb: workerUsb } }), workerUsb);
  assert.equal(windowUsb({ navigator: { usb } }), usb);
  assert.throws(() => windowUsb({ navigator: { usb: workerUsb } }), isUnavailableAs(PhoneProblem.NoWebUsb));
  assert.throws(() => browserUsb({ navigator: {} }), isUnavailableAs(PhoneProblem.NoWebUsb));
});

test("a phone id is vendor and product numbers with a serial number or none", () => {
  assert.equal(isPhoneId(PHONE_ID), true);
  assert.equal(isPhoneId({ ...PHONE_ID, serialNumber: undefined }), true);
  assert.equal(isPhoneId({ ...PHONE_ID, serialNumber: 7 }), false);
  assert.equal(isPhoneId({ vendorId: PHONE.vendorId, serialNumber: undefined }), false);
  assert.equal(isPhoneId(null), false);
});

test("asking for a phone offers devices with a fastboot interface and returns the id and name of the chosen one", async () => {
  const usb = new UsbSystemDouble([]);
  usb.choose = () => Promise.resolve(fastbootPhone());
  assert.deepEqual(await requestPhone(usb), { id: PHONE_ID, label: "Pixel" });
  assert.deepEqual(usb.requestedFilters, [[FASTBOOT_FILTER]]);
});

test("a chosen phone without a name gets the label of an unnamed phone", async () => {
  const usb = new UsbSystemDouble([]);
  usb.choose = () => Promise.resolve(fastbootPhone({ ...PHONE, productName: "" }));
  assert.deepEqual(await requestPhone(usb), { id: PHONE_ID, label: UNNAMED_PHONE_LABEL });
});

test("a device chooser closed without a choice chooses no phone", async () => {
  assert.equal(await requestPhone(new UsbSystemDouble([])), undefined);
});

test("a device chooser that the browser forbids means the page has no WebUSB", async () => {
  const usb = new UsbSystemDouble([]);
  usb.choose = () => Promise.reject(new DOMException("USB is blocked.", UsbErrorName.Security));
  await assert.rejects(requestPhone(usb), isUnavailableAs(PhoneProblem.NoWebUsb));
});

test("opening a phone claims the fastboot interface of the device with its ids", async () => {
  const phone = fastbootPhone();
  const other = fastbootPhone(OTHER_PHONE);
  await PhoneLink.open(new UsbSystemDouble([other, phone]), PHONE_ID);
  assert.equal(phone.opened, true);
  assert.deepEqual(phone.claimedInterfaces, [FASTBOOT_INTERFACE]);
  assert.equal(other.opened, false);
});

test("opening a phone without a configuration selects its first one", async () => {
  const phone = new UsbDeviceDouble(PHONE, {
    configurations: [fastbootConfiguration(3, FASTBOOT_INTERFACE), adbConfiguration(4)],
    configurationValue: 0,
  });
  await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  assert.equal(phone.configuration?.configurationValue, 3);
  assert.deepEqual(phone.claimedInterfaces, [FASTBOOT_INTERFACE]);
});

test("a phone that is not plugged in is not found", async () => {
  await assert.rejects(PhoneLink.open(new UsbSystemDouble([]), PHONE_ID), isUnavailableAs(PhoneProblem.NotFound));
});

test("a phone whose ids two devices have cannot be told apart", async () => {
  await assert.rejects(
    PhoneLink.open(new UsbSystemDouble([fastbootPhone(), fastbootPhone()]), PHONE_ID),
    isUnavailableAs(PhoneProblem.Several),
  );
});

test("a device without configurations or without a fastboot interface has no fastboot interface, and is closed again", async () => {
  const bare = new UsbDeviceDouble(PHONE, { configurations: [], configurationValue: 0 });
  const adb = new UsbDeviceDouble(PHONE, { configurations: [adbConfiguration(1)], configurationValue: 1 });
  for (const device of [bare, adb]) {
    await assert.rejects(
      PhoneLink.open(new UsbSystemDouble([device]), PHONE_ID),
      isUnavailableAs(PhoneProblem.NoFastbootInterface),
    );
    assert.equal(device.opened, false);
  }
});

test("a device the system does not let the browser open is not accessible", async () => {
  for (const refusal of [UsbErrorName.Security, UsbErrorName.Network]) {
    const phone = fastbootPhone();
    phone.refuse(DeviceStep.Open, refusal);
    await assert.rejects(
      PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID),
      isUnavailableAs(PhoneProblem.NotAccessible),
    );
  }
});

test("a fastboot interface that another program has claimed is busy, and the device is closed again", async () => {
  const phone = fastbootPhone();
  phone.refuse(DeviceStep.ClaimInterface, UsbErrorName.Network);
  await assert.rejects(
    PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID),
    isUnavailableAs(PhoneProblem.InterfaceBusy),
  );
  assert.equal(phone.opened, false);
});

test("a phone unplugged while it opens is not found", async () => {
  const phone = fastbootPhone();
  phone.refuse(DeviceStep.ClaimInterface, UsbErrorName.NotFound);
  await assert.rejects(PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID), isUnavailableAs(PhoneProblem.NotFound));
});

test("a send goes to the bulk out endpoint of the fastboot interface", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  await link.send(new TextEncoder().encode("getvar:product"));
  assert.deepEqual(
    phone.sent.map(({ bytes }) => new TextDecoder().decode(bytes)),
    ["getvar:product"],
  );
});

test("a send that the phone stalls is refused", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  phone.queueSendResults({ status: UsbTransferStatus.Stall, bytesWritten: 0 });
  await assert.rejects(link.send(new Uint8Array(4)), TransferRefused);
});

test("a send that the phone receives only in part is refused", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  phone.queueSendResults({ status: UsbTransferStatus.Ok, bytesWritten: 2 });
  await assert.rejects(link.send(new Uint8Array(4)), (failure) => failure instanceof TransferRefused && /2 of 4 bytes/.test(failure.message));
});

test("a receive asks for at most the limit and skips empty packets until one has bytes", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  const reply = new TextEncoder().encode("OKAY");
  phone.queuePackets(
    { status: UsbTransferStatus.Ok },
    { status: UsbTransferStatus.Ok, data: new DataView(new ArrayBuffer(0)) },
    { status: UsbTransferStatus.Ok, data: new DataView(reply.buffer) },
  );
  assert.deepEqual(await link.receive(64), reply);
  assert.deepEqual(phone.receiveLimits, [64, 64, 64]);
});

test("a receive that the phone stalls or babbles is refused", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  phone.queuePackets({ status: UsbTransferStatus.Stall }, { status: UsbTransferStatus.Babble });
  await assert.rejects(link.receive(64), TransferRefused);
  await assert.rejects(link.receive(64), TransferRefused);
});

test("closing a link releases the fastboot interface and closes the device", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  await link.close();
  assert.deepEqual(phone.claimedInterfaces, []);
  assert.equal(phone.opened, false);
});

test("closing a link of an unplugged phone succeeds", async () => {
  const phone = fastbootPhone();
  const usb = new UsbSystemDouble([phone]);
  const link = await PhoneLink.open(usb, PHONE_ID);
  usb.unplug(phone);
  await link.close();
});

test("an interface the system does not release is reported as not released, and the device is closed anyway", async () => {
  const phone = fastbootPhone();
  const link = await PhoneLink.open(new UsbSystemDouble([phone]), PHONE_ID);
  phone.refuse(DeviceStep.ReleaseInterface, UsbErrorName.Network);
  await assert.rejects(
    link.close(),
    (failure) => failure instanceof PhoneNotReleased && failure.refusal.name === UsbErrorName.Network,
  );
  assert.equal(phone.opened, false);
});

test("a watched phone has returned once it left and came back", async () => {
  const phone = fastbootPhone();
  const usb = new UsbSystemDouble([phone]);
  const watch = PhoneWatch.start(usb, PHONE_ID);
  let returned = false;
  const waiting = watch.returned().then(() => {
    returned = true;
  });
  usb.unplug(phone);
  await settle();
  assert.equal(watch.hasLeft, true);
  assert.equal(returned, false);
  usb.plug(fastbootPhone());
  await waiting;
  await watch.returned();
});

test("a watched phone that never left has not returned when another device comes and goes", async () => {
  const usb = new UsbSystemDouble([fastbootPhone()]);
  const watch = PhoneWatch.start(usb, PHONE_ID);
  let returned = false;
  void watch.returned().then(() => {
    returned = true;
  });
  const other = fastbootPhone(OTHER_PHONE);
  usb.plug(other);
  usb.unplug(other);
  usb.plug(fastbootPhone(OTHER_PHONE));
  await settle();
  assert.equal(watch.hasLeft, false);
  assert.equal(returned, false);
  watch.stop();
});

test("a stopped watch no longer listens to the USB system", () => {
  const usb = new UsbSystemDouble([]);
  const watch = PhoneWatch.start(usb, PHONE_ID);
  assert.notEqual(usb.listenerCount, 0);
  watch.stop();
  assert.equal(usb.listenerCount, 0);
});
