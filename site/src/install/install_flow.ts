// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { computed, reactive, ref, shallowRef } from "vue";
import {
  PhoneProblem,
  PhoneUnavailable,
  requestPhone,
  usbAvailable,
  windowUsb,
  type ChosenPhone,
} from "../device/usb.ts";
import { pageStopped } from "../engine/install_failure.ts";
import { InstallSession } from "../engine/install_session.ts";
import { WorkerMessageKind, type WorkerMessage } from "../engine/messages.ts";
import {
  failureOfPhoneProblem,
  failureOfSaveDialog,
  ListingEnd,
  listPublishedReleases,
  type Listing,
} from "../engine/page_engine.ts";
import { choosePackageFile, FileChoice, type PackageFile } from "../store/package_file.ts";
import { Stage, type InstallFailure, type ListedRelease } from "../wasm/contract.ts";
import { InstallMethod, InstallStep, METHODS, STEP_OF_STAGE } from "./install_steps.ts";
import { emptyReports, recordProgress, type RunReports } from "./run_reports.ts";

export const Phase = {
  Choosing: "choosing",
  Connecting: "connecting",
  Running: "running",
  Installed: "installed",
  Saved: "saved",
  Stopped: "stopped",
} as const;
export type Phase = (typeof Phase)[keyof typeof Phase];

const LINKED_RELEASE_PARAMETER = "release";

function linkedReleaseTag(): string | undefined {
  const tag = new URLSearchParams(window.location.search).get(LINKED_RELEASE_PARAMETER);
  return tag === null || tag === "" ? undefined : tag;
}

function preventUnload(event: BeforeUnloadEvent): void {
  event.preventDefault();
}

export function useInstallFlow() {
  const phase = ref<Phase>(Phase.Choosing);
  const listing = shallowRef<Listing | undefined>();
  const listingRetries = ref<number | undefined>();
  const chosen = shallowRef<ListedRelease | undefined>();
  const method = ref<InstallMethod>(InstallMethod.Zip);
  const packageFile = shallowRef<PackageFile | undefined>();
  const storedPackage = shallowRef<File | undefined>();
  const packageFileName = computed(() => packageFile.value?.file.name);
  const usbProblem = shallowRef<InstallFailure | undefined>();
  const zipProblem = shallowRef<InstallFailure | undefined>();
  const cancelled = ref(false);
  const reports = reactive<RunReports>(emptyReports(METHODS[InstallMethod.Zip].firstStage));
  const failure = shallowRef<InstallFailure | undefined>();
  let session: InstallSession | undefined;

  const run = computed(() => METHODS[method.value]);

  const step = computed((): InstallStep => {
    switch (phase.value) {
      case Phase.Choosing:
        return InstallStep.Release;
      case Phase.Connecting:
        return InstallStep.Phone;
      case Phase.Running:
      case Phase.Stopped:
        return STEP_OF_STAGE[reports.stage];
      case Phase.Installed:
      case Phase.Saved:
        return InstallStep.Done;
    }
  });

  const writesToPhone = computed(
    () => reports.stage === Stage.Installing || reports.stage === Stage.Sending,
  );

  async function loadReleases(): Promise<void> {
    listing.value = undefined;
    listingRetries.value = undefined;
    try {
      listing.value = await listPublishedReleases(linkedReleaseTag(), (failedAttempts) => {
        listingRetries.value = failedAttempts;
      });
    } catch (cause) {
      listing.value = { kind: ListingEnd.Failed, failure: pageStopped(String(cause)) };
      return;
    }
    if (listing.value.kind !== ListingEnd.Listed || listing.value.unlistedTag !== undefined) return;
    const [only, ...others] = listing.value.releases;
    if (only !== undefined && others.length === 0) chosen.value = only;
  }

  async function start(): Promise<void> {
    await loadReleases();
    if (listing.value?.kind !== ListingEnd.Listed) return;
    if (!usbAvailable(window)) {
      usbProblem.value = await failureOfPhoneProblem(PhoneProblem.NoWebUsb);
    }
  }

  function end(next: Phase): void {
    session?.end();
    session = undefined;
    window.removeEventListener("beforeunload", preventUnload);
    phase.value = next;
  }

  function onMessage(message: WorkerMessage): void {
    switch (message.kind) {
      case WorkerMessageKind.Progress:
        recordProgress(reports, message.progress);
        if (message.progress.stage === Stage.Done) end(Phase.Installed);
        else if (message.progress.stage === Stage.PackageSaved) void endSave();
        break;
      case WorkerMessageKind.Retrying:
        reports.failedAttempts = message.failedAttempts;
        break;
      case WorkerMessageKind.PhoneMessage:
        reports.phoneMessages.push(message.text);
        break;
      case WorkerMessageKind.Failed:
        stop(message.failure);
        break;
    }
  }

  async function endSave(): Promise<void> {
    const file = packageFile.value;
    storedPackage.value = file?.kind === FileChoice.Stored ? await file.file.getFile() : undefined;
    end(Phase.Saved);
  }

  function stop(reason: InstallFailure): void {
    failure.value = reason;
    end(Phase.Stopped);
  }

  async function choosePhone(): Promise<ChosenPhone | undefined> {
    try {
      return await requestPhone(windowUsb(window));
    } catch (cause) {
      if (!(cause instanceof PhoneUnavailable)) throw cause;
      usbProblem.value = await failureOfPhoneProblem(cause.problem);
      return undefined;
    }
  }

  const events = {
    onMessage,
    onCrash: (report: string) => {
      stop(pageStopped(report));
    },
  };

  function begin(next: InstallMethod, start: () => InstallSession): void {
    method.value = next;
    Object.assign(reports, emptyReports(METHODS[next].firstStage));
    failure.value = undefined;
    cancelled.value = false;
    phase.value = Phase.Running;
    window.addEventListener("beforeunload", preventUnload);
    session = start();
  }

  async function saveZip(): Promise<void> {
    const release = chosen.value;
    if (release === undefined) return;
    const choice = await choosePackageFile(window, release.build);
    switch (choice.kind) {
      case FileChoice.Cancelled:
        return;
      case FileChoice.DialogFailed:
        zipProblem.value = await failureOfSaveDialog();
        return;
      case FileChoice.Chosen:
      case FileChoice.Stored:
        zipProblem.value = undefined;
        packageFile.value = choice;
        begin(InstallMethod.Zip, () =>
          InstallSession.savePackage(choice.file, release.tag, events),
        );
    }
  }

  async function connect(): Promise<void> {
    const release = chosen.value;
    if (release === undefined) return;
    const phone = await choosePhone();
    if (phone === undefined) return;
    begin(InstallMethod.Flash, () =>
      InstallSession.install(phone.id, release.tag, events),
    );
  }

  function goToPhone(): void {
    method.value = InstallMethod.Flash;
    cancelled.value = false;
    phase.value = Phase.Connecting;
  }

  function goToRelease(): void {
    method.value = InstallMethod.Zip;
    phase.value = Phase.Choosing;
  }

  function pickFiles(files: File[]): void {
    session?.pickFiles(files);
  }

  const START_AGAIN: Record<InstallMethod, () => void> = {
    [InstallMethod.Zip]: goToRelease,
    [InstallMethod.Flash]: goToPhone,
  };

  const CANCEL: Record<InstallMethod, () => void> = {
    [InstallMethod.Zip]: goToRelease,
    [InstallMethod.Flash]: () => {
      goToPhone();
      cancelled.value = true;
    },
  };

  async function choosePhoneAgain(): Promise<void> {
    const phone = await choosePhone();
    if (phone !== undefined) session?.choosePhoneAgain(phone.id);
  }

  return {
    phase,
    run,
    step,
    listing,
    listingRetries,
    chosen,
    method,
    packageFileName,
    storedPackage,
    usbProblem,
    zipProblem,
    cancelled,
    reports,
    failure,
    writesToPhone,
    start,
    saveZip,
    connect,
    choosePhoneAgain,
    pickFiles,
    goToPhone,
    goToRelease,
    startAgain: () => {
      START_AGAIN[method.value]();
    },
    cancel: () => {
      end(phase.value);
      CANCEL[method.value]();
    },
    endSession: () => {
      end(phase.value);
    },
  };
}
