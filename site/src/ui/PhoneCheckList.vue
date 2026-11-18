<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import { CheckVerdict, PhoneCheckKind, type PhoneCheck, type PhoneReading } from "../wasm/contract.ts";
import { formatSize } from "./amounts.ts";
import AppIcon from "./AppIcon.vue";
import { Icon } from "./icons.ts";
import { Outcome, OUTCOME_MARK } from "./outcome.ts";

const props = defineProps<{ checks: readonly PhoneCheck[] }>();

const CHECK_LABEL: Record<PhoneCheckKind, string> = {
  [PhoneCheckKind.Model]: "Phone model",
  [PhoneCheckKind.Bootloader]: "Bootloader unlocked",
  [PhoneCheckKind.Battery]: "Battery for the install",
  [PhoneCheckKind.Slot]: "Active slot",
  [PhoneCheckKind.Partitions]: "Partition space",
  [PhoneCheckKind.TransferSize]: "Transfer size",
};

const NOT_REPORTED = "not reported";
const CHECK_LIST_NAME = "Phone checks";

const VERDICT_LOOK: Record<CheckVerdict, { icon: Icon; colours: string; row: string; text: string }> = {
  [CheckVerdict.Passed]: {
    ...OUTCOME_MARK[Outcome.Finished],
    row: "border-border",
    text: "passed",
  },
  [CheckVerdict.Failed]: {
    ...OUTCOME_MARK[Outcome.Stopped],
    row: "border-error",
    text: "failed, stopped the install",
  },
};

function shownValue(reading: PhoneReading): string {
  switch (reading.check) {
    case PhoneCheckKind.Model:
      return reading.product ?? NOT_REPORTED;
    case PhoneCheckKind.Bootloader:
      return reading.unlocked ?? NOT_REPORTED;
    case PhoneCheckKind.Battery:
      return reading.socOk ?? NOT_REPORTED;
    case PhoneCheckKind.Slot:
      return reading.currentSlot ?? NOT_REPORTED;
    case PhoneCheckKind.Partitions:
      return reading.fit === undefined ? NOT_REPORTED : `${reading.fit.fitting} of ${reading.fit.count} fit`;
    case PhoneCheckKind.TransferSize:
      return reading.downloadLimit === undefined ? NOT_REPORTED : formatSize(reading.downloadLimit);
  }
}

const rows = computed(() =>
  props.checks.map(({ verdict, reading }) => ({ kind: reading.check, verdict, value: shownValue(reading) })),
);
</script>

<template>
  <ol class="grid gap-1.5" :aria-label="CHECK_LIST_NAME">
    <li
      v-for="row in rows"
      :key="row.kind"
      :class="VERDICT_LOOK[row.verdict].row"
      class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3.5 rounded-[10px] border bg-surface-raised px-4
             py-[0.7rem] font-medium motion-safe:animate-arrive"
    >
      <span
        :class="VERDICT_LOOK[row.verdict].colours"
        class="grid size-6 place-items-center rounded-md [--icon-size:15px] [--icon-stroke:2.5]"
      >
        <AppIcon :name="VERDICT_LOOK[row.verdict].icon" />
      </span>
      <span>
        {{ CHECK_LABEL[row.kind] }}
        <span class="sr-only">{{ VERDICT_LOOK[row.verdict].text }}</span>
      </span>
      <span class="font-mono text-code text-on-surface-secondary select-text wrap-anywhere">{{ row.value }}</span>
    </li>
  </ol>
</template>
