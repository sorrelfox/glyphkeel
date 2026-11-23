// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The bridge over a real workspace: its guards (loopback, token, origin),
//! calls with typed errors, and change events. And that the desktop app
//! never links it.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use gk_devbridge::{Guard, bind, confirmation, serve};
use gk_engine::{Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{WebSocketStream, client_async};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const ORIGIN: &str = "http://127.0.0.1:5310";

async fn start(dir: &TempDir) -> u16 {
    let engine = Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        EngineConfig::new(Role::App),
    )
    .unwrap();
    let listener = bind(0).await.unwrap();
    let address = listener.local_addr().unwrap();
    assert!(address.ip().is_loopback());
    tokio::spawn(serve(
        listener,
        Arc::new(engine),
        Guard::new(TOKEN.to_owned()),
        confirmation(false),
    ));
    address.port()
}

type Socket = WebSocketStream<TcpStream>;

async fn connect(port: u16, token: &str, origin: Option<&str>) -> Result<Socket, u16> {
    let mut request = format!("ws://127.0.0.1:{port}/?token={token}")
        .into_client_request()
        .unwrap();
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("origin", HeaderValue::from_str(origin).unwrap());
    }
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    match client_async(request, stream).await {
        Ok((socket, _)) => Ok(socket),
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            Err(response.status().as_u16())
        }
        Err(other) => panic!("unexpected handshake error: {other}"),
    }
}

async fn next_json(socket: &mut Socket) -> Value {
    let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .expect("an answer within 5 s")
        .unwrap()
        .unwrap();
    let Message::Text(text) = message else {
        panic!("expected text, got {message:?}");
    };
    serde_json::from_str(text.as_str()).unwrap()
}

async fn send(socket: &mut Socket, message: Value) {
    socket
        .send(Message::text(message.to_string()))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_wrong_token_and_a_foreign_origin() {
    let dir = TempDir::new().unwrap();
    let port = start(&dir).await;
    assert_eq!(connect(port, "wrong", Some(ORIGIN)).await.err(), Some(401));
    assert_eq!(connect(port, "", Some(ORIGIN)).await.err(), Some(401));
    assert_eq!(
        connect(port, TOKEN, Some("http://evil.example"))
            .await
            .err(),
        Some(403)
    );
    assert_eq!(connect(port, TOKEN, Some("null")).await.err(), Some(403));
    assert_eq!(connect(port, TOKEN, None).await.err(), Some(403));
    assert!(
        connect(port, TOKEN, Some("http://localhost:5310"))
            .await
            .is_ok()
    );
}

#[test]
fn listens_only_within_its_ports() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let err = runtime.block_on(bind(8080)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn answers_calls_and_sends_change_events() {
    let dir = TempDir::new().unwrap();
    let port = start(&dir).await;
    let mut page = connect(port, TOKEN, Some(ORIGIN)).await.unwrap();
    let mut other = connect(port, TOKEN, Some(ORIGIN)).await.unwrap();

    send(&mut page, json!({ "id": 1, "op": "workspace.get" })).await;
    let reply = next_json(&mut page).await;
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["ok"]["name"], "Personal");
    assert_eq!(reply["ok"]["caller"]["via"], "app");

    send(
        &mut page,
        json!({ "id": 2, "op": "settings.update", "input": { "trash_days": 0 } }),
    )
    .await;
    let reply = next_json(&mut page).await;
    assert_eq!(reply["id"], 2);
    assert_eq!(reply["err"]["code"], "invalid");
    assert_eq!(reply["err"]["details"]["path"], "trash_days");

    send(&mut page, json!({ "id": 3, "op": "history.undo" })).await;
    assert_eq!(next_json(&mut page).await["err"]["code"], "not_found");

    page.send(Message::text("not json")).await.unwrap();
    assert!(next_json(&mut page).await["ipc"].is_string());

    send(
        &mut page,
        json!({ "id": 4, "op": "settings.update", "input": { "week_start": "sunday" } }),
    )
    .await;
    // The event and the answer race each other; both arrive.
    let mut got = [next_json(&mut page).await, next_json(&mut page).await];
    got.sort_by_key(|m| m.get("event").is_some());
    assert_eq!(got[0]["id"], 4);
    assert_eq!(
        got[1]["event"]["changes"][0]["fields"],
        json!(["week_start"])
    );
    let seen_by_other = next_json(&mut other).await;
    assert_eq!(seen_by_other["event"]["changes"][0]["entity"], "settings");
}

#[test]
fn the_desktop_app_never_links_the_bridge() {
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "gk-app",
            "-e",
            "normal,build",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    assert!(tree.contains("gk-engine"), "the tree looks wrong:\n{tree}");
    for banned in ["gk-devbridge", "tungstenite"] {
        assert!(!tree.contains(banned), "gk-app links {banned}");
    }
}
