<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { ref } from "vue";
import AppIcon from "./AppIcon.vue";
import { applyTheme, THEME_CHOICES, ThemeChoice } from "./theme.ts";

const current = ref<ThemeChoice>(ThemeChoice.System);

function choose(choice: ThemeChoice): void {
  current.value = choice;
  applyTheme(document.documentElement, choice);
}
</script>

<template>
  <div class="flex rounded-xl border border-border bg-surface p-0.5" role="group" aria-label="Theme">
    <button
      v-for="(choice, key) in THEME_CHOICES"
      :key="key"
      type="button"
      class="grid size-11 cursor-pointer place-items-center rounded-[10px] text-on-surface-secondary
             [--icon-size:18px] hover:text-on-surface aria-pressed:bg-accent/15 aria-pressed:text-accent"
      :aria-pressed="current === key"
      :aria-label="choice.label"
      :title="choice.label"
      @click="choose(key)"
    >
      <AppIcon :name="choice.icon" />
    </button>
  </div>
</template>
