// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import { provideGlobal } from "../browser_doubles/globals.ts";
import { PickedFile } from "./picked_file.ts";

// Node reads a Blob only asynchronously, so a picked file of the test carries its bytes for the synchronous reader of
// a worker. Its slices follow Blob.slice, which clamps the range to the size of the file.
class BytesBlob extends Blob {
  readonly contents: Uint8Array<ArrayBuffer>;

  constructor(bytes: Uint8Array<ArrayBuffer>) {
    super([bytes]);
    this.contents = bytes;
  }

  override slice(start?: number, end?: number): BytesBlob {
    return new BytesBlob(this.contents.slice(start, end));
  }
}

class BytesFile extends File {
  readonly #blob: BytesBlob;

  constructor(bytes: Uint8Array<ArrayBuffer>, name: string) {
    super([bytes], name);
    this.#blob = new BytesBlob(bytes);
  }

  override slice(start?: number, end?: number): BytesBlob {
    return this.#blob.slice(start, end);
  }
}

function provideFileReaderSync(context: TestContext): void {
  provideGlobal(
    context,
    globalThis,
    "FileReaderSync",
    class {
      readAsArrayBuffer(blob: Blob): ArrayBuffer {
        assert.ok(blob instanceof BytesBlob);
        return blob.contents.slice().buffer;
      }
    },
  );
}

const STOCK = new BytesFile(new Uint8Array([5, 6, 7, 8, 9]), "stock.zip");

test("a picked file has the size of the file and reads the bytes of each range", (context) => {
  provideFileReaderSync(context);
  const file = new PickedFile(STOCK);
  assert.equal(file.size(), 5);
  assert.deepEqual(file.read(1, 3), new Uint8Array([6, 7, 8]));
  assert.deepEqual(file.read(4, 1), new Uint8Array([9]));
});

test("a read past the end of a picked file fails as not readable", (context) => {
  provideFileReaderSync(context);
  assert.throws(
    () => new PickedFile(STOCK).read(3, 4),
    (failure) => failure instanceof DOMException && failure.name === "NotReadableError",
  );
});
