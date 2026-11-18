// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
export const Icon = {
  Check: "check",
  Alert: "alert",
  ArrowRight: "arrow_right",
  Download: "download",
  Unlocked: "unlocked",
  Power: "power",
  Cable: "cable",
  Restart: "restart",
  Layout: "layout",
  Erase: "erase",
  Copy: "copy",
  Monitor: "monitor",
  Sun: "sun",
  Moon: "moon",
} as const;
export type Icon = (typeof Icon)[keyof typeof Icon];

export const ICON_STROKES: Record<Icon, readonly string[]> = {
  [Icon.Check]: ["M5 12.5l4.5 4.5L19 7.5"],
  [Icon.Alert]: ["M12 3.5l9.5 16.5h-19z", "M12 10v4.5", "M12 17.5v0.01"],
  [Icon.ArrowRight]: ["M5 12h14", "M13 6l6 6-6 6"],
  [Icon.Download]: ["M12 3.5v11", "M7 10l5 5 5-5", "M4.5 16.5v3.5h15v-3.5"],
  [Icon.Unlocked]: ["M5.5 11h13v9.5h-13z", "M8.5 11V7.5a3.5 3.5 0 0 1 6.9-0.8", "M12 15v2"],
  [Icon.Power]: ["M12 3v8.5", "M7 6.3a7.5 7.5 0 1 0 10 0"],
  [Icon.Cable]: ["M10 2.5v3.5", "M14 2.5v3.5", "M7.5 6h9v4.5a4.5 4.5 0 0 1-9 0z", "M12 15v6.5"],
  [Icon.Restart]: ["M19.5 12a7.5 7.5 0 1 1-2.2-5.3", "M19.5 4v4.5H15"],
  [Icon.Layout]: ["M4 5h16v14H4z", "M4 10h16", "M10 10v9"],
  [Icon.Erase]: ["M4.5 7h15", "M9.5 7V4h5v3", "M6.5 7l1 13h9l1-13"],
  [Icon.Copy]: ["M8.5 8.5h11v11h-11z", "M15.5 8.5V4.5h-11v11h4"],
  [Icon.Monitor]: ["M3 4.5h18v11.5H3z", "M8 20.5h8", "M12 16v4.5"],
  [Icon.Sun]: [
    "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z",
    "M12 2.5v2",
    "M12 19.5v2",
    "M2.5 12h2",
    "M19.5 12h2",
    "M5.3 5.3l1.4 1.4",
    "M17.3 17.3l1.4 1.4",
    "M5.3 18.7l1.4-1.4",
    "M17.3 6.7l1.4-1.4",
  ],
  [Icon.Moon]: ["M19.5 14.5A8 8 0 0 1 9.5 4.5a8 8 0 1 0 10 10z"],
};
