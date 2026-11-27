// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The bridge over a real workspace: its guards (loopback, token, origin,
//! ports), calls with typed errors, and change events. And that the desktop
//! app never links it.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use gk_devbridge::{DEFAULT_PORTS, Guard, bind, check_ui_ports, confirmation, parse_ports, serve};
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
    start_with(dir, Guard::new(TOKEN.to_owned())).await
}

async fn start_with(dir: &TempDir, guard: Guard) -> u16 {
    let engine = Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        EngineConfig::new(Role::App),
    )
    .unwrap();
    let listener = bind(0, &DEFAULT_PORTS).await.unwrap();
    let address = listener.local_addr().unwrap();
    assert!(address.ip().is_loopback());
    tokio::spawn(serve(
        listener,
        Arc::new(engine),
        guard,
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

#[tokio::test(flavor = "multi_thread")]
async fn lets_in_only_the_ui_ports_it_was_started_for() {
    let dir = TempDir::new().unwrap();
    let port = start_with(&dir, Guard::for_ui_ports(TOKEN.to_owned(), &[5341, 5342])).await;
    for allowed in ["http://127.0.0.1:5341", "http://localhost:5342"] {
        assert!(
            connect(port, TOKEN, Some(allowed)).await.is_ok(),
            "{allowed} is one of ours"
        );
    }
    for foreign in [
        // The default UI port, and the neighbours of ours.
        "http://127.0.0.1:5310",
        "http://127.0.0.1:5340",
        "http://127.0.0.1:5343",
        // Ours, but not exactly.
        "https://127.0.0.1:5341",
        "http://127.0.0.1:5341/",
        "http://127.0.0.1:53410",
        "http://127.0.0.2:5341",
        "http://[::1]:5341",
        "http://evil.example:5341",
        "http://127.0.0.1",
    ] {
        assert_eq!(
            connect(port, TOKEN, Some(foreign)).await.err(),
            Some(403),
            "{foreign} is refused"
        );
    }
    // The right origin still needs the token.
    assert_eq!(
        connect(port, "wrong", Some("http://127.0.0.1:5341"))
            .await
            .err(),
        Some(401)
    );
}

#[test]
fn listens_only_within_its_ports() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let err = runtime.block_on(bind(8080, &DEFAULT_PORTS)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    let err = runtime.block_on(bind(5400, &DEFAULT_PORTS)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    let err = runtime.block_on(bind(5399, &(9000..=9010))).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn the_port_range_is_read_from_its_variable() {
    assert_eq!(DEFAULT_PORTS, 5300..=5399);
    assert_eq!(parse_ports("5300-5399"), Ok(5300..=5399));
    assert_eq!(parse_ports(" 6000-6000 "), Ok(6000..=6000));
    for wrong in [
        "",
        "5300",
        "5399-5300",
        "80-90",
        "5300-70000",
        "a-b",
        "5300–5399",
    ] {
        assert!(parse_ports(wrong).is_err(), "{wrong:?} is not a range");
    }
    assert!(check_ui_ports(&[5300, 5341, 5399], &DEFAULT_PORTS).is_ok());
    let err = check_ui_ports(&[5341, 5299], &DEFAULT_PORTS).unwrap_err();
    assert!(err.contains("5299"), "{err}");
}

/// Runs the bridge's program with `args` and `GK_PORT_RANGE` if given; it
/// must refuse to start, and its error is returned.
fn refused(dir: &TempDir, range: Option<&str>, args: &[&str]) -> String {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_gk-devbridge"));
    cmd.arg("--workspace")
        .arg(dir.path().join("Personal"))
        .args(args)
        .env_remove("GK_PORT_RANGE")
        .stdin(std::process::Stdio::null());
    if let Some(range) = range {
        cmd.env("GK_PORT_RANGE", range);
    }
    let output = cmd.output().unwrap();
    assert!(!output.status.success(), "{args:?} started the bridge");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_program_refuses_ports_outside_its_range() {
    let dir = TempDir::new().unwrap();
    let err = refused(&dir, None, &["--ui-port", "8080"]);
    assert!(err.contains("--ui-port") && err.contains("8080"), "{err}");
    let err = refused(&dir, None, &["--ui-port", "5341", "--ui-port", "5400"]);
    assert!(err.contains("5400"), "{err}");
    // The default UI port, outside a range of other ports.
    let err = refused(&dir, Some("9000-9010"), &["--port", "9001"]);
    assert!(err.contains("5310"), "{err}");
    let err = refused(
        &dir,
        Some("9000-9010"),
        &["--port", "5321", "--ui-port", "9002"],
    );
    assert!(err.contains("5321"), "{err}");
    let err = refused(&dir, Some("bogus"), &[]);
    assert!(err.contains("GK_PORT_RANGE"), "{err}");
}

#[test]
fn the_browser_flows_use_ports_the_bridge_accepts() {
    let table = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/tests/flows.json");
    let table: Value = serde_json::from_str(&std::fs::read_to_string(table).unwrap()).unwrap();
    assert_eq!(
        table["port_range"],
        json!([DEFAULT_PORTS.start(), DEFAULT_PORTS.end()]),
        "flows.json has the bridge's default range"
    );
    for flow in table["flows"].as_array().unwrap() {
        let ports = flow["ports"].as_object().unwrap();
        if !ports.contains_key("bridge") {
            continue;
        }
        for (kind, port) in ports {
            let port = u16::try_from(port.as_u64().unwrap()).unwrap();
            assert!(
                DEFAULT_PORTS.contains(&port),
                "`{}`'s {kind} port {port} is outside the bridge's ports",
                flow["name"]
            );
        }
    }
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
