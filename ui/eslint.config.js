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

// A sentence is one message with placeables, never pieces put together in
// code: word order and punctuation differ between languages. Lists of names
// are joined with Intl.ListFormat.
const translated = "CallExpression[callee.name='t']";
export const joinedText = [
  {
    selector: `BinaryExpression[operator='+']:has(> ${translated})`,
    message: "Don't join translated text with +; write one message with placeables.",
  },
  {
    selector: `TemplateLiteral:has(> ${translated})`,
    message: "Don't build text around a translated message; write one message with placeables.",
  },
  {
    selector: `CallExpression[callee.property.name='join'][callee.object.type='ArrayExpression']:has(${translated})`,
    message: "Don't join translated pieces; write one message, or use Intl.ListFormat for a list.",
  },
  {
    selector: `JSXExpressionContainer:has(> ${translated}) ~ :matches(JSXText[value=/[^\\s]/u], JSXExpressionContainer:has(> Literal[value=/[^\\s]/u]))`,
    message: "Text next to a translated message belongs in the message; use one message with placeables.",
  },
  {
    selector: `:matches(JSXText[value=/[^\\s]/u], JSXExpressionContainer:has(> Literal[value=/[^\\s]/u])) ~ JSXExpressionContainer:has(> ${translated})`,
    message: "Text next to a translated message belongs in the message; use one message with placeables.",
  },
  {
    selector: `JSXExpressionContainer:has(> ${translated}) ~ JSXExpressionContainer:has(> ${translated})`,
    message: "Two translated messages side by side make one sentence; write one message with placeables.",
  },
];

// Layout follows the writing direction, so right-to-left languages work
// later: inline styles use logical properties too. Positions taken from the
// pointer or from getBoundingClientRect are physical by nature and say so.
export const physicalStyles = [
  {
    selector: "Property[key.name=/^(margin|padding)(Left|Right)$|^border(Left|Right)|^border(Top|Bottom)(Left|Right)Radius$/]",
    message: "Use the logical property (marginInlineStart, paddingInlineEnd, borderInlineStart…).",
  },
  {
    selector: "JSXAttribute[name.name='style'] Property[key.name=/^(left|right)$/]",
    message: "Use insetInlineStart or insetInlineEnd, or explain why the position is physical.",
  },
  {
    selector: "Property[key.name=/^(textAlign|float|clear)$/][value.value=/^(left|right)$/]",
    message: "Use start or end, which follow the writing direction.",
  },
];

// The platform's own drop-down list ignores the theme (WebKitGTK opens an
// unstyled GTK list); choices use the shared Select component instead.
const nativeControls = [
  {
    selector: "JSXOpeningElement[name.name=/^(select|option|optgroup|datalist)$/]",
    message: "Use the Select component (src/components/Select.tsx), not a native <select>.",
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
      "no-restricted-syntax": [
        "error",
        ...htmlInsertion,
        ...uiText,
        ...joinedText,
        ...physicalStyles,
        ...nativeControls,
      ],
      "no-restricted-globals": ["error", ...networkGlobals],
    },
  },
  {
    // Tests may write the text they look for.
    files: ["src/**/*.test.{ts,tsx}"],
    rules: { "no-restricted-syntax": ["error", ...htmlInsertion, ...nativeControls] },
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
