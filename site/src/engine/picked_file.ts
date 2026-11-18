// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PageFile } from "../wasm/contract.ts";

export class PickedFile implements PageFile {
  readonly #file: File;
  readonly #reader = new FileReaderSync();

  constructor(file: File) {
    this.#file = file;
  }

  size(): number {
    return this.#file.size;
  }

  read(offset: number, length: number): Uint8Array {
    const bytes = new Uint8Array(
      this.#reader.readAsArrayBuffer(this.#file.slice(offset, offset + length)),
    );
    if (bytes.length !== length) {
      throw new DOMException("The picked file has fewer bytes than were asked for.", "NotReadableError");
    }
    return bytes;
  }
}
