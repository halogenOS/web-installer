<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import type { Amount, StepReport } from "../install/run_reports.ts";
import { describeStep, nameStep } from "../install/step_text.ts";
import AppIcon from "../ui/AppIcon.vue";
import { formatMegabytes, shareOf } from "../ui/amounts.ts";
import { BarEdge } from "../ui/bar_edge.ts";
import { BarTone } from "../ui/bar_tone.ts";
import BlockBar from "../ui/BlockBar.vue";
import BlockLoader from "../ui/BlockLoader.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import { Icon } from "../ui/icons.ts";
import PhoneMessages from "../ui/PhoneMessages.vue";
import ScreenFrame from "../ui/ScreenFrame.vue";
import { STEP_STATE_TEXT, StepState, stepStateAt } from "../ui/step_state.ts";
import { StepKind, type InstallPlan } from "../wasm/contract.ts";

const props = defineProps<{
  plan: InstallPlan;
  current: StepReport;
  sent: Amount | undefined;
  phoneMessages: readonly string[];
}>();
const emit = defineEmits<{ chooseAgain: [] }>();

const tiles = computed(() =>
  props.plan.install.map((step, index) => {
    const state = stepStateAt(index, props.current.index, false);
    return { index, state, description: describeStep(step), ...nameStep(step) };
  }),
);

const TILE_CLASS: Record<StepState, string> = {
  [StepState.Ahead]: "bg-surface-raised text-on-surface-secondary ring-1 ring-border",
  [StepState.Done]: "bg-accent/12 text-accent ring-1 ring-accent/25 [--icon-stroke:2.25]",
  [StepState.Current]: `bg-surface font-semibold text-on-surface ring-[1.5px] ring-accent before:absolute
    before:inset-y-0 before:left-0 before:-z-10 before:w-[calc(var(--share)*100%)] before:bg-accent/25
    motion-safe:before:transition-[width] motion-safe:before:duration-240 motion-safe:before:ease-linear`,
  [StepState.Stopped]: "bg-error/12 text-error ring-1 ring-error/40",
};

const sentShare = computed(() => (props.sent === undefined ? 0 : shareOf(props.sent)));
const restarts = computed(() => props.current.step.step === StepKind.RebootFastboot);
</script>

<template>
  <ScreenFrame title="Writing to the phone" :release="plan.build">
    <div
      class="grid grid-cols-[minmax(0,1fr)_auto] items-baseline gap-x-4 gap-y-[0.8rem] rounded-[14px] border
             border-border bg-surface-raised px-[1.35rem] pt-5 pb-[1.35rem]"
    >
      <p class="font-display text-[1.1875rem] font-bold wrap-anywhere">{{ describeStep(current.step) }}</p>
      <p class="text-sm text-on-surface-secondary">Step {{ current.index + 1 }} of {{ current.count }}</p>
      <template v-if="sent !== undefined">
        <BlockBar
          class="col-span-full [--bar-height:16px]"
          :share="sentShare"
          :tone="BarTone.Writing"
          :fill-from="BarEdge.Start"
          :rest-skipped="false"
          :label="describeStep(current.step)"
          :value-text="formatMegabytes(sent)"
        />
        <p class="col-span-full -mt-[0.3rem] justify-self-end text-sm text-on-surface-secondary">
          {{ formatMegabytes(sent) }}
        </p>
      </template>
      <BlockLoader v-else class="col-span-full" />
    </div>

    <div v-if="restarts" class="grid justify-items-start gap-4 text-pretty">
      <p>
        The phone restarts into fastbootd. If the install does not go on once the phone shows
        fastbootd, choose the phone again.
      </p>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('chooseAgain')">
        <AppIcon :name="Icon.Restart" />
        Choose the phone again
      </button>
    </div>

    <ol class="flex flex-wrap gap-[0.4rem]" aria-label="Install steps">
      <li
        v-for="tile in tiles"
        :key="tile.index"
        :class="TILE_CLASS[tile.state]"
        class="relative isolate flex min-h-[2.6rem] flex-[1_0_auto] items-center gap-[0.4rem] overflow-hidden
               rounded-lg px-[0.85rem] font-mono text-[13px]/[1.2] whitespace-nowrap ring-inset [--icon-size:15px]"
        :aria-current="tile.state === StepState.Current ? 'step' : undefined"
        :title="tile.description"
        :style="tile.state === StepState.Current ? { '--share': sentShare } : undefined"
      >
        <AppIcon v-if="tile.state === StepState.Done" :name="Icon.Check" />
        <AppIcon v-else-if="tile.icon !== undefined" :name="tile.icon" />
        <span>{{ tile.name }}</span>
        <span class="sr-only">: {{ tile.description }}, {{ STEP_STATE_TEXT[tile.state] }}</span>
      </li>
    </ol>

    <PhoneMessages :messages="phoneMessages" />

    <template #note>
      <AppIcon :name="Icon.Cable" />
      <p>Keep the phone plugged in and this page open.</p>
    </template>
  </ScreenFrame>
</template>
