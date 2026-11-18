// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PhoneId } from "../device/usb.ts";
import {
  PageMessageKind,
  readWorkerMessage,
  type PageMessage,
  type WorkerMessage,
} from "./messages.ts";

export interface InstallEvents {
  onMessage: (message: WorkerMessage) => void;
  onCrash: (report: string) => void;
}

interface CompressionWorkers {
  workers: Worker[];
  ports: MessagePort[];
}

function startCompressionWorkers(events: InstallEvents): CompressionWorkers {
  const channels = Array.from({ length: navigator.hardwareConcurrency }, () => new MessageChannel());
  const workers = channels.map(({ port1 }) => {
    const worker = new Worker(new URL("./compression_worker.ts", import.meta.url), { type: "module" });
    worker.onerror = (event) => {
      events.onCrash(event.message);
    };
    worker.postMessage(port1, [port1]);
    return worker;
  });
  return { workers, ports: channels.map(({ port2 }) => port2) };
}

export class InstallSession {
  readonly #worker: Worker;
  readonly #compressionWorkers: Worker[];

  static install(phone: PhoneId, releaseTag: string, events: InstallEvents): InstallSession {
    return new InstallSession(
      events,
      { kind: PageMessageKind.Install, phone, releaseTag },
      { workers: [], ports: [] },
    );
  }

  static savePackage(file: FileSystemFileHandle, releaseTag: string, events: InstallEvents): InstallSession {
    const compression = startCompressionWorkers(events);
    return new InstallSession(
      events,
      { kind: PageMessageKind.SavePackage, file, releaseTag, compressionPorts: compression.ports },
      compression,
    );
  }

  private constructor(events: InstallEvents, start: PageMessage, compression: CompressionWorkers) {
    this.#worker = new Worker(new URL("./engine_worker.ts", import.meta.url), { type: "module" });
    this.#compressionWorkers = compression.workers;
    this.#worker.onmessage = (event: MessageEvent<unknown>) => {
      const message = readWorkerMessage(event.data);
      if (message === undefined) events.onCrash("The engine worker sent a message that the page cannot read.");
      else events.onMessage(message);
    };
    this.#worker.onerror = (event) => {
      events.onCrash(event.message);
    };
    this.#worker.postMessage(start, compression.ports);
  }

  choosePhoneAgain(phone: PhoneId): void {
    this.#post({ kind: PageMessageKind.PhoneChosen, phone });
  }

  pickFiles(files: File[]): void {
    this.#post({ kind: PageMessageKind.FilesPicked, files });
  }

  end(): void {
    for (const worker of [this.#worker, ...this.#compressionWorkers]) worker.terminate();
  }

  #post(message: PageMessage): void {
    this.#worker.postMessage(message);
  }
}
