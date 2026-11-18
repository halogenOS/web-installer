// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export const ButtonLook = {
  Plain: "plain",
  Primary: "primary",
  Quiet: "quiet",
} as const;
export type ButtonLook = (typeof ButtonLook)[keyof typeof ButtonLook];

const CLASSES_OF_EVERY_LOOK = `inline-flex min-h-12 cursor-pointer items-center justify-center gap-[0.6rem]
  rounded-cell border px-[1.35rem] text-[0.9375rem] leading-none font-semibold transition-colors
  duration-150 [--icon-size:18px] disabled:cursor-default disabled:opacity-55 has-disabled:cursor-default
  has-disabled:opacity-55 has-focus-visible:focus-ring`;

export const BUTTON_CLASS: Record<ButtonLook, string> = {
  [ButtonLook.Plain]: `${CLASSES_OF_EVERY_LOOK} border-border text-on-surface hover:bg-hover-on-surface`,
  [ButtonLook.Primary]: `${CLASSES_OF_EVERY_LOOK} border-accent bg-accent text-on-accent
    hover:border-accent-hover hover:bg-accent-hover`,
  [ButtonLook.Quiet]: `${CLASSES_OF_EVERY_LOOK} border-transparent text-on-surface-secondary
    hover:text-on-surface`,
};
