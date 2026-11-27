// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Resources for an agent started with `--project TM`: reading and watching
//! stay within TM, in both protocol eras.

#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use common::{connect_person, lifecycles, open_as_person, workspace};
use rmcp::model::{
    ElicitationAction, ReadResourceRequestParams, ResourceContents, ServerNotification,
    SubscribeRequestParams, SubscriptionFilter,
};
use serde_json::{Value, json};

/// TM with TM-1, PRV with PRV-1, made by the person.
fn seed(options: &gk_mcp::Options) {
    let (engine, person) = open_as_person(options);
    let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
    call("project.create", json!({ "name": "Team", "key": "TM" }));
    call("project.create", json!({ "name": "Private", "key": "PRV" }));
    call("task.create", json!({ "project": "TM", "title": "Shared" }));
    call(
        "task.create",
        json!({ "project": "PRV", "title": "Secret" }),
    );
    engine.close();
}

fn text_of(contents: &[ResourceContents]) -> String {
    match &contents[0] {
        ResourceContents::TextResourceContents { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    }
}

/// Makes a task in `project` as the person, from another program.
fn person_creates(options: &gk_mcp::Options, project: &str) {
    let (engine, person) = open_as_person(options);
    engine
        .call(
            &person,
            "task.create",
            json!({ "project": project, "title": "New" }),
        )
        .unwrap();
    engine.close();
}

#[tokio::test]
#[allow(deprecated)]
async fn a_limited_agent_reads_and_watches_only_its_project() {
    let (_, mode, asked, _) = lifecycles().remove(0);
    let (_dir, mut options) = workspace();
    seed(&options);
    options.project = Some("TM".to_owned());
    let mut c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
    let read = |uri: &'static str| c.client.read_resource(ReadResourceRequestParams::new(uri));

    assert!(read("glyphkeel://tasks/TM-1").await.is_ok());
    assert!(read("glyphkeel://tasks/PRV-1").await.is_err());
    assert!(read("glyphkeel://projects/PRV").await.is_err());
    let open = text_of(
        &read("glyphkeel://queries?query=is%3Aopen")
            .await
            .unwrap()
            .contents,
    );
    assert!(open.contains("TM-1") && !open.contains("PRV"), "{open}");
    let named = text_of(
        &read("glyphkeel://queries?query=project%3APRV")
            .await
            .unwrap()
            .contents,
    );
    assert!(!named.contains("PRV-1"), "{named}");

    for uri in ["glyphkeel://tasks/PRV-1", "glyphkeel://projects/PRV"] {
        assert!(
            c.client
                .subscribe(SubscribeRequestParams::new(uri))
                .await
                .is_err(),
            "{uri}"
        );
    }
    let query = "glyphkeel://queries?query=is%3Aopen";
    c.client
        .subscribe(SubscribeRequestParams::new(query))
        .await
        .unwrap();

    // A change in the other project is not heard; one in TM is.
    person_creates(&options, "PRV");
    let quiet = tokio::time::timeout(Duration::from_millis(2500), c.updates.recv()).await;
    assert!(quiet.is_err(), "{quiet:?}");
    person_creates(&options, "TM");
    let heard = tokio::time::timeout(Duration::from_secs(10), c.updates.recv())
        .await
        .expect("an update in time")
        .expect("the channel is open");
    assert_eq!(heard, query);
    c.close().await;
}

#[tokio::test]
async fn listening_leaves_out_what_the_limited_agent_cannot_see() {
    let (_, mode, asked, _) = lifecycles().remove(1);
    let (_dir, mut options) = workspace();
    seed(&options);
    options.project = Some("TM".to_owned());
    let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
    let mine = "glyphkeel://tasks/TM-1".to_owned();
    let filter = SubscriptionFilter::builder()
        .resource_subscriptions([
            mine.clone(),
            "glyphkeel://tasks/PRV-1".to_owned(),
            "glyphkeel://projects/PRV".to_owned(),
        ])
        .build();
    let subscription = c.client.peer().listen(filter).await.unwrap();
    assert_eq!(
        subscription.acknowledged().resource_subscriptions,
        Some(vec![mine])
    );
    drop(subscription);
    c.close().await;
}

/// Calls `op` as the person, from another program.
fn person_calls(options: &gk_mcp::Options, op: &str, input: Value) -> Value {
    let (engine, person) = open_as_person(options);
    let out = engine.call(&person, op, input).unwrap();
    engine.close();
    out
}

/// TM-1's ID.
fn shared_id(options: &gk_mcp::Options) -> String {
    let out = person_calls(options, "task.get", json!({ "keys": ["TM-1"] }));
    out["items"][0]["id"].as_str().unwrap().to_owned()
}

/// Moves TM-1 (by `id`) into PRV, then changes it there.
fn move_out_and_change(options: &gk_mcp::Options, id: &str) {
    person_calls(options, "task.move", json!({ "key": id, "project": "PRV" }));
    person_calls(
        options,
        "task.update",
        json!({ "key": id, "title": "Now secret" }),
    );
}

#[tokio::test]
#[allow(deprecated)]
async fn a_watched_task_moved_out_of_the_project_is_no_longer_heard_of() {
    let (_, mode, asked, _) = lifecycles().remove(0);
    let (_dir, mut options) = workspace();
    seed(&options);
    let id = shared_id(&options);
    options.project = Some("TM".to_owned());
    let mut c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
    let uri = "glyphkeel://tasks/TM-1";
    c.client
        .subscribe(SubscribeRequestParams::new(uri))
        .await
        .unwrap();

    // Within TM, a change is heard.
    person_calls(
        &options,
        "task.update",
        json!({ "key": id, "title": "Shared!" }),
    );
    let heard = tokio::time::timeout(Duration::from_secs(10), c.updates.recv())
        .await
        .expect("an update in time")
        .expect("the channel is open");
    assert_eq!(heard, uri);

    // Moved into PRV, neither the move nor later changes are heard, and
    // the task can't be read by its old key or its ID.
    move_out_and_change(&options, &id);
    let quiet = tokio::time::timeout(Duration::from_millis(3500), c.updates.recv()).await;
    assert!(quiet.is_err(), "{quiet:?}");
    for read in [uri.to_owned(), format!("glyphkeel://tasks/{id}")] {
        assert!(
            c.client
                .read_resource(ReadResourceRequestParams::new(read.clone()))
                .await
                .is_err(),
            "{read}"
        );
    }

    // Moved back, it isn't watched any more: the subscription was dropped.
    person_calls(&options, "task.move", json!({ "key": id, "project": "TM" }));
    person_calls(
        &options,
        "task.update",
        json!({ "key": id, "title": "Back" }),
    );
    let quiet = tokio::time::timeout(Duration::from_millis(2500), c.updates.recv()).await;
    assert!(quiet.is_err(), "{quiet:?}");
    c.close().await;
}

#[tokio::test]
async fn a_listened_task_moved_out_of_the_project_is_no_longer_heard_of() {
    let (_, mode, asked, _) = lifecycles().remove(1);
    let (_dir, mut options) = workspace();
    seed(&options);
    let id = shared_id(&options);
    options.project = Some("TM".to_owned());
    let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
    let uri = "glyphkeel://tasks/TM-1".to_owned();
    let filter = SubscriptionFilter::builder()
        .resource_subscriptions([uri.clone()])
        .build();
    let mut subscription = c.client.peer().listen(filter).await.unwrap();
    assert_eq!(
        subscription.acknowledged().resource_subscriptions,
        Some(vec![uri.clone()])
    );

    let mut next_update = async || loop {
        let next = subscription.next().await.unwrap()?;
        if let ServerNotification::ResourceUpdatedNotification(n) = next {
            return Some(n.params.uri);
        }
    };

    person_calls(
        &options,
        "task.update",
        json!({ "key": id, "title": "Shared!" }),
    );
    let heard = tokio::time::timeout(Duration::from_secs(10), next_update())
        .await
        .expect("an update in time");
    assert_eq!(heard, Some(uri));

    move_out_and_change(&options, &id);
    let quiet = tokio::time::timeout(Duration::from_millis(3500), next_update()).await;
    assert!(quiet.is_err(), "{quiet:?}");
    drop(subscription);
    c.close().await;
}
