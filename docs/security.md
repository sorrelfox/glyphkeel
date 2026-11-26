<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Security

Glyphkeel keeps your tasks, wiki and files in a folder on your own computer
and never connects to the network. This page explains what it protects you
against, what it doesn't, and how to report a problem.

## Reporting a problem

Please don't open a public issue for a security problem. Report it privately
through GitHub's private vulnerability reporting, as described in
[SECURITY.md](../SECURITY.md). It also says which versions receive fixes and
how quickly you can expect an answer.

## What Glyphkeel trusts and what it doesn't

Glyphkeel trusts **you**, the person signed in to the computer, and the
programs you run as yourself. It does **not** trust the *content* that
reaches it, wherever that comes from:

- text you paste, or that another program, a CSV file, an archive or a
  folder of Markdown files brings in;
- attached files, including images;
- links in that text;
- anything an AI agent writes through the MCP server, and the agent's own
  requests.

Content may be written to look harmless and still try to run script, reach
your files, open other programs or mislead an agent. The protections below
are built for that.

## The app's window

The app shows its interface in your system's web view (WebKitGTK on Linux,
WebView2 on Windows). The page in it is treated as untrusted: even if some
content managed to run script there, it should not get further.

- **No script from content.** Markdown is turned into HTML in one place, in
  Rust, and cleaned against a short list of allowed tags and attributes. The
  page checks it once more before showing it. Raw HTML in Markdown keeps
  only a few harmless tags (`details`, `kbd`, `sub`, `sup`, `mark`, `br`,
  links and attached images). Event handlers, styles, forms, frames, SVG and
  `javascript:` or `data:` addresses are removed.
- **Diagrams and formulas** are drawn by Mermaid in its strict mode and by
  KaTeX with untrusted commands off. A diagram can't switch the strict mode
  off or bring its own CSS. Their output is cleaned before it is shown.
- **A strict content security policy.** The page runs only the app's own
  bundled script: no inline script, no `eval`, no remote addresses at all.
  Images come only from the app itself, from attached files and from
  pictures the page draws itself (content can't name a `data:` address). Inline
  styles are allowed, because the diagram, formula and code-highlighting
  libraries need them.
- **The page can't leave the app.** It can't navigate to another address or
  open windows of its own. Web and mail links open in your browser or mail
  program, and only `http`, `https` and `mailto` addresses are handed over.
- **Only the app's own commands.** The page reaches nothing of the web
  view's general API: no shell, no file system, no network. Each kind of
  window gets only the commands it needs. A task or wiki window, which shows
  one item, can't manage workspaces, imports, backups, the command-line tool
  or start-up settings.
- **The page never names a file.** Attaching, importing, exporting and
  choosing the backup folder always go through the system's own file
  chooser, shown by the app. The page names the task, the project or a
  token for the chosen file, never a path, so it can't make the app read or
  write a file you didn't choose.
- **Confirmations come from the app, not the page.** Deleting for good,
  emptying the trash, restoring a backup and letting agents do more are
  confirmed in a native dialog that script in the page can't answer.
- **Web view features the app doesn't use are off:** web storage, offline
  caches, WebRTC, cameras and microphones, WebGL and DNS prefetching.

## Attached files

- Attached images are shown to the page only as images, through the app's
  own address scheme, with their type taken from their content and
  `nosniff`. An SVG shown that way can't run script. Other files are never
  shown in the page.
- **Opening a file** opens a copy in your desktop's default program.
  Programs, scripts, installers and web pages are never opened that way;
  save them and look at them first.
- Images are checked for size before they are shown or turned into
  thumbnails, so a "decompression bomb" can't exhaust memory.

## Imports

CSV files, archives and folders of Markdown files are read with limits on
their size, the number of entries and the length of every field.

- Every name in an archive or folder must be a plain relative path: no `..`,
  no absolute paths, no drive letters, no backslashes. Symbolic links are
  refused.
- Links and images in imported Markdown are resolved only against the files
  of the import: nothing outside it is read, whatever a link says.
- A dry run checks a whole import and reports what it would do, without
  changing anything.

## Agents and the MCP server

`glyphkeel mcp` serves AI agents over standard input and output. It opens no
network port.

- **Profiles.** Every agent session has a profile: *read-only*, *standard*
  (changes and moving to the trash, nothing permanent) or *full*. You set the
  most any agent may have in Preferences; an agent that asks for more gets
  less. The engine checks the profile on every operation, whichever tool or
  prompt asked for it, so a tool can't do more than its profile allows.
- **A project scope.** An agent can be limited to one project and its
  sub-projects.
- **Limits.** Agents may make only a limited number of changes a minute, and
  messages are limited in size, so a runaway loop can't flood the workspace
  or exhaust memory.
- **Everything is recorded** under the agent's session and can be reverted.
- **Content is data.** The server tells agents that text in tasks, comments,
  wiki pages and files is user data and never instructions. This can't stop
  a determined prompt injection on its own: give an agent no more than its
  task needs, and prefer the read-only profile for agents that only need to
  read.
- **Files and agents.** Agents export and import only by plain names in the
  workspace's own `exports` folder. Attaching a file by its path is the one
  exception, because agents are often asked to attach a file they just
  wrote: an agent with the standard profile can attach any file you can
  read. Only connect agents you trust with your files, or give them the
  read-only profile.

## The command line

`glyphkeel` acts as you. It asks before anything permanent unless you pass
`--yes`, and it never asks an agent: agents use the MCP server and its
profiles.

## What Glyphkeel doesn't protect against

- **Your own account.** Anything that runs as you can read and change your
  workspace folder directly. Glyphkeel doesn't encrypt its data; use your
  system's disk encryption.
- **Programs you open files with.** A document opened from an attachment is
  shown by your desktop's program, with that program's protections.
- **Agents you allow to do more.** An agent with the standard or full profile
  can change or delete what its profile allows, within its limits. Every
  change can be reverted from the history, and backups are made every day.
- **Unsigned packages** before version 1.0 on Windows: check that you
  downloaded the installer from the project's own releases page.

## How it is checked

- A test set of known cross-site scripting payloads runs against the
  Markdown renderer and against the page's own check of its HTML.
- Tests keep each kind of window to its own commands, and check that every
  operation input that could name a file is refused from the page.
- The Markdown renderer, the query language, the CSV, archive and wiki
  imports and the MCP server's message handling are fuzzed.
- The Rust crates are checked with `cargo-deny` for known vulnerabilities,
  licences and sources, and the interface's shipped npm packages with
  `pnpm audit`.
