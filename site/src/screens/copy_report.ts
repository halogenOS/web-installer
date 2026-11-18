// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { Icon } from "../ui/icons.ts";
import type { InstallFailure } from "../wasm/contract.ts";

export const CopyState = { Ready: "ready", Copied: "copied", Refused: "refused" } as const;
export type CopyState = (typeof CopyState)[keyof typeof CopyState];

export const COPY_BUTTON: Record<CopyState, { label: string; icon: Icon }> = {
  [CopyState.Ready]: { label: "Copy report", icon: Icon.Copy },
  [CopyState.Copied]: { label: "Copied", icon: Icon.Check },
  [CopyState.Refused]: { label: "Copying refused, select the text", icon: Icon.Alert },
};

const NOT_ALLOWED = "NotAllowedError";

export async function copyReport(
  failure: InstallFailure,
  build: string | undefined,
): Promise<CopyState> {
  const lines = [failure.cause, failure.nextStep];
  if (build !== undefined) lines.push(`Release: ${build}`);
  try {
    await navigator.clipboard.writeText(lines.join("\n"));
    return CopyState.Copied;
  } catch (cause) {
    if (!(cause instanceof DOMException && cause.name === NOT_ALLOWED)) throw cause;
    return CopyState.Refused;
  }
}
