// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export const BarTone = {
  Arriving: "arriving",
  Writing: "writing",
} as const;
export type BarTone = (typeof BarTone)[keyof typeof BarTone];

export const BAR_FILL_CLASS: Record<BarTone, string> = {
  [BarTone.Arriving]: "bg-on-surface-secondary",
  [BarTone.Writing]: "bg-accent",
};
