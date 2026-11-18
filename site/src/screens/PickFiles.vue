<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import type { Amount } from "../install/run_reports.ts";
import { AskedFiles, type AwaitedFile, type InstallFailure } from "../wasm/contract.ts";
import AppIcon from "../ui/AppIcon.vue";
import { formatMegabytes, shareOf } from "../ui/amounts.ts";
import { BarEdge } from "../ui/bar_edge.ts";
import type { BarRow } from "../ui/bar_row.ts";
import { BarTone } from "../ui/bar_tone.ts";
import BarRowItem from "../ui/BarRowItem.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import { Icon } from "../ui/icons.ts";
import ScreenFrame from "../ui/ScreenFrame.vue";
import DownloadLinkList from "../ui/DownloadLinkList.vue";

const props = defineProps<{
  build: string;
  asked: AskedFiles;
  files: AwaitedFile[];
  wrongPicks: InstallFailure[];
  checked: Amount | undefined;
}>();
const emit = defineEmits<{ pick: [files: File[]]; cancel: [] }>();

const TITLE: Record<AskedFiles, string> = {
  [AskedFiles.StockFirmware]: "Get the stock firmware",
  [AskedFiles.ReleaseFiles]: "Get the release files",
};

const pickedCount = computed(() => props.files.filter((awaited) => awaited.picked).length);
const endings = computed(() => {
  const names = props.files.map(({ file }) => file.file_name);
  return [...new Set(names.map((name) => name.slice(name.lastIndexOf("."))))].join(",");
});
const checkRow = computed((): BarRow | undefined =>
  props.checked === undefined
    ? undefined
    : {
        share: shareOf(props.checked),
        tone: BarTone.Arriving,
        fillFrom: BarEdge.Start,
        state: "Checking the files",
        amount: formatMegabytes(props.checked),
        checked: false,
      },
);
const wrongPickSteps = computed(() => [...new Set(props.wrongPicks.map((refusal) => refusal.nextStep))]);

function pick(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement)) return;
  const files = Array.from(input.files ?? []);
  input.value = "";
  if (files.length === 0) return;
  emit("pick", files);
}
</script>

<template>
  <ScreenFrame :title="TITLE[asked]" :release="build">
    <ol class="grid gap-3">
      <li
        class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-[1.1rem] rounded-xl border border-border
               bg-surface-raised px-[1.15rem] py-[1.1rem]"
      >
        <span
          class="grid size-7 place-items-center rounded-cell border border-border bg-surface font-display font-bold
                 text-on-surface-secondary"
          aria-hidden="true"
        >
          1
        </span>
        <div class="grid min-w-0 gap-[0.3rem]">
          <h2 class="font-display font-bold">
            {{ files.length === 1 ? "Download the file" : `Download the ${files.length} files` }}
          </h2>
          <DownloadLinkList :files="files" />
        </div>
      </li>
      <li
        class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-[1.1rem] rounded-xl border border-border
               bg-surface-raised px-[1.15rem] py-[1.1rem] row:grid-cols-[auto_minmax(0,1fr)_auto]"
      >
        <span
          class="grid size-7 place-items-center rounded-cell bg-accent font-display font-bold text-on-accent"
          aria-hidden="true"
        >
          2
        </span>
        <div class="grid min-w-0 gap-[0.3rem]">
          <h2 class="font-display font-bold">
            {{ files.length === 1 ? "Choose the downloaded file" : "Choose the downloaded files" }}
          </h2>
          <ol v-if="checkRow !== undefined" class="mt-[0.4rem] grid">
            <BarRowItem :row="checkRow" label="Chosen files" />
          </ol>
          <p v-else-if="pickedCount > 0" class="text-sm text-on-surface-secondary">
            {{ pickedCount }} of {{ files.length }} chosen. Choose the other {{ files.length - pickedCount }}.
          </p>
          <p v-else-if="files.length > 1" class="text-sm text-on-surface-secondary">
            Select all {{ files.length }} files at once in the file dialog.
          </p>
          <div v-if="wrongPicks.length > 0" class="text-sm text-error" role="alert">
            <p v-for="(refusal, index) in wrongPicks" :key="index">{{ refusal.cause }}</p>
            <p v-for="step in wrongPickSteps" :key="step">{{ step }}</p>
          </div>
        </div>
        <label :class="BUTTON_CLASS[ButtonLook.Primary]" class="col-span-full row:col-auto">
          <input
            class="sr-only"
            type="file"
            :accept="endings"
            :multiple="files.length > 1"
            :disabled="checked !== undefined"
            @change="pick"
          >
          {{ files.length === 1 ? "Choose a file" : "Choose files" }}
          <AppIcon :name="Icon.ArrowRight" />
        </label>
      </li>
    </ol>

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('cancel')">Cancel</button>
    </template>
  </ScreenFrame>
</template>
