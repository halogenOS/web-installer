// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export class Waits<T> {
  readonly #resolvers: ((value: T) => void)[] = [];

  next(): Promise<T> {
    return new Promise((resolve) => {
      this.#resolvers.push(resolve);
    });
  }

  resolveAll(value: T): void {
    for (const resolve of this.#resolvers.splice(0)) resolve(value);
  }
}
