// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { InstallFailure } from "../wasm/contract.ts";

export function isInstallFailure(value: unknown): value is InstallFailure {
  return (
    typeof value === "object" &&
    value !== null &&
    "cause" in value &&
    typeof value.cause === "string" &&
    "nextStep" in value &&
    typeof value.nextStep === "string"
  );
}

export function pageStopped(report: string): InstallFailure {
  return {
    cause: `The page stopped. The browser reports: ${report}`,
    nextStep:
      "Reload the page, then run the install again. If this happens again, report this text to us.",
  };
}
