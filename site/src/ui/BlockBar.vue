<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { BAR_FILL_EDGE_CLASS, type BarEdge } from "./bar_edge.ts";
import { BAR_FILL_CLASS, type BarTone } from "./bar_tone.ts";

defineProps<{
  share: number;
  tone: BarTone;
  fillFrom: BarEdge;
  restSkipped: boolean;
  label: string;
  valueText: string;
}>();
</script>

<template>
  <div
    :class="restSkipped ? 'bg-skipped' : 'bg-border'"
    class="relative h-[var(--bar-height,12px)]
           mask-[repeating-linear-gradient(90deg,#000_0_5px,transparent_5px_7px)]"
    role="progressbar"
    :aria-label="label"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-valuenow="Math.round(share * 100)"
    :aria-valuetext="valueText"
    :style="{ '--share': share }"
  >
    <div
      :class="[BAR_FILL_CLASS[tone], BAR_FILL_EDGE_CLASS[fillFrom]]"
      class="absolute inset-y-0 w-[calc(var(--share)*100%)]
             motion-safe:transition-[width] motion-safe:duration-240 motion-safe:ease-linear"
    />
  </div>
</template>
