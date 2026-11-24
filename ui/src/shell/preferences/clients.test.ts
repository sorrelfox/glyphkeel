// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { MCP_CLIENTS, projectSnippet, shellWord } from "./clients";

const snippet = (id: string, command: string[]) => MCP_CLIENTS.find((client) => client.id === id)?.snippet(command) ?? "";

describe("MCP client set-up", () => {
  it("fills in the absolute program", () => {
    expect(snippet("claude-code", ["/opt/Glyph Keel/glyphkeel"])).toBe("claude mcp add --scope user glyphkeel -- '/opt/Glyph Keel/glyphkeel' mcp");
    expect(JSON.parse(snippet("cursor", ["/usr/bin/glyphkeel"]))).toEqual({ mcpServers: { glyphkeel: { command: "/usr/bin/glyphkeel", args: ["mcp"] } } });
    expect(JSON.parse(snippet("vscode", ["/usr/bin/glyphkeel"]))).toEqual({ servers: { glyphkeel: { type: "stdio", command: "/usr/bin/glyphkeel", args: ["mcp"] } } });
    expect(snippet("codex", ["/usr/bin/glyphkeel"])).toBe('[mcp_servers.glyphkeel]\ncommand = "/usr/bin/glyphkeel"\nargs = ["mcp"]');
  });

  it("runs a Flatpak's tool through flatpak, and scopes a repository to one project", () => {
    const flatpak = ["flatpak", "run", "--command=glyphkeel", "io.github.sorrelfox.Glyphkeel"];
    expect(JSON.parse(projectSnippet(flatpak, "TM"))).toEqual({
      mcpServers: { glyphkeel: { command: "flatpak", args: ["run", "--command=glyphkeel", "io.github.sorrelfox.Glyphkeel", "mcp", "--project", "TM"] } },
    });
  });

  it("quotes shell words only when needed", () => {
    expect(shellWord("glyphkeel")).toBe("glyphkeel");
    expect(shellWord("it's")).toBe(`'it'\\''s'`);
  });
});
