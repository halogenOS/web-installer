// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { browserUsb, type PhoneId } from "../device/usb.ts";
import { isInPageStorage } from "../store/install_store.ts";
import init, { install, pauseBeforeRetry, savePackage } from "../wasm/web_installer_wasm.js";
import type { InstallFailure } from "../wasm/contract.ts";
import { CompressionPorts } from "./compression_ports.ts";
import { isInstallFailure, pageStopped } from "./install_failure.ts";
import {
  PageMessageKind,
  WorkerMessageKind,
  readPageMessage,
  type WorkerMessage,
} from "./messages.ts";
import { Waits } from "./waits.ts";
import { WorkerInstallPage } from "./worker_install_page.ts";
import { WorkerPackagePage } from "./worker_package_page.ts";

function post(message: WorkerMessage): void {
  self.postMessage(message);
}

const phonesChosenAgain = new Waits<PhoneId>();

const filesPicked = new Waits<File[]>();

function failureOf(cause: unknown): InstallFailure {
  return isInstallFailure(cause) ? cause : pageStopped(String(cause));
}

async function runInstall(phone: PhoneId, releaseTag: string): Promise<void> {
  try {
    await init();
    const page = new WorkerInstallPage({
      findUsb: () => browserUsb(self),
      phone,
      phoneChosenAgain: () => phonesChosenAgain.next(),
      filesPicked: () => filesPicked.next(),
      pauseBeforeRetry,
      post,
    });
    await install(page, releaseTag);
  } catch (cause) {
    post({ kind: WorkerMessageKind.Failed, failure: failureOf(cause) });
  }
}

async function runSavePackage(
  file: FileSystemFileHandle,
  releaseTag: string,
  compressionPorts: MessagePort[],
): Promise<void> {
  let page: WorkerPackagePage | undefined;
  try {
    await init();
    page = new WorkerPackagePage(
      { filesPicked: () => filesPicked.next(), pauseBeforeRetry, post },
      await file.createWritable(),
      await isInPageStorage(file),
      new CompressionPorts(compressionPorts),
    );
    await savePackage(page, releaseTag);
  } catch (cause) {
    post({ kind: WorkerMessageKind.Failed, failure: failureOf(cause) });
    await page?.abortPackage();
  }
}

self.onmessage = (event: MessageEvent<unknown>) => {
  const message = readPageMessage(event.data);
  if (message?.kind === PageMessageKind.Install) {
    void runInstall(message.phone, message.releaseTag);
  } else if (message?.kind === PageMessageKind.SavePackage) {
    void runSavePackage(message.file, message.releaseTag, message.compressionPorts);
  } else if (message?.kind === PageMessageKind.PhoneChosen) {
    phonesChosenAgain.resolveAll(message.phone);
  } else if (message?.kind === PageMessageKind.FilesPicked) {
    filesPicked.resolveAll(message.files);
  }
};
