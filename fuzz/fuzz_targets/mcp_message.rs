// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any line an MCP client may send, to a running server over the transport
//! `glyphkeel mcp` uses: JSON-RPC parsing, both protocol eras, tool calls
//! with any arguments, prompts and completions. After each line the server
//! must still answer a ping: no line may end the session, panic or hang.
//!
//! The agent is read-only, so the workspace stays the same from one input
//! to the next; each input is one line (newlines become spaces), after a
//! prefix the first byte picks, so most inputs reach a method.

#![no_main]

use std::sync::{Mutex, OnceLock};

use libfuzzer_sys::fuzz_target;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

/// Starts of a message, so the fuzzer reaches the methods quickly.
const PREFIXES: &[&str] = &[
    "",
    r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}},"name":"get_task","arguments":"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search","arguments":{"query":"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_tasks","arguments":{"query":"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"completion/complete","params":{"ref":{"type":"ref/prompt","name":"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":"#,
    r#"{"jsonrpc":"2.0","method":"notifications/"#,
    r#"[{"jsonrpc":"2.0","id":1,"method":"#,
];

struct Client {
    runtime: tokio::runtime::Runtime,
    reader: BufReader<ReadHalf<DuplexStream>>,
    writer: WriteHalf<DuplexStream>,
    _dir: tempfile::TempDir,
}

fn client() -> &'static Mutex<Client> {
    static CLIENT: OnceLock<Mutex<Client>> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let workspace = dir.path().join("workspace");
        let mut options = gk_mcp::Options::with_workspace(workspace.to_str().expect("UTF-8"));
        options.profile = gk_api::Profile::ReadOnly;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("a runtime");
        let (reader, writer) = runtime.block_on(async {
            let server = gk_mcp::open(&options).expect("the workspace");
            let (client_io, server_io) = tokio::io::duplex(1 << 20);
            let (read, write) = tokio::io::split(server_io);
            tokio::spawn(gk_mcp::serve(
                server,
                gk_mcp::limited_transport(read, write),
            ));
            let (reader, writer) = tokio::io::split(client_io);
            (BufReader::new(reader), writer)
        });
        Mutex::new(Client {
            runtime,
            reader,
            writer,
            _dir: dir,
        })
    })
}

const PING: &str = "{\"jsonrpc\":\"2.0\",\"id\":\"fuzz-ping\",\"method\":\"ping\"}\n";

fuzz_target!(|data: &[u8]| {
    let Some((&pick, rest)) = data.split_first() else {
        return;
    };
    let mut line = PREFIXES[usize::from(pick) % PREFIXES.len()]
        .as_bytes()
        .to_vec();
    line.extend(rest.iter().map(|&b| if b == b'\n' { b' ' } else { b }));
    line.push(b'\n');
    let mut client = client()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Client {
        runtime,
        reader,
        writer,
        ..
    } = &mut *client;
    runtime.block_on(async {
        writer.write_all(&line).await.expect("the server reads");
        writer
            .write_all(PING.as_bytes())
            .await
            .expect("the server reads");
        loop {
            let mut answer = String::new();
            let n = reader
                .read_line(&mut answer)
                .await
                .expect("the server writes");
            assert!(
                n > 0,
                "the server closed the session after {:?}",
                String::from_utf8_lossy(&line)
            );
            let answer: serde_json::Value =
                serde_json::from_str(&answer).expect("the server writes JSON");
            if answer.get("id").and_then(|id| id.as_str()) == Some("fuzz-ping") {
                break;
            }
        }
    });
});
