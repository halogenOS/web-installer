<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import type {
  Amount,
  FetchedImagesReport,
  ImageReport,
  StockFirmwareReport,
} from "../install/run_reports.ts";
import { formatMegabytes, formatPercent, megabytesOf, shareOf } from "../ui/amounts.ts";
import { BarEdge } from "../ui/bar_edge.ts";
import type { BarRow } from "../ui/bar_row.ts";
import { BarTone } from "../ui/bar_tone.ts";
import BarRowItem from "../ui/BarRowItem.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import ScreenFrame from "../ui/ScreenFrame.vue";
import WaitingNote from "../ui/WaitingNote.vue";
import { BlockEdge, Stage, type BlockDownload, type InstallPlan } from "../wasm/contract.ts";

const props = defineProps<{
  build: string;
  plan: InstallPlan | undefined;
  shipped: FetchedImagesReport | undefined;
  stockFirmware: StockFirmwareReport | undefined;
  images: Partial<Record<string, ImageReport>>;
  failedAttempts: number | undefined;
  note: string | undefined;
}>();
const emit = defineEmits<{ cancel: [] }>();

const WAITING: BarRow = {
  share: 0,
  tone: BarTone.Arriving,
  fillFrom: BarEdge.Start,
  state: "Waiting",
  checked: false,
};
const CHECKED: BarRow = {
  share: 1,
  tone: BarTone.Writing,
  fillFrom: BarEdge.Start,
  state: "Checked",
  checked: true,
};
const FILL_FROM_BLOCK_EDGE: Record<BlockEdge, BarEdge> = {
  [BlockEdge.Start]: BarEdge.Start,
  [BlockEdge.End]: BarEdge.End,
};
const GIT_SHORT_COMMIT_LENGTH = 7;

function running(state: string, tone: BarTone, amount: Amount, format: (amount: Amount) => string): BarRow {
  return { share: shareOf(amount), tone, fillFrom: BarEdge.Start, state, amount: format(amount), checked: false };
}

function blockAmount({ downloaded, block }: BlockDownload): Amount {
  return { done: downloaded, total: block };
}

function blockFill(report: BlockDownload): Pick<BarRow, "share" | "fillFrom"> {
  return { share: shareOf(blockAmount(report)), fillFrom: FILL_FROM_BLOCK_EDGE[report.readFrom] };
}

function shippedRow(): BarRow {
  const { shipped } = props;
  if (shipped === undefined) return WAITING;
  switch (shipped.stage) {
    case Stage.FetchingImages:
      return running("Downloading images", BarTone.Arriving, shipped, formatMegabytes);
    case Stage.FetchedImagesChecked:
      return CHECKED;
  }
}

function builtRow(partition: string): BarRow {
  const report = props.images[partition];
  if (report === undefined) return WAITING;
  switch (report.stage) {
    case Stage.UnpackingStockImage:
      return running("Unpacking stock image", BarTone.Writing, report, formatPercent);
    case Stage.ReadingStockFiles:
      return running("Reading stock files", BarTone.Arriving, report, formatMegabytes);
    case Stage.FetchingFiles:
      return running("Downloading files", BarTone.Arriving, report, formatMegabytes);
    case Stage.Building:
      return running("Building", BarTone.Writing, report, formatPercent);
    case Stage.ImageChecked:
      return CHECKED;
  }
}

function stockFirmwareRow(report: StockFirmwareReport): BarRow {
  switch (report.stage) {
    case Stage.FetchingStockFirmware: {
      const megabytes = megabytesOf(blockAmount(report));
      return {
        ...blockFill(report),
        tone: BarTone.Arriving,
        state: "Downloading",
        amount: `${megabytes.done} MB of up to ${megabytes.total} MB`,
        checked: false,
      };
    }
    case Stage.StockFirmwareChecked:
      return CHECKED;
    case Stage.StockFirmwareBlockChecked:
      return {
        ...CHECKED,
        ...blockFill(report),
        amount: `Needed ${formatMegabytes(blockAmount(report))}`,
      };
  }
}

const shippedImages = computed(shippedRow);
const stockFirmwareImages = computed(() =>
  props.stockFirmware === undefined ? undefined : stockFirmwareRow(props.stockFirmware),
);
const builtImages = computed(() =>
  (props.plan?.built ?? []).map((partition) => ({ partition, row: builtRow(partition) })),
);

function shortCommit(commit: string): string {
  return commit.slice(0, GIT_SHORT_COMMIT_LENGTH);
}
</script>

<template>
  <ScreenFrame title="Building the images" :release="build">
    <WaitingNote v-if="plan === undefined">
      <span>Reading the release</span>
    </WaitingNote>
    <template v-else>
      <section class="grid gap-[0.35rem]" aria-labelledby="images-shipped">
        <h2 id="images-shipped" class="font-display font-bold">Ready-made images</h2>
        <p class="font-mono text-[0.8125rem] text-on-surface-secondary wrap-anywhere">{{ plan.shipped.join(" · ") }}</p>
        <ol class="mt-[0.6rem] grid gap-[0.15rem]">
          <BarRowItem :row="shippedImages" label="Ready-made images" />
        </ol>
      </section>

      <section class="grid gap-[0.35rem]" aria-labelledby="images-built">
        <h2 id="images-built" class="font-display font-bold">Built here</h2>
        <p
          v-for="source in plan.sources"
          :key="source.repository"
          class="text-sm text-on-surface-secondary wrap-anywhere"
        >
          From <span class="font-mono text-code">{{ source.repository }}</span> at
          <span class="font-mono text-code" :title="source.commit">{{ shortCommit(source.commit) }}</span>
        </p>
        <p v-if="plan.stockFiles.length > 0" class="text-sm text-on-surface-secondary wrap-anywhere">
          From the stock firmware
          <span class="font-mono text-code">{{ plan.stockFiles.map((file) => file.file_name).join(" · ") }}</span>
        </p>
        <ol class="mt-[0.6rem] grid gap-[0.15rem]">
          <BarRowItem
            v-if="stockFirmwareImages !== undefined"
            :row="stockFirmwareImages"
            label="Stock firmware"
          >
            Stock firmware
          </BarRowItem>
          <BarRowItem
            v-for="image in builtImages"
            :key="image.partition"
            :row="image.row"
            :label="image.partition"
          >
            <span class="font-mono text-code">{{ image.partition }}</span>
          </BarRowItem>
        </ol>
      </section>
    </template>

    <template #note>
      <p v-if="failedAttempts !== undefined" role="status">
        Retrying a download ({{ failedAttempts }} failed)
      </p>
      <p v-else-if="note !== undefined">{{ note }}</p>
    </template>

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('cancel')">Cancel</button>
    </template>
  </ScreenFrame>
</template>
