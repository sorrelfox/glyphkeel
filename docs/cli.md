<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The command line

`glyphkeel` does everything the app does, from a terminal or a script. It works
on the same workspace as the app, also while the app is open, and every change
it makes shows in the app at once and is recorded in the history.

```text
glyphkeel <area> <verb> [arguments] [--options]
```

`glyphkeel --help` lists the areas, and `glyphkeel <area> <verb> --help` the
options of a command. [The command reference](reference/cli.md) lists every
command.

The deb and rpm packages and the Windows installer put `glyphkeel` on your
`PATH`. In the Flatpak it is
`flatpak run --command=glyphkeel io.github.sorrelfox.Glyphkeel`. Preferences ›
Agents & CLI says whether a terminal finds it, and can install a link to it.

## Common workflows

### Projects and tasks

```sh
glyphkeel project create --name "Home" --key HOME
glyphkeel task create --project HOME --title "Renew the domain" --due-date fri --priority high
glyphkeel task list --query "project:HOME is:open sort:due"
glyphkeel task show HOME-1
glyphkeel task move HOME-1 --status "In progress"
glyphkeel task edit HOME-1 --add-labels admin --estimate 1h
glyphkeel comment add HOME-1 --body "Compared three registrars."
glyphkeel task link HOME-2 --help          # the kinds of link and their options
```

Dates take `2026-12-31` or words such as `today`, `fri`, `next mon`, `+3d` or
`in 2 weeks`. Durations take `90m`, `1.5h` or `1h 30m`.

### Today and search

```sh
glyphkeel today
glyphkeel board --all --query "priority>=high"   # My work: every project's board by category
glyphkeel search "domain renewal"
glyphkeel task list --query "is:overdue"
```

[The query language](query-language.md) explains `--query`.

### Time

```sh
glyphkeel timer start HOME-1
glyphkeel timer status
glyphkeel timer stop                        # logs the time on the task
glyphkeel time log HOME-1 --duration 45m --note "Compared prices"
glyphkeel time report --from 2026-11-01 --to 2026-11-30 --group-by project
```

### Many tasks at once

`task bulk` makes the same change to the tasks a query or a list of keys picks.
It only shows what would change unless you pass `--dry-run false`, and the
change is one step for undo and revert:

```sh
glyphkeel task bulk --query "project:HOME is:open" --changes '{"priority":"high"}'
glyphkeel task bulk --query "project:HOME is:open" --changes '{"priority":"high"}' --dry-run false
```

### Undo and history

```sh
glyphkeel activity --limit 20
glyphkeel revert <group>                     # the group ID from the activity log
glyphkeel trash list
glyphkeel trash restore <item>
```

### Backups and exports

```sh
glyphkeel backup create
glyphkeel backup list
glyphkeel export --path home.zip --projects HOME
glyphkeel import --dry-run home.zip
```

See [Backup and restore](backup-and-restore.md) and
[Export and import](guide/export-import.md).

### Opening the app

```sh
glyphkeel                     # starts the desktop app
glyphkeel open TM-41          # opens a task in the app
glyphkeel --quick-add         # opens only the quick-add window
```

## Output and `--json`

By default a command prints its result as text: a table for lists, and fields
for one item. With `--json` it prints exactly the operation's output as JSON,
described in [the schemas](reference/catalog.schema.json). That is the form for
scripts:

```sh
glyphkeel task list --query "is:open due<today" --json | jq -r '.items[].key'
```

Lists come a page at a time (`--limit` sets the size). When there are more, the
text output says `next page: --cursor …`, and the JSON has a `next_cursor`.

Other global options:

| Option | What it does |
|---|---|
| `--workspace NAME-OR-PATH`, `-w` | Work in another workspace |
| `--json` | Print the output as JSON |
| `--quiet`, `-q` | Print nothing on success; errors still go to standard error |
| `--no-color` | No colours (also with `NO_COLOR` set) |
| `--tz ZONE` | The time zone of "today" for this command, such as `Europe/Berlin` |
| `--lang LANG` | The language of help, messages and output for people, such as `de` or `pt-BR` |

**Language.** Help, error messages and output for people are in the language
`--lang` names, else the one in `GLYPHKEEL_LANG`, else the language chosen in
the app's Preferences, else the system's (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`
and `LANG` on Linux, the display language on Windows), else English. Text not
yet translated shows in English. Numbers and dates in the output for people are
written the way that language writes them. Machine text stays English, so that
scripts work everywhere: the `--json` output (an error's `message` comes with
its `message_id` and `args`), the names of commands and options, and everything
`glyphkeel mcp` gives agents.

**Input as JSON.** Every command takes `--input` with the whole input as a JSON
object, or `--input -` to read it from standard input. Options given as well
take precedence:

```sh
echo '{"project":"HOME","title":"Book the hall","priority":"low"}' | glyphkeel task create --input -
```

**Confirmations.** A command that can't be undone, such as `trash purge` or
`backup restore`, asks first. `--yes` confirms in advance.

**Exit codes.** `0` success, `1` usage error or invalid input, `2` not found,
`3` change rejected, `4` workspace or storage problem. Errors go to standard
error, with a hint where there is one.

## Workspaces

A command works in the workspace that `--workspace` names, else the one
`GLYPHKEEL_WORKSPACE` names, else the one last opened in the app or with
`glyphkeel workspace open`, else *Personal*.

```sh
glyphkeel workspace                  # the open workspace, its versions and settings
glyphkeel workspace list
glyphkeel workspace create Work
glyphkeel workspace open Work
glyphkeel -w Home task list
glyphkeel workspace encrypt          # encrypt it, or give it a new passphrase
glyphkeel workspace decrypt
```

See [Workspaces](guide/workspaces.md).

An encrypted workspace needs its passphrase: the command asks for it on the
terminal, or reads it from `GLYPHKEEL_PASSPHRASE` when there is no terminal.
`workspace encrypt` asks for the new passphrase twice, or reads it as one
line from standard input. See
[Encryption and the app lock](guide/encryption-and-lock.md).

## Shell completions and the man page

The deb and rpm packages install completions for bash, zsh and fish, and the
man page (`man glyphkeel`). Otherwise, print them yourself:

```sh
glyphkeel completions bash > ~/.local/share/bash-completion/completions/glyphkeel
glyphkeel completions zsh > ~/.zfunc/_glyphkeel        # with ~/.zfunc in your fpath
glyphkeel completions fish > ~/.config/fish/completions/glyphkeel.fish
glyphkeel man > glyphkeel.1
```

In PowerShell, add this line to your profile:

```powershell
glyphkeel completions powershell | Out-String | Invoke-Expression
```

`completions` also knows `elvish`.

## Agents

`glyphkeel mcp` runs the MCP server for agents, and `glyphkeel mcp --http` serves
them on a local address instead. See
[Working with AI agents](mcp.md).
