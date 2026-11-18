// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { Stage } from "../wasm/contract.ts";

export const InstallMethod = {
  Zip: "zip",
  Flash: "flash",
} as const;
export type InstallMethod = (typeof InstallMethod)[keyof typeof InstallMethod];

export const InstallStep = {
  Release: "release",
  Phone: "phone",
  Build: "build",
  Write: "write",
  Save: "save",
  Done: "done",
} as const;
export type InstallStep = (typeof InstallStep)[keyof typeof InstallStep];

export interface MethodRun {
  steps: readonly InstallStep[];
  firstStage: Stage;
  buildNote: string | undefined;
  stoppedTitle: string;
  againLabel: string;
}

export const METHODS: Record<InstallMethod, MethodRun> = {
  [InstallMethod.Zip]: {
    steps: [InstallStep.Release, InstallStep.Build, InstallStep.Save, InstallStep.Done],
    firstStage: Stage.ReadingRelease,
    buildNote: undefined,
    stoppedTitle: "Zip stopped",
    againLabel: "Back to the releases",
  },
  [InstallMethod.Flash]: {
    steps: [
      InstallStep.Release,
      InstallStep.Phone,
      InstallStep.Build,
      InstallStep.Write,
      InstallStep.Done,
    ],
    firstStage: Stage.Connecting,
    buildNote: "Nothing is written to your phone until every image is checked.",
    stoppedTitle: "Install stopped",
    againLabel: "Run the install again",
  },
};

export const STEP_LABEL: Record<InstallStep, string> = {
  [InstallStep.Release]: "Release",
  [InstallStep.Phone]: "Phone",
  [InstallStep.Build]: "Build",
  [InstallStep.Write]: "Install",
  [InstallStep.Save]: "Zip",
  [InstallStep.Done]: "Done",
};

export const STEP_OF_STAGE: Record<Stage, InstallStep> = {
  [Stage.Connecting]: InstallStep.Phone,
  [Stage.Checking]: InstallStep.Phone,
  [Stage.ChoosingFiles]: InstallStep.Build,
  [Stage.CheckingPickedFiles]: InstallStep.Build,
  [Stage.FetchingStockFirmware]: InstallStep.Build,
  [Stage.StockFirmwareChecked]: InstallStep.Build,
  [Stage.StockFirmwareBlockChecked]: InstallStep.Build,
  [Stage.UnpackingStockImage]: InstallStep.Build,
  [Stage.FetchingImages]: InstallStep.Build,
  [Stage.FetchedImagesChecked]: InstallStep.Build,
  [Stage.ReadingStockFiles]: InstallStep.Build,
  [Stage.FetchingFiles]: InstallStep.Build,
  [Stage.Building]: InstallStep.Build,
  [Stage.ImageChecked]: InstallStep.Build,
  [Stage.Installing]: InstallStep.Write,
  [Stage.Sending]: InstallStep.Write,
  [Stage.Done]: InstallStep.Done,
  [Stage.ReadingRelease]: InstallStep.Build,
  [Stage.ReleaseRead]: InstallStep.Build,
  [Stage.WritingPackage]: InstallStep.Save,
  [Stage.PackageSaved]: InstallStep.Done,
};
