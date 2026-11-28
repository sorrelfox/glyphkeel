// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The notification at the app's start when agents changed something since
//! the person last dismissed the digest on Today. It is optional: off
//! unless the person turns it on in Preferences (`digest_notification`),
//! and it shows only while notifications are on. Opening the app shows the
//! digest itself; the notification only says that there is something in
//! it, once per start.

use std::sync::{Arc, Weak};

use gk_api::OpError;
use gk_api::ops::common::Via;
use gk_api::ops::digest::{DigestGet, DigestInput};
use gk_engine::{Context, Engine};

use crate::messages::text;
use crate::reminders::{Notice, Notifier};

/// The notice for what agents changed since the person's marker, or `None`
/// when they changed nothing.
pub(crate) fn notice(engine: &Engine) -> Result<Option<Notice>, OpError> {
    let digest = engine.run::<DigestGet>(&Context::user(Via::App), &DigestInput::default())?;
    let counts = digest.counts;
    if counts.agent_changes == 0 {
        return Ok(None);
    }
    Ok(Some(Notice {
        title: text(
            "notification-digest",
            &[("sessions", counts.sessions.to_string())],
        ),
        body: text(
            "notification-digest-body",
            &[("changes", counts.agent_changes.to_string())],
        ),
        task: None,
    }))
}

/// Shows the notice on a thread of its own, so that the start doesn't wait
/// for the digest.
pub(crate) fn notify_at_start(engine: Weak<Engine>, notifier: Arc<dyn Notifier>) {
    let started = std::thread::Builder::new()
        .name("gk-app-digest".into())
        .spawn(move || {
            let Some(engine) = engine.upgrade() else {
                return;
            };
            match notice(&engine) {
                Ok(Some(notice)) => notifier.show(&notice),
                Ok(None) => {}
                Err(err) => eprintln!("glyphkeel-app: digest: {}", err.message),
            }
        });
    if let Err(err) = started {
        eprintln!("glyphkeel-app: cannot read the digest: {err}");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gk_api::Profile;
    use gk_engine::{EngineConfig, Role, SessionStart};
    use gk_store::WorkspaceLayout;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn open(dir: &TempDir) -> Engine {
        let mut config = EngineConfig::new(Role::App);
        config.time_zone = Some("UTC".into());
        Engine::open(&WorkspaceLayout::new(dir.path().join("W")), config).unwrap()
    }

    #[test]
    fn agents_changes_since_the_marker_make_one_notice_and_nothing_else_does() {
        let dir = TempDir::new().unwrap();
        let engine = open(&dir);
        let me = Context::user(Via::App);
        engine
            .call(
                &me,
                "project.create",
                json!({ "name": "Team", "key": "TM" }),
            )
            .unwrap();
        assert_eq!(notice(&engine).unwrap(), None, "the person's own changes");
        let session = engine
            .start_session(&SessionStart {
                name: "Helper".into(),
                client_id: None,
                client_version: None,
                profile: Profile::Standard,
            })
            .unwrap();
        let agent = Context::agent("Helper", session, Profile::Standard);
        for title in ["One", "Two"] {
            engine
                .call(
                    &agent,
                    "task.create",
                    json!({ "project": "TM", "title": title }),
                )
                .unwrap();
        }
        let shown = notice(&engine).unwrap().unwrap();
        assert_eq!(shown.title, "An agent changed things while you were away");
        assert_eq!(shown.body, "2 changes. Today shows what they were.");
        assert_eq!(shown.task, None);
        engine.call(&me, "digest.dismiss", json!({})).unwrap();
        assert_eq!(notice(&engine).unwrap(), None, "dismissed");
    }
}
