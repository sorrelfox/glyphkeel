# Security policy

## Supported versions

| Phase | Versions that receive security fixes |
|---|---|
| Before 1.0 (now) | The latest release only |
| From 1.0 | The latest minor release; the previous minor release also receives fixes for critical issues |

Glyphkeel is in early development and has no releases yet.

## Reporting a vulnerability

Please **do not** open a public issue, discussion or pull request for a
security problem.

Report it privately through GitHub private vulnerability reporting:
<https://github.com/sorrelfox/glyphkeel/security/advisories/new>
(the **Report a vulnerability** button on the repository's *Security* tab).

Include, where you can:

- the affected version (`glyphkeel --version`) and operating system;
- a description of the issue and its impact;
- a minimal workspace, file or set of steps that reproduces it;
- any suggested fix or mitigation.

## What to expect

| Step | Target |
|---|---|
| Acknowledgement of your report | Within 7 days |
| Assessment and a fix or mitigation plan | Within 30 days |
| Disclosure | Through a GitHub Security Advisory once a fix is available, with credit to the reporter unless you prefer otherwise |

If a report turns out not to be a vulnerability, we will explain why and may
ask you to file it as a regular issue.

## Scope

Glyphkeel keeps your data on your computer and never goes online. Task text,
comments, wiki pages and attachments may come from untrusted sources: pasted
in, imported, or written by an agent through MCP. In scope are, among others:

- Markdown, attachments or imported archives that run script in the app's web
  view, get past its sanitizer, or load anything from the network;
- a page in the app's web view reaching commands, files or windows it should
  not, or navigating away from the app;
- an MCP client or the command line doing more than the operations and
  permissions it was given, or bypassing a confirmation;
- input that crashes, hangs or exhausts memory in the Markdown renderer, the
  query language or the importer;
- any network connection made by the app, the command line or the MCP server.

Out of scope: problems that need an attacker who already controls the user's
account or machine, and vulnerabilities in third-party dependencies that do
not affect Glyphkeel (please report those upstream; we track advisories with
`cargo-deny` and `pnpm audit`).
