<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import { Outcome } from "../ui/outcome.ts";
import OutcomeMark from "../ui/OutcomeMark.vue";
import PhoneMessages from "../ui/PhoneMessages.vue";
import ScreenFrame from "../ui/ScreenFrame.vue";

defineProps<{ build: string; phoneMessages: readonly string[] }>();
const emit = defineEmits<{ again: [] }>();

const OTHER_SYSTEM_NOTE =
  "If the phone ran another system before, restart it into the recovery and format the data before you use it.";
</script>

<template>
  <ScreenFrame title="Installed" :release="build">
    <template #mark>
      <OutcomeMark :outcome="Outcome.Finished" />
    </template>

    <p class="text-[1.0625rem]">The phone restarts into the new system.</p>
    <p class="text-[1.0625rem]">{{ OTHER_SYSTEM_NOTE }}</p>
    <PhoneMessages :messages="phoneMessages" />

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('again')">
        Install on another phone
      </button>
    </template>
  </ScreenFrame>
</template>
