// SPDX-License-Identifier: GPL-3.0-only
import { AttemptLoad, type AttemptScheduler, type ScheduledWork } from "./request_scheduler.ts";
import {
  TransferEnd,
  TransferFailureKind,
  type FinalFailure,
  type PauseBeforeRetry,
  type TransferFailure,
  type TransferResult,
} from "../wasm/contract.ts";

export interface RemotePorts {
  request: (href: string, init: RequestInit) => Promise<Response>;
  pauseBeforeRetry: PauseBeforeRetry;
  wait: (milliseconds: number) => Promise<void>;
  scheduler: AttemptScheduler;
}

export type ReportRetry = (failedAttempts: number) => void;

export const PARTIAL_CONTENT = 206;

export interface BodySink<T> {
  begin(response: Response): void;
  chunk(bytes: Uint8Array<ArrayBuffer>): boolean;
  end(): Promise<T>;
}

export interface RemoteTransfer<T> {
  href: string;
  headers: Record<string, string>;
  sink: BodySink<T>;
  reportRetry: ReportRetry;
}

const LOST_CONNECTION: TransferFailure = { kind: TransferFailureKind.Connection };

const CROSS_ORIGIN_REFUSED: TransferFailure = { kind: TransferFailureKind.CrossOriginRefused };

export const ANONYMOUS = { credentials: "omit", referrerPolicy: "no-referrer" } as const;

interface Received<T> {
  kind: typeof TransferEnd.Received;
  value: T;
}

async function catchNetworkFailure<T, F>(work: Promise<T>, failed: F): Promise<T | F> {
  try {
    return await work;
  } catch (cause) {
    if (cause instanceof TypeError) return failed;
    throw cause;
  }
}

type SendRequest = (init: RequestInit) => Promise<Response>;

const NO_CORS_HEAD: RequestInit = { method: "HEAD", mode: "no-cors" };

const FILE_REQUEST: RequestInit = { method: "GET", mode: "cors" };

async function requestAgainAfterProbe(send: SendRequest): Promise<Response | TransferFailure> {
  const probe = await catchNetworkFailure(send(NO_CORS_HEAD), LOST_CONNECTION);
  if (!(probe instanceof Response)) return probe;
  return catchNetworkFailure(send(FILE_REQUEST), CROSS_ORIGIN_REFUSED);
}

async function requestFile(send: SendRequest): Promise<Response | TransferFailure> {
  const response = await catchNetworkFailure(send(FILE_REQUEST), LOST_CONNECTION);
  if (response instanceof Response) return response;
  return requestAgainAfterProbe(send);
}

async function failureOfStatus(response: Response): Promise<TransferFailure> {
  if (response.body !== null) await catchNetworkFailure(response.body.cancel(), undefined);
  return {
    kind: TransferFailureKind.Status,
    status: response.status,
    retryAfter: response.headers.get("retry-after"),
  };
}

async function receiveBody<T>(
  response: Response,
  sink: BodySink<T>,
): Promise<Received<T> | TransferFailure> {
  const reader = (response.body ?? new Blob().stream()).getReader();
  let bodyEnded = false;
  try {
    sink.begin(response);
    for (;;) {
      const next = await catchNetworkFailure(reader.read(), LOST_CONNECTION);
      bodyEnded = "kind" in next || next.done;
      if ("kind" in next) return next;
      if (next.done || !sink.chunk(next.value)) {
        return { kind: TransferEnd.Received, value: await sink.end() };
      }
    }
  } finally {
    if (!bodyEnded) await reader.cancel();
  }
}

function convertToFinalFailure(failure: TransferFailure): FinalFailure {
  if (failure.kind === TransferFailureKind.Connection) {
    throw new Error("The engine returned no pause for a connection that failed.");
  }
  return failure;
}

type ReadResponse<T> = (response: Response) => Promise<Received<T> | TransferFailure>;

async function attemptTransfer<T>(
  send: SendRequest,
  readResponse: ReadResponse<T>,
  endRequest: () => void,
): Promise<Received<T> | TransferFailure> {
  const response = await requestFile(send);
  endRequest();
  return response instanceof Response ? readResponse(response) : response;
}

type TriedAttempt<T> =
  | Received<T>
  | { kind: typeof TransferEnd.Failed; failure: TransferFailure; pause: number | undefined };

function loadOf<T>(tried: TriedAttempt<T>): AttemptLoad {
  if (tried.kind === TransferEnd.Received) return AttemptLoad.Delivered;
  switch (tried.failure.kind) {
    case TransferFailureKind.Status:
      return tried.pause === undefined ? AttemptLoad.Delivered : AttemptLoad.HostBusy;
    case TransferFailureKind.Connection:
    case TransferFailureKind.CrossOriginRefused:
      return AttemptLoad.RequestFailed;
  }
}

async function tryAttempt<T>(
  send: SendRequest,
  readResponse: ReadResponse<T>,
  ports: RemotePorts,
  failedAttempts: number,
  endRequest: () => void,
): Promise<{ value: TriedAttempt<T>; load: AttemptLoad }> {
  const attempt = await attemptTransfer(send, readResponse, endRequest);
  const tried: TriedAttempt<T> =
    attempt.kind === TransferEnd.Received
      ? attempt
      : {
          kind: TransferEnd.Failed,
          failure: attempt,
          pause: ports.pauseBeforeRetry(attempt, failedAttempts + 1),
        };
  return { value: tried, load: loadOf(tried) };
}

function isRefused<T>(tried: TriedAttempt<T>): boolean {
  return (
    tried.kind === TransferEnd.Failed &&
    tried.failure.kind === TransferFailureKind.CrossOriginRefused
  );
}

async function sendWithRetries<T>(
  href: string,
  send: SendRequest,
  readResponse: ReadResponse<T>,
  reportRetry: ReportRetry,
  ports: RemotePorts,
): Promise<TransferResult<T>> {
  const origin = new URL(href).origin;
  let failedAttempts = 0;
  for (;;) {
    const work: ScheduledWork<TriedAttempt<T>> = (endRequest) =>
      tryAttempt(send, readResponse, ports, failedAttempts, endRequest);
    const first = await ports.scheduler.run(origin, work);
    const tried = isRefused(first) ? await ports.scheduler.runAlone(origin, work) : first;
    if (tried.kind === TransferEnd.Received) return tried;
    if (tried.pause === undefined) {
      return { kind: TransferEnd.Failed, failure: convertToFinalFailure(tried.failure) };
    }
    failedAttempts += 1;
    reportRetry(failedAttempts);
    await ports.wait(tried.pause);
  }
}

function fetchBodyWithRetries<T>(
  transfer: RemoteTransfer<T>,
  ports: RemotePorts,
  hasBody: (response: Response) => boolean,
): Promise<TransferResult<T>> {
  return sendWithRetries(
    transfer.href,
    (init) => ports.request(transfer.href, { ...init, ...ANONYMOUS, headers: transfer.headers }),
    async (response) =>
      hasBody(response) ? receiveBody(response, transfer.sink) : failureOfStatus(response),
    transfer.reportRetry,
    ports,
  );
}

export function fetchWithRetries<T>(
  transfer: RemoteTransfer<T>,
  ports: RemotePorts,
): Promise<TransferResult<T>> {
  return fetchBodyWithRetries(transfer, ports, (response) => response.ok);
}

export function fetchRangeWithRetries<T>(
  transfer: RemoteTransfer<T>,
  ports: RemotePorts,
): Promise<TransferResult<T>> {
  return fetchBodyWithRetries(transfer, ports, (response) => response.status === PARTIAL_CONTENT);
}

export interface StatusRequest {
  href: string;
  headers: Record<string, string>;
  reportRetry: ReportRetry;
}

export function requestStatusWithRetries(
  request: StatusRequest,
  ports: RemotePorts,
): Promise<TransferResult<number>> {
  return sendWithRetries(
    request.href,
    (init) => ports.request(request.href, { ...init, ...ANONYMOUS, headers: request.headers }),
    async (response) => {
      if (!response.ok) return failureOfStatus(response);
      if (response.body !== null) await catchNetworkFailure(response.body.cancel(), undefined);
      return { kind: TransferEnd.Received, value: response.status };
    },
    request.reportRetry,
    ports,
  );
}
