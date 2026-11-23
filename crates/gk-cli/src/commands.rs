// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The command tree, built from the operation catalog: every operation with
//! a command becomes one, so a new operation needs no code here.
//!
//! A catalog command is a few words (`agents sessions`). A word in angle
//! brackets (`task show <key>`) names an input field taken as a positional
//! argument.

use clap::{Arg, ArgAction, Command};
use gk_api::{CATALOG, OpDescriptor};

use crate::input::{self, Field};

/// Global options.
pub(crate) const WORKSPACE: &str = "workspace";
pub(crate) const JSON: &str = "json";
pub(crate) const QUIET: &str = "quiet";
pub(crate) const NO_COLOR: &str = "no-color";
pub(crate) const TZ: &str = "tz";
/// Without a command: start the app with its quick-add window.
pub(crate) const QUICK_ADD: &str = "quick-add";

/// Commands of the program itself, not operations.
pub(crate) const OPEN: &str = "open";
pub(crate) const MCP: &str = "mcp";
pub(crate) const COMPLETIONS: &str = "completions";
pub(crate) const MAN: &str = "man";
#[cfg(test)]
pub(crate) const BUILT_IN: &[&str] = &[OPEN, MCP, COMPLETIONS, MAN];

/// One operation reachable from the command line.
#[derive(Debug, Clone)]
pub(crate) struct OpCommand {
    /// The command words, without positional fields.
    pub(crate) path: Vec<String>,
    pub(crate) op: &'static OpDescriptor,
    pub(crate) fields: Vec<Field>,
}

/// Every operation that has a command, in catalog order.
pub(crate) fn op_commands() -> Vec<OpCommand> {
    CATALOG
        .iter()
        .filter_map(|op| {
            let words: Vec<&str> = op.cli?.split_whitespace().collect();
            let (positional, path): (Vec<&str>, Vec<&str>) =
                words.into_iter().partition(|w| w.starts_with('<'));
            let positional: Vec<String> = positional
                .iter()
                .map(|w| {
                    w.trim_matches(|c| c == '<' || c == '>' || c == '.')
                        .to_owned()
                })
                .collect();
            Some(OpCommand {
                path: path.into_iter().map(str::to_owned).collect(),
                fields: input::fields(op, &positional),
                op,
            })
        })
        .collect()
}

/// The whole command line definition.
pub(crate) fn build(ops: &[OpCommand]) -> Command {
    let mut root = Command::new("glyphkeel")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Glyphkeel, a local task manager, from the command line.")
        .long_about(
            "Glyphkeel, a local task manager, from the command line.\n\n\
             Without a command, starts the desktop app (with `--quick-add`, only its \
             quick-add window). Every other command runs one operation on the workspace, \
             also while the app is open.",
        )
        .after_help(
            "Exit codes: 0 success, 1 usage error or invalid input, 2 not found, \
             3 change rejected, 4 workspace or storage problem.",
        )
        .subcommand_required(false)
        .arg(
            Arg::new(QUICK_ADD)
                .long(QUICK_ADD)
                .action(ArgAction::SetTrue)
                .help(
                    "Open the desktop app's quick-add window to capture a task, starting the app \
                     if it isn't running (a command for a desktop shortcut)",
                ),
        )
        .arg(
            Arg::new(WORKSPACE)
                .long(WORKSPACE)
                .short('w')
                .global(true)
                .value_name("NAME|PATH")
                .help("The workspace: a name in the data folder, or a folder path [default: Personal]"),
        )
        .arg(
            Arg::new(JSON)
                .long(JSON)
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print the operation's output as JSON"),
        )
        .arg(
            Arg::new(QUIET)
                .long(QUIET)
                .short('q')
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print nothing on success; errors still go to standard error"),
        )
        .arg(
            Arg::new(NO_COLOR)
                .long(NO_COLOR)
                .global(true)
                .action(ArgAction::SetTrue)
                .help("No colours in help and messages (also with NO_COLOR set)"),
        )
        .arg(
            Arg::new(TZ)
                .long(TZ)
                .global(true)
                .value_name("ZONE")
                .help(
                    "The time zone of \"today\" and local times for this command: \"system\" or \
                     an IANA name such as Europe/Berlin [default: the personal setting]",
                ),
        );

    let mut tree = Node::default();
    for (i, c) in ops.iter().enumerate() {
        tree.insert(&c.path, i);
    }
    for child in tree.children {
        root = root.subcommand(child.into_command(ops));
    }

    root.subcommand(
        Command::new(OPEN)
            .about("Start the desktop app, optionally at tasks or other links")
            .arg(
                Arg::new("links")
                    .value_name("LINK")
                    .num_args(0..)
                    .help("A task key (`TM-41`) or a `glyphkeel://` link"),
            ),
    )
    .subcommand(
        Command::new(MCP)
            .about("Serve the workspace to an AI agent over MCP (standard input and output)")
            .long_about(
                "Serve the workspace to an AI agent over MCP (standard input and output). \
                 The arguments are those of the MCP server; `glyphkeel mcp --help` lists them.",
            )
            .disable_help_flag(true)
            .arg(
                Arg::new("args")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true)
                    .value_parser(clap::value_parser!(std::ffi::OsString)),
            ),
    )
    .subcommand(
        Command::new(COMPLETIONS)
            .about("Print the shell completion script")
            .arg(
                Arg::new("shell")
                    .required(true)
                    .value_parser(clap::value_parser!(clap_complete::Shell)),
            ),
    )
    .subcommand(
        Command::new(MAN)
            .hide(true)
            .about("Print the man page (roff)"),
    )
}

/// A node of the command tree: a word, maybe an operation, maybe more words.
#[derive(Debug, Default)]
struct Node {
    name: String,
    op: Option<usize>,
    children: Vec<Node>,
}

impl Node {
    fn insert(&mut self, path: &[String], op: usize) {
        let Some((first, rest)) = path.split_first() else {
            self.op = Some(op);
            return;
        };
        let pos = match self.children.iter().position(|c| &c.name == first) {
            Some(p) => p,
            None => {
                self.children.push(Self {
                    name: first.clone(),
                    ..Self::default()
                });
                self.children.len() - 1
            }
        };
        self.children[pos].insert(rest, op);
    }

    fn into_command(self, ops: &[OpCommand]) -> Command {
        let mut cmd = Command::new(self.name.clone());
        let has_children = !self.children.is_empty();
        if let Some(i) = self.op {
            let c = &ops[i];
            cmd = cmd.about(c.op.summary).args(input::args(c.op, &c.fields));
            if has_children {
                cmd = cmd.args_conflicts_with_subcommands(true);
            }
        } else {
            let words: Vec<&str> = self.children.iter().map(|c| c.name.as_str()).collect();
            cmd = cmd
                .about(format!("Commands: {}", words.join(", ")))
                .subcommand_required(true)
                .arg_required_else_help(true);
        }
        for child in self.children {
            cmd = cmd.subcommand(child.into_command(ops));
        }
        cmd
    }
}

/// The operation the parsed command line names, with the matches of its
/// own command.
pub(crate) fn find<'a, 'm>(
    ops: &'a [OpCommand],
    matches: &'m clap::ArgMatches,
) -> Option<(&'a OpCommand, &'m clap::ArgMatches)> {
    let mut path = Vec::new();
    let mut m = matches;
    while let Some((name, sub)) = m.subcommand() {
        path.push(name.to_owned());
        m = sub;
    }
    ops.iter().find(|c| c.path == path).map(|c| (c, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_definition_is_valid() {
        build(&op_commands()).debug_assert();
    }

    #[test]
    fn every_command_of_the_catalog_is_reachable() {
        let ops = op_commands();
        let with_command = CATALOG.iter().filter(|d| d.cli.is_some()).count();
        assert_eq!(ops.len(), with_command);
        let root = build(&ops);
        for c in &ops {
            let mut cmd = &root;
            for word in &c.path {
                cmd = cmd
                    .find_subcommand(word)
                    .unwrap_or_else(|| panic!("`{}` is missing", c.path.join(" ")));
            }
            assert_eq!(
                cmd.get_about().map(ToString::to_string).as_deref(),
                Some(c.op.summary)
            );
        }
    }

    #[test]
    fn operations_and_built_in_commands_do_not_collide() {
        let ops = op_commands();
        for c in &ops {
            let first = c.path.first().map(String::as_str).unwrap_or_default();
            assert!(!BUILT_IN.contains(&first), "{} takes `{first}`", c.op.id);
            assert!(
                ops.iter().filter(|o| o.path == c.path).count() == 1,
                "two operations share `{}`",
                c.path.join(" ")
            );
            for f in &c.fields {
                for taken in [
                    input::INPUT_ARG,
                    input::YES_ARG,
                    WORKSPACE,
                    JSON,
                    QUIET,
                    NO_COLOR,
                    TZ,
                ] {
                    assert_ne!(f.long(), taken, "{} has a field `{}`", c.op.id, f.name);
                }
            }
        }
    }
}
