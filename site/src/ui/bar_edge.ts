// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export const BarEdge = {
  Start: "start",
  End: "end",
} as const;
export type BarEdge = (typeof BarEdge)[keyof typeof BarEdge];

export const BAR_FILL_EDGE_CLASS: Record<BarEdge, string> = {
  [BarEdge.Start]: "left-0",
  [BarEdge.End]: "right-0",
};
