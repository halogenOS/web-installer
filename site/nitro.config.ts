// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only
import { defineNitroConfig } from "nitropack/config";
import { RELEASES_PATH, RELEASE_FILE_ROUTE } from "./src/remote/release_paths.ts";

export default defineNitroConfig({
  compatibilityDate: "2026-11-18",
  preset: "node-server",
  srcDir: "server",
  imports: false,
  esbuild: { options: { target: "es2022" } },
  publicAssets: [{ dir: "../dist" }],
  handlers: [
    { route: RELEASES_PATH, method: "get", handler: "~/release_list_route.ts", lazy: false },
    { route: RELEASE_FILE_ROUTE, method: "get", handler: "~/release_file_route.ts", lazy: false },
  ],
  runtimeConfig: {
    githubToken: "",
  },
});
