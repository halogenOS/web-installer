// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { pipeline } from "node:stream/promises";
import { createError, defineEventHandler, getRouterParam, type H3Event } from "h3";
import { ReleaseFileParameter } from "../src/remote/release_paths.ts";
import { releaseSource } from "./command_line.ts";
import { FileOpening, type SourceFile } from "./release_source.ts";

const PREMATURE_CLOSE = "ERR_STREAM_PREMATURE_CLOSE";

const Status = {
  Ok: 200,
  NotFound: 404,
  BadGateway: 502,
} as const;

function isPrematureClose(cause: unknown): boolean {
  return cause instanceof Error && "code" in cause && cause.code === PREMATURE_CLOSE;
}

async function findListedFile(event: H3Event): Promise<SourceFile | undefined> {
  const release = getRouterParam(event, ReleaseFileParameter.Release);
  const file = getRouterParam(event, ReleaseFileParameter.File, { decode: true });
  const listed = (await releaseSource.listReleases()).find((candidate) => String(candidate.id) === release);
  return listed?.files.find((candidate) => candidate.name === file);
}

export default defineEventHandler(async (event) => {
  const file = await findListedFile(event);
  if (file === undefined) throw createError({ statusCode: Status.NotFound });
  const opened = await file.open();
  switch (opened.kind) {
    case FileOpening.HostRefused:
      console.error(`The host of ${event.path} answered with status ${String(opened.status)}.`);
      throw createError({ statusCode: Status.BadGateway });
    case FileOpening.HostUnreachable:
      console.error(`The host of ${event.path} could not be reached.`);
      throw createError({ statusCode: Status.BadGateway });
    case FileOpening.Opened:
      break;
  }
  event.node.res.writeHead(Status.Ok, {
    "Content-Type": "application/octet-stream",
    "Content-Length": String(file.size),
  });
  try {
    await pipeline(opened.body, event.node.res);
  } catch (cause) {
    if (!isPrematureClose(cause)) throw cause;
  }
});
