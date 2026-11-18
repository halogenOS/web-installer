<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { computed } from "vue";
import { hostsOfFetches } from "../install/release_hosts.ts";
import { FetchKind, type ListedRelease } from "../wasm/contract.ts";
import { PRIVACY_POLICY_ADDRESS } from "./privacy_policy.ts";

const props = defineProps<{ release: ListedRelease }>();

const WHAT_IT_GETS: Record<FetchKind, string> = {
  [FetchKind.BuildFile]:
    "The files of this build, which this server reads from GitHub in requests that carry no data about you.",
  [FetchKind.SourceFiles]: "Files of the phone that the build cannot include, such as radio images.",
  [FetchKind.SourceLfsObjects]: "Large files of the phone, which the repository stores in Git LFS.",
  [FetchKind.StockFirmware]: "The stock firmware of the phone maker, which the page reads files of the phone from.",
};

const hosts = computed(() => hostsOfFetches(props.release.fetches));
</script>

<template>
  <section class="grid gap-[0.9rem]" aria-labelledby="hosts-title">
    <h2 id="hosts-title" class="font-semibold">Where the files come from</h2>
    <p class="text-on-surface-secondary">
      For <span class="font-mono text-code">{{ release.build }}</span>, the page downloads from these hosts:
    </p>
    <dl class="grid gap-2">
      <div
        v-for="item in hosts"
        :key="item.host"
        class="grid gap-[0.35rem] rounded-xl border border-border bg-surface-raised px-4 py-[0.8rem]"
      >
        <dt class="font-mono text-code font-semibold text-accent wrap-anywhere">{{ item.host }}</dt>
        <dd v-for="[kind, names] in item.files" :key="kind">
          {{ WHAT_IT_GETS[kind] }}
          <span v-if="names.length > 0" class="block font-mono text-code text-on-surface-secondary wrap-anywhere">
            {{ names.join(", ") }}
          </span>
        </dd>
      </div>
    </dl>
    <p class="text-sm text-on-surface-secondary">
      A host can hand a download on to another server of its own, as GitHub does with its file storage.
    </p>
    <p class="border-l-2 border-accent pl-[0.9rem]">
      By continuing, you're giving explicit consent to downloading these files from the hosts listed above, and you are
      aware of the risks associated with it: each host receives your IP address and what your browser sends with a
      request, and a host can be in a country outside the EU for which the EU has not decided that it protects
      personal data adequately, or be a company in the United States that does not take part in the EU-U.S. Data
      Privacy Framework. The
      <a class="text-accent underline" :href="PRIVACY_POLICY_ADDRESS">privacy policy</a> has the details.
    </p>
  </section>
</template>
