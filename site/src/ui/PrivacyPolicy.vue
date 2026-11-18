<!-- SPDX-FileCopyrightText: The halogenOS Project -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->
<script setup lang="ts">
import { onBeforeUnmount, onMounted, useTemplateRef } from "vue";
import { BUTTON_CLASS, ButtonLook } from "./button_look.ts";
import { PRIVACY_POLICY_ADDRESS } from "./privacy_policy.ts";

const PRIVACY_EMAIL = "privacy@halogenos.org";
const AUTHORITY_URL = "https://www.lda.bayern.de";

const BUILD_FILES_THROUGH_THIS_SERVER =
  "The files of a build come through the server that delivers this page. " +
  "That server reads the files from GitHub and passes them on to your browser as they arrive, storing nothing. " +
  "GitHub receives neither your IP address nor anything your browser sends for these files. " +
  'The server logs your requests for the files as described under "Server logs", on the same legal basis: ' +
  "Article 6(1)(f) GDPR, the legitimate interest in a functioning, secure service.";

const dialog = useTemplateRef<HTMLDialogElement>("dialog");

function showForAddress(): void {
  if (window.location.hash === PRIVACY_POLICY_ADDRESS) dialog.value?.showModal();
}

function dropFragment(): void {
  window.history.replaceState(null, "", window.location.pathname + window.location.search);
}

onMounted(() => {
  showForAddress();
  window.addEventListener("hashchange", showForAddress);
});

onBeforeUnmount(() => {
  window.removeEventListener("hashchange", showForAddress);
});
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto max-h-[calc(100dvh-4rem)] w-[min(44rem,calc(100%-2rem))] gap-[0.9rem] rounded-2xl border
           border-border bg-surface p-(--panel-pad) text-on-surface open:grid backdrop:bg-bg/70
           backdrop:backdrop-blur-[3px] [&_a]:text-accent [&_a]:underline [&_h3]:mt-[0.9rem] [&_h3]:font-semibold
           [&_p]:text-on-surface-secondary [&_p]:wrap-anywhere"
    aria-labelledby="privacy-title"
    @close="dropFragment"
  >
    <header class="flex items-center justify-between gap-4">
      <h2
        id="privacy-title"
        class="font-display text-[clamp(1.6rem,1.3rem+1vw,2rem)] leading-[1.1] font-semibold tracking-tight"
      >
        Privacy policy
      </h2>
      <form method="dialog">
        <button type="submit" :class="BUTTON_CLASS[ButtonLook.Quiet]">Close</button>
      </form>
    </header>

    <p>
      This policy explains how personal data is processed when you use this web installer. It is provided pursuant to
      Article 13 GDPR.
    </p>

    <h3>Controller</h3>
    <p>
      Simão Gomes Viana<br>
      c/o IP-Management #10911<br>
      Ludwig-Erhard-Str. 18<br>
      20459 Hamburg<br>
      Germany
    </p>
    <p>
      Data protection enquiries: <a :href="`mailto:${PRIVACY_EMAIL}`">{{ PRIVACY_EMAIL }}</a>.
      A data protection officer has not been appointed, as the statutory thresholds of § 38 BDSG are not met.
    </p>

    <h3>What the page does on your computer</h3>
    <p>
      The page runs in your browser. It builds the images and the installable zip on your computer and writes them to
      your phone over USB from there. What your phone reports, the files you pick and the zip stay on your computer:
      the page sends none of them anywhere.
    </p>

    <h3>Server logs</h3>
    <p>
      The server that delivers this page records technical data about each request: IP address, timestamp, requested
      path, HTTP status, referrer and user agent. The logs are used to operate the server, to diagnose faults, and to
      detect and defend against attacks and abuse. They rotate automatically, are not used to profile you and are not
      combined with any other data. Legal basis: Article 6(1)(f) GDPR, the legitimate interest in a functioning,
      secure service.
    </p>

    <h3>Processors</h3>
    <p>
      One processor is involved in delivering this page. This is Hetzner Online GmbH, which provides server hosting in
      Germany as a processor under Article 28 GDPR. The server and its logs are located in the European Union.
    </p>

    <h3>The list of builds</h3>
    <p>
      When the page opens, your browser reads two things from the server that delivers this page: the list of releases
      and the description of each release shown. That server reads them from GitHub on its own schedule and
      caches them. Its requests to GitHub contain no data about you, so GitHub does not receive your IP address or
      anything that your browser sends. Your browser contacts only this server until you continue with "Download the
      installable zip" or "Flash over USB". The server logs these requests as described under "Server logs", on the
      same legal basis: Article 6(1)(f) GDPR, the legitimate interest in a functioning, secure service.
    </p>

    <h3>The files of a build and their hosts</h3>
    <p>{{ BUILD_FILES_THROUGH_THIS_SERVER }}</p>
    <p>
      When you continue with "Download the installable zip" or "Flash over USB", the page downloads what the chosen
      build needs: files of the phone that the build cannot include from public code hosts, such as
      raw.githubusercontent.com and media.githubusercontent.com of GitHub, and the stock firmware from the download
      servers of the phone maker or wherever the build names it. These hosts depend on the build and can change from
      one build to the next. Before anything is downloaded, the page lists every host of the chosen build and what it
      gets from each, and it fetches nothing from them before you continue. A host can hand a download on to another
      server of its own.
    </p>
    <p>
      Each host receives your IP address, the time, the address of the file and what your browser sends with a request,
      and processes them under its own responsibility. A host can be in a country outside the EU/EEA for which the
      European Commission has not adopted an adequacy decision, or be a company in the United States that does not
      take part in the EU-U.S. Data Privacy Framework. Authorities there can have access to the data, and you can have
      fewer rights and remedies than in the EU.
    </p>
    <p>
      Legal basis: your consent under Article 6(1)(a) GDPR, and for the transfer to a country without an adequacy
      decision your explicit consent under Article 49(1)(a) GDPR. You give both by continuing after the list of hosts.
      You can withdraw your consent at any time with effect for the future by cancelling the download or closing the
      page.
    </p>

    <h3>Storage in your browser</h3>
    <p>
      The page stores the files that it downloads, the images that it builds and, in a browser without a save dialog,
      the installable zip in the storage of your browser for this page, the origin private file system. This storage
      is strictly necessary to provide the installation that you asked for, under § 25(2) No. 2 TDDDG. Each
      installation empties it first, and a completed installation removes its files. The page deletes the files of an
      installation that stopped when the next one starts, and a stored zip when the next zip replaces it. Clearing the
      data of this site in your browser deletes them at any time. The page sets no cookies and stores nothing else on your device.
    </p>

    <h3>What we do not do</h3>
    <p>
      No advertising is displayed on this page. No web analytics, tracking pixels or social media widgets are used, and
      every script, style and font of the page comes from the server that delivers it. No personal data is sold,
      rented or transferred for marketing purposes. No automated decision-making or profiling within the meaning of
      Article 22 GDPR takes place. The page has no accounts and no forms.
    </p>

    <h3>Retention</h3>
    <p>
      Server logs are retained only as long as needed for operation and security and are rotated automatically. Data
      that must be kept under statutory retention obligations is retained accordingly and otherwise restricted from
      processing.
    </p>

    <h3>Your rights</h3>
    <p>
      Under the GDPR you have the right to obtain confirmation as to whether your data is processed and to access it
      (Article 15), to rectification (Article 16), to erasure (Article 17), to restriction of processing (Article 18),
      to data portability (Article 20), and to object to processing based on legitimate interests (Article 21). Where
      processing rests on consent, you may withdraw it at any time with effect for the future.
    </p>
    <p>To exercise any of these, write to <a :href="`mailto:${PRIVACY_EMAIL}`">{{ PRIVACY_EMAIL }}</a>.</p>

    <h3>Right to lodge a complaint</h3>
    <p>
      You may lodge a complaint with a supervisory authority, in particular in the Member State of your habitual
      residence, place of work, or the place of the alleged infringement. The authority competent for this controller
      is the Bayerisches Landesamt für Datenschutzaufsicht (BayLDA), Promenade 18, 91522 Ansbach
      (<a :href="AUTHORITY_URL" target="_blank" rel="noopener noreferrer">{{ AUTHORITY_URL }}</a>).
    </p>

    <h3>Changes</h3>
    <p>This policy may be updated as the page changes. The current version is always available on this page.</p>

    <p class="text-sm">Last updated: 18 November 2026</p>
  </dialog>
</template>
