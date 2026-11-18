// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { Icon } from "./icons.ts";

export const ThemeChoice = { System: "system", Light: "light", Dark: "dark" } as const;
export type ThemeChoice = (typeof ThemeChoice)[keyof typeof ThemeChoice];

export const THEME_CHOICES: Record<ThemeChoice, { label: string; icon: Icon }> = {
  [ThemeChoice.System]: { label: "Theme of the system", icon: Icon.Monitor },
  [ThemeChoice.Light]: { label: "Light theme", icon: Icon.Sun },
  [ThemeChoice.Dark]: { label: "Dark theme", icon: Icon.Moon },
};

export function applyTheme(root: HTMLElement, choice: ThemeChoice): void {
  root.dataset.theme = choice;
}
