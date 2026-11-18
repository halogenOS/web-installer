// SPDX-License-Identifier: GPL-3.0-only

export const QUOTA_EXCEEDED = "QuotaExceededError";

const NOT_READABLE = "NotReadableError";

function isQuotaExceeded(cause: unknown): boolean {
  return cause instanceof DOMException && cause.name === QUOTA_EXCEEDED;
}

function writeOnce(handle: FileSystemSyncAccessHandle, bytes: Uint8Array, at: number): number {
  try {
    return handle.write(bytes, { at });
  } catch (cause) {
    if (isQuotaExceeded(cause)) return 0;
    throw cause;
  }
}

export function writeUntilRefused(
  handle: FileSystemSyncAccessHandle,
  bytes: Uint8Array,
  at: number,
): number {
  let stored = 0;
  while (stored < bytes.length) {
    const rest = bytes.subarray(stored);
    const written = writeOnce(handle, rest, at + stored);
    if (written === 0 || written > rest.length) break;
    stored += written;
  }
  return stored;
}

export function readExactly(
  handle: FileSystemSyncAccessHandle,
  target: Uint8Array,
  at: number,
): void {
  let done = 0;
  while (done < target.length) {
    const rest = target.subarray(done);
    const read = handle.read(rest, { at: at + done });
    if (read === 0 || read > rest.length) {
      throw new DOMException("The file has fewer bytes than were asked for.", NOT_READABLE);
    }
    done += read;
  }
}
