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
use crate::messages::{self, text};

/// Global options.
pub(crate) const WORKSPACE: &str = "workspace";
pub(crate) const JSON: &str = "json";
pub(crate) const QUIET: &str = "quiet";
pub(crate) const NO_COLOR: &str = "no-color";
pub(crate) const TZ: &str = "tz";
pub(crate) const LANG: &str = messages::LANG_FLAG;
/// Without a command: start the app with its quick-add window.
pub(crate) const QUICK_ADD: &str = "quick-add";

/// Commands of the program itself, not operations.
pub(crate) const OPEN: &str = "open";
pub(crate) const MCP: &str = "mcp";
pub(crate) const COMPLETIONS: &str = "completions";
pub(crate) const MAN: &str = "man";
pub(crate) const HELP: &str = "help";
#[cfg(test)]
pub(crate) const BUILT_IN: &[&str] = &[OPEN, MCP, COMPLETIONS, MAN, HELP];

/// The options every command has, which are not operation fields.
const HELP_ARG: &str = "help";
const VERSION_ARG: &str = "version";

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
        // The same name in usage lines everywhere, not `glyphkeel.exe` on
        // Windows: the command runs without its extension there too.
        .bin_name("glyphkeel")
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
        )
        .arg(
            Arg::new(LANG)
                .long(LANG)
                .global(true)
                .value_name("LANG")
                .help(text("cli-lang-help", &[])),
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
        let mut cmd = child.into_command(ops);
        if cmd.get_name() == WORKSPACE {
            cmd = crate::workspace::commands(cmd);
        }
        root = root.subcommand(cmd);
    }

    let shells: Vec<String> = <clap_complete::Shell as clap::ValueEnum>::value_variants()
        .iter()
        .filter_map(clap::ValueEnum::to_possible_value)
        .map(|v| v.get_name().to_owned())
        .collect();
    let root = root
        .subcommand(
            Command::new(OPEN).about(text("cli-open-about", &[])).arg(
                Arg::new("links")
                    .value_name(text("cli-open-links-value", &[]))
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
                        .value_parser(clap::value_parser!(clap_complete::Shell))
                        .hide_possible_values(true)
                        .help(with_values(
                            &text("cli-completions-shell-help", &[]),
                            &shells,
                        )),
                ),
        )
        .subcommand(
            Command::new(MAN)
                .hide(true)
                .about(text("cli-man-about", &[])),
        )
        .subcommand(
            Command::new(HELP).about(text("cli-help-about", &[])).arg(
                Arg::new("commands")
                    .value_name(text("cli-help-commands-value", &[]))
                    .num_args(0..)
                    .help(text("cli-help-commands-help", &[])),
            ),
        );
    localize(root, "glyphkeel", true)
}

/// An option's help followed by the values it takes.
pub(crate) fn with_values(help: &str, values: &[String]) -> String {
    text(
        "cli-help-with-values",
        &[("help", help.to_owned()), ("values", values.join(", "))],
    )
}

/// The words clap writes itself, in the chosen language: the headings of
/// the help with their punctuation, the usage lines, and the help and
/// version options. Clap's own help command is replaced by [`HELP`], which
/// the program answers itself.
fn localize(mut cmd: Command, path: &str, root: bool) -> Command {
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|c| c.get_name().to_owned())
        .collect();
    for name in names {
        let sub_path = format!("{path} {name}");
        cmd = cmd.mut_subcommand(name, |c| localize(c, &sub_path, false));
    }
    let header = clap::builder::styling::Style::new().bold().underline();
    let heading = |id: &str| format!("{header}{}{header:#}", text(id, &[]));
    let has_arguments = cmd.get_positionals().any(|a| !a.is_hide_set());
    let has_commands = cmd.get_subcommands().any(|c| !c.is_hide_set());
    let mut template = format!(
        "{{before-help}}{{about-with-newline}}\n{} {{usage}}\n\n",
        heading("cli-help-usage")
    );
    if has_arguments {
        template.push_str(&format!(
            "{}\n{{positionals}}\n\n",
            heading("cli-help-arguments")
        ));
    }
    template.push_str(&format!("{}\n{{options}}", heading("cli-help-options")));
    if has_commands {
        template.push_str(&format!(
            "\n\n{}\n{{subcommands}}",
            heading("cli-help-commands")
        ));
    }
    template.push_str("{after-help}");
    let usage = usage(&cmd, path, has_commands);
    cmd = cmd
        .override_usage(usage)
        .help_template(template)
        .disable_help_subcommand(true);
    // The MCP server reads its own `--help`.
    if !cmd.is_disable_help_flag_set() {
        cmd = cmd.disable_help_flag(true).arg(
            Arg::new(HELP_ARG)
                .short('h')
                .long(HELP_ARG)
                .action(ArgAction::Help)
                .help(text("cli-help-help", &[])),
        );
    }
    if root {
        cmd = cmd.disable_version_flag(true).arg(
            Arg::new(VERSION_ARG)
                .short('V')
                .long(VERSION_ARG)
                .action(ArgAction::Version)
                .help(text("cli-version-help", &[])),
        );
    }
    cmd
}

/// The usage line of `cmd`, whose words are `path`: the words for options
/// and commands in the chosen language, the arguments by their value names
/// (`glyphkeel task show [OPTIONS] [KEY]...`).
fn usage(cmd: &Command, path: &str, has_commands: bool) -> String {
    let mut line = format!("{path} {}", text("cli-usage-options", &[]));
    for arg in cmd.get_positionals().filter(|a| !a.is_hide_set()) {
        let name = arg
            .get_value_names()
            .and_then(|names| names.first())
            .map_or_else(|| arg.get_id().as_str().to_uppercase(), ToString::to_string);
        let many = arg.get_num_args().is_some_and(|n| n.max_values() > 1)
            || matches!(arg.get_action(), ArgAction::Append);
        line.push(' ');
        line.push_str(&if arg.is_required_set() {
            format!("<{name}>")
        } else {
            format!("[{name}]")
        });
        if many {
            line.push_str("...");
        }
    }
    if has_commands {
        line.push(' ');
        line.push_str(&text(
            if cmd.is_subcommand_required_set() {
                "cli-usage-command"
            } else {
                "cli-usage-command-optional"
            },
            &[],
        ));
    }
    line
}

/// The long help of the command `words` name, for `glyphkeel help …`; the
/// program's own help without words. `None` when no command has those
/// words.
pub(crate) fn help_of(ops: &[OpCommand], words: &[String]) -> Option<String> {
    let mut root = build(ops);
    root.build();
    let mut cmd = &mut root;
    for word in words {
        cmd = cmd.find_subcommand_mut(word)?;
    }
    Some(cmd.render_long_help().to_string())
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
                cmd = cmd.about(messages::op_summary(c.op));
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
                    LANG,
                    HELP_ARG,
                ] {
                    assert_ne!(f.long(), taken, "{} has a field `{}`", c.op.id, f.name);
                }
            }
        }
    }

    /// The words of every command's help that are not messages: command
    /// and option names, the values options take, and the product's name.
    fn machine_words(cmd: &Command, out: &mut std::collections::BTreeSet<String>) {
        let mut add = |text: &str| {
            for word in text.split(|c: char| !c.is_ascii_alphabetic()) {
                out.insert(word.to_owned());
            }
        };
        add(cmd.get_name());
        for arg in cmd.get_arguments() {
            add(arg.get_id().as_str());
            add(&arg.get_id().as_str().to_uppercase());
            for name in arg.get_value_names().unwrap_or_default() {
                add(name.as_str());
            }
            if let Some(long) = arg.get_long() {
                add(long);
            }
            if let Some(short) = arg.get_short() {
                add(&short.to_string());
            }
            for value in arg.get_possible_values() {
                add(value.get_name());
            }
        }
        for sub in cmd.get_subcommands() {
            machine_words(sub, out);
        }
    }

    /// Words in plain English left in `text`: runs of ASCII letters that
    /// are not machine words (names and value names of commands and
    /// options).
    fn plain_words(text: &str, machine: &std::collections::BTreeSet<String>) -> Vec<String> {
        text.split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| !w.is_empty())
            .filter(|w| !machine.contains(*w))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn help_in_the_pseudo_locale_has_no_plain_english() {
        gk_engine::language::with_language("en-XA", || {
            let mut root = build(&op_commands());
            root.build();
            let mut machine = std::collections::BTreeSet::new();
            machine_words(&root, &mut machine);
            machine.insert("glyphkeel".to_owned());
            let mut found = Vec::new();
            let mut stack: Vec<&mut Command> = vec![&mut root];
            let mut checked = 0;
            while let Some(cmd) = stack.pop() {
                if cmd.get_name() == MCP {
                    continue;
                }
                let help = cmd.render_long_help().to_string();
                let short = cmd.render_help().to_string();
                for text in [help, short] {
                    // Usage lines too: only the words for options and
                    // commands are not names there.
                    for line in text.lines() {
                        // Words of clap's that are also machine words
                        // elsewhere, and punctuation outside a message.
                        for clap_text in ["[OPTIONS]", "<COMMAND>", "[COMMAND]", "]:"] {
                            if line.contains(clap_text) {
                                found.push(format!("{}: {clap_text} in {line:?}", cmd.get_name()));
                            }
                        }
                        let plain = plain_words(line, &machine);
                        if !plain.is_empty() {
                            found.push(format!("{}: {plain:?} in {line:?}", cmd.get_name()));
                        }
                    }
                }
                checked += 1;
                stack.extend(cmd.get_subcommands_mut());
            }
            assert!(checked > 150, "{checked}");
            assert!(
                found.is_empty(),
                "plain English in the help:\n{}",
                found.join("\n")
            );
        });
    }

    #[test]
    fn the_operations_messages_are_the_catalogs_text() {
        let normal = |s: &str| s.lines().map(str::trim).collect::<Vec<_>>().join("\n");
        let mut ids = std::collections::BTreeSet::new();
        for c in op_commands() {
            let id = messages::op_message_id(c.op.id);
            assert_eq!(
                messages::OPS.english(&id, &[]),
                normal(c.op.summary),
                "{id}"
            );
            ids.insert(id);
            let schema = gk_api::schema::portable(&(c.op.input_schema)().to_value());
            for f in &c.fields {
                let Some(text) = schema["properties"][&f.name]["description"].as_str() else {
                    continue;
                };
                let id = messages::field_message_id(c.op.id, &f.name);
                assert_eq!(messages::OPS.english(&id, &[]), normal(text), "{id}");
                ids.insert(id);
            }
        }
        let in_file: std::collections::BTreeSet<String> =
            messages::OPS.ids().into_iter().map(str::to_owned).collect();
        assert_eq!(in_file, ids, "ops.ftl differs: run `cargo xtask gen`");
    }
}
