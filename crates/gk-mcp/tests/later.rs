// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Asking the person to confirm, and watching resources, in both protocol
//! eras: `elicitation/create` and resource subscriptions in the old one,
//! `input_required` results and `subscriptions/listen` in 2026-07-28.

#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use common::{
    PersonConnection, connect, connect_person, lifecycles, open_as_person, text, workspace,
};
use gk_api::ops::history::{ActivityQuery, HistoryFeed};
use rmcp::model::{
    CallToolResult, ElicitationAction, ReadResourceRequestParams, ServerNotification,
    SubscriptionFilter,
};
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

/// A project "Home lab" (ZZ) with `tasks` tasks.
async fn home_lab(c: &PersonConnection, tasks: usize) {
    output(
        &c.call("create_project", json!({ "name": "Home lab", "key": "ZZ" }))
            .await,
    );
    for n in 1..=tasks {
        output(
            &c.call(
                "create_task",
                json!({ "project": "ZZ", "title": format!("Rack {n}") }),
            )
            .await,
        );
    }
}

#[tokio::test]
async fn an_allowed_deletion_runs_once_and_history_says_the_person_confirmed() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, options) = workspace();
        let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
        home_lab(&c, 2).await;

        let r = c.call("delete_project", json!({ "project": "ZZ" })).await;
        output(&r);
        let questions = c.questions();
        assert_eq!(questions.len(), 1, "{label}: {questions:?}");
        assert!(
            questions[0].contains("Claude Code")
                && questions[0].contains("Home lab")
                && questions[0].contains("2 tasks"),
            "{label}: {questions:?}"
        );
        c.close().await;

        let (engine, person) = open_as_person(&options);
        let page = engine
            .run::<HistoryFeed>(&person, &ActivityQuery::default())
            .unwrap();
        let deletion: Vec<_> = page
            .items
            .iter()
            .filter(|e| e.op == "project.delete")
            .collect();
        assert!(!deletion.is_empty(), "{label}");
        assert!(
            deletion
                .iter()
                .all(|e| e.confirmed_by_person && e.actor.name == "Claude Code"),
            "{label}: {deletion:?}"
        );
        assert!(
            page.items
                .iter()
                .filter(|e| e.op == "task.create")
                .all(|e| !e.confirmed_by_person),
            "{label}"
        );
        engine.close();
    }
}

#[tokio::test]
async fn a_declined_question_is_forbidden_and_changes_nothing() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, options) = workspace();
        let c = connect_person(&options, mode, asked, ElicitationAction::Decline).await;
        home_lab(&c, 1).await;

        let r = c.call("delete_project", json!({ "project": "ZZ" })).await;
        assert_eq!(r.is_error, Some(true), "{label}");
        let t = text(&r);
        assert!(
            t.starts_with("Error (forbidden)") && t.contains("didn't allow delete_project"),
            "{label}: {t}"
        );
        assert_eq!(c.questions().len(), 1, "{label}");
        output(&c.call("get_project", json!({ "project": "ZZ" })).await);
        c.close().await;
    }
}

#[tokio::test]
async fn beyond_the_profile_and_many_deletions_ask_too() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, mut options) = workspace();
        options.profile = gk_api::Profile::ReadOnly;
        let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;

        // A client that can ask sees the tools beyond its profile.
        let tools = c.client.list_all_tools().await.unwrap();
        assert!(tools.iter().any(|t| t.name == "create_task"), "{label}");

        // Each change asks; allowed, it runs.
        home_lab(&c, 21).await;
        assert_eq!(c.questions().len(), 22, "{label}");
        assert!(
            c.questions()[0].contains("its profile (read-only)"),
            "{label}: {:?}",
            c.questions()[0]
        );
        c.close().await;

        // A standard agent moving more tasks to the trash than one call may.
        let mut options = options.clone();
        options.profile = gk_api::Profile::Standard;
        let (_, mode, asked, _) = lifecycles().into_iter().find(|l| l.0 == label).unwrap();
        let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
        let keys: Vec<String> = (1..=21).map(|n| format!("ZZ-{n}")).collect();
        let r = c.call("delete_tasks", json!({ "keys": keys })).await;
        assert_eq!(output(&r)["total"], 21, "{label}");
        let questions = c.questions();
        assert_eq!(questions.len(), 1, "{label}");
        assert!(questions[0].contains("21 tasks"), "{label}: {questions:?}");
        c.close().await;
    }
}

#[tokio::test]
async fn without_a_client_that_asks_or_with_the_setting_off_nothing_asks() {
    for (label, mode, asked, _) in lifecycles() {
        // A client without elicitation: as before, the engine decides.
        let (_dir, options) = workspace();
        let c = connect(&options, mode.clone(), asked.clone(), "codex").await;
        c.call("create_project", json!({ "name": "Home lab", "key": "ZZ" }))
            .await;
        let r = c.call("delete_project", json!({ "project": "ZZ" })).await;
        assert_eq!(r.is_error, Some(true), "{label}");
        assert!(text(&r).starts_with("Error (forbidden)"), "{label}");
        c.close().await;

        // The person turned confirmations off.
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "agent.policy.update",
                json!({ "max_profile": "standard", "confirmations": false }),
            )
            .unwrap();
        engine.close();
        let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
        let r = c.call("delete_project", json!({ "project": "ZZ" })).await;
        assert_eq!(r.is_error, Some(true), "{label}");
        assert!(c.questions().is_empty(), "{label}");
        let tools = c.client.list_all_tools().await.unwrap();
        assert!(!tools.iter().any(|t| t.name == "purge_trash"), "{label}");
        c.close().await;
    }
}

/// The next resource update within a few seconds.
async fn next_update(c: &mut PersonConnection) -> String {
    tokio::time::timeout(Duration::from_secs(10), c.updates.recv())
        .await
        .expect("an update in time")
        .expect("the channel is open")
}

#[tokio::test]
#[allow(deprecated)]
async fn old_era_subscriptions_hear_of_changes() {
    let (_, mode, asked, _) = lifecycles().remove(0);
    let (_dir, options) = workspace();
    let mut c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
    home_lab(&c, 1).await;

    let task = "glyphkeel://tasks/ZZ-1";
    let read = c
        .client
        .read_resource(ReadResourceRequestParams::new(task))
        .await
        .unwrap();
    assert_eq!(read.contents.len(), 1);

    c.client
        .subscribe(rmcp::model::SubscribeRequestParams::new(task))
        .await
        .unwrap();
    let missing = c
        .client
        .subscribe(rmcp::model::SubscribeRequestParams::new(
            "glyphkeel://tasks/ZZ-99",
        ))
        .await;
    assert!(missing.is_err());

    output(
        &c.call("update_task", json!({ "key": "ZZ-1", "title": "Rack one" }))
            .await,
    );
    assert_eq!(next_update(&mut c).await, task);

    // A change made by another program reaches the agent too.
    let (engine, person) = open_as_person(&options);
    engine
        .call(
            &person,
            "task.update",
            json!({ "key": "ZZ-1", "title": "Rack 1" }),
        )
        .unwrap();
    assert_eq!(next_update(&mut c).await, task);
    engine.close();
    c.close().await;
}

#[tokio::test]
async fn subscriptions_listen_hears_of_task_and_query_changes() {
    let (_, mode, asked, _) = lifecycles().remove(1);
    let (_dir, options) = workspace();
    let c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;
    home_lab(&c, 1).await;

    let task = "glyphkeel://tasks/ZZ-1".to_owned();
    let query = "glyphkeel://queries?query=project%3AZZ".to_owned();
    let filter = SubscriptionFilter::builder()
        .resource_subscriptions([
            task.clone(),
            query.clone(),
            "glyphkeel://nothing".to_owned(),
        ])
        .build();
    let mut subscription = c.client.peer().listen(filter).await.unwrap();
    assert_eq!(
        subscription.acknowledged().resource_subscriptions,
        Some(vec![task.clone(), query.clone()])
    );

    // A new task changes the query's result, not the watched task.
    output(
        &c.call("create_task", json!({ "project": "ZZ", "title": "Cables" }))
            .await,
    );
    let mut heard = Vec::new();
    while heard.is_empty() {
        let next = tokio::time::timeout(Duration::from_secs(10), subscription.next())
            .await
            .expect("a notification in time")
            .unwrap()
            .expect("the subscription is open");
        if let ServerNotification::ResourceUpdatedNotification(n) = next {
            heard.push(n.params.uri);
        }
    }
    assert_eq!(heard, vec![query.clone()]);

    output(
        &c.call("update_task", json!({ "key": "ZZ-1", "title": "Rack one" }))
            .await,
    );
    let mut heard = std::collections::BTreeSet::new();
    while !heard.contains(&task) {
        let next = tokio::time::timeout(Duration::from_secs(10), subscription.next())
            .await
            .expect("a notification in time")
            .unwrap()
            .expect("the subscription is open");
        if let ServerNotification::ResourceUpdatedNotification(n) = next {
            heard.insert(n.params.uri);
        }
    }
    drop(subscription);
    c.close().await;
}
