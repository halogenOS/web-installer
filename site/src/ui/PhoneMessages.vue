<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
defineProps<{ messages: readonly string[] }>();
</script>

<template>
  <details v-if="messages.length > 0" class="mt-7 text-[13px] text-on-surface-secondary">
    <summary class="flex min-h-11 w-fit cursor-pointer items-center">
      Messages from the phone ({{ messages.length }})
    </summary>
    <pre
      class="mt-1 max-h-48 overflow-auto rounded-lg border border-border bg-surface-raised px-4 py-3
             font-mono text-[12.5px]/[1.6] whitespace-pre-wrap text-on-surface"
    >{{ messages.join("\n") }}</pre>
  </details>
</template>
