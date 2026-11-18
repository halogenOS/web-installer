<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import AppIcon from "../ui/AppIcon.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import FailureNotice from "../ui/FailureNotice.vue";
import { Icon } from "../ui/icons.ts";
import ScreenFrame from "../ui/ScreenFrame.vue";
import WaitingNote from "../ui/WaitingNote.vue";
import type { InstallFailure, ListedRelease } from "../wasm/contract.ts";

defineProps<{
  release: ListedRelease;
  connecting: boolean;
  cancelled: boolean;
  usbProblem: InstallFailure | undefined;
}>();
const emit = defineEmits<{ connect: []; back: []; cancel: [] }>();

const NEEDS = [
  { icon: Icon.Unlocked, label: "Bootloader unlocked" },
  { icon: Icon.Power, label: "Phone in fastboot mode" },
  { icon: Icon.Cable, label: "USB cable to this computer" },
] as const;

const RISK_TITLE = "Your phone may not keep working";
const RISK_TEXT =
  "This page writes the images of the release to your phone over the USB cable. " +
  "Parts of your phone may stop working after the install. " +
  "We disclaim liability for harm from the install to your phone or its data, as far as the law allows.";
const RISK_ACCEPTANCE = "I accept the risk";

const riskAccepted = defineModel<boolean>("riskAccepted", { required: true });
</script>

<template>
  <ScreenFrame title="Connect the phone" :release="release.build">
    <div
      class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-[0.9rem] rounded-xl border-[1.5px] border-error bg-error/8
             px-[1.15rem] py-4"
      role="note"
      aria-labelledby="risk-title"
    >
      <span class="grid size-7 place-items-center rounded-[7px] bg-error text-surface [--icon-size:17px]">
        <AppIcon :name="Icon.Alert" />
      </span>
      <div class="grid gap-1 leading-[1.45]">
        <p id="risk-title" class="font-display text-[1.0625rem] font-bold text-error">{{ RISK_TITLE }}</p>
        <p>{{ RISK_TEXT }}</p>
        <label class="mt-2 flex w-fit cursor-pointer items-center gap-3 font-semibold">
          <input
            v-model="riskAccepted"
            :disabled="connecting"
            class="size-5 cursor-pointer accent-error"
            type="checkbox"
          >
          {{ RISK_ACCEPTANCE }}
        </label>
      </div>
    </div>

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
      <WaitingNote v-if="connecting">
        <span>Reading the phone</span>
      </WaitingNote>
      <p v-else-if="cancelled" role="status">Install cancelled. Nothing was written to your phone.</p>
    </template>

    <template #actions>
      <button
        v-if="connecting"
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
          :disabled="!riskAccepted"
          @click="emit('connect')"
        >
          Connect phone
          <AppIcon :name="Icon.ArrowRight" />
        </button>
      </template>
    </template>
  </ScreenFrame>
</template>
