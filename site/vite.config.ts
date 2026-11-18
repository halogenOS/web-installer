// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { emitFontLicences } from "./tools/font_licences.ts";

export default defineConfig({
  plugins: [vue(), tailwindcss(), emitFontLicences()],
  worker: { format: "es" },
  server: { fs: { allow: [".", "../website/public"] } },
  build: {
    target: "es2022",
    sourcemap: false,
  },
});
