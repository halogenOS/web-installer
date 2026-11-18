// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { PageFile } from "../wasm/contract.ts";
import { readRangeBlocking } from "./fetch.ts";

export class RangedFile implements PageFile {
  readonly #href: string;
  readonly #size: number;

  constructor(href: string, size: number) {
    this.#href = href;
    this.#size = size;
  }

  size(): number {
    return this.#size;
  }

  read(offset: number, length: number): Uint8Array {
    return readRangeBlocking(this.#href, offset, length);
  }
}
