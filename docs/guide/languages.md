<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Languages

## The interface language

Glyphkeel's text is in English for now. Every word it shows, in the app and on
the command line, comes from translation files, so other languages can be added
without changing the program.

Choose the language in Preferences › **Language** › **Language of the
interface**. **System** follows your computer, and falls back to English when
Glyphkeel has no text in that language. The change applies at once.

## Dates, numbers and durations

Dates, times and numbers are written as your system's locale writes them, also
when the text is English. Preferences › Language shows the locale in use.

Dates and durations that you type take these forms:

- dates as `2026-12-31`, or in words such as `today`, `fri`, `next mon`,
  `in 2 weeks` or `dec 1`;
- durations as `90m`, `1.5h` (or `1,5h`), `1h 30m`, `1:30`, `2d` or `1w`.

The [query language](../query-language.md) and the names of command-line options
and MCP tools stay in English, so that scripts and agents work everywhere.

## Spelling

The editor checks spelling as you type. Right-click a marked word for
suggestions.

- **On Linux** it uses the system's English (US) Hunspell dictionary:
  `hunspell-en-US` on Fedora, `hunspell-en-us` on Debian and Ubuntu. Without
  one, nothing is marked.
- **On Windows** it uses the languages added in Settings › Time & language ›
  Language & region.

## Translating Glyphkeel

The texts are [Fluent](https://projectfluent.org/) files, with one folder per
language:

- `ui/src/i18n/en/` holds the app's texts;
- `crates/*/i18n/en/` hold the texts of the command line, the MCP server and the
  engine's messages.

A translation is a copy of these folders under its language code, such as `de/`.
Fluent handles plurals and word order, so a translation never needs to glue
words together. If you would like to translate Glyphkeel, please open an issue
first; [CONTRIBUTING.md](../../CONTRIBUTING.md) describes how to send a change.
