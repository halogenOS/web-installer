// SPDX-License-Identifier: GPL-3.0-only
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";
import globals from "globals";

const FETCH_MODULE = "src/remote/fetch.ts";
const WORKER_MODULE = "src/engine/install_session.ts";
const USB_MODULE = "src/device/usb.ts";
const STORAGE_MODULE = "src/store/install_store.ts";

const ONLY_NAMED_MODULES = `Only ${FETCH_MODULE} may send requests, and only ${WORKER_MODULE} may start a worker.`;
const ONLY_THE_USB_MODULE = `Only ${USB_MODULE} may use USB.`;
const ONLY_THE_STORAGE_MODULE = `Only ${STORAGE_MODULE} may store data of the page.`;

const IMPORT_EXPRESSION = { selector: "ImportExpression", message: ONLY_NAMED_MODULES };

function propertySelectors(name) {
  return [
    `MemberExpression[property.name='${name}']`,
    `MemberExpression[property.value='${name}']`,
    `ObjectPattern > Property[key.name='${name}']`,
    `ObjectPattern > Property[key.value='${name}']`,
    `CallExpression[callee.object.name='Reflect'] > Literal[value='${name}']`,
  ];
}

const CAPABILITIES = [
  {
    globals: ["fetch", "XMLHttpRequest"],
    selectors: [],
    modules: [FETCH_MODULE],
    message: ONLY_NAMED_MODULES,
  },
  { globals: ["Worker"], selectors: [], modules: [WORKER_MODULE], message: ONLY_NAMED_MODULES },
  {
    globals: ["WebSocket", "EventSource", "Image", "SharedWorker", "importScripts"],
    selectors: [],
    modules: [],
    message: ONLY_NAMED_MODULES,
  },
  {
    globals: ["indexedDB", "localStorage", "sessionStorage", "caches"],
    selectors: propertySelectors("storage"),
    modules: [STORAGE_MODULE],
    message: ONLY_THE_STORAGE_MODULE,
  },
  {
    globals: [],
    selectors: propertySelectors("usb"),
    modules: [USB_MODULE],
    message: ONLY_THE_USB_MODULE,
  },
];
const GLOBAL_OBJECTS = ["window", "globalThis", "self"];
const NAMED_MODULES = [...new Set(CAPABILITIES.flatMap((capability) => capability.modules))];

function restrictedGlobals(capabilities) {
  return capabilities.flatMap(({ globals, message }) => globals.map((name) => ({ name, message })));
}

function restrictedSyntax(capabilities) {
  return [
    IMPORT_EXPRESSION,
    ...capabilities.flatMap(({ selectors, message }) =>
      selectors.map((selector) => ({ selector, message })),
    ),
  ];
}

function blockOf(module) {
  const restricted = CAPABILITIES.filter((capability) => !capability.modules.includes(module));
  return {
    files: [module],
    rules: {
      "no-restricted-globals": ["error", ...restrictedGlobals(restricted)],
      "no-restricted-syntax": ["error", ...restrictedSyntax(restricted)],
    },
  };
}

const restrictedProperties = [
  ...GLOBAL_OBJECTS.flatMap((object) =>
    CAPABILITIES.flatMap(({ globals, message }) =>
      globals.map((property) => ({ object, property, message })),
    ),
  ),
  { object: "navigator", property: "sendBeacon", message: ONLY_NAMED_MODULES },
];

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      ".output/**",
      ".nitro/**",
      "node_modules/**",
      "src/wasm/web_installer_wasm.js",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: { parser: tseslint.parser },
    },
  },
  {
    linterOptions: { noInlineConfig: true },
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
      parserOptions: { ecmaVersion: "latest", sourceType: "module" },
    },
    rules: {
      "@typescript-eslint/ban-ts-comment": [
        "error",
        { "ts-expect-error": true, "ts-ignore": true, "ts-nocheck": true },
      ],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "never" }],
      "@typescript-eslint/no-non-null-assertion": "error",
      "id-denylist": ["error", "util", "utils", "misc", "helper", "helpers"],
      "vue/max-attributes-per-line": ["error", { singleline: { max: 3 } }],
      "vue/singleline-html-element-content-newline": "off",
      "vue/html-self-closing": [
        "error",
        { html: { normal: "always", void: "never", component: "always" } },
      ],
    },
  },
  {
    files: ["src/**/*.ts", "src/**/*.vue", "server/**/*.ts", "tools/**/*.ts", "vite.config.ts", "nitro.config.ts"],
    languageOptions: {
      parserOptions: {
        extraFileExtensions: [".vue"],
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "@typescript-eslint/no-unsafe-assignment": "error",
      "@typescript-eslint/no-unsafe-member-access": "error",
      "@typescript-eslint/no-unsafe-call": "error",
      "@typescript-eslint/no-unsafe-return": "error",
      "@typescript-eslint/no-unsafe-argument": "error",
      "@typescript-eslint/no-unsafe-type-assertion": "error",
      "@typescript-eslint/switch-exhaustiveness-check": "error",
    },
  },
  {
    files: ["src/**/*.ts", "src/**/*.vue"],
    rules: {
      "no-restricted-properties": ["error", ...restrictedProperties],
      "no-restricted-syntax": ["error", ...restrictedSyntax(CAPABILITIES)],
    },
  },
  {
    files: ["src/**/*.ts", "src/**/*.vue"],
    ignores: NAMED_MODULES,
    rules: {
      "no-restricted-globals": ["error", ...restrictedGlobals(CAPABILITIES)],
    },
  },
  ...NAMED_MODULES.map(blockOf),
);
