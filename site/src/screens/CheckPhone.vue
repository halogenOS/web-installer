<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import PhoneCheckList from "../ui/PhoneCheckList.vue";
import ScreenFrame from "../ui/ScreenFrame.vue";
import WaitingNote from "../ui/WaitingNote.vue";
import type { PhoneCheck } from "../wasm/contract.ts";

defineProps<{ build: string; checks: readonly PhoneCheck[] }>();
const emit = defineEmits<{ cancel: [] }>();

const NOTHING_WRITTEN_YET = "No images written to your phone";
</script>

<template>
  <ScreenFrame title="Checking the phone" :release="build">
    <PhoneCheckList :checks="checks" />

    <template #note>
      <WaitingNote>
        <span>{{ NOTHING_WRITTEN_YET }}</span>
      </WaitingNote>
    </template>

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('cancel')">Cancel</button>
    </template>
  </ScreenFrame>
</template>
