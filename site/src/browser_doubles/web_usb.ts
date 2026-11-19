// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import {
  UsbConnection,
  UsbDirection,
  UsbTransferStatus,
  UsbTransferType,
  type UsbAlternate,
  type UsbConfiguration,
  type UsbConnectionListener,
  type UsbDevice,
  type UsbErrorName,
  type UsbFilter,
  type UsbInResult,
  type UsbOutResult,
  type WindowUsbSystem,
} from "../device/usb.ts";

// The steps and error names follow the WebUSB standard, https://usb.spec.whatwg.org/ (read 2026-11-19).
const UsbRefusal = { NotFound: "NotFoundError", InvalidState: "InvalidStateError" } as const;
type UsbRefusal = (typeof UsbRefusal)[keyof typeof UsbRefusal];

function refusal(name: UsbRefusal | UsbErrorName, message: string): DOMException {
  return new DOMException(message, name);
}

const ENDPOINT_IN = 1;
const ENDPOINT_OUT = 2;

// The interface match of match_fastboot_with_serial in fastboot/fastboot.cpp of platform/system/core at
// android-17.0.0_r1 (read 2026-11-19).
const FASTBOOT_CODES = { interfaceClass: 0xff, interfaceSubclass: 0x42, interfaceProtocol: 0x03 } as const;

// adb shares class and subclass with fastboot and uses protocol 0x01 (ADB_CLASS, ADB_SUBCLASS and ADB_PROTOCOL in adb.h
// of platform/packages/modules/adb at android-17.0.0_r1, read 2026-11-19).
const ADB_CODES = { interfaceClass: 0xff, interfaceSubclass: 0x42, interfaceProtocol: 0x01 } as const;

export const FASTBOOT_FILTER = {
  classCode: FASTBOOT_CODES.interfaceClass,
  subclassCode: FASTBOOT_CODES.interfaceSubclass,
  protocolCode: FASTBOOT_CODES.interfaceProtocol,
} as const;

function bulkAlternate(codes: typeof FASTBOOT_CODES | typeof ADB_CODES): UsbAlternate {
  return {
    ...codes,
    endpoints: [
      { endpointNumber: ENDPOINT_IN, direction: UsbDirection.In, type: UsbTransferType.Bulk },
      { endpointNumber: ENDPOINT_OUT, direction: UsbDirection.Out, type: UsbTransferType.Bulk },
    ],
  };
}

export function fastbootConfiguration(configurationValue: number, interfaceNumber: number): UsbConfiguration {
  return { configurationValue, interfaces: [{ interfaceNumber, alternate: bulkAlternate(FASTBOOT_CODES) }] };
}

export function adbConfiguration(configurationValue: number): UsbConfiguration {
  return { configurationValue, interfaces: [{ interfaceNumber: 0, alternate: bulkAlternate(ADB_CODES) }] };
}

export const DeviceStep = {
  Open: "open",
  SelectConfiguration: "select_configuration",
  ClaimInterface: "claim_interface",
  ReleaseInterface: "release_interface",
  Close: "close",
} as const;
export type DeviceStep = (typeof DeviceStep)[keyof typeof DeviceStep];

export interface DeviceIds {
  vendorId: number;
  productId: number;
  serialNumber?: string | null;
  productName?: string | null;
}

export interface SentTransfer {
  endpointNumber: number;
  bytes: Uint8Array;
}

export interface DeviceSetup {
  configurations: readonly UsbConfiguration[];
  configurationValue: number;
}

export class UsbDeviceDouble implements UsbDevice {
  readonly vendorId: number;
  readonly productId: number;
  readonly serialNumber: string | null;
  readonly productName: string | null;
  readonly configurations: readonly UsbConfiguration[];
  readonly sent: SentTransfer[] = [];
  readonly receiveLimits: number[] = [];
  #configurationValue: number;
  #connected = true;
  #opened = false;
  readonly #claimed = new Set<number>();
  readonly #refusals = new Map<DeviceStep, UsbErrorName>();
  readonly #packets: UsbInResult[] = [];
  readonly #sendResults: UsbOutResult[] = [];

  constructor(ids: DeviceIds, setup: DeviceSetup) {
    this.vendorId = ids.vendorId;
    this.productId = ids.productId;
    this.serialNumber = ids.serialNumber ?? null;
    this.productName = ids.productName ?? null;
    this.configurations = setup.configurations;
    this.#configurationValue = setup.configurationValue;
  }

  get configuration(): UsbConfiguration | null {
    return this.configurations.find((configuration) => configuration.configurationValue === this.#configurationValue) ?? null;
  }

  get opened(): boolean {
    return this.#opened;
  }

  get claimedInterfaces(): readonly number[] {
    return [...this.#claimed];
  }

  refuse(step: DeviceStep, name: UsbErrorName): void {
    this.#refusals.set(step, name);
  }

  queuePackets(...packets: UsbInResult[]): void {
    this.#packets.push(...packets);
  }

  queueSendResults(...results: UsbOutResult[]): void {
    this.#sendResults.push(...results);
  }

  disconnect(): void {
    this.#connected = false;
  }

  async open(): Promise<void> {
    this.#requireConnected();
    if (this.#opened) return;
    this.#refuseIfAsked(DeviceStep.Open);
    this.#opened = true;
  }

  async close(): Promise<void> {
    this.#requireConnected();
    if (!this.#opened) return;
    this.#refuseIfAsked(DeviceStep.Close);
    this.#claimed.clear();
    this.#opened = false;
  }

  async selectConfiguration(configurationValue: number): Promise<void> {
    this.#requireConnected();
    if (!this.configurations.some((configuration) => configuration.configurationValue === configurationValue)) {
      throw refusal(UsbRefusal.NotFound, `The device has no configuration ${String(configurationValue)}.`);
    }
    if (!this.#opened) throw refusal(UsbRefusal.InvalidState, "The device is not open.");
    this.#refuseIfAsked(DeviceStep.SelectConfiguration);
    this.#claimed.clear();
    this.#configurationValue = configurationValue;
  }

  async claimInterface(interfaceNumber: number): Promise<void> {
    this.#requireInterface(interfaceNumber);
    if (this.#claimed.has(interfaceNumber)) return;
    this.#refuseIfAsked(DeviceStep.ClaimInterface);
    this.#claimed.add(interfaceNumber);
  }

  async releaseInterface(interfaceNumber: number): Promise<void> {
    this.#requireInterface(interfaceNumber);
    if (!this.#claimed.has(interfaceNumber)) return;
    this.#refuseIfAsked(DeviceStep.ReleaseInterface);
    this.#claimed.delete(interfaceNumber);
  }

  async transferOut(endpointNumber: number, data: Uint8Array): Promise<UsbOutResult> {
    this.#requireEndpoint(endpointNumber, UsbDirection.Out);
    this.sent.push({ endpointNumber, bytes: data.slice() });
    return this.#sendResults.shift() ?? { status: UsbTransferStatus.Ok, bytesWritten: data.length };
  }

  async transferIn(endpointNumber: number, length: number): Promise<UsbInResult> {
    this.#requireEndpoint(endpointNumber, UsbDirection.In);
    this.receiveLimits.push(length);
    const packet = this.#packets.shift();
    if (packet === undefined) throw new Error("The test queued no further packet from the phone.");
    return packet;
  }

  #refuseIfAsked(step: DeviceStep): void {
    const name = this.#refusals.get(step);
    if (name !== undefined) throw refusal(name, `The device refused the step ${step}.`);
  }

  #requireConnected(): void {
    if (!this.#connected) throw refusal(UsbRefusal.NotFound, "The device is no longer connected.");
  }

  #requireConfigured(): UsbConfiguration {
    this.#requireConnected();
    const configuration = this.configuration;
    if (!this.#opened || configuration === null) {
      throw refusal(UsbRefusal.InvalidState, "The device is not open in a configuration.");
    }
    return configuration;
  }

  #requireInterface(interfaceNumber: number): void {
    const configuration = this.#requireConfigured();
    if (!configuration.interfaces.some((found) => found.interfaceNumber === interfaceNumber)) {
      throw refusal(UsbRefusal.NotFound, `The configuration has no interface ${String(interfaceNumber)}.`);
    }
  }

  #requireEndpoint(endpointNumber: number, direction: UsbDirection): void {
    const claimed = this.#requireConfigured().interfaces.filter((found) => this.#claimed.has(found.interfaceNumber));
    const endpoint = claimed
      .flatMap((found) => found.alternate.endpoints)
      .find((found) => found.endpointNumber === endpointNumber && found.direction === direction);
    if (endpoint === undefined) {
      throw refusal(UsbRefusal.NotFound, `No claimed interface has the ${direction} endpoint ${String(endpointNumber)}.`);
    }
  }
}

export class UsbSystemDouble implements WindowUsbSystem {
  readonly requestedFilters: (readonly UsbFilter[])[] = [];
  choose: () => Promise<UsbDevice> = () => Promise.reject(refusal(UsbRefusal.NotFound, "No device was chosen."));
  readonly #devices: UsbDeviceDouble[] = [];
  readonly #listeners: Record<UsbConnection, Set<UsbConnectionListener>> = {
    [UsbConnection.Connect]: new Set(),
    [UsbConnection.Disconnect]: new Set(),
  };

  constructor(devices: readonly UsbDeviceDouble[]) {
    this.#devices.push(...devices);
  }

  get listenerCount(): number {
    return Object.values(this.#listeners).reduce((sum, listeners) => sum + listeners.size, 0);
  }

  getDevices(): Promise<readonly UsbDevice[]> {
    return Promise.resolve([...this.#devices]);
  }

  addEventListener(type: UsbConnection, listener: UsbConnectionListener): void {
    this.#listeners[type].add(listener);
  }

  removeEventListener(type: UsbConnection, listener: UsbConnectionListener): void {
    this.#listeners[type].delete(listener);
  }

  requestDevice(options: { filters: readonly UsbFilter[] }): Promise<UsbDevice> {
    this.requestedFilters.push(options.filters);
    return this.choose();
  }

  plug(device: UsbDeviceDouble): void {
    this.#devices.push(device);
    for (const listener of this.#listeners[UsbConnection.Connect]) listener({ device });
  }

  unplug(device: UsbDeviceDouble): void {
    this.#devices.splice(this.#devices.indexOf(device), 1);
    device.disconnect();
    for (const listener of this.#listeners[UsbConnection.Disconnect]) listener({ device });
  }
}
