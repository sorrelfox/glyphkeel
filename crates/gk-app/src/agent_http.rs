// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The local HTTP endpoint for agents whose client can't start a program,
//! served by the app over the workspace it has open.
//!
//! It is off by default. Preferences › Agents turns it on, chooses the
//! port (or a free one each start) and shows, copies and regenerates the
//! token. All three are personal settings (`agent_http` in the shared
//! `settings.json`), so `glyphkeel mcp --http` uses the same token.
//! Turning it on asks the person in a native dialog, as raising what
//! agents may do does: script in the page alone can't open it. It
//! listens on 127.0.0.1 only and follows the workspace: it stops while
//! the workspace closes and starts again over the one opened next.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use gk_api::{OpError, Profile, Toolset};
use gk_engine::Engine;
use gk_mcp::{HttpEndpoint, Setup};
use gk_store::{AgentHttp, UserSettings};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_settings::AppSettings;
use crate::ipc::Workspace;

/// What the endpoint should serve with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wanted {
    /// The port; 0 for a free one.
    pub(crate) port: u16,
    /// The bearer token.
    pub(crate) token: String,
}

/// The endpoint of the open workspace, as wanted and as running.
#[derive(Debug, Default)]
pub(crate) struct AgentEndpoint {
    state: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    wanted: Option<Wanted>,
    running: Option<HttpEndpoint>,
    /// Why it didn't start, if it didn't.
    error: Option<OpError>,
}

impl AgentEndpoint {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Serves as `wanted` over `engine` (`None`: off). A change of port or
    /// token starts it again; the same wish keeps it running.
    pub(crate) fn set(&self, wanted: Option<Wanted>, engine: Option<Arc<Engine>>) {
        let mut state = self.lock();
        if state.wanted == wanted && (state.running.is_some() || wanted.is_none()) {
            return;
        }
        if let Some(running) = state.running.take() {
            running.stop();
        }
        state.wanted = wanted;
        state.error = None;
        if let Some(engine) = engine {
            start(&mut state, engine);
        }
    }

    /// Stops serving while the workspace closes; the wish stays.
    pub(crate) fn pause(&self) {
        if let Some(running) = self.lock().running.take() {
            running.stop();
        }
    }

    /// Serves again, over the workspace opened now, if it is wanted.
    pub(crate) fn resume(&self, engine: Arc<Engine>) {
        let mut state = self.lock();
        if state.running.is_none() {
            start(&mut state, engine);
        }
    }

    /// The URL it serves at, and why it doesn't serve when it is wanted.
    pub(crate) fn status(&self) -> (Option<String>, Option<OpError>) {
        let state = self.lock();
        (
            state.running.as_ref().map(HttpEndpoint::url),
            state.error.clone(),
        )
    }
}

fn start(state: &mut Inner, engine: Arc<Engine>) {
    let Some(wanted) = state.wanted.clone() else {
        return;
    };
    match HttpEndpoint::start(engine, setup(), wanted.port, wanted.token, false) {
        Ok(running) => {
            state.running = Some(running);
            state.error = None;
        }
        Err(e) => {
            eprintln!(
                "glyphkeel-app: the agents' HTTP endpoint didn't start: {}",
                e.message
            );
            state.error = Some(e);
        }
    }
}

/// The agents the app's endpoint serves: as `glyphkeel mcp` without
/// options; `/mcp/full` and `/mcp/read-only` serve those profiles.
fn setup() -> Setup {
    Setup {
        profile: Profile::Standard,
        toolsets: Toolset::defaults(),
        project: None,
        agent_name: None,
    }
}

/// What the page sees.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct View {
    /// Whether the person turned it on.
    pub(crate) enabled: bool,
    /// The chosen port; absent: a free one each start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) port: Option<u16>,
    /// Where it serves now; absent while it doesn't.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    /// The token clients send; absent until the first time it is on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) token: Option<String>,
    /// Why it doesn't serve although it is on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<OpError>,
}

/// A change from Preferences; fields left out keep their value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Patch {
    #[serde(default)]
    pub(crate) enabled: Option<bool>,
    /// The port, 1024 to 65535; 0 for a free one each start.
    #[serde(default)]
    pub(crate) port: Option<u16>,
}

/// The settings as stored in `file`.
fn stored(file: Option<&Path>) -> AgentHttp {
    file.map(UserSettings::load)
        .and_then(|s| s.agent_http)
        .unwrap_or_default()
}

/// Changes the stored settings with `change`, keeping the file's other
/// fields, and returns them.
fn change(file: Option<&Path>, change: impl FnOnce(&mut AgentHttp)) -> Result<AgentHttp, OpError> {
    let Some(file) = file else {
        return Err(crate::messages::error(
            "agent-http-no-settings-file",
            &[],
            OpError::storage,
        ));
    };
    let mut user = UserSettings::load(file);
    let http = user.agent_http.get_or_insert_with(AgentHttp::default);
    change(http);
    let http = http.clone();
    user.save(file).map_err(|err| {
        crate::messages::error(
            "agent-http-not-stored",
            &[
                ("file", file.display().to_string()),
                ("error", err.to_string()),
            ],
            OpError::storage,
        )
    })?;
    Ok(http)
}

/// What the endpoint should serve with, for the stored settings.
fn wanted(http: &AgentHttp) -> Option<Wanted> {
    let token = http.token.clone().filter(|t| !t.trim().is_empty())?;
    http.enabled.then(|| Wanted {
        port: http.port.unwrap_or(0),
        token,
    })
}

fn view(http: &AgentHttp, workspace: &Workspace) -> View {
    let (url, error) = workspace.agent_http_status();
    View {
        enabled: http.enabled,
        port: http.port,
        url,
        token: http.token.clone(),
        error: error.filter(|_| http.enabled),
    }
}

/// Starts the endpoint at the app's start when the person left it on.
pub(crate) fn start_saved(settings: &AppSettings, workspace: &Workspace) {
    let http = stored(settings.user_file());
    workspace.set_agent_http(wanted(&http));
}

/// Applies `patch` to the settings in `file` and to the endpoint. Turning
/// it on asks the person first; a token is made the first time.
fn update(file: Option<&Path>, workspace: &Workspace, patch: &Patch) -> Result<View, OpError> {
    if let Some(port) = patch.port
        && port != 0
        && port < 1024
    {
        return Err(crate::messages::error(
            "agent-http-port-range",
            &[("port", port.to_string())],
            |m| OpError::invalid_at("port", m),
        ));
    }
    let before = stored(file);
    if patch.enabled == Some(true) && !before.enabled {
        workspace.confirm_agent_http()?;
    }
    let http = change(file, |http| {
        if let Some(enabled) = patch.enabled {
            http.enabled = enabled;
        }
        if let Some(port) = patch.port {
            http.port = (port != 0).then_some(port);
        }
        if http.enabled && http.token.as_deref().is_none_or(|t| t.trim().is_empty()) {
            http.token = Some(gk_mcp::new_token());
        }
    })?;
    workspace.set_agent_http(wanted(&http));
    Ok(view(&http, workspace))
}

/// Makes a new token: clients with the old one are refused from now on.
fn regenerate(file: Option<&Path>, workspace: &Workspace) -> Result<View, OpError> {
    let http = change(file, |http| http.token = Some(gk_mcp::new_token()))?;
    workspace.set_agent_http(wanted(&http));
    Ok(view(&http, workspace))
}

/// The endpoint's settings and state, for Preferences.
#[tauri::command]
pub(crate) fn agent_http(
    settings: State<'_, AppSettings>,
    workspace: State<'_, Arc<Workspace>>,
) -> View {
    view(&stored(settings.user_file()), &workspace)
}

/// Turns the endpoint on or off, or changes its port. Waits off the main
/// thread, because turning it on shows the native dialog.
#[tauri::command]
pub(crate) async fn update_agent_http(
    settings: State<'_, AppSettings>,
    workspace: State<'_, Arc<Workspace>>,
    patch: Patch,
) -> Result<View, OpError> {
    let file: Option<PathBuf> = settings.user_file().map(Path::to_path_buf);
    let workspace = Arc::clone(&workspace);
    tauri::async_runtime::spawn_blocking(move || update(file.as_deref(), &workspace, &patch))
        .await
        .map_err(|e| {
            crate::messages::error(
                "operation-stopped",
                &[("error", e.to_string())],
                OpError::storage,
            )
        })?
}

/// Makes a new token for the endpoint.
#[tauri::command]
pub(crate) fn regenerate_agent_token(
    settings: State<'_, AppSettings>,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<View, OpError> {
    regenerate(settings.user_file(), &workspace)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    use gk_engine::{Confirmation, EngineConfig, Role};
    use gk_store::WorkspaceLayout;

    use super::*;

    fn workspace(dir: &tempfile::TempDir, confirm: Confirmation) -> Workspace {
        Workspace::open(
            &WorkspaceLayout::new(dir.path().join("Personal")),
            EngineConfig::new(Role::App),
            confirm,
        )
    }

    /// The status code of a request to `url` with `token`.
    fn status(url: &str, token: &str) -> u16 {
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let request = format!(
            "POST /{path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let mut stream = TcpStream::connect(host).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut answer = [0_u8; 64];
        let n = stream.read(&mut answer).unwrap();
        let line = String::from_utf8_lossy(&answer[..n]);
        line.split(' ').nth(1).unwrap().parse().unwrap()
    }

    #[test]
    fn turning_it_on_asks_makes_a_token_and_serves_on_loopback() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let refused = workspace(&dir, Confirmation::Refuse);
        let on = Patch {
            enabled: Some(true),
            port: None,
        };
        // Script in the page alone can't turn it on.
        let e = update(Some(&file), &refused, &on).unwrap_err();
        assert_eq!(e.code, gk_api::ErrorCode::Forbidden);
        assert!(!stored(Some(&file)).enabled);
        refused.close();

        let ws = workspace(&dir, Confirmation::Given);
        let view = update(Some(&file), &ws, &on).unwrap();
        assert!(view.enabled);
        let token = view.token.clone().unwrap();
        assert_eq!(token.len(), 64);
        let url = view.url.clone().unwrap();
        assert!(
            url.starts_with("http://127.0.0.1:") && url.ends_with("/mcp"),
            "{url}"
        );
        assert_eq!(status(&url, "wrong"), 401);
        assert_ne!(status(&url, &token), 401);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // A new token: the old one is refused.
        let again = regenerate(Some(&file), &ws).unwrap();
        let new = again.token.clone().unwrap();
        assert_ne!(new, token);
        let url = again.url.unwrap();
        assert_eq!(status(&url, &token), 401);
        assert_ne!(status(&url, &new), 401);

        // Off: nothing listens; the token stays for next time.
        let off = update(
            Some(&file),
            &ws,
            &Patch {
                enabled: Some(false),
                port: None,
            },
        )
        .unwrap();
        assert_eq!(off.url, None);
        assert_eq!(off.token, Some(new));
        assert!(
            TcpStream::connect(url.trim_start_matches("http://").split('/').next().unwrap())
                .is_err()
        );
        ws.close();
    }

    #[test]
    fn it_follows_the_workspace_and_reports_a_taken_port() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let ws = workspace(&dir, Confirmation::Given);
        let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = taken.local_addr().unwrap().port();
        let view = update(
            Some(&file),
            &ws,
            &Patch {
                enabled: Some(true),
                port: Some(port),
            },
        )
        .unwrap();
        assert_eq!(view.url, None);
        assert_eq!(view.error.unwrap().code, gk_api::ErrorCode::Conflict);
        drop(taken);

        let view = update(
            Some(&file),
            &ws,
            &Patch {
                enabled: None,
                port: Some(0),
            },
        )
        .unwrap();
        assert!(view.url.is_some());
        // Reopening the workspace starts it again over the new engine.
        ws.reopen().unwrap();
        assert!(ws.agent_http_status().0.is_some());
        let e = update(
            Some(&file),
            &ws,
            &Patch {
                enabled: None,
                port: Some(80),
            },
        )
        .unwrap_err();
        assert_eq!(e.code, gk_api::ErrorCode::Invalid);
        ws.close();
        assert_eq!(ws.agent_http_status().0, None);
    }
}
