// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import type { Plugin } from "vite";

const FONT_FILE_PATH = /@fontsource-variable\/(?<font>[^/]+)\//;

export function emitFontLicences(): Plugin {
  const require = createRequire(import.meta.url);
  return {
    name: "emit-font-licences",
    apply: "build",
    async generateBundle(_options, bundle) {
      const fonts = new Set<string>();
      for (const file of Object.values(bundle)) {
        if (file.type !== "asset") continue;
        for (const path of file.originalFileNames) {
          const font = FONT_FILE_PATH.exec(path)?.groups?.font;
          if (font !== undefined) fonts.add(font);
        }
      }
      for (const font of fonts) {
        this.emitFile({
          type: "asset",
          fileName: `licenses/${font}.txt`,
          source: await readFile(require.resolve(`@fontsource-variable/${font}/LICENSE`)),
        });
      }
    },
  };
}
