// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import { provideGlobal } from "../browser_doubles/globals.ts";
import {
  OriginPrivateFiles,
  entryNames,
  provideOriginPrivateFiles,
  readStoredFile,
} from "../browser_doubles/origin_private_files.ts";
import { InstallStore, createPackageFile, isInPageStorage } from "./install_store.ts";

const ROOMY = 1 << 20;

function isQuotaExceeded(failure: unknown): failure is DOMException {
  return failure instanceof DOMException && failure.name === "QuotaExceededError";
}

// The store puts its files into one folder of page storage. A store that has created a file shows which folder that
// is, so a test can lay out what an earlier run left there.
async function folderOfStore(files: OriginPrivateFiles): Promise<FileSystemDirectoryHandle> {
  const store = new InstallStore();
  await store.create("probe", 0);
  const [name] = await entryNames(files.root);
  await store.removeAll();
  assert.ok(name !== undefined);
  return files.root.getDirectoryHandle(name, { create: true });
}

async function writeFile(directory: FileSystemDirectoryHandle, name: string, bytes: Uint8Array): Promise<void> {
  const file = await directory.getFileHandle(name, { create: true });
  assert.ok(file.createSyncAccessHandle !== undefined);
  const handle = await file.createSyncAccessHandle();
  handle.write(bytes, { at: 0 });
  handle.close();
}

interface StoreInPageStorage {
  store: InstallStore;
  folder: () => Promise<FileSystemDirectoryHandle>;
}

function storeWithFolder(context: TestContext, quota: number): StoreInPageStorage {
  const files = provideOriginPrivateFiles(context, quota);
  const store = new InstallStore();
  return {
    store,
    folder: async () => {
      const [name] = await entryNames(files.root);
      assert.ok(name !== undefined);
      return files.root.getDirectoryHandle(name);
    },
  };
}

test("a created file has the asked size, and its bytes read back as written", async (context) => {
  const { store, folder } = storeWithFolder(context, ROOMY);
  await store.create("boot.img", 6);
  const file = store.file("boot.img");
  file.writeAt(2, new Uint8Array([1, 2]));
  file.flush();
  assert.equal(file.size(), 6);
  assert.deepEqual(file.read(1, 3), new Uint8Array([0, 1, 2]));
  assert.deepEqual(await readStoredFile(await folder(), "boot.img"), new Uint8Array([0, 0, 1, 2, 0, 0]));
});

test("a file fails to be created as full storage when the storage has no room for its size", async (context) => {
  const { store } = storeWithFolder(context, 4);
  await assert.rejects(store.create("boot.img", 8), isQuotaExceeded);
});

test("a write fails as full storage when its bytes do not fit", async (context) => {
  const { store } = storeWithFolder(context, 6);
  await store.create("boot.img", 4);
  assert.throws(() => {
    store.file("boot.img").writeAt(3, new Uint8Array([1, 2, 3, 4]));
  }, isQuotaExceeded);
});

test("a read past the end of a stored file fails as not readable", async (context) => {
  const { store } = storeWithFolder(context, ROOMY);
  await store.create("boot.img", 4);
  assert.throws(
    () => store.file("boot.img").read(2, 3),
    (failure) => failure instanceof DOMException && failure.name === "NotReadableError",
  );
});

test("a sink writes each piece after the one before, starting at its offset", async (context) => {
  const { store } = storeWithFolder(context, ROOMY);
  await store.create("super.img", 6);
  const sink = store.file("super.img").sinkFrom(1);
  assert.equal(sink.write(new Uint8Array([1, 2])), 2);
  assert.equal(sink.write(new Uint8Array([3])), 1);
  sink.flush();
  assert.deepEqual(store.file("super.img").read(0, 6), new Uint8Array([0, 1, 2, 3, 0, 0]));
});

test("a file that was not created or kept is no file of the store", async (context) => {
  const { store } = storeWithFolder(context, ROOMY);
  assert.throws(() => store.file("boot.img"), /no stored file boot.img/);
});

test("keeping files keeps the listed files with their sizes and removes every other entry of the folder", async (context) => {
  const files = provideOriginPrivateFiles(context, ROOMY);
  const folder = await folderOfStore(files);
  await writeFile(folder, "boot.img", new Uint8Array([1, 2, 3]));
  await writeFile(folder, "vendor.img", new Uint8Array([4, 5]));
  await writeFile(await folder.getDirectoryHandle("leftover", { create: true }), "part", new Uint8Array([6]));
  const store = new InstallStore();
  assert.deepEqual(await store.keep(["boot.img", "dtbo.img"]), [{ name: "boot.img", size: 3 }]);
  assert.deepEqual(await entryNames(folder), ["boot.img"]);
  assert.deepEqual(store.file("boot.img").read(0, 3), new Uint8Array([1, 2, 3]));
});

test("a removed file is closed and gone from the folder and from the store", async (context) => {
  const { store, folder } = storeWithFolder(context, ROOMY);
  await store.create("boot.img", 2);
  await store.create("dtbo.img", 2);
  await store.remove("boot.img");
  assert.deepEqual(await entryNames(await folder()), ["dtbo.img"]);
  assert.throws(() => store.file("boot.img"), /no stored file boot.img/);
});

test("removing all files closes them and removes the folder of the store, and leaves other entries in page storage", async (context) => {
  const files = provideOriginPrivateFiles(context, ROOMY);
  await files.root.getDirectoryHandle("other", { create: true });
  const store = new InstallStore();
  await store.create("boot.img", 2);
  await store.create("dtbo.img", 2);
  await store.removeAll();
  assert.deepEqual(await entryNames(files.root), ["other"]);
  assert.throws(() => store.file("dtbo.img"), /no stored file dtbo.img/);
});

test("the free storage is the quota less what page storage uses", async (context) => {
  const { store } = storeWithFolder(context, 100);
  await store.create("boot.img", 30);
  assert.equal(await store.freeStorage(), 70);
});

test("the free storage cannot be told when the browser does not report quota and usage", async (context) => {
  const files = new OriginPrivateFiles(ROOMY);
  const silentStorage: Pick<StorageManager, "estimate" | "getDirectory"> = {
    estimate: () => Promise.resolve({}),
    getDirectory: () => Promise.resolve(files.root),
  };
  provideGlobal(context, navigator, "storage", silentStorage);
  await assert.rejects(new InstallStore().freeStorage(), /does not report how much/);
});

test("a package file is the only file in a folder of its own in page storage", async (context) => {
  const files = provideOriginPrivateFiles(context, ROOMY);
  await createPackageFile("first.zip");
  const file = await createPackageFile("second.zip");
  const path = await files.root.resolve(file);
  assert.ok(path !== null);
  const [folder] = path;
  assert.ok(folder !== undefined);
  assert.deepEqual(path, [folder, "second.zip"]);
  assert.deepEqual(await entryNames(await files.root.getDirectoryHandle(folder)), ["second.zip"]);
});

test("a file is in page storage only when page storage contains it", async (context) => {
  const files = provideOriginPrivateFiles(context, ROOMY);
  const inside = await files.root.getFileHandle("inside.zip", { create: true });
  const outside = await new OriginPrivateFiles(ROOMY).root.getFileHandle("outside.zip", { create: true });
  assert.equal(await isInPageStorage(inside), true);
  assert.equal(await isInPageStorage(outside), false);
});
