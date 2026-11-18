// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export const StepState = {
  Done: "done",
  Current: "current",
  Stopped: "stopped",
  Ahead: "ahead",
} as const;
export type StepState = (typeof StepState)[keyof typeof StepState];

export const STEP_STATE_TEXT: Record<StepState, string> = {
  [StepState.Done]: "done",
  [StepState.Current]: "current step",
  [StepState.Stopped]: "stopped here",
  [StepState.Ahead]: "ahead",
};

export function stepStateAt(position: number, current: number, stopped: boolean): StepState {
  if (position < current) return StepState.Done;
  if (position > current) return StepState.Ahead;
  return stopped ? StepState.Stopped : StepState.Current;
}
