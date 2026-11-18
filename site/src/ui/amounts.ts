// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { Amount } from "../install/run_reports.ts";

const BYTES_PER_MEGABYTE = 1_000_000;

const WHOLE_MEGABYTES_FROM = 10;

const LANGUAGE = "en";

const PERCENT = new Intl.NumberFormat(LANGUAGE, { style: "percent", maximumFractionDigits: 0 });

export function shareOf({ done, total }: Amount): number {
  return total === 0 ? 1 : Math.min(done / total, 1);
}

export function formatPercent(amount: Amount): string {
  return PERCENT.format(shareOf(amount));
}

export interface MegabyteTexts {
  done: string;
  total: string;
}

/** Both values of the amount in megabytes, rounded to the precision that the total sets. */
export function megabytesOf({ done, total }: Amount): MegabyteTexts {
  const totalMegabytes = total / BYTES_PER_MEGABYTE;
  const format = new Intl.NumberFormat(LANGUAGE, {
    maximumFractionDigits: totalMegabytes < WHOLE_MEGABYTES_FROM ? 1 : 0,
  });
  return { done: format.format(done / BYTES_PER_MEGABYTE), total: format.format(totalMegabytes) };
}

export function formatMegabytes(amount: Amount): string {
  const { done, total } = megabytesOf(amount);
  return `${done} of ${total} MB`;
}

export function formatSize(bytes: number): string {
  return `${new Intl.NumberFormat(LANGUAGE).format(Math.ceil(bytes / BYTES_PER_MEGABYTE))} MB`;
}
