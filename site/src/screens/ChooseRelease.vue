<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { ListingEnd, type Listing } from "../engine/page_engine.ts";
import AppIcon from "../ui/AppIcon.vue";
import { BUTTON_CLASS, ButtonLook } from "../ui/button_look.ts";
import FailureNotice from "../ui/FailureNotice.vue";
import ReleaseHosts from "../ui/ReleaseHosts.vue";
import { Icon } from "../ui/icons.ts";
import ScreenFrame from "../ui/ScreenFrame.vue";
import WaitingNote from "../ui/WaitingNote.vue";
import type { InstallFailure, ListedRelease } from "../wasm/contract.ts";

defineProps<{
  listing: Listing | undefined;
  retries: number | undefined;
  zipProblem: InstallFailure | undefined;
  usbProblem: InstallFailure | undefined;
}>();
const chosen = defineModel<ListedRelease | undefined>("chosen", { required: true });
const emit = defineEmits<{ saveZip: []; flash: []; reload: [] }>();

const unlistedReleaseNotice = (tag: string): string =>
  `No release to install matches ${tag}. Choose a release from the list below.`;
</script>

<template>
  <ScreenFrame title="Choose a release">
    <WaitingNote v-if="listing === undefined">
      <span>Loading releases</span>
      <span v-if="retries !== undefined" class="text-sm text-on-surface-secondary">
        Retrying a download ({{ retries }} failed)
      </span>
    </WaitingNote>

    <FailureNotice v-else-if="listing.kind === ListingEnd.Failed" :failure="listing.failure" />

    <p v-else-if="listing.releases.length === 0">There is no release to install yet.</p>

    <template v-else>
      <p
        v-if="listing.unlistedTag !== undefined"
        class="border-l-2 border-accent pl-[0.9rem] wrap-anywhere"
        role="status"
      >
        {{ unlistedReleaseNotice(listing.unlistedTag) }}
      </p>
      <fieldset class="grid gap-[0.6rem]">
        <legend class="sr-only">Release</legend>
        <label
          v-for="release in listing.releases"
          :key="release.device"
          class="group grid cursor-pointer grid-cols-[auto_1fr] items-center gap-x-4 gap-y-[0.15rem] rounded-xl
                 border-[1.5px] border-border bg-surface-raised px-[1.15rem] py-4 transition-colors duration-150
                 hover:border-on-surface-secondary has-checked:border-accent has-checked:bg-accent/10
                 has-focus-visible:focus-ring"
        >
          <input
            v-model="chosen"
            class="sr-only"
            type="radio"
            name="release"
            :value="release"
          >
          <span
            class="row-span-2 grid size-5 place-items-center rounded-[5px] border-[1.5px] border-on-surface-secondary
                   group-has-checked:border-accent after:size-2.5 after:scale-0 after:rounded-[2px] after:bg-accent
                   after:transition-transform after:duration-150 group-has-checked:after:scale-100"
            aria-hidden="true"
          />
          <span class="flex items-baseline gap-[0.6rem] font-display text-[1.0625rem] font-bold">
            {{ release.device }}
            <span
              v-if="release.series !== undefined"
              class="rounded-md border border-border px-[0.45rem] py-[0.1rem] font-mono text-xs font-normal
                     tracking-[0.02em] text-on-surface-secondary transition-colors duration-150
                     group-has-checked:border-accent/50 group-has-checked:text-accent"
            >
              v{{ release.series }}
            </span>
          </span>
          <span class="font-mono text-code text-on-surface-secondary wrap-anywhere">{{ release.build }}</span>
        </label>
      </fieldset>
      <ReleaseHosts v-if="chosen !== undefined" :release="chosen" />
    </template>

    <FailureNotice v-if="zipProblem !== undefined" :failure="zipProblem" />

    <template #actions>
      <button
        v-if="listing?.kind === ListingEnd.Failed"
        type="button"
        :class="BUTTON_CLASS[ButtonLook.Primary]"
        @click="emit('reload')"
      >
        Try again
      </button>
      <template v-else>
        <button
          v-if="usbProblem === undefined"
          type="button"
          :class="BUTTON_CLASS[ButtonLook.Quiet]"
          :disabled="chosen === undefined"
          @click="emit('flash')"
        >
          Flash over USB
        </button>
        <button
          type="button"
          :class="BUTTON_CLASS[ButtonLook.Primary]"
          :disabled="chosen === undefined"
          @click="emit('saveZip')"
        >
          <AppIcon :name="Icon.Download" />
          Download the installable zip
        </button>
      </template>
    </template>
  </ScreenFrame>
</template>
