<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import { OUTCOME_MARK, type Outcome } from "./outcome.ts";

defineProps<{ outcome: Outcome }>();
</script>

<template>
  <span
    :class="OUTCOME_MARK[outcome].colours"
    class="mb-[0.6rem] grid size-14 place-items-center rounded-[14px] [--icon-size:30px] [--icon-stroke:2.25]
           motion-safe:animate-arrive motion-safe:[animation-delay:120ms]"
  >
    <AppIcon :name="OUTCOME_MARK[outcome].icon" />
  </span>
</template>
