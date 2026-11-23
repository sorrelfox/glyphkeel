// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The development bridge: the same `op` the desktop app offers its page,
//! and the same change events, over a WebSocket on loopback. With it the
//! web UI runs in an ordinary browser against a real workspace, for the
//! browser tests.
//!
//! Guards:
//! - it listens on 127.0.0.1 only;
//! - a connection must carry the bridge's random token (`?token=`), or it
//!   gets HTTP 401;
//! - its `Origin` must be the UI's development server, or it gets HTTP 403;
//! - it is a separate program that the app never links, and it refuses to
//!   run from a release build.
//!
//! Protocol, one JSON text message each:
//! - page → bridge: `{ "id": 1, "op": "workspace.get", "input": {} }`;
//! - bridge → page: `{ "id": 1, "ok": <output> }` or
//!   `{ "id": 1, "err": <OpError> }`; a message the bridge can't read gets
//!   `{ "id": …, "ipc": "<reason>" }`;
//! - bridge → page: `{ "event": <Changed> }` after every commit.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use gk_api::ops::common::Via;
use gk_api::{Changed, OpError};
use gk_engine::{Confirmation, Context, Engine};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;

/// The ports the bridge may use.
pub const PORTS: std::ops::RangeInclusive<u16> = 5320..=5329;
/// The bridge's default port.
pub const DEFAULT_PORT: u16 = 5321;
/// The UI's development server, the only origin allowed to connect.
pub const UI_PORT: u16 = 5310;

/// How many events wait for a slow page before it is disconnected.
const EVENT_BACKLOG: usize = 65_536;

/// What the bridge accepts.
#[derive(Debug, Clone)]
pub struct Guard {
    /// The token every connection must carry.
    pub token: String,
    /// The `Origin` values allowed to connect.
    pub origins: Vec<String>,
}

impl Guard {
    /// A guard with `token`, for pages from the UI's development server.
    #[must_use]
    pub fn new(token: String) -> Self {
        Self {
            token,
            origins: vec![
                format!("http://127.0.0.1:{UI_PORT}"),
                format!("http://localhost:{UI_PORT}"),
            ],
        }
    }

    /// Checks a WebSocket upgrade: the origin first, then the token.
    #[allow(clippy::result_large_err)] // the handshake's own error type
    fn check(&self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        let origin = request
            .headers()
            .get("origin")
            .and_then(|o| o.to_str().ok())
            .unwrap_or_default();
        if !self.origins.iter().any(|allowed| allowed == origin) {
            return Err(refuse(StatusCode::FORBIDDEN, "origin not allowed"));
        }
        let token_ok = request.uri().query().is_some_and(|query| {
            query
                .split('&')
                .filter_map(|pair| pair.strip_prefix("token="))
                .any(|token| constant_time_eq(token.as_bytes(), self.token.as_bytes()))
        });
        if !token_ok {
            return Err(refuse(StatusCode::UNAUTHORIZED, "wrong or missing token"));
        }
        Ok(response)
    }
}

fn refuse(status: StatusCode, why: &str) -> ErrorResponse {
    eprintln!("gk-devbridge: refused a connection: {why}");
    let mut response = ErrorResponse::new(Some(why.to_owned()));
    *response.status_mut() = status;
    response
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 128 random bits from the operating system, as hex.
///
/// # Errors
/// When `/dev/urandom` can't be read.
pub fn random_token() -> std::io::Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Binds to `port` on loopback only.
///
/// # Errors
/// When the port is taken or outside [`PORTS`] (0 picks a free one, for
/// tests).
pub async fn bind(port: u16) -> std::io::Result<TcpListener> {
    if port != 0 && !PORTS.contains(&port) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "the bridge's port must be within {}–{}",
                PORTS.start(),
                PORTS.end()
            ),
        ));
    }
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await
}

/// How the bridge answers `permanent` operations: there is no dialog, so
/// the answer is fixed when it starts.
#[must_use]
pub fn confirmation(yes: bool) -> Confirmation {
    if yes {
        Confirmation::Given
    } else {
        Confirmation::Refuse
    }
}

/// Serves `engine` on `listener` until the task is dropped.
pub async fn serve(
    listener: TcpListener,
    engine: Arc<Engine>,
    guard: Guard,
    confirm: Confirmation,
) {
    let (events, _) = broadcast::channel::<Arc<Changed>>(EVENT_BACKLOG);
    let from_engine = engine.subscribe();
    let to_pages = events.clone();
    // The engine's events arrive on a blocking channel; one thread passes
    // them on in order.
    std::thread::spawn(move || {
        for event in from_engine {
            let _ = to_pages.send(Arc::new(event));
        }
    });
    let guard = Arc::new(guard);
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            continue;
        };
        let connection = Connection {
            engine: Arc::clone(&engine),
            guard: Arc::clone(&guard),
            confirm: confirm.clone(),
            events: events.subscribe(),
        };
        tokio::spawn(async move {
            if let Err(err) = connection.run(stream).await {
                eprintln!("gk-devbridge: {peer}: {err}");
            }
        });
    }
}

struct Connection {
    engine: Arc<Engine>,
    guard: Arc<Guard>,
    confirm: Confirmation,
    events: broadcast::Receiver<Arc<Changed>>,
}

impl Connection {
    async fn run(self, stream: TcpStream) -> anyhow::Result<()> {
        let Self {
            engine,
            guard,
            confirm,
            mut events,
        } = self;
        // The handshake's callback must return its own (large) error type.
        #[allow(clippy::result_large_err)]
        let check = |req: &Request, res| guard.check(req, res);
        let socket = tokio_tungstenite::accept_hdr_async(stream, check).await?;
        let (mut sink, mut source) = socket.split();
        let (out, mut outgoing) = mpsc::unbounded_channel::<String>();

        let writer = tokio::spawn(async move {
            while let Some(text) = outgoing.recv().await {
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
        });
        let event_out = out.clone();
        let forwarder = tokio::spawn(async move {
            // A page that falls too far behind is dropped; it reconnects and
            // refetches what it shows.
            while let Ok(event) = events.recv().await {
                if event_out
                    .send(json!({ "event": *event }).to_string())
                    .is_err()
                {
                    break;
                }
            }
        });

        while let Some(message) = source.next().await {
            let text = match message? {
                Message::Text(text) => text.to_string(),
                Message::Close(_) => break,
                _ => continue,
            };
            let (engine, confirm, out) = (Arc::clone(&engine), confirm.clone(), out.clone());
            // Calls run side by side, as they do in the app.
            tokio::task::spawn_blocking(move || {
                let _ = out.send(answer(&engine, &confirm, &text).to_string());
            });
        }
        forwarder.abort();
        drop(out);
        let _ = writer.await;
        Ok(())
    }
}

/// The reply to one message from the page.
fn answer(engine: &Engine, confirm: &Confirmation, text: &str) -> Value {
    let Ok(request) = serde_json::from_str::<Value>(text) else {
        return json!({ "id": Value::Null, "ipc": "the message is not JSON" });
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(op) = request.get("op").and_then(Value::as_str) else {
        return json!({ "id": id, "ipc": "the message names no operation" });
    };
    let input = request.get("input").cloned().unwrap_or(Value::Null);
    let mut ctx = Context::user(Via::App).with_confirmation(confirm.clone());
    ctx.request_id = request
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match engine.call(&ctx, op, input) {
        Ok(output) => json!({ "id": id, "ok": output }),
        Err(err) => json!({ "id": id, "err": error_json(&err) }),
    }
}

fn error_json(err: &OpError) -> Value {
    serde_json::to_value(err)
        .unwrap_or_else(|_| json!({ "code": "storage", "message": err.message }))
}
