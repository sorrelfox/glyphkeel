# gk-seed: test data

A development tool. It writes a deterministic data set as an **unpacked export
archive**: the folder layout and records of the app's export, so the engine can load
it through its ordinary archive import. The same set and seed always give
byte-identical files.

```sh
cargo run -p gk-seed -- --set S --seed 1 --out /tmp/glyphkeel-s
cargo run --release -p gk-seed -- --set L --seed 1 --out /tmp/glyphkeel-l
cargo run -p gk-seed -- --set hostile --out /tmp/glyphkeel-hostile
```

`--out` must not exist yet or be empty. `--seed` defaults to 1.

## Data sets

| Set | For | Contents |
|---|---|---|
| `S` | function tests | 3 projects, one of them with 2 sub-projects (5 in all, one archived); 200 tasks; 500 comments; 50 attached files; 10 wiki pages; about 45 tasks with dates; dependencies, other relations, subtasks, time entries, claims by agent sessions, a few tasks in the trash |
| `L` | performance tests | 1 project; 20,000 tasks, exactly 1,000 of them open; 50,000 comments; 2,000 files; 500 tasks with start and due dates; 300 dependencies (`blocks`); 300 wiki pages; 5,000 time entries |
| `hostile` | renderer, sanitizer and web-view tests | 1 project (`HOST`), one task per hostile item, hostile file names and SVG, a wiki page with a hostile title; `fixtures.json` says what each item exercises and what must hold |

Descriptions are Markdown with checklists, task keys, wiki links, callouts, formulas,
Mermaid diagrams, code and tables. Titles include Cyrillic, Chinese, Japanese, Greek,
Hebrew, Arabic and emoji. Every name is synthetic: the person at the computer is "Me",
agent sessions are called "Sample agent …", and links go to `example.invalid`.

The hostile items stay within the engine's limits (titles up to 500 characters,
Markdown up to 1 MB), so they can be imported and then rendered.

## Layout

```
manifest.json                      format, version, generator, projects, counts, SHA-256 of every other file
agent_sessions.jsonl               agent sessions referred to by history, comments and claims
projects/<KEY>/project.json        project, statuses, types, labels, views, layouts
projects/<KEY>/tasks.jsonl         one task per line, with labels, relations, claim, revision id
projects/<KEY>/tasks/<KEY>-<n>.md  the description (no file when it is empty)
projects/<KEY>/comments.jsonl      comments, each with its earlier texts
projects/<KEY>/time.jsonl          time entries
projects/<KEY>/revisions.jsonl     description history of tasks and the project; wiki revisions
projects/<KEY>/wiki.jsonl          wiki pages: tree, ranks, slugs, current revision
projects/<KEY>/wiki/<path>.md      the current text of each page, the tree as folders
projects/<KEY>/files.jsonl         attachments: owner, name, type, size, SHA-256
projects/<KEY>/activity.jsonl      history rows; `seq` is one sequence across the archive
files/<sha256>                     attached files, once per distinct content
fixtures.json                      (hostile set only) what each task exercises
```

The `.jsonl` files hold one JSON object per line, each line ending in `\n`. Fields
follow the columns of the workspace database. The record types in `src/model.rs` are
the schema: they reject unknown fields, and the tests read every generated file back
through them.

| Kind | Format |
|---|---|
| IDs | UUIDv7, lower-case hyphenated text |
| Moments | integer milliseconds since 1970 UTC |
| Days | `YYYY-MM-DD` |
| Durations | integer minutes |
| Ranks | 7 lower-case base-36 digits, spaced so many items fit between neighbours |
| Enums | lower-case text (`todo`, `in_progress`, `blocks`, …) |

Derived values are filled in and consistent with the records: checklist counts,
spent time, `closed_at`, `updated_at`. The engine recomputes them on import and is the
authority; the tool's checklist rule is a simplified form of the GFM rule.

## Determinism

The tool draws from its own SplitMix64 generator, not from a crate, so a dependency
update can never change the output. Time starts at Monday 2026-01-05 09:00 UTC; the
system clock is never read. Maps are ordered and files are written in path order.

## Tests

```sh
cargo test -p gk-seed
```

They check that two runs of the binary give identical folders, that every file reads
back through the record types unchanged, that the planned sizes hold exactly, and the
data-model rules across records: unique IDs and keys, hierarchy depth and cycles,
acyclic dependencies, dates, derived values, file hashes, the history sequence.
