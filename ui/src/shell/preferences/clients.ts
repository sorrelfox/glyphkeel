// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * How common MCP clients are told to start `glyphkeel mcp`. Client formats
 * change, so they live here, apart from the page, and each release checks
 * them again. The program is filled in from where the app finds it, as an
 * absolute path.
 */

import { type Platform } from "../../platform";

/** What a snippet is written in, for its label. */
export type SnippetFormat = "shell" | "json" | "toml";

export interface McpClient {
  id: string;
  /** The client's own name; product names are not translated. */
  name: string;
  format: SnippetFormat;
  /** The file it goes in, or `undefined` for a command to run. */
  file?: string;
  /** The same file as Windows names it, where that differs. */
  windowsFile?: string;
  /** The text to copy, for the command that starts the tool (program and leading arguments). */
  snippet: (command: readonly string[]) => string;
}

/** The program and the arguments that serve MCP, project-scoped when a key is given. */
function server(command: readonly string[], project?: string): { command: string; args: string[] } {
  const [program = "glyphkeel", ...leading] = command;
  return { command: program, args: [...leading, "mcp", ...(project === undefined ? [] : ["--project", project])] };
}

/** Quotes a word for a POSIX shell when it needs it. */
export function shellWord(word: string): string {
  return /^[\w@%+=:,./-]+$/u.test(word) ? word : `'${word.replaceAll("'", `'\\''`)}'`;
}

/** A TOML basic string. */
function tomlString(text: string): string {
  return JSON.stringify(text);
}

/** `{"mcpServers": {"glyphkeel": …}}`, the layout most clients read. */
function mcpServersJson(command: readonly string[], project?: string): string {
  return JSON.stringify({ mcpServers: { glyphkeel: server(command, project) } }, null, 2);
}

export const MCP_CLIENTS: readonly McpClient[] = [
  {
    id: "claude-code",
    name: "Claude Code",
    format: "shell",
    snippet: (command) => {
      const { command: program, args } = server(command);
      return ["claude", "mcp", "add", "--scope", "user", "glyphkeel", "--", program, ...args].map(shellWord).join(" ");
    },
  },
  {
    id: "claude-desktop",
    name: "Claude Desktop",
    format: "json",
    file: "claude_desktop_config.json",
    snippet: (command) => mcpServersJson(command),
  },
  {
    id: "codex",
    name: "Codex",
    format: "toml",
    file: "~/.codex/config.toml",
    windowsFile: "%USERPROFILE%\\.codex\\config.toml",
    snippet: (command) => {
      const { command: program, args } = server(command);
      return [`[mcp_servers.glyphkeel]`, `command = ${tomlString(program)}`, `args = [${args.map(tomlString).join(", ")}]`].join("\n");
    },
  },
  {
    id: "cursor",
    name: "Cursor",
    format: "json",
    file: "~/.cursor/mcp.json",
    windowsFile: "%USERPROFILE%\\.cursor\\mcp.json",
    snippet: (command) => mcpServersJson(command),
  },
  {
    id: "vscode",
    name: "VS Code",
    format: "json",
    file: ".vscode/mcp.json",
    snippet: (command) => JSON.stringify({ servers: { glyphkeel: { type: "stdio", ...server(command) } } }, null, 2),
  },
];

/** The file a client's set-up goes in, as the platform names it; `undefined` for a command to run. */
export function clientFile(client: McpClient, on: Platform): string | undefined {
  return on === "windows" ? (client.windowsFile ?? client.file) : client.file;
}

/** A repository's `.mcp.json` that limits the agent to one project and its sub-projects. */
export function projectSnippet(command: readonly string[], project: string): string {
  return mcpServersJson(command, project);
}
