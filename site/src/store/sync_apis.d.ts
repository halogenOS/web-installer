// SPDX-License-Identifier: GPL-3.0-only

interface FileSystemSyncAccessHandle {
  write(buffer: Uint8Array, options?: { at: number }): number;
  read(buffer: Uint8Array, options?: { at: number }): number;
  truncate(size: number): void;
  getSize(): number;
  flush(): void;
  close(): void;
}

declare class FileReaderSync {
  readAsArrayBuffer(blob: Blob): ArrayBuffer;
}

interface FileSystemFileHandle {
  createSyncAccessHandle?(): Promise<FileSystemSyncAccessHandle>;
}
