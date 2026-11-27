<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Security

Glyphkeel keeps your tasks, wiki and files in a folder on your own computer
and never connects to the network. The only thing that listens is the
optional local endpoint for agents, which is off unless you turn it on
([below](#the-local-http-endpoint-for-agents)). This page explains what it protects you
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
network port. The app (or `glyphkeel mcp --http`) can also serve them on a
local address; see the next section.

- **Profiles.** Every agent session has a profile: *read-only*, *standard*
  (changes and moving to the trash, nothing permanent) or *full*. You set the
  most any agent may have in Preferences; an agent that asks for more gets
  less. The engine checks the profile on every operation, whichever tool or
  prompt asked for it, so a tool can't do more than its profile allows.
- **Asking you.** When an agent's client can show you a question, an agent
  may ask to make one change its profile doesn't allow, and is made to ask
  before deleting a project, purging the trash or moving more tasks to the
  trash than one call may. The question comes from Glyphkeel, through the
  client ("Claude Code wants to delete the project Home lab with 42 tasks
  for good. Allow?"). Only what you allow runs, once, and the history notes
  that you confirmed it; declining, or not answering within five minutes,
  refuses the change. The answer is tied to that one call: a 2026-07-28
  client sends it back with a random, single-use handle that the server
  keeps, so an agent can't reuse or forge it. Operations only the app may
  run (backups, settings about agents) are never offered, asked or not. You
  can turn the questions off in Preferences › Agents & CLI; turning them on
  again asks you in a window of the desktop.
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
- **Files and agents.** Every path an agent gives (to attach a file, to
  import an archive, a CSV file or a folder of Markdown files, and where an
  export goes) is checked by the engine before the operation runs, whichever
  tool it came through. Agents may use only the workspace's own `exports`
  folder (a plain file name means a file there) and the folders you add in
  Preferences › Agents & CLI. Other absolute paths, `..`, hidden files and
  folders, Windows' reserved names and streams, and links that lead outside
  these folders are refused; the workspace's own folder and its backups stay
  off limits even inside a folder you added. The path is resolved once,
  following links, and the operation uses exactly the place that was
  checked. Only you can add folders, from the folder chooser; the setting is
  personal and applies to every agent on this computer. See
  [Files on disk](mcp.md#files-on-disk).

## The local HTTP endpoint for agents

For MCP clients that connect to an address instead of starting a program,
the app can serve agents over HTTP (Preferences › Agents & CLI › **Local
HTTP endpoint**), and so can `glyphkeel mcp --http` without the app.

- **Off by default.** Turning it on in the app asks you in a window of the
  desktop, so script in the page can't open it.
- **This computer only.** It listens on 127.0.0.1, never on another
  address, on the port you choose or a free one. It makes no connection of
  its own.
- **A token.** Every request must carry `Authorization: Bearer <token>`;
  anything else gets `401`. The token is 256 random bits, made by the app
  (or the first `glyphkeel mcp --http`) and kept in your personal settings
  file, which only your account may read. **New token** in Preferences makes
  another one and locks out every client with the old one.
- **No web pages.** A request must name the endpoint itself in `Host`
  (`127.0.0.1:<port>` or `localhost:<port>`), so a web site can't reach it by
  pointing a name of its own at your computer (DNS rebinding), and may carry
  no `Origin` but the endpoint's own, so a page open in your browser can't
  use it.
- **The same rules as over standard input and output.** The same profiles
  and cap, limits, project scope, file checks and questions; `/mcp` serves
  the standard profile and `/mcp/read-only` and `/mcp/full` those profiles.
  Each client's work is one agent session in the history.

Any program running as you that can read your settings file can read the
token too, as it could read the workspace itself.

## The command line

`glyphkeel` acts as you. It asks before anything permanent unless you pass
`--yes`, and it never asks an agent: agents use the MCP server and its
profiles.

## Encrypted workspaces

A workspace can be encrypted with a passphrase
([Encryption and the app lock](guide/encryption-and-lock.md)). It is off by
default; for the strongest protection at rest, use your system's disk
encryption, which covers everything.

- **How.** The database is encrypted with SQLCipher 4: AES-256 in CBC mode
  on each page, with an HMAC-SHA512 per page, so a changed page is noticed.
  The key comes from the passphrase with PBKDF2-HMAC-SHA512, 256 000
  rounds, and a random salt kept at the start of the file. These are
  SQLCipher's own defaults, so the file also opens in SQLCipher's tools with
  the passphrase. The write-ahead log is encrypted the same way. On Linux the
  cryptography is the system's OpenSSL.
- **What is encrypted:** the database (tasks, wiki pages, comments, history,
  settings) and the backups of an encrypted workspace.
- **What isn't:** attached files and their thumbnails, exports, the settings
  files, and copies other programs keep. Encrypting a workspace writes a new
  file and removes the old one, but the old blocks may stay on the disk,
  above all on SSDs; disk encryption covers them.
- **The passphrase is never stored.** Each program keeps it in memory while
  it has the workspace open, wiped when it ends. Glyphkeel doesn't use the
  system's keyring.
- **Several programs.** The app, commands and agent servers each need the
  passphrase. The app and commands ask for it; for scripts and agent servers
  there is the environment variable `GLYPHKEEL_PASSPHRASE`. Anything that
  can read that variable, or the file it is set in (such as an MCP client's
  configuration), can open the workspace; other programs running as you can
  often read a process's environment. Use it only where that file is as
  private as the workspace, and prefer typing the passphrase.
- **Changes need the workspace to themselves.** Encrypting, changing the
  passphrase and decrypting refuse while another program has the workspace
  open, make a backup first, and check the new file before it replaces the
  old one in one step.
- **No recovery.** Without the passphrase the data can't be read, by anyone.
- **Windows.** The Windows build can't encrypt yet; it opens plain
  workspaces only.

## The app lock

The app can lock after an idle time or on demand
([Encryption and the app lock](guide/encryption-and-lock.md#the-app-lock)).
While it is locked, the app refuses every request for data from its windows
(not only hides it), the windows drop what they had loaded, and reminder
notifications don't name the task. It unlocks with the encrypted workspace's
passphrase, or with a lock passphrase, kept only as a salted PBKDF2 check in
the app's settings. The account's password is not used. The lock is a
screen lock for the app: the workspace stays open, so it doesn't protect
against programs running as you.

## What Glyphkeel doesn't protect against

- **Your own account.** Anything that runs as you can read and change your
  workspace folder directly, and while a program has an encrypted workspace
  open, its key is in that program's memory. Attached files are never
  encrypted. Use your system's disk encryption.
- **Programs you open files with.** A document opened from an attachment is
  shown by your desktop's program, with that program's protections.
- **Files you put where agents may look.** Anything in the `exports` folder
  or in a folder you added can be attached and then read back by an agent
  with the standard or full profile. Add only folders meant for agents, not
  your home folder. A backup folder you chose yourself outside the data
  folder is not known to the check: don't add a folder around it.
- **Changes while a path is checked.** Another program running as you could
  swap a file for a link between the check and the read. That needs a
  program on your account already, which can read your files anyway.
- **Programs on your computer with the token.** While the HTTP endpoint is
  on, a program that has the token works as an agent. Turn the endpoint off
  when no client needs it, and make a new token if you think it leaked.
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
