// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { isPhoneId, type PhoneId } from "../device/usb.ts";
import {
  AskedFiles,
  BlockEdge,
  CheckVerdict,
  PhoneAnswer,
  PhoneCheckKind,
  Stage,
  type InstallFailure,
  type Progress,
} from "../wasm/contract.ts";
import { isInstallFailure } from "./install_failure.ts";

export const PageMessageKind = {
  Install: "install",
  PhoneChosen: "phone_chosen",
  FilesPicked: "files_picked",
  SavePackage: "save_package",
} as const;

export type PageMessage =
  | {
      kind: typeof PageMessageKind.Install;
      phone: PhoneId;
      releaseTag: string;
    }
  | { kind: typeof PageMessageKind.PhoneChosen; phone: PhoneId }
  | { kind: typeof PageMessageKind.FilesPicked; files: File[] }
  | {
      kind: typeof PageMessageKind.SavePackage;
      file: FileSystemFileHandle;
      releaseTag: string;
      compressionPorts: MessagePort[];
    };

export const WorkerMessageKind = {
  Progress: "progress",
  Retrying: "retrying",
  PhoneMessage: "phone_message",
  Failed: "failed",
} as const;

export type WorkerMessage =
  | { kind: typeof WorkerMessageKind.Progress; progress: Progress }
  | { kind: typeof WorkerMessageKind.Retrying; failedAttempts: number }
  | { kind: typeof WorkerMessageKind.PhoneMessage; text: string }
  | { kind: typeof WorkerMessageKind.Failed; failure: InstallFailure };

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasNumbers(value: Record<string, unknown>, ...keys: string[]): boolean {
  return keys.every((key) => typeof value[key] === "number");
}

function hasTexts(value: Record<string, unknown>, ...keys: string[]): boolean {
  return keys.every((key) => typeof value[key] === "string");
}

function isListedFile(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasTexts(value, "file_name", "address", "sha256") &&
    hasNumbers(value, "size")
  );
}

function isFileList(value: unknown): value is File[] {
  return Array.isArray(value) && value.every((item) => item instanceof File);
}

function isPortList(value: unknown): value is MessagePort[] {
  return Array.isArray(value) && value.every((item) => item instanceof MessagePort);
}

function isTextList(value: unknown): boolean {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isRecordList(value: unknown, check: (item: Record<string, unknown>) => boolean): boolean {
  return Array.isArray(value) && value.every((item) => isRecord(item) && check(item));
}

const BLOCK_EDGES: readonly unknown[] = Object.values(BlockEdge);

function isBlockDownload(value: Record<string, unknown>): boolean {
  return hasNumbers(value, "downloaded", "block") && BLOCK_EDGES.includes(value.readFrom);
}

function hasPlan(value: Record<string, unknown>): boolean {
  return (
    hasTexts(value, "device", "build") &&
    isTextList(value.shipped) &&
    isTextList(value.built) &&
    isRecordList(value.sources, (source) => hasTexts(source, "repository", "commit")) &&
    Array.isArray(value.stockFiles) &&
    value.stockFiles.every(isListedFile) &&
    isRecordList(value.install, (step) => hasTexts(step, "step"))
  );
}

const ASKED_FILES: readonly unknown[] = Object.values(AskedFiles);
const CHECK_VERDICTS: readonly unknown[] = Object.values(CheckVerdict);
const PHONE_CHECK_KINDS: readonly unknown[] = Object.values(PhoneCheckKind);
const PHONE_ANSWERS: readonly unknown[] = Object.values(PhoneAnswer);

function isPhoneCheckKind(value: unknown): value is PhoneCheckKind {
  return PHONE_CHECK_KINDS.includes(value);
}

function isOptionalText(value: unknown): boolean {
  return value === undefined || typeof value === "string";
}

function isOptionalAnswer(value: unknown): boolean {
  return value === undefined || PHONE_ANSWERS.includes(value);
}

function isOptionalPartitionFit(value: unknown): boolean {
  return value === undefined || (isRecord(value) && hasNumbers(value, "fitting", "count"));
}

const PHONE_READING_FIELDS: Record<PhoneCheckKind, (reading: Record<string, unknown>) => boolean> = {
  [PhoneCheckKind.Model]: (reading) => isOptionalText(reading.product),
  [PhoneCheckKind.Bootloader]: (reading) => isOptionalAnswer(reading.unlocked),
  [PhoneCheckKind.Battery]: (reading) => isOptionalAnswer(reading.socOk),
  [PhoneCheckKind.Slot]: (reading) => isOptionalText(reading.currentSlot),
  [PhoneCheckKind.Partitions]: (reading) => isOptionalPartitionFit(reading.fit),
  [PhoneCheckKind.TransferSize]: (reading) =>
    reading.downloadLimit === undefined || typeof reading.downloadLimit === "number",
};

function isPhoneReading(value: unknown): boolean {
  return isRecord(value) && isPhoneCheckKind(value.check) && PHONE_READING_FIELDS[value.check](value);
}

const PROGRESS_FIELDS: Record<Stage, (value: Record<string, unknown>) => boolean> = {
  [Stage.Connecting]: () => true,
  [Stage.Checking]: hasPlan,
  [Stage.PhoneChecked]: (value) => CHECK_VERDICTS.includes(value.verdict) && isPhoneReading(value.reading),
  [Stage.ChoosingFiles]: (value) =>
    ASKED_FILES.includes(value.asked) &&
    isRecordList(
      value.files,
      (awaited) => isListedFile(awaited.file) && typeof awaited.picked === "boolean",
    ) &&
    Array.isArray(value.wrongPicks) &&
    value.wrongPicks.every(isInstallFailure),
  [Stage.CheckingPickedFiles]: (value) => hasNumbers(value, "done", "total"),
  [Stage.FetchingStockFirmware]: isBlockDownload,
  [Stage.StockFirmwareChecked]: () => true,
  [Stage.StockFirmwareBlockChecked]: isBlockDownload,
  [Stage.UnpackingStockImage]: (value) =>
    hasTexts(value, "partition") && hasNumbers(value, "done", "total"),
  [Stage.FetchingImages]: (value) => hasNumbers(value, "done", "total"),
  [Stage.FetchedImagesChecked]: () => true,
  [Stage.ReadingStockFiles]: (value) =>
    hasTexts(value, "partition") && hasNumbers(value, "done", "total"),
  [Stage.FetchingFiles]: (value) =>
    hasTexts(value, "partition") && hasNumbers(value, "done", "total"),
  [Stage.Building]: (value) => hasTexts(value, "partition") && hasNumbers(value, "done", "total"),
  [Stage.ImageChecked]: (value) => hasTexts(value, "partition"),
  [Stage.Installing]: (value) =>
    hasNumbers(value, "index", "count") && isRecord(value.step) && hasTexts(value.step, "step"),
  [Stage.Sending]: (value) => hasNumbers(value, "done", "total"),
  [Stage.Done]: () => true,
  [Stage.ReadingRelease]: () => true,
  [Stage.ReleaseRead]: hasPlan,
  [Stage.WritingPackage]: (value) => hasNumbers(value, "done", "total"),
  [Stage.PackageSaved]: () => true,
};

const STAGES: readonly unknown[] = Object.values(Stage);

function isStage(value: unknown): value is Stage {
  return STAGES.includes(value);
}

function isProgress(value: unknown): value is Progress {
  return isRecord(value) && isStage(value.stage) && PROGRESS_FIELDS[value.stage](value);
}

export function readWorkerMessage(data: unknown): WorkerMessage | undefined {
  if (!isRecord(data)) return undefined;
  const { kind, progress, failedAttempts, text, failure } = data;
  if (kind === WorkerMessageKind.Progress && isProgress(progress)) return { kind, progress };
  if (kind === WorkerMessageKind.Retrying && typeof failedAttempts === "number") {
    return { kind, failedAttempts };
  }
  if (kind === WorkerMessageKind.PhoneMessage && typeof text === "string") return { kind, text };
  if (kind === WorkerMessageKind.Failed && isInstallFailure(failure)) return { kind, failure };
  return undefined;
}

export function readPageMessage(data: unknown): PageMessage | undefined {
  if (!isRecord(data)) return undefined;
  const { kind, phone, releaseTag, file, files, compressionPorts } = data;
  if (kind === PageMessageKind.FilesPicked && isFileList(files)) return { kind, files };
  if (
    kind === PageMessageKind.SavePackage &&
    file instanceof FileSystemFileHandle &&
    typeof releaseTag === "string" &&
    isPortList(compressionPorts)
  ) {
    return { kind, file, releaseTag, compressionPorts };
  }
  if (!isPhoneId(phone)) return undefined;
  if (kind === PageMessageKind.PhoneChosen) return { kind, phone };
  if (kind === PageMessageKind.Install && typeof releaseTag === "string") return { kind, phone, releaseTag };
  return undefined;
}
