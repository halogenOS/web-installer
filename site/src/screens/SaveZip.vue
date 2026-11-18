<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import type { Amount } from "../install/run_reports.ts";
import { formatMegabytes, shareOf } from "../ui/amounts.ts";
import { BarEdge } from "../ui/bar_edge.ts";
import type { BarRow } from "../ui/bar_row.ts";
import { BarTone } from "../ui/bar_tone.ts";
import BarRowItem from "../ui/BarRowItem.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import ScreenFrame from "../ui/ScreenFrame.vue";

const props = defineProps<{
  build: string;
  fileName: string | undefined;
  packageWritten: Amount | undefined;
}>();
const emit = defineEmits<{ cancel: [] }>();

const LABEL = "Zip written";

const row = computed((): BarRow => {
  const amount = props.packageWritten;
  if (amount === undefined) {
    return { share: 0, tone: BarTone.Writing, fillFrom: BarEdge.Start, state: "Waiting", checked: false };
  }
  return {
    share: shareOf(amount),
    tone: BarTone.Writing,
    fillFrom: BarEdge.Start,
    state: formatMegabytes(amount),
    checked: amount.done === amount.total,
  };
});
</script>

<template>
  <ScreenFrame title="Writing the zip" :release="build">
    <section class="grid gap-[0.35rem]" aria-labelledby="zip-file">
      <h2 id="zip-file" class="font-display font-bold">Saving to</h2>
      <p v-if="fileName !== undefined" class="font-mono text-[0.8125rem] text-on-surface-secondary wrap-anywhere">
        {{ fileName }}
      </p>
      <ol class="mt-[0.6rem] grid gap-[0.15rem]">
        <BarRowItem :row="row" :label="LABEL">{{ LABEL }}</BarRowItem>
      </ol>
    </section>

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('cancel')">Cancel</button>
    </template>
  </ScreenFrame>
</template>
