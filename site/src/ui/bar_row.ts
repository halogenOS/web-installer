// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import type { BarEdge } from "./bar_edge.ts";
import type { BarTone } from "./bar_tone.ts";

export interface BarRow {
  share: number;
  tone: BarTone;
  fillFrom: BarEdge;
  state: string;
  amount?: string;
  checked: boolean;
}
