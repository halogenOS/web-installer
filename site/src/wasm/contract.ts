// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PhoneProblem } from "../device/usb.ts";

export interface PageFile {
  size(): number;
  read(offset: number, length: number): Uint8Array;
}

export interface PickedPageFile {
  name: string;
  file: PageFile;
}

export interface PageSink {
  write(bytes: Uint8Array): number;
  flush(): void;
}

export interface PageLink {
  send(bytes: Uint8Array): Promise<void>;
  receive(limit: number): Promise<Uint8Array>;
}

export const OpeningKind = { Opened: "opened", Unavailable: "unavailable" } as const;

export type PhoneOpening =
  | { [OpeningKind.Opened]: PageLink }
  | { [OpeningKind.Unavailable]: PhoneProblem };

export const TransferFailureKind = {
  Connection: "connection",
  CrossOriginRefused: "cross_origin_refused",
  Status: "status",
} as const;

export type TransferFailure =
  | { kind: typeof TransferFailureKind.Connection }
  | { kind: typeof TransferFailureKind.CrossOriginRefused }
  | {
      kind: typeof TransferFailureKind.Status;
      status: number;
      retryAfter: string | null;
    };

export class RangeReadFailure extends Error {
  override readonly name = "RangeReadFailure";
  readonly failure: TransferFailure;

  constructor(failure: TransferFailure) {
    super(`The range request failed: ${failure.kind}.`);
    this.failure = failure;
  }
}

export type PauseBeforeRetry = (failure: TransferFailure, attempt: number) => number | undefined;

export const TransferEnd = { Received: "received", Failed: "failed" } as const;

export type FinalFailure = Exclude<TransferFailure, { kind: typeof TransferFailureKind.Connection }>;

export type TransferResult<T> =
  | { kind: typeof TransferEnd.Received; value: T }
  | { kind: typeof TransferEnd.Failed; failure: FinalFailure };

export interface FileDigest {
  restart(): void;
  update(piece: Uint8Array): void;
  free(): void;
}

export interface RangeArrival {
  restart(): void;
  take(piece: Uint8Array): number;
  free(): void;
}

export interface KeptFile {
  name: string;
  size: number;
}

export interface ReleaseSite {
  fetchReleases(): Promise<TransferResult<string>>;
  fetchReleaseManifest(release: bigint): Promise<TransferResult<string>>;
}

export interface ListedRelease {
  tag: string;
  device: string;
  build: string;
  series: string | undefined;
  fetches: ReleaseFetch[];
}

export interface ReleasesToChoose {
  releases: ListedRelease[];
  unlistedTag: string | undefined;
}

export const FetchKind = {
  BuildFile: "build_file",
  SourceFiles: "source_files",
  SourceLfsObjects: "source_lfs_objects",
  StockFirmware: "stock_firmware",
} as const;
export type FetchKind = (typeof FetchKind)[keyof typeof FetchKind];

export type ReleaseFetch =
  | { kind: typeof FetchKind.BuildFile; address: string }
  | { kind: typeof FetchKind.SourceFiles; repository: string; address: string }
  | { kind: typeof FetchKind.SourceLfsObjects; repository: string; address: string }
  | { kind: typeof FetchKind.StockFirmware; file_name: string; address: string };

export interface StoragePage extends ReleaseSite {
  keepFiles(names: string[]): Promise<KeptFile[]>;
  createFile(name: string, size: number): Promise<void>;
  download(
    address: string,
    name: string,
    offset: number,
    size: number,
    digest: FileDigest,
  ): Promise<TransferResult<number>>;
  downloadRange(
    address: string,
    name: string,
    offset: number,
    length: number,
    arrival: RangeArrival,
  ): Promise<TransferResult<number>>;
  file(name: string): PageFile;
  fileFrom(name: string, offset: number): PageSink;
  canReadByRange(address: string): Promise<boolean>;
  canDownload(address: string): Promise<boolean>;
  remoteFile(address: string, size: number): PageFile;
  waitBeforeRetry(failedAttempts: number, milliseconds: number): Promise<void>;
  pickedFiles(): Promise<PickedPageFile[]>;
  removeFile(name: string): Promise<void>;
  removeAllFiles(): Promise<void>;
  freeStorage(): Promise<number>;
  report(progress: Progress): void;
}

export interface InstallPage extends StoragePage {
  openPhone(): Promise<PhoneOpening>;
  reopenPhone(): Promise<PhoneOpening>;
  showPhoneMessage(text: string): void;
}

export const ChunkOperationKind = {
  Replace: "replace",
  ReplaceBz: "replace_bz",
  ReplaceXz: "replace_xz",
} as const;
export type ChunkOperationKind = (typeof ChunkOperationKind)[keyof typeof ChunkOperationKind];

export interface PackagePage extends StoragePage {
  writePackage(bytes: Uint8Array<ArrayBuffer>): Promise<void>;
  closePackage(): Promise<void>;
  packageInPageStorage(): boolean;
  chunkCompressors(): number;
  compressChunk(kind: ChunkOperationKind, chunk: Uint8Array<ArrayBuffer>): Promise<Uint8Array>;
}

export const StepKind = {
  Flash: "flash",
  RebootFastboot: "reboot_fastboot",
  UpdateSuper: "update_super",
  Erase: "erase",
} as const;

export type Step =
  | { step: typeof StepKind.Flash; partition: string; logical: boolean }
  | { step: typeof StepKind.RebootFastboot }
  | { step: typeof StepKind.UpdateSuper; super_device: string; sha256: string; size: number }
  | { step: typeof StepKind.Erase; partition: string; if_wipe: boolean };

export interface StockPackage {
  file_name: string;
  address: string;
  sha256: string;
  size: number;
}

export const AskedFiles = { StockFirmware: "stock_firmware", ReleaseFiles: "release_files" } as const;
export type AskedFiles = (typeof AskedFiles)[keyof typeof AskedFiles];

export interface AskedFile {
  file_name: string;
  address: string;
  sha256: string;
  size: number;
}

export interface AwaitedFile {
  file: AskedFile;
  picked: boolean;
}

export interface Source {
  repository: string;
  commit: string;
}

export const Stage = {
  Connecting: "connecting",
  Checking: "checking",
  PhoneChecked: "phone_checked",
  ChoosingFiles: "choosing_files",
  CheckingPickedFiles: "checking_picked_files",
  FetchingStockFirmware: "fetching_stock_firmware",
  StockFirmwareChecked: "stock_firmware_checked",
  StockFirmwareBlockChecked: "stock_firmware_block_checked",
  UnpackingStockImage: "unpacking_stock_image",
  FetchingImages: "fetching_images",
  FetchedImagesChecked: "fetched_images_checked",
  ReadingStockFiles: "reading_stock_files",
  FetchingFiles: "fetching_files",
  Building: "building",
  ImageChecked: "image_checked",
  Installing: "installing",
  Sending: "sending",
  Done: "done",
  ReadingRelease: "reading_release",
  ReleaseRead: "release_read",
  WritingPackage: "writing_package",
  PackageSaved: "package_saved",
} as const;
export type Stage = (typeof Stage)[keyof typeof Stage];

export const BlockEdge = { Start: "start", End: "end" } as const;
export type BlockEdge = (typeof BlockEdge)[keyof typeof BlockEdge];

export interface BlockDownload {
  downloaded: number;
  block: number;
  readFrom: BlockEdge;
}

export interface InstallPlan {
  device: string;
  build: string;
  shipped: string[];
  built: string[];
  sources: Source[];
  stockFiles: StockPackage[];
  install: Step[];
}

export const CheckVerdict = { Passed: "passed", Failed: "failed" } as const;
export type CheckVerdict = (typeof CheckVerdict)[keyof typeof CheckVerdict];

export const PhoneCheckKind = {
  Model: "model",
  Bootloader: "bootloader",
  Battery: "battery",
  Slot: "slot",
  Partitions: "partitions",
  TransferSize: "transfer_size",
} as const;
export type PhoneCheckKind = (typeof PhoneCheckKind)[keyof typeof PhoneCheckKind];

export const PhoneAnswer = { Yes: "yes", No: "no" } as const;
export type PhoneAnswer = (typeof PhoneAnswer)[keyof typeof PhoneAnswer];

export type PhoneReading =
  | { check: typeof PhoneCheckKind.Model; product: string | undefined }
  | { check: typeof PhoneCheckKind.Bootloader; unlocked: PhoneAnswer | undefined }
  | { check: typeof PhoneCheckKind.Battery; socOk: PhoneAnswer | undefined }
  | { check: typeof PhoneCheckKind.Slot; currentSlot: string | undefined }
  | { check: typeof PhoneCheckKind.Partitions; fit: { fitting: number; count: number } | undefined }
  | { check: typeof PhoneCheckKind.TransferSize; downloadLimit: number | undefined };

export interface PhoneCheck {
  verdict: CheckVerdict;
  reading: PhoneReading;
}

export type Progress =
  | { stage: typeof Stage.Connecting }
  | ({ stage: typeof Stage.Checking } & InstallPlan)
  | ({ stage: typeof Stage.PhoneChecked } & PhoneCheck)
  | {
      stage: typeof Stage.ChoosingFiles;
      asked: AskedFiles;
      files: AwaitedFile[];
      wrongPicks: InstallFailure[];
    }
  | { stage: typeof Stage.CheckingPickedFiles; done: number; total: number }
  | ({ stage: typeof Stage.FetchingStockFirmware } & BlockDownload)
  | { stage: typeof Stage.StockFirmwareChecked }
  | ({ stage: typeof Stage.StockFirmwareBlockChecked } & BlockDownload)
  | { stage: typeof Stage.UnpackingStockImage; partition: string; done: number; total: number }
  | { stage: typeof Stage.FetchingImages; done: number; total: number }
  | { stage: typeof Stage.FetchedImagesChecked }
  | { stage: typeof Stage.ReadingStockFiles; partition: string; done: number; total: number }
  | { stage: typeof Stage.FetchingFiles; partition: string; done: number; total: number }
  | { stage: typeof Stage.Building; partition: string; done: number; total: number }
  | { stage: typeof Stage.ImageChecked; partition: string }
  | { stage: typeof Stage.Installing; index: number; count: number; step: Step }
  | { stage: typeof Stage.Sending; done: number; total: number }
  | { stage: typeof Stage.Done }
  | { stage: typeof Stage.ReadingRelease }
  | ({ stage: typeof Stage.ReleaseRead } & InstallPlan)
  | { stage: typeof Stage.WritingPackage; done: number; total: number }
  | { stage: typeof Stage.PackageSaved };

export interface InstallFailure {
  cause: string;
  nextStep: string;
}
