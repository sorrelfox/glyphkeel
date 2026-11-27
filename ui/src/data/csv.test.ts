// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { csvCell } from "./csv";

describe("CSV cells", () => {
  it("defuses every start a spreadsheet runs as a formula", () => {
    expect(csvCell("=SUM(A1)")).toBe("'=SUM(A1)");
    expect(csvCell("+1")).toBe("'+1");
    expect(csvCell("-1+2")).toBe("'-1+2");
    expect(csvCell("@cmd")).toBe("'@cmd");
    expect(csvCell("\t=1")).toBe("'\t=1");
    expect(csvCell("\r=1")).toBe("\"'\r=1\"");
  });

  it("keeps numbers and plain text, and quotes what needs it", () => {
    expect(csvCell("90")).toBe("90");
    expect(csvCell("a=b")).toBe("a=b");
    expect(csvCell("")).toBe("");
    expect(csvCell('Say "hi", then go')).toBe('"Say ""hi"", then go"');
    expect(csvCell('=1,"x"')).toBe("\"'=1,\"\"x\"\"\"");
  });
});
