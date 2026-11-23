// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Raw JSON lines: the protocol edge cases a typed client can't send.

#![allow(clippy::unwrap_used)]

mod common;

use common::workspace;
use gk_mcp::Options;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};
use tokio::task::JoinHandle;

struct Raw {
    reader: BufReader<ReadHalf<DuplexStream>>,
    writer: WriteHalf<DuplexStream>,
    server: JoinHandle<()>,
}

impl Raw {
    fn start(options: &Options) -> Self {
        let server = gk_mcp::open(options).unwrap();
        let (client_io, server_io) = tokio::io::duplex(1 << 20);
        let server = tokio::spawn(gk_mcp::serve(server, server_io));
        let (reader, writer) = tokio::io::split(client_io);
        Self {
            reader: BufReader::new(reader),
            writer,
            server,
        }
    }

    /// Sends one message and returns the response with the same ID,
    /// skipping notifications.
    async fn request(&mut self, message: Value) -> Value {
        let mut line = message.to_string();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.unwrap();
        loop {
            let mut answer = String::new();
            let n = self.reader.read_line(&mut answer).await.unwrap();
            assert!(n > 0, "the server closed the stream");
            let answer: Value = serde_json::from_str(&answer).unwrap();
            if answer.get("id") == message.get("id") {
                return answer;
            }
        }
    }

    async fn close(self) {
        drop(self.writer);
        drop(self.reader);
        self.server.await.unwrap();
    }
}

fn meta(version: &str, client: Option<&str>) -> Value {
    let mut meta = json!({
        "io.modelcontextprotocol/protocolVersion": version,
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    if let Some(name) = client {
        meta["io.modelcontextprotocol/clientInfo"] = json!({"name": name, "version": "1.0"});
    }
    meta
}

#[tokio::test]
async fn stateless_requests_and_their_edge_cases() {
    let (_dir, mut options) = workspace();
    options.agent_name = Some("Helper".to_owned());
    let mut raw = Raw::start(&options);

    // tools/list as the very first message.
    let r = raw
        .request(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list",
                        "params": {"_meta": meta("2026-07-28", None)}}))
        .await;
    assert_eq!(r["result"]["tools"][0]["name"], "get_workspace", "{r}");
    assert_eq!(r["result"]["resultType"], "complete", "{r}");
    assert!(r["result"]["ttlMs"].is_u64(), "{r}");

    // Without clientInfo the agent is named by --agent-name.
    let r = raw
        .request(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                        "params": {"name": "get_workspace", "arguments": {},
                                   "_meta": meta("2026-07-28", None)}}))
        .await;
    assert_eq!(
        r["result"]["structuredContent"]["caller"]["actor"]["name"], "Helper",
        "{r}"
    );
    assert_eq!(r["result"]["isError"], false, "{r}");

    // Missing _meta, partial _meta and an unknown version: JSON-RPC errors.
    let bad = [
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list",
               "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list",
               "params": {"_meta": meta("2099-01-01", Some("x"))}}),
    ];
    for message in bad {
        let r = raw.request(message.clone()).await;
        assert!(r["error"]["code"].is_i64(), "{message} -> {r}");
    }

    // The process keeps running.
    let r = raw
        .request(
            json!({"jsonrpc": "2.0", "id": 6, "method": "server/discover",
                        "params": {"_meta": meta("2026-07-28", Some("x"))}}),
        )
        .await;
    assert!(
        r["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!("2025-11-25")),
        "{r}"
    );
    assert!(
        r["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("not instructions")
    );
    raw.close().await;
}

#[tokio::test]
async fn legacy_results_carry_no_result_type() {
    let (_dir, options) = workspace();
    let mut raw = Raw::start(&options);
    let r = raw
        .request(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                                   "clientInfo": {"name": "claude-code", "version": "2.1.330"}}}))
        .await;
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18", "{r}");
    assert!(r["result"].get("resultType").is_none(), "{r}");
    let r = raw
        .request(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}))
        .await;
    assert!(r["result"].get("resultType").is_none(), "{r}");
    assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 3, "{r}");
    let r = raw
        .request(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                        "params": {"name": "get_workspace", "arguments": {}}}))
        .await;
    assert_eq!(
        r["result"]["structuredContent"]["caller"]["actor"]["name"],
        "Claude Code"
    );
    raw.close().await;
}
