// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig, globalIgnores } from "eslint/config";
import globals from "globals";
import tseslint from "typescript-eslint";

// HTML reaches the page only through the reader component, after Rust's
// sanitizer. Every other way of inserting HTML is an error.
const htmlInsertion = [
  {
    selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']",
    message: "Insert HTML only through the reader component.",
  },
  {
    selector: "AssignmentExpression > MemberExpression.left[property.name=/^(innerHTML|outerHTML)$/]",
    message: "Insert HTML only through the reader component.",
  },
  {
    selector: "CallExpression[callee.property.name=/^(insertAdjacentHTML|write|writeln|createContextualFragment)$/]",
    message: "Insert HTML only through the reader component.",
  },
];

// Every word the UI shows comes from the Fluent message files (src/i18n), so
// it can be translated: no text between tags, and no literal text in the
// attributes people read.
const uiText = [
  {
    selector: "JSXText[value=/\\p{L}/u]",
    message: "UI text comes from the Fluent message files (src/i18n); use t().",
  },
  {
    selector: "JSXElement > JSXExpressionContainer > Literal[value=/\\p{L}/u]",
    message: "UI text comes from the Fluent message files (src/i18n); use t().",
  },
  {
    selector:
      "JSXAttribute[name.name=/^(aria-label|aria-description|aria-placeholder|aria-roledescription|aria-valuetext|title|placeholder|alt|label)$/] > Literal",
    message: "UI text comes from the Fluent message files (src/i18n); use t().",
  },
];

// The app never goes online: the page talks to Rust over Tauri's IPC only.
const networkGlobals = ["fetch", "XMLHttpRequest", "WebSocket", "EventSource", "Request"].map((name) => ({
  name,
  message: "The app has no network access; call the engine through the IPC client.",
}));

export default defineConfig([
  globalIgnores(["dist", "node_modules", "playwright-report", "test-results", "src/gen"]),
  {
    files: ["**/*.{ts,tsx}"],
    extends: [js.configs.recommended, tseslint.configs.strictTypeChecked, reactHooks.configs.flat["recommended-latest"]],
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "no-eval": "error",
      "no-implied-eval": "off",
      "@typescript-eslint/no-implied-eval": "error",
      "no-new-func": "error",
      "no-script-url": "error",
      "no-restricted-syntax": ["error", ...htmlInsertion, ...uiText],
      "no-restricted-globals": ["error", ...networkGlobals],
    },
  },
  {
    // Tests may write the text they look for.
    files: ["src/**/*.test.{ts,tsx}"],
    rules: { "no-restricted-syntax": ["error", ...htmlInsertion] },
  },
  {
    // Build and test configuration runs in Node, not in the app.
    files: ["*.config.{js,ts}", "tests/**/*.ts"],
    languageOptions: { globals: globals.node },
    rules: { "no-restricted-globals": "off" },
  },
  {
    files: ["**/*.js"],
    extends: [js.configs.recommended],
    languageOptions: { globals: globals.node },
  },
]);
