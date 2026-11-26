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

mod commands;
mod input;
mod launch;
mod messages;
mod output;
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
        return gk_mcp::run_stdio(&mcp_args);
    }
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
/// parse error is a usage error, exit code 1.
fn usage_error(e: &clap::Error) -> ExitCode {
    let _ = e.print();
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
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
    let engine = Engine::open(&layout, config)?;
    let ctx = Context::user(Via::Cli).with_confirmation(confirm);
    let result = engine.call(&ctx, cmd.op.id, input);
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
            messages::text("cli-confirm", &[("summary", req.op.summary.to_owned())])
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
}

/// Finds `mcp` behind the global options. Everything after it belongs to
/// the MCP server, which has options of its own; a global `--workspace` or
/// `--tz` given before `mcp` is passed on unless the server's own arguments
/// name one.
fn prescan(args: &[OsString]) -> Prescan {
    let mut pre = Prescan::default();
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

#[cfg(test)]
mod tests {
    use super::*;

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
