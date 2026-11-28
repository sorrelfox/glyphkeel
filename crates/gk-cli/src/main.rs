// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel`: the command line. It reaches the engine through the operation
//! catalog, as the desktop app and the MCP server do: every operation with a
//! command is a subcommand, its input fields are options, and its output is
//! printed for people or, with `--json`, exactly as the catalog describes it.
//!
//! Without a command, or with `glyphkeel open`, it starts the desktop app
//! (`glyphkeel --quick-add`: with only its quick-add window, or hands that
//! to the running app);
//! `glyphkeel mcp` serves MCP over standard input and output.
//!
//! Exit codes: 0 success, 1 usage error or invalid input, 2 not found,
//! 3 change rejected, 4 workspace or storage problem.

mod board;
mod commands;
mod dashboard;
mod input;
mod launch;
mod messages;
mod output;
mod passphrase;
mod workspace;

use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;

use clap::ArgMatches;
use clap::error::ErrorKind;
use gk_api::OpError;
use gk_api::ops::common::Via;
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role, personal_time_zone};
use gk_store::UserSettings;

use crate::commands::OpCommand;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let pre = prescan(&args);
    if let Some(mcp_args) = pre.mcp {
        // The MCP server's text is for agents: it stays English.
        return gk_mcp::run_stdio(&mcp_args);
    }
    messages::choose_language(pre.lang.as_deref());
    let ops = commands::op_commands();
    let color = if pre.no_color || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        clap::ColorChoice::Never
    } else {
        clap::ColorChoice::Auto
    };
    let matches = match commands::build(&ops)
        .color(color)
        .try_get_matches_from(&args)
    {
        Ok(m) => m,
        Err(e) => return usage_error(&e),
    };
    let out = Out {
        json: matches.get_flag(commands::JSON),
        quiet: matches.get_flag(commands::QUIET),
    };
    match run(&ops, &matches, out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprint!("{}", output::error(&e, out.json));
            ExitCode::from(e.code.exit_code())
        }
    }
}

/// Help and version go to standard output with exit code 0; every other
/// parse error is a usage error, exit code 1, told in the chosen language.
fn usage_error(e: &clap::Error) -> ExitCode {
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = e.print();
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{}", usage_text(e));
            ExitCode::from(1)
        }
    }
}

/// A usage error in the chosen language, from what clap found. A command
/// that only groups others shows its help.
fn usage_text(e: &clap::Error) -> String {
    use clap::error::{ContextKind, ContextValue};
    if e.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
        return e.render().to_string();
    }
    let message = usage_message(e);
    let one = |kind| match e.get(kind) {
        Some(ContextValue::String(s)) => Some(s.clone()),
        Some(ContextValue::Strings(v)) => Some(v.join(", ")),
        _ => None,
    };
    let mut out = messages::text("cli-error", &[("message", message)]);
    out.push('\n');
    let suggestion = one(ContextKind::SuggestedSubcommand)
        .or_else(|| one(ContextKind::SuggestedArg))
        .or_else(|| one(ContextKind::SuggestedValue));
    if let Some(suggestion) = suggestion {
        out.push_str(&messages::text(
            "cli-usage-did-you-mean",
            &[("suggestion", suggestion)],
        ));
        out.push('\n');
    }
    if let Some(ContextValue::StyledStr(usage)) = e.get(ContextKind::Usage) {
        let usage = usage.to_string();
        let usage = usage.trim().split_once(' ').map_or("", |(_, rest)| rest);
        out.push('\n');
        out.push_str(&messages::text(
            "cli-usage-line",
            &[("usage", usage.trim().to_owned())],
        ));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&messages::text("cli-usage-more", &[]));
    out.push('\n');
    out
}

/// The message of usage error `e`: the one for its kind with what clap
/// found filled in, else the kind's message without it, else
/// `cli-usage-other`.
fn usage_message(e: &clap::Error) -> String {
    let id = usage_message_id(e.kind());
    usage_message_with_context(e).unwrap_or_else(|| match id {
        "cli-usage-output-failed" => messages::text("cli-usage-output-failed-plain", &[]),
        id if usage_message_args(id) => messages::text("cli-usage-other", &[]),
        id => messages::text(id, &[]),
    })
}

/// Whether message `id` has placeables, so it can't be shown without what
/// clap found.
fn usage_message_args(id: &str) -> bool {
    !matches!(id, "cli-usage-invalid-utf8" | "cli-usage-other")
}

/// The message for every kind of usage error clap has.
fn usage_message_id(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::InvalidValue | ErrorKind::ValueValidation | ErrorKind::TooManyValues => {
            "cli-usage-invalid-value"
        }
        ErrorKind::UnknownArgument => "cli-usage-unknown-argument",
        ErrorKind::InvalidSubcommand => "cli-usage-unknown-command",
        ErrorKind::NoEquals => "cli-usage-no-equals",
        ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => "cli-usage-value-count",
        ErrorKind::ArgumentConflict => "cli-usage-conflict",
        ErrorKind::MissingRequiredArgument => "cli-usage-missing-arguments",
        ErrorKind::MissingSubcommand => "cli-usage-missing-command",
        ErrorKind::InvalidUtf8 => "cli-usage-invalid-utf8",
        ErrorKind::Io | ErrorKind::Format => "cli-usage-output-failed",
        // Help and version are shown, not reported; a kind a later clap
        // adds is a mistake on the command line all the same.
        _ => "cli-usage-other",
    }
}

/// The message of `e` with what clap found filled in; `None` when clap
/// didn't say enough.
fn usage_message_with_context(e: &clap::Error) -> Option<String> {
    use clap::error::{ContextKind, ContextValue};
    let one = |kind| match e.get(kind) {
        Some(ContextValue::String(s)) => Some(s.clone()),
        Some(ContextValue::Strings(v)) => Some(v.join(", ")),
        Some(ContextValue::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    let id = usage_message_id(e.kind());
    Some(match e.kind() {
        ErrorKind::UnknownArgument => messages::text(id, &[("arg", one(ContextKind::InvalidArg)?)]),
        ErrorKind::InvalidSubcommand => {
            messages::text(id, &[("command", one(ContextKind::InvalidSubcommand)?)])
        }
        ErrorKind::MissingSubcommand => {
            messages::text(id, &[("command", one(ContextKind::InvalidSubcommand)?)])
        }
        ErrorKind::MissingRequiredArgument => {
            messages::text(id, &[("args", one(ContextKind::InvalidArg)?)])
        }
        ErrorKind::NoEquals => messages::text(id, &[("arg", one(ContextKind::InvalidArg)?)]),
        ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => {
            let arg = one(ContextKind::InvalidArg)?;
            match (
                one(ContextKind::ExpectedNumValues),
                one(ContextKind::ActualNumValues),
            ) {
                (Some(count), Some(given)) => {
                    messages::text(id, &[("arg", arg), ("count", count), ("given", given)])
                }
                _ => messages::text("cli-usage-wrong-values", &[("arg", arg)]),
            }
        }
        ErrorKind::ArgumentConflict => {
            let arg = one(ContextKind::InvalidArg)?;
            match one(ContextKind::PriorArg) {
                Some(other) if other != arg => {
                    messages::text("cli-usage-conflict", &[("arg", arg), ("other", other)])
                }
                _ => messages::text("cli-usage-repeated", &[("arg", arg)]),
            }
        }
        ErrorKind::InvalidValue | ErrorKind::ValueValidation | ErrorKind::TooManyValues => {
            let arg = one(ContextKind::InvalidArg)?;
            let value = one(ContextKind::InvalidValue)?;
            let reason = std::error::Error::source(e).map(ToString::to_string);
            match (value.is_empty(), reason, one(ContextKind::ValidValue)) {
                (true, _, _) => messages::text("cli-usage-no-value", &[("arg", arg)]),
                (false, Some(reason), _) => messages::text(
                    "cli-usage-invalid-value-why",
                    &[("arg", arg), ("value", value), ("reason", reason)],
                ),
                (false, None, Some(values)) => messages::text(
                    "cli-usage-invalid-value-choices",
                    &[("arg", arg), ("value", value), ("values", values)],
                ),
                (false, None, None) => {
                    messages::text("cli-usage-invalid-value", &[("arg", arg), ("value", value)])
                }
            }
        }
        ErrorKind::Io | ErrorKind::Format => {
            messages::text(id, &[("error", std::error::Error::source(e)?.to_string())])
        }
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy)]
struct Out {
    json: bool,
    quiet: bool,
}

fn run(ops: &[OpCommand], matches: &ArgMatches, out: Out) -> Result<(), OpError> {
    let workspace = matches
        .get_one::<String>(commands::WORKSPACE)
        .map(String::as_str);
    let tz = matches.get_one::<String>(commands::TZ).map(String::as_str);
    let quick_add = matches.get_flag(commands::QUICK_ADD);
    if quick_add && matches.subcommand().is_some() {
        return Err(messages::hint(
            messages::error("cli-quick-add-alone", &[], OpError::invalid),
            "cli-quick-add-alone-hint",
            &[],
        ));
    }
    match matches.subcommand() {
        None if quick_add => launch::start(&[launch::QUICK_ADD.to_owned()]),
        None => launch::start(&[]),
        Some((commands::OPEN, m)) => {
            let given: Vec<String> = m
                .get_many::<String>("links")
                .map(|v| v.cloned().collect())
                .unwrap_or_default();
            launch::start(&launch::links(&given)?)
        }
        Some((commands::COMPLETIONS, m)) => {
            let shell = *m
                .get_one::<clap_complete::Shell>("shell")
                .unwrap_or_else(|| unreachable!("required by the definition"));
            let mut cmd = commands::build(ops);
            let mut buf = Vec::new();
            clap_complete::generate(shell, &mut cmd, "glyphkeel", &mut buf);
            print_bytes(&buf)
        }
        Some((commands::HELP, m)) => {
            let words: Vec<String> = m
                .get_many::<String>("commands")
                .map(|v| v.cloned().collect())
                .unwrap_or_default();
            let help = commands::help_of(ops, &words).ok_or_else(|| {
                messages::error(
                    "cli-help-unknown",
                    &[("command", words.join(" "))],
                    OpError::invalid,
                )
            })?;
            print_bytes(help.as_bytes())
        }
        Some((commands::MAN, _)) => {
            let mut buf = Vec::new();
            clap_mangen::Man::new(commands::build(ops))
                .render(&mut buf)
                .map_err(|e| {
                    messages::error(
                        "cli-man-failed",
                        &[("error", e.to_string())],
                        OpError::storage,
                    )
                })?;
            print_bytes(&buf)
        }
        Some((commands::WORKSPACE, m)) if m.subcommand().is_some() => {
            let Some((word, sub)) = m.subcommand() else {
                unreachable!("checked above")
            };
            let (human, value) = workspace::run(word, sub, workspace)?;
            if out.quiet {
                return Ok(());
            }
            let text = if out.json {
                output::json(&value)
            } else {
                human
            };
            print_bytes(text.as_bytes())
        }
        Some(_) => {
            let (cmd, m) = commands::find(ops, matches)
                .unwrap_or_else(|| unreachable!("every other command is an operation"));
            let input = input::collect(&cmd.fields, m)?;
            let confirm = if m.try_get_one::<bool>(input::YES_ARG).ok().flatten() == Some(&true) {
                Confirmation::Given
            } else {
                ask_on_terminal()
            };
            let value = call(workspace, tz, cmd, input, confirm)?;
            if out.quiet {
                return Ok(());
            }
            let text = if out.json {
                output::json(&value)
            } else if cmd.op.id == "board.get" {
                board::show(&value)
            } else if cmd.op.id == "dashboard.show" {
                dashboard::show(&value)
            } else if cmd.op.id == "dashboard.widget" {
                dashboard::widget(&value)
            } else if let Some(csv) = value.get("csv").and_then(serde_json::Value::as_str) {
                // An operation asked for CSV text prints just that, ready to
                // save or pipe.
                csv.to_owned()
            } else {
                output::human(&value)
            };
            print_bytes(text.as_bytes())
        }
    }
}

/// Opens the workspace for this one command, runs the operation and closes
/// it again. "Today" and local times are in the zone `tz` names, else in the
/// person's own zone from the personal settings.
fn call(
    workspace: Option<&str>,
    tz: Option<&str>,
    cmd: &OpCommand,
    input: serde_json::Value,
    confirm: Confirmation,
) -> Result<serde_json::Value, OpError> {
    let layout = workspace::resolve(workspace)?;
    let mut config = EngineConfig::new(Role::Cli);
    config.time_zone = personal_time_zone(tz, &UserSettings::load_default())?;
    output::set_time_zone(config.time_zone.clone());
    config.agent_folders = gk_engine::AgentFolders::Personal;
    passphrase::unlock(&layout)?;
    let engine = Engine::open(&layout, config)?;
    let ctx = Context::user(Via::Cli).with_confirmation(confirm);
    let mut result = engine.call(&ctx, cmd.op.id, input.clone());
    // An encrypted export to import: ask for its passphrase and try again.
    let mut tries = 0;
    while let Err(err) = &result
        && passphrase::wants_export_passphrase(err)
        && tries < 3
        && passphrase::ask_export().unwrap_or(false)
    {
        tries += 1;
        result = engine.call(&ctx, cmd.op.id, input.clone());
    }
    // Someone who only uses the command line and agents still gets a daily
    // backup. It never fails the command: the app and the next command try
    // again.
    if !engine.must_exit() {
        let _ = engine.backup_if_overdue();
    }
    engine.close();
    result
}

/// A permanent operation asks on the terminal; without one it is refused
/// unless `--yes` is given.
fn ask_on_terminal() -> Confirmation {
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Confirmation::Refuse;
    }
    Confirmation::Ask(Arc::new(|req| {
        eprint!(
            "{} ",
            messages::text("cli-confirm", &[("summary", messages::op_summary(req.op))])
        );
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let yes = messages::text("cli-confirm-yes", &[]);
        std::io::stdin().read_line(&mut answer).is_ok()
            && yes
                .split_whitespace()
                .any(|word| word == answer.trim().to_lowercase())
    }))
}

/// Writes to standard output; a closed pipe (`glyphkeel … | head`) is not an
/// error.
fn print_bytes(bytes: &[u8]) -> Result<(), OpError> {
    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(bytes).and_then(|()| stdout.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(messages::error(
            "cli-output-failed",
            &[("error", e.to_string())],
            OpError::storage,
        )),
    }
}

/// What is known before clap parses the command line.
#[derive(Debug, Default, PartialEq)]
struct Prescan {
    /// The arguments for the MCP server, when the command is `mcp`.
    mcp: Option<Vec<OsString>>,
    no_color: bool,
    /// The language `--lang` names, wherever it is given.
    lang: Option<String>,
}

/// Finds `mcp` behind the global options. Everything after it belongs to
/// the MCP server, which has options of its own; a global `--workspace` or
/// `--tz` given before `mcp` is passed on unless the server's own arguments
/// name one.
fn prescan(args: &[OsString]) -> Prescan {
    let mut pre = Prescan {
        lang: lang_flag(args),
        ..Prescan::default()
    };
    let mut workspace: Option<OsString> = None;
    let mut tz: Option<OsString> = None;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].to_string_lossy();
        match arg.as_ref() {
            "--no-color" => pre.no_color = true,
            "--json" | "--quiet" | "-q" => {}
            "--workspace" | "-w" => {
                i += 1;
                workspace = args.get(i).cloned();
            }
            a if a.starts_with("--workspace=") => {
                workspace = Some(OsString::from(&a["--workspace=".len()..]));
            }
            "--tz" => {
                i += 1;
                tz = args.get(i).cloned();
            }
            a if a.starts_with("--tz=") => {
                tz = Some(OsString::from(&a["--tz=".len()..]));
            }
            "--lang" => i += 1,
            a if a.starts_with("--lang=") => {}
            "mcp" => {
                let mut rest: Vec<OsString> = args[i + 1..].to_vec();
                let names_one = rest.iter().any(|a| {
                    let a = a.to_string_lossy();
                    a == "--workspace" || a.starts_with("--workspace=")
                });
                if let (Some(w), false) = (workspace, names_one) {
                    rest.splice(0..0, [OsString::from("--workspace"), w]);
                }
                let names_zone = rest.iter().any(|a| {
                    let a = a.to_string_lossy();
                    a == "--tz" || a.starts_with("--tz=")
                });
                if let (Some(z), false) = (tz, names_zone) {
                    rest.splice(0..0, [OsString::from("--tz"), z]);
                }
                pre.mcp = Some(rest);
                return pre;
            }
            _ => return pre,
        }
        i += 1;
    }
    pre
}

/// The language `--lang` names, before the help is built in it: the
/// option is global, so it may follow the command's words. Arguments after
/// `--` are values.
fn lang_flag(args: &[OsString]) -> Option<String> {
    let mut rest = args.iter().skip(1).map(|a| a.to_string_lossy());
    while let Some(arg) = rest.next() {
        if arg == "--" {
            break;
        }
        if arg == "--lang" {
            return rest.next().map(|v| v.into_owned());
        }
        if let Some(value) = arg.strip_prefix("--lang=") {
            return Some(value.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_option_is_found_anywhere_and_not_passed_to_mcp() {
        assert_eq!(
            prescan(&os(&["glyphkeel", "task", "show", "--lang", "de", "TM-1"]))
                .lang
                .as_deref(),
            Some("de")
        );
        assert_eq!(
            prescan(&os(&["glyphkeel", "--lang=fr", "--help"]))
                .lang
                .as_deref(),
            Some("fr")
        );
        assert_eq!(
            prescan(&os(&["glyphkeel", "x", "--", "--lang", "de"])).lang,
            None
        );
        let pre = prescan(&os(&[
            "glyphkeel",
            "--lang",
            "de",
            "mcp",
            "--profile",
            "full",
        ]));
        assert_eq!(pre.mcp, Some(os(&["--profile", "full"])));
    }

    /// Every kind of error clap 4 has.
    const ALL_KINDS: &[ErrorKind] = &[
        ErrorKind::InvalidValue,
        ErrorKind::UnknownArgument,
        ErrorKind::InvalidSubcommand,
        ErrorKind::NoEquals,
        ErrorKind::ValueValidation,
        ErrorKind::TooManyValues,
        ErrorKind::TooFewValues,
        ErrorKind::WrongNumberOfValues,
        ErrorKind::ArgumentConflict,
        ErrorKind::MissingRequiredArgument,
        ErrorKind::MissingSubcommand,
        ErrorKind::InvalidUtf8,
        ErrorKind::DisplayHelp,
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        ErrorKind::DisplayVersion,
        ErrorKind::Io,
        ErrorKind::Format,
    ];

    /// The kinds that are shown rather than reported as a mistake.
    const SHOWN: &[ErrorKind] = &[
        ErrorKind::DisplayHelp,
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        ErrorKind::DisplayVersion,
    ];

    #[test]
    fn every_error_kind_has_a_message() {
        let cli = gk_engine::Messages::new(messages::CLI_FTL);
        for kind in ALL_KINDS.iter().filter(|k| !SHOWN.contains(k)) {
            let id = usage_message_id(*kind);
            assert_ne!(id, "cli-usage-other", "{kind:?} has no message of its own");
            assert!(cli.has(id), "{kind:?}: no message {id}");
            // Without what clap found, the message still is in the
            // chosen language.
            gk_engine::language::with_language("en-XA", || {
                let text = usage_text(&clap::Error::raw(*kind, "the English text\n"));
                assert!(!text.contains("English"), "{kind:?}: {text}");
                assert!(text.starts_with('['), "{kind:?}: {text}");
            });
        }
    }

    /// Words of plain English in `text`: runs of ASCII letters except the
    /// ones in `names`.
    fn plain(text: &str, names: &[&str]) -> Vec<String> {
        text.split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| !w.is_empty() && !names.contains(w))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn usage_errors_are_in_the_chosen_language() {
        let cases: &[(&[&str], ErrorKind)] = &[
            (&["task", "list", "--bogus"], ErrorKind::UnknownArgument),
            (&["tsak"], ErrorKind::InvalidSubcommand),
            (
                &["task", "list", "--limit", "x"],
                ErrorKind::ValueValidation,
            ),
            (
                &["task", "list", "--group-by", "zz"],
                ErrorKind::InvalidValue,
            ),
            (&["task", "list", "--limit"], ErrorKind::InvalidValue),
            (
                &["task", "list", "--limit", "1", "--limit", "2"],
                ErrorKind::ArgumentConflict,
            ),
            (&["project", "create"], ErrorKind::MissingRequiredArgument),
        ];
        gk_engine::language::with_language("en-XA", || {
            let ops = commands::op_commands();
            for (args, kind) in cases {
                let argv = std::iter::once(&"glyphkeel").chain(args.iter());
                let err = commands::build(&ops)
                    .try_get_matches_from(argv)
                    .expect_err("a usage error");
                assert_eq!(err.kind(), *kind, "{args:?}");
                let text = usage_text(&err);
                let mut names: Vec<&str> = vec!["glyphkeel", "LIMIT", "GROUP", "BY", "NAME", "KEY"];
                names.extend(
                    args.iter()
                        .flat_map(|a| a.split(|c: char| !c.is_ascii_alphabetic())),
                );
                names.extend([
                    "status", "priority", "label", "parent", "due", "week", "project", "type",
                    "trash", "task", "key", "name",
                ]);
                let found = plain(&text, &names);
                assert!(found.is_empty(), "{args:?}: {found:?} in {text}");
            }
        });
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn mcp_gets_everything_after_it() {
        let pre = prescan(&os(&[
            "glyphkeel",
            "--json",
            "mcp",
            "--profile",
            "full",
            "--help",
        ]));
        assert_eq!(pre.mcp, Some(os(&["--profile", "full", "--help"])));
    }

    #[test]
    fn a_global_workspace_reaches_mcp() {
        let pre = prescan(&os(&[
            "glyphkeel",
            "-w",
            "Work",
            "mcp",
            "--profile",
            "full",
        ]));
        assert_eq!(
            pre.mcp,
            Some(os(&["--workspace", "Work", "--profile", "full"]))
        );
        let pre = prescan(&os(&[
            "glyphkeel",
            "--workspace=Work",
            "mcp",
            "--workspace",
            "Home",
        ]));
        assert_eq!(pre.mcp, Some(os(&["--workspace", "Home"])));
    }

    #[test]
    fn a_global_time_zone_reaches_mcp() {
        let pre = prescan(&os(&["glyphkeel", "--tz", "Asia/Tokyo", "mcp"]));
        assert_eq!(pre.mcp, Some(os(&["--tz", "Asia/Tokyo"])));
        let pre = prescan(&os(&[
            "glyphkeel",
            "--tz=UTC",
            "mcp",
            "--tz",
            "Europe/Berlin",
        ]));
        assert_eq!(pre.mcp, Some(os(&["--tz", "Europe/Berlin"])));
    }

    #[test]
    fn mcp_as_a_value_is_not_the_command() {
        assert_eq!(
            prescan(&os(&["glyphkeel", "--workspace", "mcp", "status"])).mcp,
            None
        );
        assert_eq!(prescan(&os(&["glyphkeel", "activity", "mcp"])).mcp, None);
    }
}
