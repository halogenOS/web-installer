// SPDX-License-Identifier: GPL-3.0-only
import type { PageLink } from "../wasm/contract.ts";
import fastbootInterface from "./fastboot_interface.json" with { type: "json" };

const FASTBOOT_INTERFACE = fastbootInterface.interface;

export const UsbErrorName = {
  NotFound: "NotFoundError",
  Security: "SecurityError",
  Network: "NetworkError",
} as const;
export type UsbErrorName = (typeof UsbErrorName)[keyof typeof UsbErrorName];

export const PhoneProblem = {
  NoWebUsb: "no_web_usb",
  NotFound: "not_found",
  Several: "several",
  NotAccessible: "not_accessible",
  NoFastbootInterface: "no_fastboot_interface",
  InterfaceBusy: "interface_busy",
} as const;
export type PhoneProblem = (typeof PhoneProblem)[keyof typeof PhoneProblem];

export class PhoneUnavailable extends Error {
  override name = "PhoneUnavailable";
  readonly problem: PhoneProblem;

  constructor(problem: PhoneProblem, message: string, options?: ErrorOptions) {
    super(message, options);
    this.problem = problem;
  }
}

export class TransferRefused extends Error {
  override name = "TransferRefused";
}

export class PhoneNotReleased extends Error {
  override name = "PhoneNotReleased";
  readonly refusal: DOMException;

  constructor(refusal: DOMException, options?: ErrorOptions) {
    super("The browser did not release the phone.", options);
    this.refusal = refusal;
  }
}

export const UsbDirection = { In: "in", Out: "out" } as const;
export type UsbDirection = (typeof UsbDirection)[keyof typeof UsbDirection];

export const UsbTransferType = {
  Bulk: "bulk",
  Interrupt: "interrupt",
  Isochronous: "isochronous",
} as const;
export type UsbTransferType = (typeof UsbTransferType)[keyof typeof UsbTransferType];

export const UsbTransferStatus = { Ok: "ok", Stall: "stall", Babble: "babble" } as const;
export type UsbTransferStatus = (typeof UsbTransferStatus)[keyof typeof UsbTransferStatus];

export interface UsbEndpoint {
  readonly endpointNumber: number;
  readonly direction: UsbDirection;
  readonly type: UsbTransferType;
  readonly packetSize: number;
}

export interface UsbAlternate {
  readonly interfaceClass: number;
  readonly interfaceSubclass: number;
  readonly interfaceProtocol: number;
  readonly endpoints: readonly UsbEndpoint[];
}

export interface UsbInterface {
  readonly interfaceNumber: number;
  readonly alternate: UsbAlternate;
}

export interface UsbConfiguration {
  readonly configurationValue: number;
  readonly interfaces: readonly UsbInterface[];
}

export interface UsbInResult {
  readonly status: UsbTransferStatus;
  readonly data?: DataView | undefined;
}

export interface UsbOutResult {
  readonly status: Exclude<UsbTransferStatus, typeof UsbTransferStatus.Babble>;
  readonly bytesWritten: number;
}

export interface UsbDevice {
  readonly vendorId: number;
  readonly productId: number;
  readonly productName?: string | null | undefined;
  readonly serialNumber?: string | null | undefined;
  readonly configuration: UsbConfiguration | null;
  readonly configurations: readonly UsbConfiguration[];
  open(): Promise<void>;
  close(): Promise<void>;
  selectConfiguration(configurationValue: number): Promise<void>;
  claimInterface(interfaceNumber: number): Promise<void>;
  releaseInterface(interfaceNumber: number): Promise<void>;
  transferIn(endpointNumber: number, length: number): Promise<UsbInResult>;
  transferOut(endpointNumber: number, data: Uint8Array): Promise<UsbOutResult>;
}

export type UsbFilter = typeof FASTBOOT_INTERFACE;

export const UsbConnection = { Connect: "connect", Disconnect: "disconnect" } as const;
export type UsbConnection = (typeof UsbConnection)[keyof typeof UsbConnection];

export interface UsbConnectionEvent {
  readonly device: UsbDevice;
}

export type UsbConnectionListener = (event: UsbConnectionEvent) => void;

export interface UsbSystem {
  getDevices(): Promise<readonly UsbDevice[]>;
  addEventListener(type: UsbConnection, listener: UsbConnectionListener): void;
  removeEventListener(type: UsbConnection, listener: UsbConnectionListener): void;
}

export interface WindowUsbSystem extends UsbSystem {
  requestDevice(options: { filters: readonly UsbFilter[] }): Promise<UsbDevice>;
}

export interface UsbScope {
  readonly navigator: object;
}

function isUsbSystem(value: unknown): value is UsbSystem {
  return (
    typeof value === "object" &&
    value !== null &&
    "getDevices" in value &&
    typeof value.getDevices === "function" &&
    "addEventListener" in value &&
    typeof value.addEventListener === "function" &&
    "removeEventListener" in value &&
    typeof value.removeEventListener === "function"
  );
}

function isWindowUsbSystem(usb: UsbSystem): usb is WindowUsbSystem {
  return "requestDevice" in usb && typeof usb.requestDevice === "function";
}

function usbOf(scope: UsbScope): UsbSystem | undefined {
  const usb: unknown = "usb" in scope.navigator ? scope.navigator.usb : undefined;
  return isUsbSystem(usb) ? usb : undefined;
}

function noWebUsb(): PhoneUnavailable {
  return new PhoneUnavailable(PhoneProblem.NoWebUsb, "This browser does not support WebUSB.");
}

export function usbAvailable(scope: UsbScope): boolean {
  return usbOf(scope) !== undefined;
}

export function browserUsb(scope: UsbScope): UsbSystem {
  const usb = usbOf(scope);
  if (usb === undefined) throw noWebUsb();
  return usb;
}

export function windowUsb(scope: UsbScope): WindowUsbSystem {
  const usb = usbOf(scope);
  if (usb === undefined || !isWindowUsbSystem(usb)) throw noWebUsb();
  return usb;
}

export interface PhoneId {
  vendorId: number;
  productId: number;
  serialNumber: string | undefined;
}

export function isPhoneId(value: unknown): value is PhoneId {
  return (
    typeof value === "object" &&
    value !== null &&
    "vendorId" in value &&
    typeof value.vendorId === "number" &&
    "productId" in value &&
    typeof value.productId === "number" &&
    "serialNumber" in value &&
    (value.serialNumber === undefined || typeof value.serialNumber === "string")
  );
}

function idOf(device: UsbDevice): PhoneId {
  return {
    vendorId: device.vendorId,
    productId: device.productId,
    serialNumber: device.serialNumber ?? undefined,
  };
}

function hasId(device: UsbDevice, id: PhoneId): boolean {
  const { vendorId, productId, serialNumber } = idOf(device);
  return (
    vendorId === id.vendorId && productId === id.productId && serialNumber === id.serialNumber
  );
}

export interface ChosenPhone {
  id: PhoneId;
  label: string;
}

export const UNNAMED_PHONE_LABEL = "Unnamed phone";

function labelOf({ productName }: UsbDevice): string {
  return productName || UNNAMED_PHONE_LABEL;
}

function isNamed(cause: unknown, name: UsbErrorName): boolean {
  return cause instanceof DOMException && cause.name === name;
}

export async function requestPhone(usb: WindowUsbSystem): Promise<ChosenPhone | undefined> {
  try {
    const device = await usb.requestDevice({ filters: [FASTBOOT_INTERFACE] });
    return { id: idOf(device), label: labelOf(device) };
  } catch (cause) {
    if (isNamed(cause, UsbErrorName.NotFound)) return undefined;
    if (isNamed(cause, UsbErrorName.Security)) {
      throw new PhoneUnavailable(
        PhoneProblem.NoWebUsb,
        "The browser does not let this page use USB.",
        { cause },
      );
    }
    throw cause;
  }
}

interface FastbootInterface {
  interfaceNumber: number;
  input: UsbEndpoint;
  output: UsbEndpoint;
}

function findFastbootInterface(configuration: UsbConfiguration | null): FastbootInterface {
  for (const { interfaceNumber, alternate } of configuration?.interfaces ?? []) {
    const speaksFastboot =
      alternate.interfaceClass === FASTBOOT_INTERFACE.classCode &&
      alternate.interfaceSubclass === FASTBOOT_INTERFACE.subclassCode &&
      alternate.interfaceProtocol === FASTBOOT_INTERFACE.protocolCode;
    const bulk = (direction: UsbDirection): UsbEndpoint | undefined =>
      alternate.endpoints.find(
        (endpoint) => endpoint.type === UsbTransferType.Bulk && endpoint.direction === direction,
      );
    const input = bulk(UsbDirection.In);
    const output = bulk(UsbDirection.Out);
    if (speaksFastboot && input !== undefined && output !== undefined) {
      return { interfaceNumber, input, output };
    }
  }
  throw new PhoneUnavailable(
    PhoneProblem.NoFastbootInterface,
    "The device does not have a fastboot interface with a bulk endpoint in each direction.",
  );
}

async function findDevice(usb: UsbSystem, id: PhoneId): Promise<UsbDevice> {
  const devices = (await usb.getDevices()).filter((device) => hasId(device, id));
  const [device, ...others] = devices;
  if (device === undefined) {
    throw new PhoneUnavailable(PhoneProblem.NotFound, "The phone is not among the devices.");
  }
  if (others.length > 0) {
    throw new PhoneUnavailable(
      PhoneProblem.Several,
      `${devices.length} devices have the ids of the phone.`,
    );
  }
  return device;
}

async function refusedAs<Result>(
  refused: PhoneProblem,
  step: () => Promise<Result>,
): Promise<Result> {
  try {
    return await step();
  } catch (cause) {
    if (isNamed(cause, UsbErrorName.Network) || isNamed(cause, UsbErrorName.Security)) {
      throw new PhoneUnavailable(refused, "The system does not let the browser use the phone.", {
        cause,
      });
    }
    if (isNamed(cause, UsbErrorName.NotFound)) {
      throw new PhoneUnavailable(PhoneProblem.NotFound, "The phone was unplugged.", { cause });
    }
    throw cause;
  }
}

async function releasing(release: () => Promise<void>, options?: ErrorOptions): Promise<void> {
  try {
    await release();
  } catch (cause) {
    if (!(cause instanceof DOMException)) throw cause;
    if (cause.name === UsbErrorName.NotFound) return;
    if (cause.name === UsbErrorName.Network) throw new PhoneNotReleased(cause, options);
    throw cause;
  }
}

async function claimFastbootInterface(device: UsbDevice): Promise<FastbootInterface> {
  await refusedAs(PhoneProblem.NotAccessible, () => device.open());
  if (device.configuration === null) {
    const [first] = device.configurations;
    if (first === undefined) {
      throw new PhoneUnavailable(
        PhoneProblem.NoFastbootInterface,
        "The device does not have a configuration.",
      );
    }
    await refusedAs(PhoneProblem.NotAccessible, () =>
      device.selectConfiguration(first.configurationValue),
    );
  }
  const fastboot = findFastbootInterface(device.configuration);
  await refusedAs(PhoneProblem.InterfaceBusy, () =>
    device.claimInterface(fastboot.interfaceNumber),
  );
  return fastboot;
}

export class PhoneWatch {
  readonly #usb: UsbSystem;
  readonly #id: PhoneId;
  #left = false;
  #listed = true;
  readonly #waiting = new Set<() => void>();
  readonly #onDisconnect: UsbConnectionListener = ({ device }) => {
    if (!hasId(device, this.#id)) return;
    this.#left = true;
    this.#listed = false;
  };
  readonly #onConnect: UsbConnectionListener = ({ device }) => {
    if (!hasId(device, this.#id)) return;
    this.#listed = true;
    if (this.#hasReturned) for (const resume of this.#waiting) resume();
  };

  private constructor(usb: UsbSystem, id: PhoneId) {
    this.#usb = usb;
    this.#id = id;
  }

  static start(usb: UsbSystem, id: PhoneId): PhoneWatch {
    const watch = new PhoneWatch(usb, id);
    usb.addEventListener(UsbConnection.Disconnect, watch.#onDisconnect);
    usb.addEventListener(UsbConnection.Connect, watch.#onConnect);
    return watch;
  }

  get hasLeft(): boolean {
    return this.#left;
  }

  get #hasReturned(): boolean {
    return this.#left && this.#listed;
  }

  async returned(): Promise<void> {
    if (this.#hasReturned) return;
    let resume = (): void => undefined;
    const ended = new Promise<void>((resolve) => {
      resume = resolve;
    });
    this.#waiting.add(resume);
    try {
      await ended;
    } finally {
      this.#waiting.delete(resume);
    }
  }

  stop(): void {
    this.#usb.removeEventListener(UsbConnection.Disconnect, this.#onDisconnect);
    this.#usb.removeEventListener(UsbConnection.Connect, this.#onConnect);
  }
}

export class PhoneLink implements PageLink {
  readonly #device: UsbDevice;
  readonly #fastboot: FastbootInterface;

  private constructor(device: UsbDevice, fastboot: FastbootInterface) {
    this.#device = device;
    this.#fastboot = fastboot;
  }

  static async open(usb: UsbSystem, id: PhoneId): Promise<PhoneLink> {
    const device = await findDevice(usb, id);
    try {
      return new PhoneLink(device, await claimFastbootInterface(device));
    } catch (cause) {
      await releasing(() => device.close(), { cause });
      throw cause;
    }
  }

  async send(bytes: Uint8Array): Promise<void> {
    await this.#transferOut(bytes);
    if (bytes.length > 0 && bytes.length % this.#fastboot.output.packetSize === 0) {
      await this.#transferOut(new Uint8Array(0));
    }
  }

  async receive(limit: number): Promise<Uint8Array> {
    for (;;) {
      const { status, data } = await this.#device.transferIn(
        this.#fastboot.input.endpointNumber,
        limit,
      );
      requireOk(status, "receive");
      if (data !== undefined && data.byteLength > 0) {
        return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      }
    }
  }

  async close(): Promise<void> {
    try {
      await releasing(() => this.#device.releaseInterface(this.#fastboot.interfaceNumber));
    } catch (failure) {
      await this.#closeDevice({ cause: failure });
      throw failure;
    }
    await this.#closeDevice();
  }

  #closeDevice(options?: ErrorOptions): Promise<void> {
    return releasing(() => this.#device.close(), options);
  }

  async #transferOut(bytes: Uint8Array): Promise<void> {
    const { status, bytesWritten } = await this.#device.transferOut(
      this.#fastboot.output.endpointNumber,
      bytes,
    );
    requireOk(status, "send");
    if (bytesWritten !== bytes.length) {
      throw new TransferRefused(`The phone received ${bytesWritten} of ${bytes.length} bytes.`);
    }
  }
}

function requireOk(status: UsbTransferStatus, direction: "send" | "receive"): void {
  if (status !== UsbTransferStatus.Ok) {
    throw new TransferRefused(`The phone ended a ${direction} with the status ${status}.`);
  }
}
