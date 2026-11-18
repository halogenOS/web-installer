// SPDX-License-Identifier: GPL-3.0-only
import {
  RangeReadFailure,
  TransferEnd,
  TransferFailureKind,
  type PauseBeforeRetry,
  type TransferResult,
} from "../wasm/contract.ts";
import {
  PARTIAL_CONTENT,
  fetchRangeWithRetries,
  fetchWithRetries,
  requestStatusWithRetries,
  type BodySink,
  type RemotePorts,
  type RemoteTransfer,
  type ReportRetry,
} from "./transfer.ts";
import type { AttemptScheduler } from "./request_scheduler.ts";

const NETWORK_ERROR = "NetworkError";

export function waitFor(milliseconds: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

function byteRange(offset: number, length: number): string {
  return `bytes=${String(offset)}-${String(offset + length - 1)}`;
}

function networkPorts(pauseBeforeRetry: PauseBeforeRetry, scheduler: AttemptScheduler): RemotePorts {
  return {
    request: (href, init) => fetch(href, init),
    pauseBeforeRetry,
    wait: waitFor,
    scheduler,
  };
}

export function fetchRemote<T>(
  transfer: RemoteTransfer<T>,
  pauseBeforeRetry: PauseBeforeRetry,
  scheduler: AttemptScheduler,
): Promise<TransferResult<T>> {
  return fetchWithRetries(transfer, networkPorts(pauseBeforeRetry, scheduler));
}

export function fetchRemoteRange<T>(
  href: string,
  range: { offset: number; length: number },
  sink: BodySink<T>,
  reportRetry: ReportRetry,
  pauseBeforeRetry: PauseBeforeRetry,
  scheduler: AttemptScheduler,
): Promise<TransferResult<T>> {
  return fetchRangeWithRetries(
    { href, headers: { Range: byteRange(range.offset, range.length) }, sink, reportRetry },
    networkPorts(pauseBeforeRetry, scheduler),
  );
}

export async function canReadByRange(
  href: string,
  pauseBeforeRetry: PauseBeforeRetry,
  reportRetry: ReportRetry,
  scheduler: AttemptScheduler,
): Promise<boolean> {
  const result = await requestStatusWithRetries(
    { href, headers: { Range: byteRange(0, 1) }, reportRetry },
    networkPorts(pauseBeforeRetry, scheduler),
  );
  return result.kind === TransferEnd.Received && result.value === PARTIAL_CONTENT;
}

export async function canDownload(
  href: string,
  pauseBeforeRetry: PauseBeforeRetry,
  reportRetry: ReportRetry,
  scheduler: AttemptScheduler,
): Promise<boolean> {
  const result = await requestStatusWithRetries(
    { href, headers: {}, reportRetry },
    networkPorts(pauseBeforeRetry, scheduler),
  );
  return result.kind === TransferEnd.Received;
}

export function readRangeBlocking(href: string, offset: number, length: number): Uint8Array {
  if (length === 0) return new Uint8Array(0);
  const request = new XMLHttpRequest();
  request.open("GET", href, false);
  request.setRequestHeader("Range", byteRange(offset, length));
  request.responseType = "arraybuffer";
  try {
    request.send();
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === NETWORK_ERROR) {
      throw new RangeReadFailure({ kind: TransferFailureKind.Connection });
    }
    throw cause;
  }
  if (request.status !== PARTIAL_CONTENT) {
    throw new RangeReadFailure({
      kind: TransferFailureKind.Status,
      status: request.status,
      retryAfter: request.getResponseHeader("retry-after"),
    });
  }
  const body: unknown = request.response;
  if (!(body instanceof ArrayBuffer && body.byteLength === length)) {
    throw new DOMException(
      `The response to the range of ${String(length)} bytes from ${String(offset)} has ` +
        `status ${String(request.status)} and does not contain those bytes.`,
      "NotReadableError",
    );
  }
  return new Uint8Array(body);
}
