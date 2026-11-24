// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type FluentResource } from "@fluent/bundle";

/**
 * The pseudo-locale: English with every letter accented, every vowel
 * doubled (about a third longer, like many translations) and each message
 * in brackets, so that text the messages don't cover, clipped text and
 * joined strings stand out on screenshots. Placeables (names, numbers,
 * dates) stay as they are. Only development builds offer it.
 */
export const PSEUDO_LOCALE = "en-XA";

const ACCENTED: Readonly<Record<string, string>> = {
  a: "á", b: "ƀ", c: "ç", d: "đ", e: "é", f: "ƒ", g: "ĝ", h: "ĥ", i: "í", j: "ĵ", k: "ķ", l: "ĺ", m: "ɱ",
  n: "ñ", o: "ó", p: "þ", q: "ǫ", r: "ŕ", s: "š", t: "ţ", u: "ú", v: "ṽ", w: "ŵ", x: "ẋ", y: "ý", z: "ž",
  A: "Á", B: "Ɓ", C: "Ç", D: "Đ", E: "É", F: "Ƒ", G: "Ĝ", H: "Ĥ", I: "Í", J: "Ĵ", K: "Ķ", L: "Ĺ", M: "Ṁ",
  N: "Ñ", O: "Ó", P: "Þ", Q: "Ǫ", R: "Ŕ", S: "Š", T: "Ţ", U: "Ú", V: "Ṽ", W: "Ŵ", X: "Ẋ", Y: "Ý", Z: "Ž",
};

const VOWELS = new Set("aeiouyAEIOUY");

/** One piece of message text, accented and lengthened. */
export function pseudoText(text: string): string {
  let out = "";
  for (const char of text) {
    const accented = ACCENTED[char] ?? char;
    out += VOWELS.has(char) ? accented + accented.toLowerCase() : accented;
  }
  return out;
}

type Pattern = string | PatternElement[];
type PatternElement = string | { type: string; variants?: { value: Pattern }[] };

function pseudoPattern(pattern: Pattern, outer: boolean): Pattern {
  const elements: PatternElement[] = typeof pattern === "string" ? [pattern] : pattern;
  const changed = elements.map((element): PatternElement => {
    if (typeof element === "string") {
      return pseudoText(element);
    }
    if (element.type === "select" && element.variants) {
      return { ...element, variants: element.variants.map((variant) => ({ ...variant, value: pseudoPattern(variant.value, false) })) };
    }
    return element;
  });
  return outer ? ["[", ...changed, "]"] : changed;
}

/** Turns the messages of `resource` into the pseudo-locale, in place. */
export function pseudoResource(resource: FluentResource): FluentResource {
  for (const entry of resource.body) {
    // Terms are parts of other messages: they get no brackets of their own.
    const isTerm = entry.id.startsWith("-");
    if (entry.value !== null) {
      (entry as { value: Pattern }).value = pseudoPattern(entry.value, !isTerm);
    }
    for (const [name, value] of Object.entries(entry.attributes)) {
      entry.attributes[name] = pseudoPattern(value, !isTerm) as typeof value;
    }
  }
  return resource;
}
