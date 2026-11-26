// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { MERMAID_SECURE } from "./libraries";

describe("diagram settings", () => {
  it("a diagram's own settings can't bring raw CSS or switch the strict mode off", async () => {
    const { default: mermaid } = await import("mermaid");
    mermaid.initialize({ startOnLoad: false, securityLevel: "strict", htmlLabels: false, secure: MERMAID_SECURE });
    const hostile =
      '%%{init: {"securityLevel": "loose", "htmlLabels": true, "themeCSS": "} body { display: none } x {", "fontFamily": "x} body{color:red", "themeVariables": {"primaryColor": "red"}}}%%\ngraph TD\n  A --> B';
    await mermaid.parse(hostile);
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- the only way to read the settings a diagram ends up with
    const config = mermaid.mermaidAPI.getConfig();
    expect(config.securityLevel).toBe("strict");
    expect(config.htmlLabels).toBe(false);
    expect(config.themeCSS ?? "").not.toContain("display");
    expect(config.fontFamily ?? "").not.toContain("color");
    const variables = config.themeVariables as Record<string, unknown> | undefined;
    expect(variables?.primaryColor).not.toBe("red");
  });
});
