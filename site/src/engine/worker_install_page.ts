// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import {
  PhoneLink,
  PhoneUnavailable,
  PhoneWatch,
  type PhoneId,
  type UsbSystem,
} from "../device/usb.ts";
import { OpeningKind, type InstallPage, type PhoneOpening } from "../wasm/contract.ts";
import { WorkerMessageKind } from "./messages.ts";
import { WorkerStoragePage, type StoragePorts } from "./worker_storage_page.ts";

export interface WorkerPorts extends StoragePorts {
  findUsb: () => UsbSystem;
  phone: PhoneId;
  phoneChosenAgain: () => Promise<PhoneId>;
}

interface OpenPhone {
  link: PhoneLink;
  watch: PhoneWatch;
}

export class WorkerInstallPage extends WorkerStoragePage implements InstallPage {
  readonly #ports: WorkerPorts;
  #phone: PhoneId;
  #open: OpenPhone | undefined;

  constructor(ports: WorkerPorts) {
    super(ports);
    this.#ports = ports;
    this.#phone = ports.phone;
  }

  async openPhone(): Promise<PhoneOpening> {
    try {
      const usb = this.#ports.findUsb();
      const link = await PhoneLink.open(usb, this.#phone);
      this.#open = { link, watch: PhoneWatch.start(usb, this.#phone) };
      return { [OpeningKind.Opened]: link };
    } catch (cause) {
      if (cause instanceof PhoneUnavailable) {
        return { [OpeningKind.Unavailable]: cause.problem };
      }
      throw cause;
    }
  }

  async reopenPhone(): Promise<PhoneOpening> {
    const open = this.#open;
    if (open === undefined) throw new Error("The phone was reopened before it was opened.");
    await open.link.close();
    const phone = this.#phone;
    this.#phone = await Promise.race([
      open.watch.returned().then(() => phone),
      this.#ports.phoneChosenAgain(),
    ]);
    open.watch.stop();
    return this.openPhone();
  }

  showPhoneMessage(text: string): void {
    this.post({ kind: WorkerMessageKind.PhoneMessage, text });
  }
}
