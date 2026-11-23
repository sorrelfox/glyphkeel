// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * KaTeX, Mermaid and Shiki, shared by the reader and the editor. Each loads
 * on first use, as its own chunk, with its grammars, fonts and styles
 * bundled: nothing comes from the network.
 */

import type { HighlighterCore, LanguageInput } from "shiki/core";

type Katex = typeof import("katex").default;
type Mermaid = typeof import("mermaid").default;

let katexLoading: Promise<Katex> | null = null;

/** Loads KaTeX and its styles on the first formula. */
export function loadKatex(): Promise<Katex> {
  katexLoading ??= Promise.all([import("katex"), import("katex/dist/katex.min.css")]).then(([module]) => module.default);
  return katexLoading;
}

/** Draws a formula into `element`, or its error in place. KaTeX runs with `trust` off. */
export async function drawMath(element: HTMLElement, tex: string, display: boolean): Promise<void> {
  const katex = await loadKatex();
  try {
    katex.render(tex, element, { displayMode: display, throwOnError: true, trust: false, strict: "ignore", maxSize: 50, maxExpand: 1000 });
    element.classList.remove("gk-render-error");
  } catch (error) {
    element.textContent = error instanceof Error ? error.message : String(error);
    element.classList.add("gk-render-error");
  }
}

let mermaidLoading: Promise<Mermaid> | null = null;
let mermaidTheme = "";

/** A colour token as a plain `rgb()` value, which Mermaid can mix. */
function resolvedColor(probe: HTMLElement, token: string): string {
  probe.style.color = `var(${token})`;
  return getComputedStyle(probe).color;
}

/** Mermaid's theme from the app's current theme tokens. */
function themeVariables(): Record<string, string> {
  const probe = document.createElement("span");
  probe.hidden = true;
  document.body.append(probe);
  try {
    const color = (token: string) => resolvedColor(probe, token);
    return {
      background: color("--surface"),
      primaryColor: color("--sunken"),
      primaryBorderColor: color("--border-strong"),
      primaryTextColor: color("--fg"),
      secondaryColor: color("--surface"),
      tertiaryColor: color("--bg"),
      lineColor: color("--fg-muted"),
      textColor: color("--fg"),
      noteBkgColor: color("--sunken"),
      noteTextColor: color("--fg"),
      fontFamily: getComputedStyle(document.body).fontFamily,
    };
  } finally {
    probe.remove();
  }
}

/**
 * Loads Mermaid in its strict mode, themed by the app's tokens. Labels are
 * SVG text: the sanitizer removes the HTML labels Mermaid would put in
 * `foreignObject`.
 */
async function loadMermaid(): Promise<Mermaid> {
  mermaidLoading ??= import("mermaid").then((module) => module.default);
  const mermaid = await mermaidLoading;
  const variables = themeVariables();
  const key = JSON.stringify(variables);
  if (key !== mermaidTheme) {
    mermaidTheme = key;
    mermaid.initialize({ startOnLoad: false, securityLevel: "strict", htmlLabels: false, theme: "base", themeVariables: variables });
  }
  return mermaid;
}

let diagramCount = 0;

/** A diagram's SVG, unsanitized: the caller inserts it through the diagram profile. */
export async function renderDiagram(source: string): Promise<string> {
  const mermaid = await loadMermaid();
  diagramCount += 1;
  const { svg } = await mermaid.render(`gk-diagram-${String(diagramCount)}`, source);
  return svg;
}

/** The theme key diagrams were last drawn with; a change means they are drawn again. */
export function diagramTheme(): string {
  return mermaidTheme;
}

let highlighterLoading: Promise<HighlighterCore> | null = null;
const loadedLanguages = new Set<string>();

/**
 * Shiki with the JavaScript regular-expression engine (no WebAssembly) and
 * a theme of CSS variables: colours come from the `--syn-token-*` names,
 * which the reader maps to the design's syntax tokens.
 */
async function loadHighlighter(): Promise<HighlighterCore> {
  highlighterLoading ??= Promise.all([import("shiki/core"), import("shiki/engine/javascript")]).then(([core, engine]) => {
    const theme = core.createCssVariablesTheme({ name: "gk", variablePrefix: "--syn-", variableDefaults: {}, fontStyle: true });
    return core.createHighlighterCore({
      themes: [{ ...theme, tokenColors: (theme.tokenColors ?? []).map((rule) => ({ ...rule, scope: safeScopes(rule.scope) })) }],
      langs: [],
      engine: engine.createJavaScriptRegexEngine({ forgiving: true }),
    });
  });
  return highlighterLoading;
}

/** Names every object inherits; the app freezes `Object.prototype`, so they can't be set as plain keys. */
const INHERITED = new Set(Object.getOwnPropertyNames(Object.prototype));

/**
 * A theme rule's scopes without those that have an inherited name as a part
 * ("meta.instance.constructor"): the highlighter files rules under each part
 * in a plain object, which fails for such names.
 */
function safeScopes(scope: string | string[] | undefined): string[] {
  const scopes = scope === undefined ? [] : Array.isArray(scope) ? scope : scope.split(",");
  return scopes.map((each) => each.trim()).filter((each) => each !== "" && !each.split(/[\s.]+/).some((part) => INHERITED.has(part)));
}

/** Highlights code as HTML, or answers null for a language Shiki doesn't know. */
export async function highlight(code: string, language: string): Promise<string | null> {
  const { bundledLanguages } = await import("shiki/langs");
  const load = (bundledLanguages as Record<string, LanguageInput | undefined>)[language];
  if (!load) {
    return null;
  }
  const highlighter = await loadHighlighter();
  if (!loadedLanguages.has(language)) {
    await highlighter.loadLanguage(load);
    loadedLanguages.add(language);
  }
  return highlighter.codeToHtml(code, { lang: language, theme: "gk" });
}
