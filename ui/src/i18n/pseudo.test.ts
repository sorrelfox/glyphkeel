// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { createBundle, MESSAGES } from "./i18n";
import { PSEUDO_LOCALE, pseudoText } from "./pseudo";

describe("the pseudo-locale", () => {
  it("accents letters and lengthens text by doubling vowels", () => {
    expect(pseudoText("Today")).toBe("Ţóóđááýý");
    expect(pseudoText("Ctrl + 1")).toBe("Çţŕĺ + 1");
  });

  it("brackets every message and keeps placeables and plurals working", () => {
    const bundle = createBundle(PSEUDO_LOCALE);
    const format = (id: string, args?: Record<string, string | number>) => {
      const message = bundle.getMessage(id);
      return message?.value ? bundle.formatPattern(message.value, args).replace(/[⁨⁩]/g, "") : "";
    };
    expect(format("preferences-title")).toBe("[Þŕééƒééŕééñçééš]");
    expect(format("agents-sessions-more", { count: 1 })).toBe("[Óóñéé óóĺđééŕ šééššííóóñ ííš ñóóţ šĥóóŵñ.]");
    expect(format("agents-sessions-more", { count: 3 })).toContain("3");
  });

  it("is English underneath", () => {
    expect(Object.keys(MESSAGES)).not.toContain(PSEUDO_LOCALE);
  });
});
