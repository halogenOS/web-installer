<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
const BLOCKS = [0, 1, 2] as const;
</script>

<template>
  <span class="inline-flex gap-[3px]" aria-hidden="true">
    <i
      v-for="block in BLOCKS"
      :key="block"
      class="size-[7px] rounded-[2px] bg-accent motion-safe:animate-block-pulse"
      :style="{ '--block': block }"
    />
  </span>
</template>
