<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import BlockLoader from "./BlockLoader.vue";
</script>

<template>
  <p class="flex flex-wrap items-center gap-x-[0.85rem] gap-y-2 text-on-surface" role="status">
    <BlockLoader />
    <slot />
  </p>
</template>
