<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import FileDownloadLink from "../ui/FileDownloadLink.vue";
import { Outcome } from "../ui/outcome.ts";
import OutcomeMark from "../ui/OutcomeMark.vue";
import ScreenFrame from "../ui/ScreenFrame.vue";
import { StepKind, type InstallPlan } from "../wasm/contract.ts";

const props = defineProps<{
  plan: InstallPlan;
  fileName: string | undefined;
  storedZip: File | undefined;
}>();
const emit = defineEmits<{ again: [] }>();

const erasesForOtherSystems = computed(() =>
  props.plan.install.some((step) => step.step === StepKind.Erase && step.if_wipe),
);
</script>

<template>
  <ScreenFrame :title="storedZip === undefined ? 'Zip saved' : 'Zip ready'" :release="plan.build">
    <template #mark>
      <OutcomeMark :outcome="Outcome.Finished" />
    </template>

    <p v-if="fileName !== undefined" class="font-mono text-code">{{ fileName }}</p>
    <p class="text-[1.0625rem]">
      Restart your phone into the recovery of this release, then send the zip with
      <span class="font-mono text-code">adb sideload</span>.
    </p>
    <p v-if="erasesForOtherSystems" class="text-[1.0625rem]">
      If your phone runs another system, format the data in the recovery before you send the zip.
    </p>

    <template #actions>
      <button type="button" :class="BUTTON_CLASS[ButtonLook.Plain]" @click="emit('again')">Back to the releases</button>
      <FileDownloadLink v-if="storedZip !== undefined" :file="storedZip">Save the zip</FileDownloadLink>
    </template>
  </ScreenFrame>
</template>
