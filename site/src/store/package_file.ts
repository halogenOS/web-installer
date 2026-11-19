// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { createPackageFile } from "./install_store.ts";

const ABORTED = "AbortError";

const ZIP_FILE = { description: "OTA package", accept: { "application/zip": [".zip"] } };

export const FileChoice = {
  Chosen: "chosen",
  Stored: "stored",
  Cancelled: "cancelled",
  DialogFailed: "dialog_failed",
} as const;

export type PackageFile =
  | { kind: typeof FileChoice.Chosen; file: FileSystemFileHandle }
  | { kind: typeof FileChoice.Stored; file: FileSystemFileHandle };

export type PackageFileChoice =
  | PackageFile
  | { kind: typeof FileChoice.Cancelled }
  | { kind: typeof FileChoice.DialogFailed };

export async function choosePackageFile(
  window: Pick<Window, "showSaveFilePicker">,
  build: string,
): Promise<PackageFileChoice> {
  const name = `${build}.zip`;
  const showSaveFilePicker = window.showSaveFilePicker;
  if (showSaveFilePicker === undefined) {
    return { kind: FileChoice.Stored, file: await createPackageFile(name) };
  }
  try {
    const file = await showSaveFilePicker.call(window, { suggestedName: name, types: [ZIP_FILE] });
    return { kind: FileChoice.Chosen, file };
  } catch (cause) {
    if (!(cause instanceof DOMException)) throw cause;
    if (cause.name === ABORTED) return { kind: FileChoice.Cancelled };
    return { kind: FileChoice.DialogFailed };
  }
}
