// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// @vitest-environment node

import { Linter } from "eslint";
import stylelint from "stylelint";
import tseslint from "typescript-eslint";
import { describe, expect, it } from "vitest";

import { builtText, joinedText, localeFormats, physicalStyles } from "../eslint.config.js";

const root = new URL("..", import.meta.url).pathname;

/** The messages the app's rules for joined and built text, locale formats and physical layout give for a piece of component code. */
function eslintMessages(code: string): string[] {
  const messages = new Linter().verify(
    code,
    {
      files: ["**/*.tsx"],
      languageOptions: { parser: tseslint.parser, parserOptions: { ecmaFeatures: { jsx: true } } },
      rules: { "no-restricted-syntax": ["error", ...joinedText, ...physicalStyles, ...builtText, ...localeFormats] },
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

describe("the lint for text built in code", () => {
  it("refuses text chosen by a condition, in templates, toasts and titles", () => {
    const messages = eslintMessages(
      component(`
  const toasts = { show: (_: { text: string }) => undefined };
  const win = { setTitle: (_: string) => undefined };
  toasts.show({ text: "Saved the task" });
  win.setTitle(\`Task \${String(n)}\`);
  document.title = "Glyphkeel";
  const count = n === 1 ? "one task" : \`\${String(n)} tasks\`;
  return (
    <p aria-label={n > 0 ? "Open" : "Closed"} title={\`Task \${String(n)}\`}>
      {n > 0 ? "Open" : t("x")}
      {n > 0 && "Late"}
      {\`\${String(n)} tasks\`}
      {count}
    </p>
  );`),
    );
    expect(messages).toHaveLength(11);
  });

  it("refuses an engine error's English shown as it is", () => {
    const messages = eslintMessages(
      component(`
  const failure = { error: { message: "x" } };
  const problem = { message: "x" };
  return <p title={failure.error.message}>{problem.message}</p>;`),
    );
    expect(messages).toHaveLength(2);
  });

  it("accepts messages chosen by a condition, and codes", () => {
    const messages = eslintMessages(
      component(`
  const kind = n === 1 ? "one" : "other";
  return (
    <p aria-label={t(n > 0 ? "x" : "y")} data-kind={kind} className={\`chip hue-\${kind}\`}>
      {t(n === 1 ? "x" : "y", { count: n })}
    </p>
  );`),
    );
    expect(messages).toEqual([]);
  });
});

describe("the lint for locale formats", () => {
  it("refuses formatting with the browser's default locale", () => {
    const messages = eslintMessages(
      component(`
  const a = new Date().toLocaleDateString();
  const b = n.toLocaleString(undefined, { style: "percent" });
  const c = new Intl.NumberFormat().format(n);
  const d = ["b", "a"].sort((x, y) => x.localeCompare(y));
  return <p>{a}{b}{c}{d}</p>;`),
    );
    expect(messages).toHaveLength(4);
  });

  it("accepts the active locale, and reading the system's time zone", () => {
    const messages = eslintMessages(
      component(`
  const locale = "de-CH";
  const a = new Date().toLocaleDateString(locale);
  const b = new Intl.NumberFormat(locale).format(n);
  const c = ["b", "a"].sort((x, y) => x.localeCompare(y, locale));
  const zone = new Intl.DateTimeFormat().resolvedOptions().timeZone;
  return <p>{a}{b}{c}{zone}</p>;`),
    );
    expect(messages).toEqual([]);
  });
});
