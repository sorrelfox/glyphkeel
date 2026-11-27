// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The MCP server: tools built from the operation catalog, served over
//! standard input and output, or over an opt-in local HTTP endpoint.
//!
//! An agent's MCP client starts `glyphkeel mcp`, which calls [`run_stdio`]
//! with the arguments after `mcp`. The server opens the workspace directly,
//! so the desktop app doesn't need to be running. One run is one agent
//! session: every change records the agent's name, `via = mcp` and the
//! session, and the person can revert the whole session later.
//!
//! The tools come from the catalog's tool list, in its order, filtered by
//! the agent's profile (capped by the workspace setting) and the toolsets.
//! Results carry `structuredContent` and a short Markdown text; errors are
//! `isError` results with text only.
//!
//! The prompts come from the catalog crate's prompt list, filtered the
//! same way: a prompt is listed when every tool it uses is listed.

mod confirm;
mod errors;
mod guard;
mod http;
mod messages;
mod options;
mod prompts;
mod random;
mod render;
mod resources;
mod server;
mod session;
mod tools;

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use gk_api::OpError;
use gk_engine::{Engine, EngineConfig, Role};
use rmcp::ServiceExt;
use rmcp::transport::IntoTransport;
use rmcp::transport::async_rw::AsyncRwTransport;

pub use errors::error_text;
pub use guard::WRITES_PER_MINUTE;
pub use http::HttpEndpoint;
pub use options::{DEFAULT_WORKSPACE, Options, workspace_layout};
pub use random::new_token;
pub use server::{INSTRUCTIONS, SERVER_NAME, Server, Setup};
pub use session::{ClientInfo, display_name};

/// How often the server does its background work (taking over the jobs
/// lock, checkpoints) and checks that the workspace wasn't upgraded.
const TICK: Duration = Duration::from_secs(1);

/// Runs `glyphkeel mcp` with `args`, the arguments after `mcp`, until the
/// client closes standard input.
///
/// Exit codes: 0 when the input closed, 1 for a usage error, otherwise the
/// error's exit code (4 when the workspace can't be opened).
pub fn run_stdio(args: &[OsString]) -> ExitCode {
    let options = match Options::parse_args(args) {
        Ok(options) => options,
        Err(e) => {
            // `--help` and `--version` print and succeed; the rest are usage
            // errors. Both go where clap sends them.
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    init_logging(&options);
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!(error = %e, "starting the runtime failed");
            eprintln!("glyphkeel mcp: {e}");
            return ExitCode::from(4);
        }
    };
    if options.http {
        drop(runtime);
        return run_http(&options);
    }
    let result = runtime.block_on(async {
        let server = open(&options)?;
        serve(server, stdio_transport()).await;
        Ok::<(), OpError>(())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("glyphkeel mcp: {}", error_text(&e));
            ExitCode::from(e.code.exit_code())
        }
    }
}

/// `glyphkeel mcp --http`: the HTTP endpoint without the app, until Ctrl+C
/// or a newer program upgrades the workspace.
fn run_http(options: &Options) -> ExitCode {
    let started = open_engine(options).and_then(|engine| {
        let (token, port) = http_settings(options)?;
        let port = options.port.or(port).unwrap_or(0);
        HttpEndpoint::start(Arc::new(engine), options.setup(), port, token, true)
    });
    let endpoint = match started {
        Ok(endpoint) => endpoint,
        Err(e) => {
            eprintln!("glyphkeel mcp: {}", error_text(&e));
            return ExitCode::from(e.code.exit_code());
        }
    };
    eprintln!(
        "{}",
        messages::text("mcp-http-listening", &[("url", endpoint.url())])
    );
    let stop = endpoint.stopper();
    let _ = std::thread::Builder::new()
        .name("stop-signal".to_owned())
        .spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                runtime.block_on(async {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => stop.cancel(),
                        () = stop.cancelled() => {}
                    }
                });
            }
        });
    endpoint.wait();
    ExitCode::SUCCESS
}

/// The token and port of the HTTP endpoint from the personal settings; a
/// token is created and saved when there is none.
fn http_settings(options: &Options) -> Result<(String, Option<u16>), OpError> {
    let file = options
        .settings_file
        .clone()
        .or_else(gk_store::user_settings_file)
        .ok_or_else(|| messages::error("mcp-http-no-settings-file", &[], OpError::storage))?;
    let mut settings = gk_store::UserSettings::load(&file);
    let http = settings.agent_http.get_or_insert_with(Default::default);
    let port = http.port;
    let shown = [("file", file.display().to_string())];
    if let Some(token) = http.token.clone().filter(|t| !t.trim().is_empty()) {
        eprintln!("{}", messages::text("mcp-http-token-where", &shown));
        return Ok((token, port));
    }
    let token = random::new_token();
    http.token = Some(token.clone());
    settings.save(&file).map_err(|e| {
        messages::error(
            "mcp-http-failed",
            &[
                ("port", port.unwrap_or(0).to_string()),
                ("error", e.to_string()),
            ],
            OpError::storage,
        )
    })?;
    eprintln!("{}", messages::text("mcp-http-token-created", &shown));
    eprintln!("{}", messages::text("mcp-http-token-where", &shown));
    Ok((token, port))
}

/// The largest message a client may send: one line of JSON-RPC. A longer
/// line is dropped unread (the client gets no answer to it) instead of
/// growing the read buffer until memory runs out. It leaves room for an
/// attachment of about 12 MB sent as Base64; larger files are attached by
/// path.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Standard input and output as the server's transport.
fn stdio_transport() -> AsyncRwTransport<rmcp::RoleServer, tokio::io::Stdin, tokio::io::Stdout> {
    limited_transport(tokio::io::stdin(), tokio::io::stdout())
}

/// A transport over `read` and `write`, one JSON-RPC message a line, with
/// [`MAX_MESSAGE_BYTES`] as the limit on each incoming message.
pub fn limited_transport<R, W>(read: R, write: W) -> AsyncRwTransport<rmcp::RoleServer, R, W>
where
    R: Send + tokio::io::AsyncRead + Unpin,
    W: Send + tokio::io::AsyncWrite + Unpin + 'static,
{
    AsyncRwTransport::new_server(read, write).with_max_line_length(MAX_MESSAGE_BYTES)
}

/// Opens the workspace the options name and builds the handler.
///
/// # Errors
/// When the workspace can't be found, opened or is too new.
pub fn open(options: &Options) -> Result<Server, OpError> {
    let engine = open_engine(options)?;
    Ok(Server::new(
        engine,
        options.profile,
        options.toolsets(),
        options.project.clone(),
        options.agent_name.clone(),
    ))
}

/// Opens the workspace the options name for agents.
///
/// # Errors
/// As [`open`].
pub fn open_engine(options: &Options) -> Result<Engine, OpError> {
    let layout = options.layout()?;
    let mut config = EngineConfig::new(Role::Mcp);
    let settings = match &options.settings_file {
        Some(file) => gk_store::UserSettings::load(file),
        None => gk_store::UserSettings::load_default(),
    };
    config.time_zone = gk_engine::personal_time_zone(options.tz.as_deref(), &settings)?;
    config.agent_folders = match (&options.agent_folders, &options.settings_file) {
        (Some(folders), _) => gk_engine::AgentFolders::Fixed(folders.clone()),
        (None, Some(file)) => gk_engine::AgentFolders::File(file.clone()),
        (None, None) => gk_engine::AgentFolders::Personal,
    };
    let engine = Engine::open(&layout, config)?;
    tracing::info!(
        workspace = %layout.root().display(),
        profile = %options.profile,
        "workspace open"
    );
    Ok(engine)
}

/// Serves `server` over `transport` until the client closes it or a newer
/// program upgrades the workspace, then ends the session and closes the
/// workspace.
pub async fn serve<T, E, A>(server: Server, transport: T)
where
    T: IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let handle = server.clone();
    match server.serve(transport).await {
        Ok(running) => {
            let mut token = Some(running.cancellation_token());
            let peer = running.peer().clone();
            let waiting = running.waiting();
            tokio::pin!(waiting);
            let mut tick = tokio::time::interval(TICK);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = &mut waiting => break,
                    _ = tick.tick() => {
                        if let Err(e) = handle.engine().jobs_tick() {
                            tracing::warn!(error = %e, "background work failed");
                        }
                        // Clients of the old era learn of a new tool list
                        // by a notification; newer ones by the list's
                        // short cache time.
                        if peer.peer_info().is_some() && handle.tool_list_changed() {
                            let _ = peer.notify_tool_list_changed().await;
                        }
                        if peer.peer_info().is_some() && handle.prompt_list_changed() {
                            let _ = peer.notify_prompt_list_changed().await;
                        }
                        if handle.engine().must_exit()
                            && let Some(token) = token.take()
                        {
                            tracing::warn!("a newer program upgraded the workspace; stopping");
                            token.cancel();
                        }
                    }
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "the client left before the session started"),
    }
    if let Some(engine) = handle.finish() {
        engine.close();
    }
}

fn init_logging(options: &Options) {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(options.log_level)
        .with_target(false)
        .try_init();
}
