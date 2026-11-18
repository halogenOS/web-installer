<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import logoUrl from "../../website/public/logo.png";
import { Phase, useInstallFlow } from "./install/install_flow.ts";
import { InstallStep } from "./install/install_steps.ts";
import { phoneChecksOfStop } from "./install/run_reports.ts";
import { describeStep } from "./install/step_text.ts";
import BuildImages from "./screens/BuildImages.vue";
import CheckPhone from "./screens/CheckPhone.vue";
import ChooseRelease from "./screens/ChooseRelease.vue";
import ConnectPhone from "./screens/ConnectPhone.vue";
import InstallDone from "./screens/InstallDone.vue";
import InstallStopped from "./screens/InstallStopped.vue";
import PickFiles from "./screens/PickFiles.vue";
import SaveZip from "./screens/SaveZip.vue";
import WriteImages from "./screens/WriteImages.vue";
import ZipSaved from "./screens/ZipSaved.vue";
import { formatPercent } from "./ui/amounts.ts";
import PageBackdrop from "./ui/PageBackdrop.vue";
import StepRail from "./ui/StepRail.vue";
import PrivacyPolicy from "./ui/PrivacyPolicy.vue";
import { PRIVACY_POLICY_ADDRESS } from "./ui/privacy_policy.ts";
import ThemeSwitch from "./ui/ThemeSwitch.vue";
import { Stage } from "./wasm/contract.ts";

const flow = useInstallFlow();
const { phase, step, reports } = flow;

const checkingPhone = computed(
  () => phase.value === Phase.Running && step.value === InstallStep.Phone && reports.plan !== undefined,
);

const screenKey = computed(() => `${phase.value}:${step.value}:${checkingPhone.value}`);

watch(screenKey, async () => {
  await nextTick();
  document.getElementById("screen-title")?.focus();
});

const pickingFiles = computed(
  () => reports.stage === Stage.ChoosingFiles || reports.stage === Stage.CheckingPickedFiles,
);

const activity = computed((): string => {
  if (phase.value !== Phase.Running) return "";
  const plan = reports.plan;
  if (step.value === InstallStep.Build && plan !== undefined) {
    const checked = plan.built.filter(
      (partition) => reports.images[partition]?.stage === Stage.ImageChecked,
    ).length;
    return `${checked} of ${plan.built.length} images built and checked`;
  }
  if (step.value === InstallStep.Write && reports.step !== undefined) {
    return `Step ${reports.step.index + 1} of ${reports.step.count}: ${describeStep(reports.step.step)}`;
  }
  if (step.value === InstallStep.Save && reports.packageWritten !== undefined) {
    return `${formatPercent(reports.packageWritten)} of the zip written`;
  }
  return "";
});

onMounted(() => {
  void flow.start();
});

onBeforeUnmount(flow.endSession);
</script>

<template>
  <div
    class="relative isolate grid min-h-dvh grid-cols-[1rem_minmax(0,1fr)_1rem] grid-rows-[auto_auto_1fr]
           [--panel-pad:clamp(1.5rem,1rem+2vw,2.75rem)] [--panel-top:clamp(0rem,1vh,0.75rem)]
           rail:h-dvh rail:grid-cols-[minmax(1rem,1fr)_minmax(0,44rem)_minmax(1rem,1fr)]
           rail:grid-rows-[auto_minmax(0,1fr)]"
  >
    <PageBackdrop />

    <header
      class="col-span-full flex min-h-(--field-clearance) items-start justify-between gap-4
             px-[clamp(1rem,3vw,2.5rem)] pt-[clamp(0.85rem,2.2svh,2.2rem)]"
    >
      <p class="flex min-h-11 items-center gap-[0.7rem] text-wedge-ink">
        <img
          class="block h-auto w-[clamp(32px,4.4vw,42px)]"
          :src="logoUrl"
          alt=""
          width="512"
          height="349"
        >
        <span class="font-display text-[clamp(1.05rem,2.6vw,1.4rem)] leading-none font-light tracking-tight">
          halogenOS
        </span>
      </p>
      <div class="flex gap-3">
        <a
          class="flex items-center rounded-xl border border-border bg-surface px-4 text-sm text-on-surface-secondary
                 hover:text-on-surface"
          :href="PRIVACY_POLICY_ADDRESS"
        >
          Privacy policy
        </a>
        <ThemeSwitch />
      </div>
    </header>

    <StepRail
      class="col-start-2 row-start-2 justify-self-start pt-1 rail:col-start-1 rail:mr-[clamp(2rem,4vw,4.5rem)]
             rail:justify-self-end rail:pt-[calc(var(--panel-top)+var(--panel-pad)+0.4rem)]"
      :steps="flow.run.value.steps"
      :current="step"
      :stopped="phase === Phase.Stopped"
    />

    <main
      class="col-start-2 row-start-3 pt-4 pb-16 rail:row-start-2 rail:min-h-0 rail:pt-(--panel-top)
             rail:pb-[clamp(1rem,3svh,2rem)]"
    >
      <div
        :key="screenKey"
        class="rounded-2xl border border-border bg-surface p-(--panel-pad) text-on-surface rail:max-h-full
               rail:overflow-y-auto rail:overscroll-contain rail:scroll-shadows
               [scrollbar-color:var(--color-border)_transparent]"
      >
        <ChooseRelease
          v-if="phase === Phase.Choosing"
          v-model:chosen="flow.chosen.value"
          :listing="flow.listing.value"
          :retries="flow.listingRetries.value"
          :zip-problem="flow.zipProblem.value"
          :usb-problem="flow.usbProblem.value"
          @save-zip="flow.saveZip"
          @flash="flow.goToPhone"
          @reload="flow.start"
        />
        <InstallStopped
          v-else-if="phase === Phase.Stopped && flow.failure.value !== undefined"
          :failure="flow.failure.value"
          :build="reports.plan?.build"
          :phone-messages="reports.phoneMessages"
          :phone-checks="phoneChecksOfStop(reports)"
          :title="flow.run.value.stoppedTitle"
          :again-label="flow.run.value.againLabel"
          @again="flow.startAgain"
        />
        <CheckPhone
          v-else-if="checkingPhone && reports.plan !== undefined"
          :build="reports.plan.build"
          :checks="reports.phoneChecks"
          @cancel="flow.cancel"
        />
        <ConnectPhone
          v-else-if="step === InstallStep.Phone && flow.chosen.value !== undefined"
          v-model:risk-accepted="flow.riskAccepted.value"
          :release="flow.chosen.value"
          :connecting="phase === Phase.Running"
          :cancelled="flow.cancelled.value"
          :usb-problem="flow.usbProblem.value"
          @connect="flow.connect"
          @back="flow.goToRelease"
          @cancel="flow.cancel"
        />
        <PickFiles
          v-else-if="pickingFiles && reports.plan !== undefined && reports.fileWait !== undefined"
          :build="reports.plan.build"
          :asked="reports.fileWait.asked"
          :files="reports.fileWait.files"
          :wrong-picks="reports.fileWait.wrongPicks"
          :checked="reports.pickedFilesChecked"
          @pick="flow.pickFiles"
          @cancel="flow.cancel"
        />
        <BuildImages
          v-else-if="step === InstallStep.Build && flow.chosen.value !== undefined"
          :build="flow.chosen.value.build"
          :plan="reports.plan"
          :shipped="reports.shipped"
          :stock-firmware="reports.stockFirmware"
          :images="reports.images"
          :failed-attempts="reports.failedAttempts"
          :note="flow.run.value.buildNote"
          @cancel="flow.cancel"
        />
        <SaveZip
          v-else-if="step === InstallStep.Save && reports.plan !== undefined"
          :build="reports.plan.build"
          :file-name="flow.packageFileName.value"
          :package-written="reports.packageWritten"
          @cancel="flow.cancel"
        />
        <WriteImages
          v-else-if="step === InstallStep.Write && reports.plan !== undefined && reports.step !== undefined"
          :plan="reports.plan"
          :current="reports.step"
          :sent="reports.sent"
          :phone-messages="reports.phoneMessages"
          @choose-again="flow.choosePhoneAgain"
        />
        <InstallDone
          v-else-if="phase === Phase.Installed && reports.plan !== undefined"
          :build="reports.plan.build"
          :phone-messages="reports.phoneMessages"
          @again="flow.goToRelease"
        />
        <ZipSaved
          v-else-if="phase === Phase.Saved && reports.plan !== undefined"
          :plan="reports.plan"
          :file-name="flow.packageFileName.value"
          :stored-zip="flow.storedPackage.value"
          @again="flow.goToRelease"
        />
      </div>
    </main>

    <p class="sr-only" aria-live="polite">{{ activity }}</p>
    <PrivacyPolicy />
  </div>
</template>
