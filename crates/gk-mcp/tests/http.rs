// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The local HTTP endpoint: loopback only, the token required, `Host` and
//! `Origin` checked, and the same agent sessions as over standard input
//! and output, in both protocol eras.

#![allow(clippy::unwrap_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use common::{golden_names, open_as_person, workspace};
use gk_mcp::HttpEndpoint;
use serde_json::{Value, json};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn start(options: &gk_mcp::Options) -> HttpEndpoint {
    let engine = Arc::new(gk_mcp::open_engine(options).unwrap());
    HttpEndpoint::start(engine, options.setup(), 0, TOKEN.to_owned(), false).unwrap()
}

/// The status code of one raw `initialize` POST with `headers`.
fn status(endpoint: &HttpEndpoint, headers: &[(&str, String)]) -> u16 {
    let body = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "raw", "version": "1" }
        }
    })
    .to_string();
    let mut request = format!(
        "POST /mcp HTTP/1.1\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&body);
    let mut stream = TcpStream::connect(endpoint.addr()).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut answer = Vec::new();
    let mut buf = [0_u8; 1024];
    // The status line is enough; an event stream may stay open.
    while !answer.windows(2).any(|w| w == b"\r\n") {
        let n = stream.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        answer.extend_from_slice(&buf[..n]);
    }
    let line = String::from_utf8_lossy(&answer);
    line.split(' ').nth(1).unwrap().parse().unwrap()
}

#[test]
fn the_endpoint_wants_the_token_and_its_own_host_and_origin() {
    let (_dir, options) = workspace();
    let endpoint = start(&options);
    assert!(endpoint.addr().ip().is_loopback());
    assert_eq!(endpoint.addr().ip().to_string(), "127.0.0.1");
    let port = endpoint.addr().port();
    assert_eq!(endpoint.url(), format!("http://127.0.0.1:{port}/mcp"));
    let host = ("Host", format!("127.0.0.1:{port}"));
    let bearer = ("Authorization", format!("Bearer {TOKEN}"));

    // No token, or a wrong one.
    assert_eq!(status(&endpoint, std::slice::from_ref(&host)), 401);
    assert_eq!(
        status(
            &endpoint,
            &[host.clone(), ("Authorization", "Bearer nope".to_owned())]
        ),
        401
    );
    // A web page elsewhere, with the token: refused by its Origin.
    assert_eq!(
        status(
            &endpoint,
            &[
                host.clone(),
                bearer.clone(),
                ("Origin", "http://evil.example".to_owned())
            ]
        ),
        403
    );
    // A name that points at 127.0.0.1 (DNS rebinding): refused by its Host.
    assert_eq!(
        status(
            &endpoint,
            &[("Host", format!("evil.example:{port}")), bearer.clone()]
        ),
        403
    );
    // The endpoint's own host and origin, with the token.
    assert_eq!(
        status(
            &endpoint,
            &[
                host.clone(),
                bearer.clone(),
                ("Origin", format!("http://127.0.0.1:{port}"))
            ]
        ),
        200
    );
    assert_eq!(
        status(&endpoint, &[("Host", format!("localhost:{port}")), bearer]),
        200
    );
    endpoint.stop();
}

/// What one request got back: the status, the headers (names in lower
/// case) and the JSON-RPC messages of the body.
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    messages: Vec<Value>,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The result of the request with `id`.
    fn result(&self, id: u64) -> &Value {
        self.messages
            .iter()
            .find(|m| m["id"] == id)
            .map(|m| &m["result"])
            .unwrap_or_else(|| panic!("no answer to {id}: {:?}", self.messages))
    }
}

/// A POST of `body` to `path` with the token and `headers`, read to its end.
fn post(endpoint: &HttpEndpoint, path: &str, headers: &[(&str, String)], body: &Value) -> Answer {
    let body = body.to_string();
    let port = endpoint.addr().port();
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&body);
    let mut stream = TcpStream::connect(endpoint.addr()).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_lowercase(), v.trim().to_owned()))
        .collect();
    let mut body = raw[split + 4..].to_vec();
    if headers
        .iter()
        .any(|(n, v)| n == "transfer-encoding" && v.contains("chunked"))
    {
        body = dechunk(&body);
    }
    let text = String::from_utf8_lossy(&body).into_owned();
    let messages = if text.trim_start().starts_with('{') {
        vec![serde_json::from_str(&text).unwrap()]
    } else {
        text.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .filter_map(|d| serde_json::from_str(d.trim()).ok())
            .collect()
    };
    Answer {
        status,
        headers,
        messages,
    }
}

/// A chunked body joined.
fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(end) = body.windows(2).position(|w| w == b"\r\n") {
        let size =
            usize::from_str_radix(String::from_utf8_lossy(&body[..end]).trim(), 16).unwrap_or(0);
        if size == 0 || body.len() < end + 2 + size {
            break;
        }
        out.extend_from_slice(&body[end + 2..end + 2 + size]);
        body = &body[(end + 4 + size).min(body.len())..];
    }
    out
}

fn tool_names(list: &Value) -> Vec<String> {
    list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn an_old_era_client_over_http_is_one_agent_session() {
    let (_dir, options) = workspace();
    let endpoint = start(&options);
    let init = post(
        &endpoint,
        "/mcp",
        &[],
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "claude-code", "version": "2.1.330" }
            }
        }),
    );
    assert_eq!(init.status, 200);
    assert_eq!(init.result(1)["serverInfo"]["name"], "glyphkeel");
    let session = (
        "Mcp-Session-Id",
        init.header("mcp-session-id").unwrap().to_owned(),
    );
    let version = ("MCP-Protocol-Version", "2025-11-25".to_owned());
    let done = post(
        &endpoint,
        "/mcp",
        &[session.clone(), version.clone()],
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );
    assert_eq!(done.status, 202);

    let list = post(
        &endpoint,
        "/mcp",
        &[session.clone(), version.clone()],
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    );
    assert_eq!(tool_names(list.result(2)), golden_names("standard"));
    for id in [3, 4] {
        let call = post(
            &endpoint,
            "/mcp",
            &[session.clone(), version.clone()],
            &json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": "get_workspace", "arguments": {} } }),
        );
        let out = &call.result(id)["structuredContent"];
        assert_eq!(out["caller"]["via"], "mcp");
        assert_eq!(out["caller"]["actor"]["name"], "Claude Code");
    }
    endpoint.stop();

    let (engine, person) = open_as_person(&options);
    let sessions = engine.call(&person, "agent.sessions", json!({})).unwrap();
    assert_eq!(sessions["total"], 1, "{sessions}");
    let s = &sessions["items"][0];
    assert_eq!(s["client"], "claude-code");
    assert!(s["ended_at"].is_string(), "{s}");
    assert_eq!(s["reads"], 2, "{s}");
    engine.close();
}

#[test]
fn a_2026_client_over_http_shares_one_session_and_profiles_have_paths() {
    let (_dir, options) = workspace();
    let endpoint = start(&options);
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "claude-code", "version": "2.1.330" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let headers = |method: &str, name: Option<&str>| {
        let mut h = vec![
            ("MCP-Protocol-Version", "2026-07-28".to_owned()),
            ("Mcp-Method", method.to_owned()),
        ];
        if let Some(name) = name {
            h.push(("Mcp-Name", name.to_owned()));
        }
        h
    };
    let list = post(
        &endpoint,
        "/mcp/read-only",
        &headers("tools/list", None),
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": { "_meta": meta } }),
    );
    assert_eq!(list.status, 200, "{:?}", list.messages);
    assert_eq!(tool_names(list.result(1)), golden_names("read-only"));
    for id in [2, 3] {
        let call = post(
            &endpoint,
            "/mcp",
            &headers("tools/call", Some("get_workspace")),
            &json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": "get_workspace", "arguments": {}, "_meta": meta } }),
        );
        assert_eq!(call.status, 200, "{:?}", call.messages);
        let out = &call.result(id)["structuredContent"];
        assert_eq!(out["caller"]["profile"], "standard");
    }
    endpoint.stop();

    // The two calls to /mcp are one session; the list on /mcp/read-only
    // started none.
    let (engine, person) = open_as_person(&options);
    let sessions = engine.call(&person, "agent.sessions", json!({})).unwrap();
    assert_eq!(sessions["total"], 1, "{sessions}");
    assert_eq!(sessions["items"][0]["reads"], 2);
    assert!(sessions["items"][0]["ended_at"].is_string());
    engine.close();
}

#[test]
fn a_taken_port_is_a_conflict_and_an_empty_token_is_refused() {
    let (_dir, options) = workspace();
    let first = start(&options);
    let engine = Arc::new(gk_mcp::open_engine(&options).unwrap());
    let e = HttpEndpoint::start(
        Arc::clone(&engine),
        options.setup(),
        first.addr().port(),
        TOKEN.to_owned(),
        false,
    )
    .unwrap_err();
    assert_eq!(e.code, gk_api::ErrorCode::Conflict);
    let e = HttpEndpoint::start(engine, options.setup(), 0, " ".to_owned(), false).unwrap_err();
    assert_eq!(e.code, gk_api::ErrorCode::Invalid);
    first.stop();
}
