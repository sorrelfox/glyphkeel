// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel mcp` as its own program: watching changes that other
//! programs make, and `--http` serving agents without the app, with the
//! token from the personal settings.

#![allow(clippy::unwrap_used)]

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use common::{Agent, Workspace, ZONE, glyphkeel};
use serde_json::json;

#[test]
fn changes_from_another_program_reach_a_watching_agent() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Home lab", "--key", "ZZ"]);
    ws.ok(&["-q", "task", "create", "--project", "ZZ", "--title", "Rack"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "ZZ",
        "--title",
        "Cables",
    ]);

    let mut agent = Agent::start(ws.home(), &ws.path(), "watcher");
    let task = "glyphkeel://tasks/ZZ-1";
    let query = "glyphkeel://queries?query=project%3AZZ+is%3Aopen";
    agent.request("resources/subscribe", json!({ "uri": task }));
    agent.request("resources/subscribe", json!({ "uri": query }));
    let wait = Duration::from_secs(15);

    // The command line is another program: the agent's server hears of the
    // change by looking at the workspace, not from its own engine.
    ws.ok(&["-q", "task", "edit", "ZZ-1", "--title", "Rack one"]);
    let mut heard = Vec::new();
    while heard.len() < 2 {
        let n = agent
            .notification("notifications/resources/updated", wait)
            .expect("an update in time");
        heard.push(n["params"]["uri"].as_str().unwrap().to_owned());
    }
    heard.sort();
    // A new title changes the task and the query's result (the titles are
    // in it).
    assert_eq!(heard, [query, task]);

    // A change to another task changes the query's result only.
    ws.ok(&["-q", "task", "edit", "ZZ-2", "--title", "Cables two"]);
    let n = agent
        .notification("notifications/resources/updated", wait)
        .expect("an update in time");
    assert_eq!(n["params"]["uri"], query);
    assert!(
        agent
            .notification("notifications/resources/updated", Duration::from_secs(3))
            .is_none(),
        "the watched task didn't change"
    );
    agent.finish();
}

/// A port on 127.0.0.1 that was free a moment ago.
fn free_port() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

fn settings_file(home: &Path) -> PathBuf {
    home.join("config")
        .join("glyphkeel")
        .join(gk_store::USER_SETTINGS_FILE)
}

/// `glyphkeel mcp --http` running headless, and what it writes on standard
/// error.
struct Endpoint {
    child: Child,
    /// Its standard error, kept open while it runs.
    _lines: Receiver<String>,
    said: Vec<String>,
    port: u16,
}

impl Endpoint {
    /// Starts it with `args` after `mcp --http` and waits until it says
    /// where it listens.
    fn start(ws: &Workspace, args: &[&str]) -> Self {
        let mut child = glyphkeel(ws.home())
            .arg("--workspace")
            .arg(ws.path())
            .args(["--tz", ZONE, "mcp", "--http"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = BufReader::new(child.stderr.take().unwrap());
        let (sink, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in stderr.lines() {
                let Ok(line) = line else { return };
                if sink.send(line).is_err() {
                    return;
                }
            }
        });
        let mut said = Vec::new();
        let port = loop {
            let line = lines
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("no address in time: {said:?}"));
            said.push(line.clone());
            if let Some((_, rest)) = line.split_once("http://127.0.0.1:") {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                break digits.parse().unwrap();
            }
        };
        // What it says after the address (where the token is).
        while let Ok(line) = lines.recv_timeout(Duration::from_millis(300)) {
            said.push(line);
        }
        Self {
            child,
            _lines: lines,
            said,
            port,
        }
    }

    /// The status and body of an `initialize` POST sending `token`.
    fn initialize(&self, token: Option<&str>) -> (u16, String) {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "curl", "version": "8" }
            }
        })
        .to_string();
        let mut request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.port,
            body.len()
        );
        if let Some(token) = token {
            request.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(&body);
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status = text.split(' ').nth(1).unwrap().parse().unwrap();
        (status, text)
    }

    /// Stops it as Ctrl+C does and checks that it ends well.
    fn stop(self) {
        #[cfg(unix)]
        {
            use std::time::Instant;
            let mut this = self;
            let pid = this.child.id().to_string();
            let sent = std::process::Command::new("kill")
                .args(["-INT", &pid])
                .status()
                .unwrap();
            assert!(sent.success());
            let until = Instant::now() + Duration::from_secs(20);
            loop {
                if let Some(status) = this.child.try_wait().unwrap() {
                    assert!(status.success(), "it ended with {status}");
                    break;
                }
                assert!(Instant::now() < until, "it didn't stop on Ctrl+C");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_http_makes_a_token_when_there_is_none_and_serves_on_the_port_given() {
    let ws = Workspace::new();
    let file = settings_file(ws.home());
    assert!(!file.exists());
    let port = free_port();
    let endpoint = Endpoint::start(&ws, &["--port", &port.to_string()]);
    assert_eq!(endpoint.port, port, "{:?}", endpoint.said);
    let said = endpoint.said.join("\n");
    assert!(said.contains("Created a token"), "{said}");
    assert!(said.contains("agent_http.token"), "{said}");

    // The token is in the personal settings, readable by its owner only.
    let settings = gk_store::UserSettings::load(&file);
    let token = settings.agent_http.unwrap().token.unwrap();
    assert!(token.len() >= 32, "{token}");
    assert!(!said.contains(&token), "the token isn't printed");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{mode:o}");
    }

    assert_eq!(endpoint.initialize(None).0, 401);
    assert_eq!(endpoint.initialize(Some("not-the-token")).0, 401);
    let (status, body) = endpoint.initialize(Some(&token));
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"glyphkeel\""), "{body}");
    endpoint.stop();
}

#[test]
fn mcp_http_uses_the_token_and_port_of_the_personal_settings() {
    let ws = Workspace::new();
    let file = settings_file(ws.home());
    let port = free_port();
    let token = "a-token-the-person-chose-0123456789";
    let settings = gk_store::UserSettings {
        agent_http: Some(gk_store::AgentHttp {
            enabled: false,
            port: Some(port),
            token: Some(token.to_owned()),
        }),
        ..gk_store::UserSettings::default()
    };
    settings.save(&file).unwrap();
    let before = std::fs::read(&file).unwrap();

    let endpoint = Endpoint::start(&ws, &[]);
    assert_eq!(endpoint.port, port, "{:?}", endpoint.said);
    let said = endpoint.said.join("\n");
    assert!(!said.contains("Created a token"), "{said}");
    assert_eq!(
        std::fs::read(&file).unwrap(),
        before,
        "the file is unchanged"
    );
    assert_eq!(endpoint.initialize(None).0, 401);
    assert_eq!(endpoint.initialize(Some(token)).0, 200);
    endpoint.stop();

    // A port given on the command line wins over the setting.
    let other = free_port();
    let endpoint = Endpoint::start(&ws, &["--port", &other.to_string()]);
    assert_eq!(endpoint.port, other, "{:?}", endpoint.said);
    assert_eq!(endpoint.initialize(Some(token)).0, 200);
    endpoint.stop();
}

#[test]
fn mcp_http_on_a_taken_port_fails_with_a_conflict() {
    let ws = Workspace::new();
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let out = ws.run(&["mcp", "--http", "--port", &port]);
    assert_ne!(common::code(&out), 0);
    let err = common::stderr(&out);
    assert!(err.contains(&port) && err.contains("in use"), "{err}");
    // `--port` needs `--http`.
    let out = ws.run(&["mcp", "--port", &port]);
    assert_ne!(common::code(&out), 0);
}
