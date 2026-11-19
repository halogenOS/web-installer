// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { TestContext } from "node:test";
import { provideGlobal } from "./globals.ts";

// The steps and error names follow the File System standard, https://fs.spec.whatwg.org/ (read 2026-11-19).
const FileSystemRefusal = {
  NotFound: "NotFoundError",
  TypeMismatch: "TypeMismatchError",
  InvalidModification: "InvalidModificationError",
  NoModificationAllowed: "NoModificationAllowedError",
  InvalidState: "InvalidStateError",
  QuotaExceeded: "QuotaExceededError",
} as const;
type FileSystemRefusal = (typeof FileSystemRefusal)[keyof typeof FileSystemRefusal];

function refusal(name: FileSystemRefusal, message: string): DOMException {
  return new DOMException(message, name);
}

class SyncHandleDouble implements FileSystemSyncAccessHandle {
  readonly #file: FileDouble;
  readonly #files: OriginPrivateFiles;
  #closed = false;
  #cursor = 0;

  constructor(file: FileDouble, files: OriginPrivateFiles) {
    this.#file = file;
    this.#files = files;
  }

  read(buffer: Uint8Array, options?: { at: number }): number {
    this.#requireOpen();
    const at = options?.at ?? this.#cursor;
    const found = this.#file.bytes.subarray(at, at + buffer.length);
    buffer.set(found);
    this.#cursor = at + found.length;
    return found.length;
  }

  write(buffer: Uint8Array, options?: { at: number }): number {
    this.#requireOpen();
    const at = options?.at ?? this.#cursor;
    this.#resize(Math.max(this.#file.bytes.length, at + buffer.length));
    this.#file.bytes.set(buffer, at);
    this.#cursor = at + buffer.length;
    return buffer.length;
  }

  truncate(size: number): void {
    this.#requireOpen();
    this.#resize(size);
    this.#cursor = Math.min(this.#cursor, size);
  }

  getSize(): number {
    this.#requireOpen();
    return this.#file.bytes.length;
  }

  flush(): void {
    this.#requireOpen();
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#file.unlock();
  }

  #requireOpen(): void {
    if (this.#closed) throw refusal(FileSystemRefusal.InvalidState, "The access handle is closed.");
  }

  #resize(size: number): void {
    if (size - this.#file.bytes.length > this.#files.room()) {
      throw refusal(FileSystemRefusal.QuotaExceeded, `${this.#file.name} cannot grow to ${String(size)} bytes.`);
    }
    const resized = new Uint8Array(size);
    resized.set(this.#file.bytes.subarray(0, size));
    this.#file.bytes = resized;
  }
}

class FileDouble implements FileSystemFileHandle {
  readonly kind = "file";
  readonly name: string;
  bytes = new Uint8Array(0);
  readonly #files: OriginPrivateFiles;
  #locked = false;

  constructor(name: string, files: OriginPrivateFiles) {
    this.name = name;
    this.#files = files;
  }

  isSameEntry(other: FileSystemHandle): Promise<boolean> {
    return Promise.resolve(other === this);
  }

  getFile(): Promise<File> {
    return Promise.resolve(new File([this.bytes], this.name));
  }

  createWritable(): Promise<FileSystemWritableFileStream> {
    return Promise.reject(new Error("The tests write no stored file through a writable stream."));
  }

  createSyncAccessHandle(): Promise<FileSystemSyncAccessHandle> {
    if (this.#locked) {
      return Promise.reject(refusal(FileSystemRefusal.NoModificationAllowed, `${this.name} has an open access handle.`));
    }
    this.#locked = true;
    return Promise.resolve(new SyncHandleDouble(this, this.#files));
  }

  isLocked(): boolean {
    return this.#locked;
  }

  unlock(): void {
    this.#locked = false;
  }

  size(): number {
    return this.bytes.length;
  }

  markRemoved(): void {
    this.bytes = new Uint8Array(0);
  }
}

type EntryDouble = DirectoryDouble | FileDouble;

class DirectoryDouble implements FileSystemDirectoryHandle {
  readonly kind = "directory";
  readonly name: string;
  readonly #files: OriginPrivateFiles;
  readonly #entries = new Map<string, EntryDouble>();
  #removed = false;

  constructor(name: string, files: OriginPrivateFiles) {
    this.name = name;
    this.#files = files;
  }

  isSameEntry(other: FileSystemHandle): Promise<boolean> {
    return Promise.resolve(other === this);
  }

  async getDirectoryHandle(name: string, options?: FileSystemGetDirectoryOptions): Promise<FileSystemDirectoryHandle> {
    return this.#child(name, options?.create, DirectoryDouble, () => new DirectoryDouble(name, this.#files));
  }

  async getFileHandle(name: string, options?: FileSystemGetFileOptions): Promise<FileSystemFileHandle> {
    return this.#child(name, options?.create, FileDouble, () => new FileDouble(name, this.#files));
  }

  // The standard leaves the refusal of a file with an open access handle to the file system. Chromium refuses it with
  // NoModificationAllowedError.
  async removeEntry(name: string, options?: FileSystemRemoveOptions): Promise<void> {
    const entry = this.#entries.get(name);
    if (this.#removed || entry === undefined) throw refusal(FileSystemRefusal.NotFound, `${name} does not exist.`);
    if (entry instanceof DirectoryDouble && entry.#entries.size > 0 && options?.recursive !== true) {
      throw refusal(FileSystemRefusal.InvalidModification, `${name} is a folder with entries.`);
    }
    if (entry.isLocked()) throw refusal(FileSystemRefusal.NoModificationAllowed, `${name} has an open access handle.`);
    this.#entries.delete(name);
    entry.markRemoved();
  }

  async resolve(possibleDescendant: FileSystemHandle): Promise<string[] | null> {
    return this.#pathTo(possibleDescendant) ?? null;
  }

  async *entries(): FileSystemDirectoryHandleAsyncIterator<[string, FileSystemHandle]> {
    this.#requirePresent();
    for (const entry of this.#entries) yield entry;
  }

  async *keys(): FileSystemDirectoryHandleAsyncIterator<string> {
    this.#requirePresent();
    for (const name of this.#entries.keys()) yield name;
  }

  async *values(): FileSystemDirectoryHandleAsyncIterator<FileSystemHandle> {
    this.#requirePresent();
    for (const entry of this.#entries.values()) yield entry;
  }

  [Symbol.asyncIterator](): FileSystemDirectoryHandleAsyncIterator<[string, FileSystemHandle]> {
    return this.entries();
  }

  isLocked(): boolean {
    return [...this.#entries.values()].some((entry) => entry.isLocked());
  }

  size(): number {
    return [...this.#entries.values()].reduce((sum, entry) => sum + entry.size(), 0);
  }

  markRemoved(): void {
    this.#removed = true;
    for (const entry of this.#entries.values()) entry.markRemoved();
    this.#entries.clear();
  }

  #requirePresent(): void {
    if (this.#removed) throw refusal(FileSystemRefusal.NotFound, `The folder ${this.name} was removed.`);
  }

  #child<E extends EntryDouble>(
    name: string,
    create: boolean | undefined,
    kind: abstract new (...parts: never[]) => E,
    make: () => E,
  ): E {
    this.#requirePresent();
    const found = this.#entries.get(name);
    if (found === undefined) {
      if (create !== true) throw refusal(FileSystemRefusal.NotFound, `${name} does not exist.`);
      const made = make();
      this.#entries.set(name, made);
      return made;
    }
    if (!(found instanceof kind)) throw refusal(FileSystemRefusal.TypeMismatch, `${name} is a ${found.kind}.`);
    return found;
  }

  #pathTo(target: FileSystemHandle): string[] | undefined {
    if (target === this) return [];
    for (const [name, entry] of this.#entries) {
      const rest = entry instanceof DirectoryDouble ? entry.#pathTo(target) : entry === target ? [] : undefined;
      if (rest !== undefined) return [name, ...rest];
    }
    return undefined;
  }
}

export class OriginPrivateFiles {
  readonly root: FileSystemDirectoryHandle;
  readonly #directory: DirectoryDouble;
  readonly #quota: number;

  constructor(quota: number) {
    this.#quota = quota;
    this.#directory = new DirectoryDouble("", this);
    this.root = this.#directory;
  }

  room(): number {
    return this.#quota - this.#directory.size();
  }

  estimate(): StorageEstimate {
    return { quota: this.#quota, usage: this.#directory.size() };
  }
}

export function provideOriginPrivateFiles(context: TestContext, quota: number): OriginPrivateFiles {
  const files = new OriginPrivateFiles(quota);
  const storage: StorageManager = {
    estimate: () => Promise.resolve(files.estimate()),
    getDirectory: () => Promise.resolve(files.root),
    persist: () => Promise.reject(new Error("The page does not ask for persistent storage.")),
    persisted: () => Promise.reject(new Error("The page does not ask for persistent storage.")),
  };
  provideGlobal(context, navigator, "storage", storage);
  provideGlobal(context, globalThis, "FileSystemFileHandle", FileDouble);
  provideGlobal(context, globalThis, "FileSystemDirectoryHandle", DirectoryDouble);
  return files;
}

export async function readStoredFile(directory: FileSystemDirectoryHandle, name: string): Promise<Uint8Array> {
  return new Uint8Array(await (await (await directory.getFileHandle(name)).getFile()).arrayBuffer());
}

export async function entryNames(directory: FileSystemDirectoryHandle): Promise<string[]> {
  const names: string[] = [];
  for await (const name of directory.keys()) names.push(name);
  return names.sort();
}
