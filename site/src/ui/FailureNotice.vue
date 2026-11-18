<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import type { InstallFailure } from "../wasm/contract.ts";
import AppIcon from "./AppIcon.vue";
import { Icon } from "./icons.ts";

defineProps<{ failure: InstallFailure }>();
</script>

<template>
  <div class="grid gap-4" role="alert">
    <p class="select-text wrap-anywhere">{{ failure.cause }}</p>
    <p
      class="flex items-start gap-3 rounded-xl border border-border bg-surface-raised px-[1.1rem] py-[0.9rem]
             font-medium"
    >
      <AppIcon class="mt-0.5 text-accent" :name="Icon.ArrowRight" />
      <span>{{ failure.nextStep }}</span>
    </p>
  </div>
</template>
