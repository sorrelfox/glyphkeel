// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The person's own time zone: chosen in the personal settings, checked
//! against the zone database, and changed while the workspace is open.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{
    Context, Engine, EngineConfig, Role, check_time_zone, personal_time_zone, time_zones,
};
use gk_store::{UserSettings, WorkspaceLayout};
use serde_json::{Value, json};

/// 2026-11-23T11:00:00Z: already the 24th in Kiritimati (UTC+14), still
/// the 22nd at UTC-12.
const NOW: i64 = 1_795_431_600_000;

fn open(dir: &tempfile::TempDir, zone: Option<&str>) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(|| NOW);
    config.time_zone = zone.map(str::to_owned);
    Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn logged_day(e: &Engine) -> String {
    call(e, "time.log", json!({ "key": "TM-1", "duration": "15m" }))["entry"]["date"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn today_follows_the_chosen_zone_and_its_changes() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir, Some("Pacific/Kiritimati"));
    assert_eq!(e.time_zone().as_deref(), Some("Pacific/Kiritimati"));
    call(&e, "project.create", json!({ "name": "T", "key": "TM" }));
    call(&e, "task.create", json!({ "project": "TM", "title": "A" }));
    assert_eq!(logged_day(&e), "2026-11-24");

    e.set_time_zone("Etc/GMT+12").unwrap();
    assert_eq!(logged_day(&e), "2026-11-22");

    let err = e.set_time_zone("Mars/Olympus_Mons").unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(
        logged_day(&e),
        "2026-11-22",
        "a refused zone changes nothing"
    );

    e.set_time_zone("system").unwrap();
    assert_ne!(e.time_zone().as_deref(), Some("Etc/GMT+12"));
}

#[test]
fn zone_names_are_checked_against_the_zone_database() {
    assert_eq!(check_time_zone("system").unwrap(), None);
    assert_eq!(check_time_zone("  ").unwrap(), None);
    assert_eq!(
        check_time_zone("Europe/Berlin").unwrap().as_deref(),
        Some("Europe/Berlin")
    );
    assert_eq!(
        check_time_zone("Europe/Atlantis").unwrap_err().code,
        ErrorCode::Invalid
    );
}

#[test]
fn a_zone_given_for_one_run_wins_over_the_settings() {
    let settings = UserSettings {
        time_zone: Some("Asia/Tokyo".into()),
        ..UserSettings::default()
    };
    assert_eq!(
        personal_time_zone(None, &settings).unwrap().as_deref(),
        Some("Asia/Tokyo")
    );
    assert_eq!(
        personal_time_zone(Some("UTC"), &settings)
            .unwrap()
            .as_deref(),
        Some("UTC")
    );
    assert_eq!(personal_time_zone(Some("system"), &settings).unwrap(), None);
    assert!(personal_time_zone(Some("Nowhere/Here"), &settings).is_err());
    // An unknown zone in the file falls back to the computer's.
    let broken = UserSettings {
        time_zone: Some("Nowhere/Here".into()),
        ..UserSettings::default()
    };
    assert_eq!(personal_time_zone(None, &broken).unwrap(), None);
    assert_eq!(
        personal_time_zone(None, &UserSettings::default()).unwrap(),
        None
    );
}

#[test]
fn the_zone_list_has_names_and_current_offsets() {
    let zones = time_zones(NOW);
    let find = |name: &str| zones.iter().find(|z| z.name == name).unwrap();
    assert_eq!(find("Europe/Berlin").offset_seconds, 3600);
    assert_eq!(find("Pacific/Kiritimati").offset_seconds, 14 * 3600);
    assert_eq!(find("UTC").offset_seconds, 0);
    assert!(zones.windows(2).all(|w| w[0].name < w[1].name));
}
