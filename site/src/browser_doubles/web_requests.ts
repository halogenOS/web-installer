// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { TestContext } from "node:test";
import { readRequestedRange } from "../../server/requested_range.ts";
import { contentRangeHeader } from "../remote/byte_range.ts";
import { provideGlobal } from "./globals.ts";

// Status codes of RFC 9110, section 15.
export const HttpStatus = {
  Ok: 200,
  PartialContent: 206,
  NotFound: 404,
  ServiceUnavailable: 503,
} as const;

export interface SentRequest {
  href: string;
  init: RequestInit;
  headers: Headers;
}

// A reply throws a TypeError where fetch fails to reach the host, as fetch does.
export type Reply = (request: SentRequest) => Response;

export function replyInTurn(...replies: Reply[]): Reply {
  let next = 0;
  return (request) => {
    const reply = replies[next];
    next += 1;
    if (reply === undefined) throw new Error(`The test expects ${String(replies.length)} requests, not more.`);
    return reply(request);
  };
}

export function sendRequests(reply: Reply, sent: SentRequest[]): (href: string, init: RequestInit) => Promise<Response> {
  return async (href, init) => {
    const request = { href, init, headers: new Headers(init.headers) };
    sent.push(request);
    return reply(request);
  };
}

export function provideFetch(context: TestContext, reply: Reply): SentRequest[] {
  const sent: SentRequest[] = [];
  provideGlobal(context, globalThis, "fetch", sendRequests(reply, sent));
  return sent;
}

export const lostConnection: Reply = () => {
  throw new TypeError("The connection to the host failed.");
};

export interface RangeAnswer {
  status: number;
  body: Uint8Array<ArrayBuffer>;
  headers: Headers;
}

// The host sends the asked range of bytes as 206, and the whole file as 200 without a range it can send.
export function answerRange(file: Uint8Array<ArrayBuffer>, range: string | null): RangeAnswer {
  const asked = readRequestedRange(range ?? undefined, file.length);
  if (asked === undefined) return { status: HttpStatus.Ok, body: file, headers: new Headers() };
  return {
    status: HttpStatus.PartialContent,
    body: file.slice(asked.offset, asked.offset + asked.length),
    headers: new Headers({ "content-range": contentRangeHeader(asked, file.length) }),
  };
}

export function servedFile(file: Uint8Array<ArrayBuffer>): Reply {
  return (request) => {
    const { status, body, headers } = answerRange(file, request.headers.get("range"));
    return new Response(body, { status, headers });
  };
}

export function bodyBrokenAfter(sentBytes: Uint8Array<ArrayBuffer>, status: number): Reply {
  return () => {
    let sent = false;
    const body = new ReadableStream<Uint8Array<ArrayBuffer>>({
      pull: (controller) => {
        if (sent) controller.error(new TypeError("The connection broke in the middle of the body."));
        else controller.enqueue(sentBytes);
        sent = true;
      },
    });
    return new Response(body, { status });
  };
}

export function statusReply(status: number, headers: Record<string, string> = {}): Reply {
  return () => new Response(null, { status, headers });
}

export interface SentSyncRequest {
  method: string;
  href: string;
  headers: Headers;
}

// A sync reply throws a DOMException named NetworkError where the request fails, as XMLHttpRequest does.
export type SyncReply = (request: SentSyncRequest) => RangeAnswer;

export function provideXmlHttpRequest(context: TestContext, reply: SyncReply): SentSyncRequest[] {
  const sent: SentSyncRequest[] = [];
  class SyncRequestDouble {
    responseType: XMLHttpRequestResponseType = "";
    status = 0;
    response: ArrayBuffer | null = null;
    #request: SentSyncRequest | undefined;
    #headers = new Headers();

    open(method: string, href: string, async: boolean): void {
      if (async) throw new Error("The page sends only synchronous requests through XMLHttpRequest.");
      this.#request = { method, href, headers: new Headers() };
    }

    setRequestHeader(name: string, value: string): void {
      this.#request?.headers.set(name, value);
    }

    send(): void {
      const request = this.#request;
      if (request === undefined) throw new DOMException("The request was not opened.", "InvalidStateError");
      sent.push(request);
      const answer = reply(request);
      this.status = answer.status;
      this.#headers = answer.headers;
      this.response = this.responseType === "arraybuffer" ? answer.body.slice().buffer : null;
    }

    getResponseHeader(name: string): string | null {
      return this.#headers.get(name);
    }
  }
  provideGlobal(context, globalThis, "XMLHttpRequest", SyncRequestDouble);
  return sent;
}
