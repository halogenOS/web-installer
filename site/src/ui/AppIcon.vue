<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { ICON_STROKES, type Icon } from "./icons.ts";

defineProps<{ name: Icon }>();
</script>

<template>
  <svg
    class="size-[var(--icon-size,20px)] flex-none fill-none stroke-current [stroke-linecap:round]
           [stroke-linejoin:round] [stroke-width:var(--icon-stroke,1.75)]"
    viewBox="0 0 24 24"
    aria-hidden="true"
    focusable="false"
  >
    <path v-for="stroke in ICON_STROKES[name]" :key="stroke" :d="stroke" />
  </svg>
</template>
