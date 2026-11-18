<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import type { AwaitedFile } from "../wasm/contract.ts";
import AppIcon from "./AppIcon.vue";
import { formatSize } from "./amounts.ts";
import { BUTTON_CLASS, ButtonLook } from "./button_look.ts";
import { Icon } from "./icons.ts";

defineProps<{ files: AwaitedFile[] }>();

function hostOf(address: string): string {
  return new URL(address).host;
}
</script>

<template>
  <ol class="mt-[0.4rem] grid divide-y divide-border overflow-hidden rounded-[10px] border border-border bg-surface">
    <li
      v-for="{ file, picked } in files"
      :key="file.address"
      class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-[0.9rem] py-[0.55rem] pr-[0.55rem] pl-[0.95rem]
             transition-colors duration-150"
      :class="{ 'bg-accent/10': picked }"
    >
      <div class="grid min-w-0 gap-[0.15rem]">
        <span class="font-mono text-code wrap-anywhere">{{ file.file_name }}</span>
        <span class="text-[0.8125rem] text-on-surface-secondary">
          {{ formatSize(file.size) }} · {{ hostOf(file.address) }}
        </span>
      </div>
      <span
        v-if="picked"
        class="inline-flex min-h-12 items-center gap-[0.4rem] px-[0.85rem] text-sm font-semibold text-accent
               [--icon-size:18px]"
      >
        <AppIcon :name="Icon.Check" />
        Chosen
      </span>
      <a
        v-else
        :class="BUTTON_CLASS[ButtonLook.Quiet]"
        :href="file.address"
        target="_blank"
        rel="noopener noreferrer"
      >
        <AppIcon :name="Icon.Download" />
        Download
      </a>
    </li>
  </ol>
</template>
