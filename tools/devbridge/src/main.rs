// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `gk-devbridge`: serves a workspace's operations and change events on a
//! loopback WebSocket for the web UI in a browser. Development only.
//!
//! ```sh
//! cargo run -p gk-devbridge -- --workspace /tmp/gk-dev
//! pnpm --filter ui exec vite --mode bridge
//! # then open the page URL it prints
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use gk_devbridge::{DEFAULT_PORT, Guard, UI_PORT, bind, confirmation, random_token, serve};
use gk_engine::{Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;

#[derive(Debug, Parser)]
#[command(
    name = "gk-devbridge",
    about = "Serves a workspace to the web UI in a browser, on loopback (development only)"
)]
struct Args {
    /// The workspace folder; created if it doesn't exist.
    #[arg(long)]
    workspace: PathBuf,
    /// The port on 127.0.0.1, within 5320–5329.
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
    /// The token pages must send; random unless given.
    #[arg(long)]
    token: Option<String>,
    /// How permanent operations are answered, since there is no dialog.
    #[arg(long, value_parser = ["yes", "no"], default_value = "no")]
    confirm: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("gk-devbridge: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    // The bridge is a development tool. A release build of it is a
    // mistake, so it doesn't start.
    if !cfg!(debug_assertions) {
        bail!("this is a development tool and does not run from a release build");
    }
    let args = Args::parse();
    let token = match args.token {
        Some(token) if token.len() >= 16 => token,
        Some(_) => bail!("--token must have at least 16 characters"),
        None => random_token().context("no random token")?,
    };
    let layout = WorkspaceLayout::new(&args.workspace);
    let engine = Engine::open(&layout, EngineConfig::new(Role::App))
        .map_err(|e| anyhow::anyhow!("{}", e.message))
        .with_context(|| format!("cannot open {}", args.workspace.display()))?;
    let engine = Arc::new(engine);
    let _watcher = engine
        .watch_other_processes(Duration::from_millis(250))
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let listener = bind(args.port)
            .await
            .with_context(|| format!("cannot listen on 127.0.0.1:{}", args.port))?;
        let port = listener.local_addr()?.port();
        let mut out = std::io::stdout().lock();
        writeln!(out, "BRIDGE ws://127.0.0.1:{port}/?token={token}")?;
        writeln!(
            out,
            "PAGE http://127.0.0.1:{UI_PORT}/?bridge={port}&token={token}"
        )?;
        out.flush()?;
        drop(out);
        serve(
            listener,
            engine,
            Guard::new(token),
            confirmation(args.confirm == "yes"),
        )
        .await;
        Ok(())
    })
}
