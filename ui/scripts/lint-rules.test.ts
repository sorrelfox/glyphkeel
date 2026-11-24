// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// @vitest-environment node

import { Linter } from "eslint";
import stylelint from "stylelint";
import tseslint from "typescript-eslint";
import { describe, expect, it } from "vitest";

import { joinedText, physicalStyles } from "../eslint.config.js";

const root = new URL("..", import.meta.url).pathname;

/** The messages the app's rules for joined text and physical layout give for a piece of component code. */
function eslintMessages(code: string): string[] {
  const messages = new Linter().verify(
    code,
    {
      files: ["**/*.tsx"],
      languageOptions: { parser: tseslint.parser, parserOptions: { ecmaFeatures: { jsx: true } } },
      rules: { "no-restricted-syntax": ["error", ...joinedText, ...physicalStyles] },
    },
    "Sample.tsx",
  );
  expect(messages.filter((message) => message.fatal)).toEqual([]);
  return messages.map((message) => message.message);
}

/** The rules stylelint breaks in a piece of a component stylesheet. */
async function stylelintRules(code: string): Promise<string[]> {
  const result = await stylelint.lint({ code, codeFilename: `${root}src/screens/LintSample.module.css`, cwd: root });
  return (result.results[0]?.warnings ?? []).map((warning) => warning.rule);
}

const component = (body: string) => `
import { useT } from "../i18n/I18n";
export function Sample({ n }: { n: number }) {
  const t = useT();
  ${body}
}
`;

describe("the lint for joined text", () => {
  it("refuses translated text joined with +, in a template or with join", () => {
    const messages = eslintMessages(
      component(`
  const a = t("x") + String(n);
  const b = \`\${t("x")} \${String(n)}\`;
  const c = [t("x"), t("y")].join(", ");
  return <p>{a}{b}{c}</p>;`),
    );
    expect(messages).toHaveLength(3);
    expect(messages.every((message) => /translated|message/.test(message))).toBe(true);
  });

  it("refuses punctuation or a second message next to a translated message", () => {
    const messages = eslintMessages(
      component(`
  return (
    <p>
      {t("x")}: {n}
      <span>({t("x")})</span>
      <span>{t("x")}{" · "}</span>
      <span>
        {t("x")} {t("y")}
      </span>
    </p>
  );`),
    );
    expect(messages.length).toBeGreaterThanOrEqual(4);
  });

  it("accepts one message with placeables, and messages in separate elements", () => {
    const messages = eslintMessages(
      component(`
  return (
    <p>
      {t("x", { count: n })}
      <b>{t("y")}</b>
    </p>
  );`),
    );
    expect(messages).toEqual([]);
  });
});

describe("the lint for physical layout", () => {
  it("refuses left and right in stylesheets", async () => {
    const rules = await stylelintRules(`
.a { margin-left: 4px; padding-right: 2px; border-left: 1px solid var(--line); }
.b { position: absolute; left: 0; right: 0; }
.c { text-align: left; float: right; border-top-left-radius: var(--r-s); }
`);
    expect(rules.filter((rule) => rule === "property-disallowed-list")).toHaveLength(6);
    expect(rules.filter((rule) => rule === "declaration-property-value-disallowed-list")).toHaveLength(2);
  });

  it("accepts the logical properties", async () => {
    const rules = await stylelintRules(`
.a { margin-inline-start: 4px; padding-inline-end: 2px; border-inline-start: 1px solid var(--line); }
.b { position: absolute; inset-inline: 0; text-align: start; }
`);
    expect(rules).toEqual([]);
  });

  it("refuses physical properties in inline styles", () => {
    const messages = eslintMessages(
      component(`
  return <p style={{ marginLeft: 4, left: n, textAlign: "right" }}>{t("x")}</p>;`),
    );
    expect(messages).toHaveLength(3);
  });
});
