// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { Icon } from "./icons.ts";

export const Outcome = { Finished: "finished", Stopped: "stopped" } as const;
export type Outcome = (typeof Outcome)[keyof typeof Outcome];

export const OUTCOME_MARK: Record<Outcome, { icon: Icon; colours: string }> = {
  [Outcome.Finished]: { icon: Icon.Check, colours: "bg-accent text-on-accent" },
  [Outcome.Stopped]: { icon: Icon.Alert, colours: "bg-error/12 text-error" },
};
