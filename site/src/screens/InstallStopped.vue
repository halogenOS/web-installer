<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { ref } from "vue";
import AppIcon from "../ui/AppIcon.vue";
import FailureNotice from "../ui/FailureNotice.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import { Outcome } from "../ui/outcome.ts";
import OutcomeMark from "../ui/OutcomeMark.vue";
import PhoneMessages from "../ui/PhoneMessages.vue";
import ScreenFrame from "../ui/ScreenFrame.vue";
import type { InstallFailure } from "../wasm/contract.ts";
import { COPY_BUTTON, copyReport, CopyState } from "./copy_report.ts";

const props = defineProps<{
  failure: InstallFailure;
  build: string | undefined;
  phoneMessages: readonly string[];
  title: string;
  againLabel: string;
}>();
const emit = defineEmits<{ again: [] }>();

const copy = ref<CopyState>(CopyState.Ready);

async function copyFailure(): Promise<void> {
  copy.value = await copyReport(props.failure, props.build);
}
</script>

<template>
  <ScreenFrame :title="title" :release="build">
    <template #mark>
      <OutcomeMark :outcome="Outcome.Stopped" />
    </template>

    <FailureNotice :failure="failure" />
    <PhoneMessages :messages="phoneMessages" />

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="copyFailure">
        <AppIcon :name="COPY_BUTTON[copy].icon" />
        {{ COPY_BUTTON[copy].label }}
      </button>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Primary]" @click="emit('again')">
        {{ againLabel }}
      </button>
    </template>
  </ScreenFrame>
</template>
