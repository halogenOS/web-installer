// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import {
  Stage,
  type InstallFailure,
  type InstallPlan,
  type Progress,
  type AskedFiles,
  type AwaitedFile,
  type Step,
} from "../wasm/contract.ts";

export interface Amount {
  done: number;
  total: number;
}

export type ImageReport = Extract<
  Progress,
  {
    stage:
      | typeof Stage.UnpackingStockImage
      | typeof Stage.ReadingStockFiles
      | typeof Stage.FetchingFiles
      | typeof Stage.Building
      | typeof Stage.ImageChecked;
  }
>;

export interface FileWait {
  asked: AskedFiles;
  files: AwaitedFile[];
  wrongPicks: InstallFailure[];
}

export type FetchedImagesReport = Extract<
  Progress,
  { stage: typeof Stage.FetchingImages | typeof Stage.FetchedImagesChecked }
>;

export type StockFirmwareReport = Extract<
  Progress,
  {
    stage:
      | typeof Stage.FetchingStockFirmware
      | typeof Stage.StockFirmwareChecked
      | typeof Stage.StockFirmwareBlockChecked;
  }
>;

export interface StepReport {
  index: number;
  count: number;
  step: Step;
}

export interface RunReports {
  stage: Stage;
  plan: InstallPlan | undefined;
  fileWait: FileWait | undefined;
  pickedFilesChecked: Amount | undefined;
  stockFirmware: StockFirmwareReport | undefined;
  shipped: FetchedImagesReport | undefined;
  images: Partial<Record<string, ImageReport>>;
  step: StepReport | undefined;
  sent: Amount | undefined;
  packageWritten: Amount | undefined;
  failedAttempts: number | undefined;
  phoneMessages: string[];
}

export function emptyReports(stage: Stage): RunReports {
  return {
    stage,
    plan: undefined,
    fileWait: undefined,
    pickedFilesChecked: undefined,
    stockFirmware: undefined,
    shipped: undefined,
    images: {},
    step: undefined,
    sent: undefined,
    packageWritten: undefined,
    failedAttempts: undefined,
    phoneMessages: [],
  };
}

export function recordProgress(reports: RunReports, progress: Progress): void {
  reports.stage = progress.stage;
  reports.failedAttempts = undefined;
  switch (progress.stage) {
    case Stage.Connecting:
    case Stage.Done:
    case Stage.ReadingRelease:
    case Stage.PackageSaved:
      break;
    case Stage.Checking:
    case Stage.ReleaseRead: {
      const { device, build, shipped, built, sources, stockFiles, install } = progress;
      reports.plan = { device, build, shipped, built, sources, stockFiles, install };
      break;
    }
    case Stage.ChoosingFiles:
      reports.fileWait = {
        asked: progress.asked,
        files: progress.files,
        wrongPicks: progress.wrongPicks,
      };
      reports.pickedFilesChecked = undefined;
      break;
    case Stage.CheckingPickedFiles:
      reports.pickedFilesChecked = { done: progress.done, total: progress.total };
      break;
    case Stage.FetchingStockFirmware:
    case Stage.StockFirmwareChecked:
    case Stage.StockFirmwareBlockChecked:
      reports.stockFirmware = progress;
      break;
    case Stage.FetchingImages:
    case Stage.FetchedImagesChecked:
      reports.shipped = progress;
      break;
    case Stage.UnpackingStockImage:
    case Stage.ReadingStockFiles:
    case Stage.FetchingFiles:
    case Stage.Building:
    case Stage.ImageChecked:
      reports.images[progress.partition] = progress;
      break;
    case Stage.Installing:
      reports.step = { index: progress.index, count: progress.count, step: progress.step };
      reports.sent = undefined;
      break;
    case Stage.Sending:
      reports.sent = { done: progress.done, total: progress.total };
      break;
    case Stage.WritingPackage:
      reports.packageWritten = { done: progress.done, total: progress.total };
      break;
  }
}
