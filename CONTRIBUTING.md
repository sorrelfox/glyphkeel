# Contributing to Glyphkeel

Thanks for your interest in Glyphkeel. The project is in early development, so
the code base changes quickly. Issues are for confirmed bugs and accepted
features. Security problems must be reported privately, as described in
[SECURITY.md](SECURITY.md).

## What you need

- **Rust** through [rustup](https://rustup.rs/). The toolchain (version,
  `rustfmt` and `clippy`) is pinned in `rust-toolchain.toml`, and rustup
  installs it on first use.
- **Node** 24 (the exact version is in `.node-version`) with Corepack, which
  provides the pnpm version pinned in `package.json`: run `corepack enable`
  once.
- **The desktop app's system libraries** on Linux: WebKitGTK 4.1, GTK 3 and
  libsoup 3, with their development files.
  - Debian and Ubuntu: `sudo apt install build-essential pkg-config
    libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev`
  - Fedora: `sudo dnf install gcc gcc-c++ pkgconf-pkg-config
    webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel`
- **Podman** (or Docker) for the full check suites.

## Building and running

```sh
pnpm install --frozen-lockfile   # the UI's build tools, exactly as locked
pnpm build                       # the UI, into ui/dist
cargo build -p gk-app --features tauri/custom-protocol
./target/debug/glyphkeel-app     # the app, with the UI embedded

pnpm tauri dev                   # the app against the UI's development server
cargo run -p gk-cli -- --help    # the command line, `glyphkeel`
```

Without the `tauri/custom-protocol` feature, a debug build of the app loads the
UI from the development server at `http://127.0.0.1:5310` instead of the
embedded files (`pnpm dev` starts it; `pnpm tauri dev` starts both).

`cargo xtask` is the project's task runner (the `xtask/` crate):

| Command | What it does |
|---|---|
| `cargo xtask ci` | Runs the full check suites in containers, on every supported distribution; see [Checks](#checks) |
| `cargo xtask check-deps` | Checks the dependency rules below |
| `cargo xtask check-headers` | Checks that every source file starts with the licence header |
| `cargo xtask notices` | Writes `THIRD-PARTY-NOTICES.txt` and checks every shipped licence; `--check` fails when the file is out of date |

## Repository layout

```
crates/     the Rust crates: the engine's libraries, the command line, the app
ui/         the React UI: src/, its styles and tests
xtask/      the task runner (cargo xtask …)
ci/         the container images for the check suites
```

| Crate | Role |
|---|---|
| `gk-core` | Value types, validation and derived values. No I/O |
| `gk-query` | The task query language: parser, printer, SQL generation |
| `gk-markdown` | Markdown: one parser and renderer, sanitized HTML, checklists, references |
| `gk-store` | SQLite schema, migrations, repositories, full-text search |
| `gk-api` | The operation catalog: input and output types, metadata, errors. No logic |
| `gk-engine` | Workspaces, operations, history, change events, files, backups, jobs |
| `gk-mcp` | The MCP server, built from the catalog |
| `gk-cli` | The `glyphkeel` command, which also serves MCP |
| `gk-app` | The `glyphkeel-app` desktop app (Tauri): windows, commands, the web view |

**Dependency rules**, checked by `cargo xtask check-deps`:

- Only `gk-app` links Tauri, GTK and the web view. The command line and every
  library build and run without a display.
- Neither shipped program links an HTTP client or a TLS stack: Glyphkeel never
  goes online. `deny.toml` bans them as well.
- The app reaches the engine only through `gk-engine` and `gk-api`, and the UI
  reaches Rust only through the IPC client generated from the catalog.

## Code requirements

### Licence header

Every source file starts with the licence header, in its own comment syntax:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors
```

JSON files can't carry comments and are exempt.

### Rust

- Formatted with `rustfmt` (`cargo fmt --all`).
- `cargo clippy --workspace --all-targets -- -D warnings` passes. Lint levels
  live in the root `Cargo.toml`. If a lint must be silenced, use a narrowly
  scoped `#[expect(...)]` with a comment that says why.
- No `unsafe`: `unsafe_code` is forbidden workspace-wide, so no crate or
  module can allow it.
- No `unwrap()` outside tests; return errors or document the invariant with
  `expect("…")`.

### UI

- **Design tokens only.** Colours, radii, font sizes and control heights come
  from `ui/src/styles/tokens.css`; stylelint rejects literal values elsewhere.
  `tokens.css` and `components.css` are the design's own stylesheets and keep
  their values and class names.
- **No HTML insertion** outside the reader component (`innerHTML`,
  `dangerouslySetInnerHTML` and the like are lint errors).
- **No network APIs** (`fetch`, `WebSocket`, `EventSource`…): the page talks to
  the app over Tauri's IPC only.
- TypeScript runs in strict mode; `pnpm typecheck` and `pnpm lint` pass.

## Checks

While working, run only the tests that your change can affect:

```sh
cargo test -p gk-query parse::              # one crate, one module
pnpm --filter ui exec vitest related src/shell/HeaderBar.tsx --run
pnpm --filter ui exec playwright test tests/flow/smoke.spec.ts
```

Before every push, run the static checks:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm lint && pnpm typecheck
```

The full suites run before a pre-release, in containers of each supported
distribution (Ubuntu 22.04 and 24.04, Debian 12, Fedora), each with its own
WebKitGTK:

```sh
cargo xtask ci --list                              # suites and distributions
cargo xtask ci                                     # everything, everywhere
cargo xtask ci --distro ubuntu-24.04 --suite static,rust
cargo xtask ci --here --suite ui                   # on this machine, no container
```

Podman is used when it is installed, Docker otherwise. The images, and the
volumes that cache builds between runs, are all named `glyphkeel-ci…`.
`cargo xtask ci --distro ubuntu-22.04 --suite package` builds the deb and rpm
packages the way a release does: both programs, the menu entry, the shell
completions (bash, zsh, fish) and the man page. A rebuild of the same version
takes `--package-release N` for the rpm's release number.

The checks of what ships run from the host, on the packages built in the
Ubuntu 22.04 image and installed in a clean container of each distribution
with no network:

```sh
cargo xtask ci --suite packages        # files, both programs, a command-line session
cargo xtask ci --suite offline         # never online: every process's connections traced
cargo xtask ci --suite perf --distro fedora   # timings and memory with data set L
cargo xtask ci --suite pseudo --distro ubuntu-22.04   # screenshots in a pseudo-locale
cargo xtask ci --suite flatpak         # flatpak-builder on this machine (needs the source lists)
cargo xtask check-bundle               # the built UI names no remote address
```

Their results land in `target/ci-cache/results/<distro>/<suite>/` (the
performance table is `perf.md`). `perf` only reports by default, even when a
measure couldn't be taken; `--perf-enforce` fails it on a result over budget
or a missing measure. Set `GLYPHKEEL_CI_PREFIX` to give the
images and containers of a second checkout their own names.

The Windows build is checked from Linux too, in a container that cross-compiles
for `x86_64-pc-windows-msvc` and runs the tests under Wine, without a display:

```sh
cargo xtask ci --distro windows-cross                      # every Windows suite
cargo xtask ci --distro windows-cross --suite build,clippy,test
cargo xtask ci --distro windows-cross --suite smoke,nsis,install
cargo xtask ci --distro windows-cross --release-dir DIR    # also copy the installer to DIR/<version>/windows/
```

Its suites are `build`, `clippy` (with `-D warnings` for the Windows
target), `test` (every crate's tests under Wine), `smoke` (a command-line and
MCP session), `nsis` (the installer, from the `dist` profile, unsigned) and
`install` (a silent install and uninstall under Wine, which checks the
folders, the `PATH` entry, the `glyphkeel://` handler and that uninstalling
keeps the data). `windows-cross` is never part of the default run and is
given on its own. The first `nsis` run needs the network once: the Tauri
bundler downloads its NSIS helpers and the WebView2 bootstrapper. Wine is
not Windows: NTFS behaviour, WebView2, SmartScreen and the GUI are checked by
hand on a real Windows 10 and 11.

GitHub Actions run only when a pre-release tag (`v0.1.0-rc.1`) is pushed: the
same suites in the same containers, a Windows build with the unit tests and
the NSIS installer, the packages, an SBOM, checksums and build-provenance attestations, published as a
GitHub pre-release. Pushes and pull requests start no runs, so run the checks
above yourself. A final release (`v0.1.0`, tagged on the same commit as its
last pre-release) builds nothing: it republishes that pre-release's files.
Because of that, `Cargo.toml` already carries the final version while the
pre-releases are made. [docs/releasing.md](docs/releasing.md) has the steps,
the Flatpak repository on GitHub Pages and its one-time set-up.

## Dependencies

A new dependency, Rust or JavaScript, needs a reason, written in the pull
request and next to the dependency (a comment in `Cargo.toml`; the pull
request for `package.json`, which has no comments).

- **Licences:** shipped code must be compatible with GPL-3.0-or-later. The
  allowlist is in `licences.toml`, and `cargo xtask notices --check` applies
  it to every shipped crate, npm package and bundled component; `cargo deny
  check` applies its copy in `deny.toml` to the crates, and the two lists
  must stay the same. After a dependency changes, run `cargo xtask notices`
  and commit the new `THIRD-PARTY-NOTICES.txt`. Development-only tools may
  use any open-source licence.
- **Copied code:** code or data taken from another project goes under
  `third_party/<name>/`, with a `PROVENANCE.md` (where it came from, which
  version, what was changed) and the project's licence text. Test data taken
  from elsewhere keeps the same two files next to it.
- **npm supply chain** (`pnpm-workspace.yaml`): versions are exact, the
  lockfile is committed and installs are frozen, versions younger than seven
  days are refused, and install scripts never run unless allowed by name.

## Commits, branches and pull requests

- **Branches:** all work goes to `dev`; `main` holds releases only. Open pull
  requests against `dev`.
- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/),
  for example `feat(board): drag cards between lanes` or
  `fix(store): keep ranks unique after an import`. The changelog is generated
  from them (`cliff.toml`).
- **Sign-off:** every commit carries a `Signed-off-by` line (`git commit -s`),
  certifying the [Developer Certificate of Origin](https://developercertificate.org/).
  No contributor licence agreement is needed.
- Describe what a change does in words; don't refer to private notes or
  tracker numbers that readers can't see.

## Licence

By contributing, you agree that your contributions are licensed under the
GNU General Public License, version 3 or later, like the rest of Glyphkeel.
