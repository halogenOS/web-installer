// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

export function monotonicMilliseconds(): number {
  return performance.now();
}

export const AttemptLoad = {
  Delivered: "delivered",
  HostBusy: "host_busy",
  RequestFailed: "request_failed",
} as const;
export type AttemptLoad = (typeof AttemptLoad)[keyof typeof AttemptLoad];

export type ScheduledWork<R> = (endRequest: () => void) => Promise<{ value: R; load: AttemptLoad }>;

export interface AttemptScheduler {
  run<R>(origin: string, work: ScheduledWork<R>): Promise<R>;
  runAlone<R>(origin: string, work: ScheduledWork<R>): Promise<R>;
}

class Window {
  #size = 1;
  #started = 0;
  #running = 0;
  #roundFrom = 0;

  get size(): number {
    return this.#size;
  }

  hasRoom(): boolean {
    return this.#running < this.#size;
  }

  isIdle(): boolean {
    return this.#running === 0;
  }

  begin(): number {
    this.#running += 1;
    return this.#started++;
  }

  end(): void {
    this.#running -= 1;
  }

  isInRound(attemptNumber: number): boolean {
    return attemptNumber >= this.#roundFrom;
  }

  startRound(): void {
    this.#roundFrom = this.#started;
  }

  resize(delta: number): void {
    this.#size = Math.max(1, this.#size + delta);
  }

  halve(attemptNumber: number): boolean {
    if (!this.isInRound(attemptNumber)) return false;
    this.resize(-this.#size / 2);
    this.startRound();
    return true;
  }
}

class AllHostsWindow extends Window {
  #slowStart = true;

  grow(): void {
    if (this.hasRoom()) return;
    this.resize(this.#slowStart ? 1 : 1 / this.size);
  }

  shrink(attemptNumber: number): void {
    if (this.halve(attemptNumber)) this.#slowStart = false;
  }
}

interface AnsweredRequest {
  time: number;
  ranAlone: boolean;
  requesting: number;
}

const WindowVerdict = {
  Grow: "grow",
  Hold: "hold",
  Shrink: "shrink",
} as const;
type WindowVerdict = (typeof WindowVerdict)[keyof typeof WindowVerdict];

const APPLY_VERDICT: Record<WindowVerdict, (window: Window) => void> = {
  [WindowVerdict.Grow]: (window) => {
    if (!window.hasRoom()) window.resize(1);
  },
  [WindowVerdict.Hold]: () => undefined,
  [WindowVerdict.Shrink]: (window) => {
    window.resize(-1);
  },
};

class RequestDelay {
  #requesting = 0;
  #timeWithoutWaiting: number | undefined;

  requestStarted(): void {
    this.#requesting += 1;
  }

  requestAnswered(time: number, ranAlone: boolean): AnsweredRequest {
    const answered = { time, ranAlone, requesting: this.#requesting };
    this.#requesting -= 1;
    return answered;
  }

  record(request: AnsweredRequest): void {
    if (request.ranAlone) this.#timeWithoutWaiting = request.time;
  }

  verdictOf(round: AnsweredRequest[]): WindowVerdict {
    if (this.#timeWithoutWaiting === undefined) return WindowVerdict.Hold;
    const time = mean(round.map((request) => request.time));
    const requestsWaiting =
      time === 0 ? 0 : mean(round.map((request) => request.requesting)) * (1 - this.#timeWithoutWaiting / time);
    return requestsWaiting < 1 ? WindowVerdict.Grow : WindowVerdict.Shrink;
  }
}

function mean(values: number[]): number {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

class AttemptRequest {
  readonly #now: () => number;
  readonly #startedAt: number;
  readonly #delay: RequestDelay;
  readonly #ranAlone: boolean;
  #answered: AnsweredRequest | undefined;

  constructor(now: () => number, delay: RequestDelay, ranAlone: boolean) {
    this.#now = now;
    this.#startedAt = now();
    this.#delay = delay;
    this.#ranAlone = ranAlone;
    delay.requestStarted();
  }

  end(): void {
    this.#answered ??= this.#answer();
  }

  answered(): AnsweredRequest {
    return (this.#answered ??= this.#answer());
  }

  #answer(): AnsweredRequest {
    return this.#delay.requestAnswered(this.#now() - this.#startedAt, this.#ranAlone);
  }
}

class HostAttempts {
  readonly window = new Window();
  readonly delay = new RequestDelay();
  readonly waiting = new StartQueue();
  #round: AnsweredRequest[] = [];

  adjustToRequest(attemptNumber: number, request: AnsweredRequest): void {
    this.delay.record(request);
    if (!this.window.isInRound(attemptNumber)) return;
    this.#round.push(request);
    if (this.#round.length < this.window.size) return;
    APPLY_VERDICT[this.delay.verdictOf(this.#round)](this.window);
    this.window.startRound();
    this.#round = [];
  }

  halve(attemptNumber: number): void {
    if (this.window.halve(attemptNumber)) this.#round = [];
  }
}

interface RunningAttempt {
  host: HostAttempts;
  hostNumber: number;
  all: AllHostsWindow;
  allNumber: number;
  request: AttemptRequest;
}

const ADJUST_WINDOWS: Record<AttemptLoad, (attempt: RunningAttempt, request: AnsweredRequest) => void> = {
  [AttemptLoad.Delivered]: (attempt, request) => {
    attempt.host.adjustToRequest(attempt.hostNumber, request);
    attempt.all.grow();
  },
  [AttemptLoad.HostBusy]: (attempt) => {
    attempt.host.halve(attempt.hostNumber);
  },
  [AttemptLoad.RequestFailed]: (attempt) => {
    attempt.host.halve(attempt.hostNumber);
    attempt.all.shrink(attempt.allNumber);
  },
};

class StartQueue {
  #starts: (() => void)[] = [];
  #head = 0;

  isEmpty(): boolean {
    return this.#head === this.#starts.length;
  }

  push(start: () => void): void {
    this.#starts.push(start);
  }

  startOldest(): void {
    const start = this.#starts[this.#head];
    this.#head += 1;
    if (this.#head * 2 >= this.#starts.length) {
      this.#starts = this.#starts.slice(this.#head);
      this.#head = 0;
    }
    start?.();
  }
}

export class RequestScheduler implements AttemptScheduler {
  readonly #hosts = new Map<string, HostAttempts>();
  readonly #all = new AllHostsWindow();
  readonly #aloneWaiting = new StartQueue();
  #aloneRequest: AttemptRequest | undefined;
  readonly #now: () => number;

  constructor(now: () => number) {
    this.#now = now;
  }

  run<R>(origin: string, work: ScheduledWork<R>): Promise<R> {
    const host = this.#hostAt(origin);
    return this.#schedule(host, host.waiting, false, work);
  }

  runAlone<R>(origin: string, work: ScheduledWork<R>): Promise<R> {
    return this.#schedule(this.#hostAt(origin), this.#aloneWaiting, true, work);
  }

  async #schedule<R>(
    host: HostAttempts,
    queue: StartQueue,
    alone: boolean,
    work: ScheduledWork<R>,
  ): Promise<R> {
    const attempt = await new Promise<RunningAttempt>((started) => {
      queue.push(() => {
        const request = new AttemptRequest(this.#now, host.delay, alone || host.window.size === 1);
        if (alone) this.#aloneRequest = request;
        started({
          host,
          hostNumber: host.window.begin(),
          all: this.#all,
          allNumber: this.#all.begin(),
          request,
        });
      });
      this.#startWaiting();
    });
    const endRequest = (): void => {
      attempt.request.end();
      if (this.#aloneRequest !== attempt.request) return;
      this.#aloneRequest = undefined;
      this.#startWaiting();
    };
    try {
      const { value, load } = await work(endRequest);
      ADJUST_WINDOWS[load](attempt, attempt.request.answered());
      return value;
    } finally {
      host.window.end();
      this.#all.end();
      endRequest();
      this.#startWaiting();
    }
  }

  #hostAt(origin: string): HostAttempts {
    let host = this.#hosts.get(origin);
    if (host === undefined) {
      host = new HostAttempts();
      this.#hosts.set(origin, host);
    }
    return host;
  }

  #startWaiting(): void {
    if (this.#aloneRequest !== undefined) return;
    if (!this.#aloneWaiting.isEmpty()) {
      if (this.#all.isIdle()) this.#aloneWaiting.startOldest();
      return;
    }
    let started = true;
    while (started) {
      started = false;
      for (const host of this.#hosts.values()) {
        if (host.waiting.isEmpty() || !host.window.hasRoom() || !this.#all.hasRoom()) continue;
        host.waiting.startOldest();
        started = true;
      }
    }
  }
}
