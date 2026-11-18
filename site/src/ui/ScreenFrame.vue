<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
defineProps<{ title: string; release?: string | undefined }>();
</script>

<template>
  <section class="grid gap-8 motion-safe:animate-arrive" aria-labelledby="screen-title">
    <header class="grid gap-[0.6rem]">
      <slot name="mark" />
      <h1
        id="screen-title"
        class="font-display text-[clamp(1.9rem,1.5rem+1.2vw,2.45rem)] leading-[1.1] font-semibold tracking-tight
               text-balance focus:outline-none"
        tabindex="-1"
      >
        {{ title }}
      </h1>
      <p v-if="release" class="font-mono text-code text-on-surface-secondary wrap-anywhere">{{ release }}</p>
    </header>
    <div class="grid gap-7">
      <slot />
    </div>
    <footer
      v-if="$slots.note || $slots.actions"
      class="flex flex-wrap items-center justify-between gap-x-6 gap-y-4 border-t border-border pt-6"
    >
      <div
        class="flex min-w-0 flex-[1_1_16rem] items-center gap-[0.6rem] text-sm text-on-surface-secondary
               [--icon-size:18px]"
      >
        <slot name="note" />
      </div>
      <div class="ml-auto flex flex-wrap gap-3">
        <slot name="actions" />
      </div>
    </footer>
  </section>
</template>
