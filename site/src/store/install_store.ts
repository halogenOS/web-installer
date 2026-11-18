// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { KeptFile, PageFile, PageSink } from "../wasm/contract.ts";
import { QUOTA_EXCEEDED, readExactly, writeUntilRefused } from "./sync_transfer.ts";

const INSTALL_DIRECTORY = "install";

const PACKAGE_DIRECTORY = "package";

const NOT_FOUND = "NotFoundError";

function storageFull(name: string, size: number): DOMException {
  return new DOMException(
    `The browser did not give the stored file ${name} its ${size} bytes.`,
    QUOTA_EXCEEDED,
  );
}

class SinkFrom implements PageSink {
  readonly #file: SyncFile;
  #end: number;

  constructor(file: SyncFile, offset: number) {
    this.#file = file;
    this.#end = offset;
  }

  write(bytes: Uint8Array): number {
    this.#file.writeAt(this.#end, bytes);
    this.#end += bytes.length;
    return bytes.length;
  }

  flush(): void {
    this.#file.flush();
  }
}

class SyncFile implements PageFile {
  readonly #name: string;
  readonly #handle: FileSystemSyncAccessHandle;

  constructor(name: string, handle: FileSystemSyncAccessHandle) {
    this.#name = name;
    this.#handle = handle;
  }

  size(): number {
    return this.#handle.getSize();
  }

  read(offset: number, length: number): Uint8Array {
    const bytes = new Uint8Array(length);
    readExactly(this.#handle, bytes, offset);
    return bytes;
  }

  resize(size: number): void {
    this.#handle.truncate(size);
    if (this.#handle.getSize() !== size) throw storageFull(this.#name, size);
  }

  sinkFrom(offset: number): PageSink {
    return new SinkFrom(this, offset);
  }

  writeAt(offset: number, bytes: Uint8Array): void {
    if (writeUntilRefused(this.#handle, bytes, offset) < bytes.length) {
      throw storageFull(this.#name, offset + bytes.length);
    }
  }

  flush(): void {
    this.#handle.flush();
  }

  close(): void {
    this.#handle.close();
  }
}

async function removeDirectory(root: FileSystemDirectoryHandle, name: string): Promise<void> {
  try {
    await root.removeEntry(name, { recursive: true });
  } catch (cause) {
    if (!(cause instanceof DOMException && cause.name === NOT_FOUND)) throw cause;
  }
}

async function emptyDirectory(name: string): Promise<FileSystemDirectoryHandle> {
  const root = await navigator.storage.getDirectory();
  await removeDirectory(root, name);
  return root.getDirectoryHandle(name, { create: true });
}

export async function createPackageFile(name: string): Promise<FileSystemFileHandle> {
  return (await emptyDirectory(PACKAGE_DIRECTORY)).getFileHandle(name, { create: true });
}

export async function isInPageStorage(file: FileSystemHandle): Promise<boolean> {
  return (await (await navigator.storage.getDirectory()).resolve(file)) !== null;
}

export class InstallStore {
  #directory: Promise<FileSystemDirectoryHandle> | undefined;
  readonly #files = new Map<string, SyncFile>();

  #installDirectory(): Promise<FileSystemDirectoryHandle> {
    this.#directory ??= navigator.storage
      .getDirectory()
      .then((root) => root.getDirectoryHandle(INSTALL_DIRECTORY, { create: true }));
    return this.#directory;
  }

  async #open(name: string): Promise<SyncFile> {
    const opened = this.#files.get(name);
    if (opened !== undefined) return opened;
    const entry = await (await this.#installDirectory()).getFileHandle(name, { create: true });
    if (entry.createSyncAccessHandle === undefined) {
      throw new Error("This browser cannot write files of the page synchronously.");
    }
    const file = new SyncFile(name, await entry.createSyncAccessHandle());
    this.#files.set(name, file);
    return file;
  }

  // A listing may skip entries once the folder changes under it, so the listing ends before any entry goes.
  async keep(names: readonly string[]): Promise<KeptFile[]> {
    const directory = await this.#installDirectory();
    const entries: FileSystemHandle[] = [];
    for await (const entry of directory.values()) entries.push(entry);
    const kept: KeptFile[] = [];
    for (const entry of entries) {
      if (entry.kind === "file" && names.includes(entry.name)) {
        kept.push({ name: entry.name, size: (await this.#open(entry.name)).size() });
      } else {
        await directory.removeEntry(entry.name, { recursive: true });
      }
    }
    return kept;
  }

  async freeStorage(): Promise<number> {
    const { quota, usage } = await navigator.storage.estimate();
    if (quota === undefined || usage === undefined) {
      throw new Error("The browser does not report how much it lets the page store.");
    }
    return quota - usage;
  }

  async create(name: string, size: number): Promise<void> {
    (await this.#open(name)).resize(size);
  }

  file(name: string): SyncFile {
    const file = this.#files.get(name);
    if (file === undefined) throw new Error(`The install has no stored file ${name}.`);
    return file;
  }

  async remove(name: string): Promise<void> {
    this.file(name).close();
    this.#files.delete(name);
    await (await this.#installDirectory()).removeEntry(name);
  }

  async removeAll(): Promise<void> {
    for (const file of this.#files.values()) file.close();
    this.#files.clear();
    await removeDirectory(await navigator.storage.getDirectory(), INSTALL_DIRECTORY);
  }
}
