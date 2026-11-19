// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test } from "node:test";
import { OriginPrivateFiles, provideOriginPrivateFiles } from "../browser_doubles/origin_private_files.ts";
import { FileChoice, choosePackageFile } from "./package_file.ts";

const BUILD = "xos-17.0-20261119-guacamole";
const ROOMY = 1 << 20;

function windowWithDialog(dialog: (options?: SaveFilePickerOptions) => Promise<FileSystemFileHandle>): Pick<Window, "showSaveFilePicker"> {
  return { showSaveFilePicker: dialog };
}

test("a browser without a save dialog stores the package as the build name in page storage", async (context) => {
  const files = provideOriginPrivateFiles(context, ROOMY);
  const choice = await choosePackageFile({}, BUILD);
  assert.ok(choice.kind === FileChoice.Stored);
  assert.equal(choice.file.name, `${BUILD}.zip`);
  assert.notEqual(await files.root.resolve(choice.file), null);
});

test("a save dialog suggests the build name as zip file, and the chosen file is the package", async () => {
  const chosen = await new OriginPrivateFiles(ROOMY).root.getFileHandle("chosen.zip", { create: true });
  const asked: (SaveFilePickerOptions | undefined)[] = [];
  const dialogWindow = windowWithDialog((options) => {
    asked.push(options);
    return Promise.resolve(chosen);
  });
  assert.deepEqual(await choosePackageFile(dialogWindow, BUILD), { kind: FileChoice.Chosen, file: chosen });
  assert.deepEqual(
    asked.map((options) => [options?.suggestedName, options?.types?.[0]?.accept]),
    [[`${BUILD}.zip`, { "application/zip": [".zip"] }]],
  );
});

test("a save dialog closed without a choice cancels the package", async () => {
  const dialogWindow = windowWithDialog(() => Promise.reject(new DOMException("The user aborted a request.", "AbortError")));
  assert.deepEqual(await choosePackageFile(dialogWindow, BUILD), { kind: FileChoice.Cancelled });
});

test("a save dialog that fails in any other way is a failed dialog", async () => {
  const dialogWindow = windowWithDialog(() => Promise.reject(new DOMException("The file cannot be chosen.", "SecurityError")));
  assert.deepEqual(await choosePackageFile(dialogWindow, BUILD), { kind: FileChoice.DialogFailed });
});

test("a failure of the dialog that is no DOMException goes on to the caller", async () => {
  const failure = new TypeError("The options cannot be read.");
  const dialogWindow = windowWithDialog(() => Promise.reject(failure));
  await assert.rejects(choosePackageFile(dialogWindow, BUILD), failure);
});
