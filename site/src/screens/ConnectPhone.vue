<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import AppIcon from "../ui/AppIcon.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import FailureNotice from "../ui/FailureNotice.vue";
import { Icon } from "../ui/icons.ts";
import ScreenFrame from "../ui/ScreenFrame.vue";
import WaitingNote from "../ui/WaitingNote.vue";
import { Stage, type InstallFailure, type ListedRelease } from "../wasm/contract.ts";

defineProps<{
  release: ListedRelease;
  stage: Stage | undefined;
  cancelled: boolean;
  usbProblem: InstallFailure | undefined;
}>();
const emit = defineEmits<{ connect: []; back: []; cancel: [] }>();

const NEEDS = [
  { icon: Icon.Unlocked, label: "Bootloader unlocked" },
  { icon: Icon.Power, label: "Phone in fastboot mode" },
  { icon: Icon.Cable, label: "USB cable to this computer" },
] as const;
</script>

<template>
  <ScreenFrame title="Connect the phone" :release="release.build">
    <ul class="grid grid-cols-[repeat(auto-fit,minmax(10.5rem,1fr))] gap-3">
      <li
        v-for="need in NEEDS"
        :key="need.label"
        class="grid grid-cols-[auto_minmax(0,1fr)] content-start items-center gap-[0.9rem] rounded-xl border
               border-border bg-surface-raised px-4 py-[0.8rem] leading-[1.35] font-medium row:grid-cols-1
               row:items-stretch row:px-[1.15rem] row:pt-[1.1rem] row:pb-[1.2rem]"
      >
        <span class="grid size-10 place-items-center rounded-[10px] bg-accent/12 text-accent [--icon-size:22px]">
          <AppIcon :name="need.icon" />
        </span>
        <span>{{ need.label }}</span>
      </li>
    </ul>

    <FailureNotice v-if="usbProblem !== undefined" :failure="usbProblem" />

    <template #note>
      <WaitingNote v-if="stage !== undefined">
        <span>{{ stage === Stage.Checking ? "Checking the phone" : "Reading the phone" }}</span>
      </WaitingNote>
      <p v-else-if="cancelled" role="status">Install cancelled. Nothing was written to your phone.</p>
    </template>

    <template #actions>
      <button
        v-if="stage !== undefined"
        type="button"
        :class="BUTTON_CLASS[ButtonLook.Plain]"
        @click="emit('cancel')"
      >
        Cancel
      </button>
      <template v-else>
        <button type="button" :class="BUTTON_CLASS[ButtonLook.Quiet]" @click="emit('back')">Back</button>
        <button
          v-if="usbProblem === undefined"
          type="button"
          :class="BUTTON_CLASS[ButtonLook.Primary]"
          @click="emit('connect')"
        >
          Connect phone
          <AppIcon :name="Icon.ArrowRight" />
        </button>
      </template>
    </template>
  </ScreenFrame>
</template>
