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
use crate::messages::text;

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

/// Commands renamed in this version: the old words still run the
/// operation for one more version, without showing in help or completions.
pub(crate) const RENAMED: &[(&str, &str)] = &[("status", "workspace")];

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
        .about(text("cli-about", &[]))
        .long_about(format!(
            "{}\n\n{}",
            text("cli-about", &[]),
            text("cli-long-about", &[])
        ))
        .after_help(text("cli-exit-codes", &[]))
        .subcommand_required(false)
        .arg(
            Arg::new(QUICK_ADD)
                .long(QUICK_ADD)
                .action(ArgAction::SetTrue)
                .help(text("cli-quick-add-help", &[])),
        )
        .arg(
            Arg::new(WORKSPACE)
                .long(WORKSPACE)
                .short('w')
                .global(true)
                .value_name("NAME|PATH")
                .help(text("cli-workspace-help", &[])),
        )
        .arg(
            Arg::new(JSON)
                .long(JSON)
                .global(true)
                .action(ArgAction::SetTrue)
                .help(text("cli-json-help", &[])),
        )
        .arg(
            Arg::new(QUIET)
                .long(QUIET)
                .short('q')
                .global(true)
                .action(ArgAction::SetTrue)
                .help(text("cli-quiet-help", &[])),
        )
        .arg(
            Arg::new(NO_COLOR)
                .long(NO_COLOR)
                .global(true)
                .action(ArgAction::SetTrue)
                .help(text("cli-no-color-help", &[])),
        )
        .arg(
            Arg::new(TZ)
                .long(TZ)
                .global(true)
                .value_name("ZONE")
                .help(text("cli-tz-help", &[])),
        );

    let mut tree = Node::default();
    for (i, c) in ops.iter().enumerate() {
        tree.insert(&c.path, i);
    }
    for (old, new) in RENAMED {
        let Some(i) = ops.iter().position(|c| c.path == [*new]) else {
            continue;
        };
        match tree.children.iter_mut().find(|c| c.name == *old) {
            Some(node) if node.op.is_none() => {
                node.op = Some(i);
                node.renamed = true;
            }
            Some(_) => {}
            None => tree.children.push(Node {
                name: (*old).to_owned(),
                op: Some(i),
                renamed: true,
                children: Vec::new(),
            }),
        }
    }
    for child in tree.children {
        root = root.subcommand(child.into_command(ops));
    }

    root.subcommand(
        Command::new(OPEN).about(text("cli-open-about", &[])).arg(
            Arg::new("links")
                .value_name("LINK")
                .num_args(0..)
                .help(text("cli-open-links-help", &[])),
        ),
    )
    .subcommand(
        Command::new(MCP)
            .about(text("cli-mcp-about", &[]))
            .long_about(text("cli-mcp-long-about", &[]))
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
            .about(text("cli-completions-about", &[]))
            .arg(
                Arg::new("shell")
                    .required(true)
                    .value_parser(clap::value_parser!(clap_complete::Shell)),
            ),
    )
    .subcommand(
        Command::new(MAN)
            .hide(true)
            .about(text("cli-man-about", &[])),
    )
}

/// A node of the command tree: a word, maybe an operation, maybe more words.
#[derive(Debug, Default)]
struct Node {
    name: String,
    op: Option<usize>,
    /// The operation is here under its old name only.
    renamed: bool,
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
            cmd = cmd.args(input::args(c.op, &c.fields));
            if !self.renamed {
                cmd = cmd.about(c.op.summary);
            } else if has_children {
                cmd = cmd.about(group_about(&self.children));
            } else {
                cmd = cmd.hide(true);
            }
            if has_children {
                cmd = cmd.args_conflicts_with_subcommands(true);
            }
        } else {
            cmd = cmd
                .about(group_about(&self.children))
                .subcommand_required(true)
                .arg_required_else_help(true);
        }
        for child in self.children {
            cmd = cmd.subcommand(child.into_command(ops));
        }
        cmd
    }
}

/// The help line of a command that only groups others.
fn group_about(children: &[Node]) -> String {
    let words: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    text("cli-group-about", &[("commands", words.join(", "))])
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
    if let [word] = path.as_slice()
        && let Some((_, new)) = RENAMED.iter().find(|(old, _)| old == word)
    {
        path = vec![(*new).to_owned()];
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
