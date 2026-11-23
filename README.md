# Glyphkeel

Glyphkeel is a task manager for your desktop that keeps everything on your
own computer. It has boards, lists and a wiki written in Markdown, a command
line, and an MCP server so that coding agents can read and update your tasks
through the same operations you use. It never goes online.

**Status:** early development. There are no releases yet, and nothing here is
ready for daily use. Linux comes first; Windows follows after 1.0.

## Building from source

See [CONTRIBUTING.md](CONTRIBUTING.md) for the tools you need. In short:

```sh
corepack enable                 # provides the pinned pnpm
pnpm install --frozen-lockfile  # the UI's build tools
pnpm build                      # the UI
cargo build -p gk-app --features tauri/custom-protocol
./target/debug/glyphkeel-app    # the desktop app
cargo run -p gk-cli -- --help   # the command line, `glyphkeel`
```

## Licence

Glyphkeel is free software, licensed under the
[GNU General Public License, version 3 or later](LICENSE).
