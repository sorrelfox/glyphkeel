// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { htmlToMarkdown } from "./turndown";

describe("pasted HTML becomes Markdown", () => {
  it("without scripts or styles", () => {
    expect(htmlToMarkdown("<p>hi</p><script>alert(1)</script><style>p{}</style><noscript>x</noscript>")).toBe("hi");
  });

  it("with one space after list markers", () => {
    expect(htmlToMarkdown("<ul><li>one</li><li>two<ul><li>inner</li></ul></li></ul>")).toBe("- one\n- two\n  - inner");
    expect(htmlToMarkdown('<ol start="3"><li>three</li><li>four</li></ol>')).toBe("3. three\n4. four");
  });

  it("without the bold wrapper of Google Docs, but with its styled bold and italic", () => {
    const docs =
      '<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:400">Plain </span>' +
      '<span style="font-weight:700">strong</span><span style="font-weight:400"> and </span><span style="font-style:italic">slanted</span></p></b>';
    expect(htmlToMarkdown(docs)).toBe("Plain **strong** and *slanted*");
  });

  it("keeps spaces outside the markers of styled spans", () => {
    expect(htmlToMarkdown('<p>a<span style="font-weight:bold"> b </span>c</p>')).toBe("a **b** c");
  });

  it("with spreadsheet cells as a table whose first row is the header", () => {
    const sheet = "<table><tbody><tr><td>Name</td><td>Size</td></tr><tr><td>a|b</td><td>2</td></tr><tr><td>c</td></tr></tbody></table>";
    expect(htmlToMarkdown(sheet)).toBe("| Name | Size |\n| --- | --- |\n| a\\|b | 2 |\n| c |  |");
  });

  it("keeps tables that have a header row as GFM tables", () => {
    const table = "<table><thead><tr><th>A</th><th>B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>";
    expect(htmlToMarkdown(table)).toBe("| A | B |\n| --- | --- |\n| 1 | 2 |");
  });

  it("with strikethrough as two tildes", () => {
    expect(htmlToMarkdown("<p><del>old</del> <s>gone</s> new</p>")).toBe("~~old~~ ~~gone~~ new");
  });

  it("with headings, emphasis, code and links in the usual form", () => {
    expect(htmlToMarkdown('<h2>Title</h2><p><em>a</em> <strong>b</strong> <code>c</code> <a href="https://example.com">d</a></p><pre><code>x = 1\n</code></pre>')).toBe(
      "## Title\n\n*a* **b** `c` [d](https://example.com)\n\n```\nx = 1\n```",
    );
  });

  it("runs nothing from hostile HTML", () => {
    const out = htmlToMarkdown('<img src=x onerror="window.pwned=1">hi<iframe src="javascript:alert(1)"></iframe>');
    expect((window as unknown as { pwned?: number }).pwned).toBeUndefined();
    expect(out).toBe("![](x)hi");
  });
});
