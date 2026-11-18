// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { Icon } from "../ui/icons.ts";
import { StepKind, type Step } from "../wasm/contract.ts";

export function describeStep(step: Step): string {
  switch (step.step) {
    case StepKind.Flash:
      return `Writing ${step.partition}`;
    case StepKind.RebootFastboot:
      return "Restarting into fastbootd";
    case StepKind.UpdateSuper:
      return "Writing the partition layout";
    case StepKind.Erase:
      return `Erasing ${step.partition}`;
  }
}

export function nameStep(step: Step): { name: string; icon: Icon | undefined } {
  switch (step.step) {
    case StepKind.Flash:
      return { name: step.partition, icon: undefined };
    case StepKind.RebootFastboot:
      return { name: "fastbootd", icon: Icon.Restart };
    case StepKind.UpdateSuper:
      return { name: step.super_device, icon: Icon.Layout };
    case StepKind.Erase:
      return { name: step.partition, icon: Icon.Erase };
  }
}
