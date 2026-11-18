<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { InstallStep, STEP_LABEL } from "../install/install_steps.ts";
import AppIcon from "./AppIcon.vue";
import { Icon } from "./icons.ts";
import { STEP_STATE_TEXT, StepState, stepStateAt } from "./step_state.ts";

const props = defineProps<{ steps: readonly InstallStep[]; current: InstallStep; stopped: boolean }>();

const STEP_CLASS: Record<StepState, { step: string; mark: string }> = {
  [StepState.Ahead]: { step: "font-medium text-on-bg-secondary", mark: "border-border bg-bg" },
  [StepState.Done]: { step: "font-medium text-on-bg", mark: "border-accent/35 bg-accent/12 text-accent" },
  [StepState.Current]: { step: "font-semibold text-on-bg", mark: "border-accent bg-accent text-on-accent" },
  [StepState.Stopped]: { step: "font-semibold text-error", mark: "border-error bg-error text-surface" },
};

function numberOf(step: InstallStep): number {
  return props.steps.indexOf(step) + 1;
}

function stateOf(step: InstallStep): StepState {
  if (step === InstallStep.Done && props.current === InstallStep.Done) return StepState.Done;
  return stepStateAt(numberOf(step), numberOf(props.current), props.stopped);
}
</script>

<template>
  <nav aria-label="Steps of the install">
    <ol
      class="relative grid grid-flow-col justify-start gap-[0.6rem] rail:grid-flow-row rail:gap-[1.65rem]
             rail:before:absolute rail:before:inset-y-[14px] rail:before:left-[13px] rail:before:w-0.5
             rail:before:bg-border"
    >
      <li
        v-for="step in steps"
        :key="step"
        :class="STEP_CLASS[stateOf(step)].step"
        class="relative flex items-center gap-[0.55rem] rail:gap-[0.9rem]"
        :aria-current="step === current ? 'step' : undefined"
      >
        <span
          :class="STEP_CLASS[stateOf(step)].mark"
          class="grid size-7 place-items-center rounded-cell border-[1.5px] font-display text-[13px] leading-none
                 font-bold [--icon-size:16px] [--icon-stroke:2.25]"
        >
          <AppIcon v-if="stateOf(step) === StepState.Done" :name="Icon.Check" />
          <AppIcon v-else-if="stateOf(step) === StepState.Stopped" :name="Icon.Alert" />
          <template v-else>{{ numberOf(step) }}</template>
        </span>
        <span :class="{ 'max-rail:sr-only': step !== current }">{{ STEP_LABEL[step] }}</span>
        <span class="sr-only">, {{ STEP_STATE_TEXT[stateOf(step)] }}</span>
      </li>
    </ol>
  </nav>
</template>
