<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import AppIcon from "./AppIcon.vue";
import type { BarRow } from "./bar_row.ts";
import BlockBar from "./BlockBar.vue";
import { Icon } from "./icons.ts";

const props = defineProps<{ row: BarRow; label: string }>();

const valueText = computed(() =>
  props.row.amount === undefined ? props.row.state : `${props.row.state}, ${props.row.amount}`,
);
</script>

<template>
  <li
    :class="$slots.default
      ? 'row:grid-cols-[7.5rem_minmax(0,1fr)_10.5rem]'
      : 'row:grid-cols-[minmax(0,1fr)_10.5rem]'"
    class="grid min-h-9 grid-cols-[minmax(0,1fr)_auto] items-center gap-x-[1.1rem] gap-y-[0.4rem] row:gap-y-0"
  >
    <span v-if="$slots.default" class="overflow-hidden text-sm text-ellipsis"><slot /></span>
    <BlockBar
      class="col-span-full row-start-2 row:col-auto row:row-auto"
      :share="row.share"
      :tone="row.tone"
      :fill-from="row.fillFrom"
      :rest-skipped="row.checked"
      :label="label"
      :value-text="valueText"
    />
    <span
      :class="row.checked ? 'font-semibold text-accent' : 'text-on-surface-secondary'"
      class="grid justify-items-end text-sm/tight whitespace-nowrap [--icon-size:17px] [--icon-stroke:2.25]"
    >
      <span
        :class="{ 'text-on-surface': row.amount !== undefined }"
        class="flex items-center gap-[0.35rem]"
      >
        <AppIcon v-if="row.checked" :name="Icon.Check" />
        {{ row.state }}
      </span>
      <span v-if="row.amount !== undefined" class="text-[0.8125rem] tabular-nums">{{ row.amount }}</span>
    </span>
  </li>
</template>
