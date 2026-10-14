#![cfg(not(target_arch = "wasm32"))]

//! Release acceptance of the v0.1.x import against a library written by an official release.
//!
//! Ignored by default. Inputs (see docs/testing/native-upgrade-verification.md):
//! - `RSSR_V01_LIBRARY_DIR`: a copy of a data directory written by an official v0.1.x CLI;
//! - `RSSR_V01_SNAPSHOT`: the `legacy-snapshot.json` taken from that directory
//!   (`scripts/verify_release_upgrade.py`), every row of every table.
//!
//! The test imports the directory into a new library next to it and compares every column of
//! every row with the snapshot.

use std::path::PathBuf;

use rssr_domain::{AppStateSnapshot, UserSettings, fold_for_search};
use rssr_infra::store::LibraryStore;
use rssr_infra::store::import::{V01Library, import_v01};
use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde_json::{Map, Value};

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

fn rows(conn: &Connection, sql: &str) -> Vec<Map<String, Value>> {
    let mut statement = conn.prepare(sql).unwrap();
    let names: Vec<String> = statement.column_names().into_iter().map(str::to_owned).collect();
    let mut rows = statement.query([]).unwrap();
    let mut result = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index).unwrap() {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => Value::from(value),
                ValueRef::Real(value) => Value::from(value),
                ValueRef::Text(text) => Value::from(String::from_utf8_lossy(text).into_owned()),
                ValueRef::Blob(_) => panic!("unexpected blob in {name}"),
            };
            object.insert(name.clone(), value);
        }
        result.push(object);
    }
    result
}

/// Every column of the snapshot row equals the imported row.
fn assert_columns(table: &str, expected: &Map<String, Value>, actual: &Map<String, Value>) {
    for (column, value) in expected {
        assert_eq!(actual.get(column), Some(value), "{table}.{column} of {expected:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release acceptance: needs a library written by an official v0.1.x CLI"]
async fn an_official_release_library_imports_without_loss() {
    let dir = env_path("RSSR_V01_LIBRARY_DIR");
    let snapshot: Value =
        serde_json::from_slice(&std::fs::read(env_path("RSSR_V01_SNAPSHOT")).unwrap()).unwrap();
    let target = dir.join("library.sqlite3");
    let _ = std::fs::remove_file(&target);
    let source = V01Library::find(&dir).expect("a v0.1 index database");
    let outcome = import_v01(&source, &target).expect("import");
    eprintln!("import outcome: {outcome:?}");

    let conn = Connection::open(&target).unwrap();
    let table = |name: &str| snapshot[name].as_array().cloned().unwrap_or_default();

    let feeds = rows(&conn, "SELECT * FROM feeds ORDER BY id");
    assert_eq!(feeds.len(), table("feeds").len(), "feed count");
    for (expected, actual) in table("feeds").iter().zip(&feeds) {
        assert_columns("feeds", expected.as_object().unwrap(), actual);
        assert_eq!(actual["custom_title"], Value::Null);
        for ticket in ["refresh_issued", "refresh_published", "refresh_diagnosed"] {
            assert_eq!(actual[ticket], Value::from(0), "{ticket}");
        }
    }

    let entries = rows(&conn, "SELECT * FROM entries ORDER BY id");
    assert_eq!(entries.len(), table("entries").len(), "entry count");
    let live: std::collections::BTreeSet<i64> =
        entries.iter().map(|entry| entry["id"].as_i64().unwrap()).collect();
    let bodies = rows(&conn, "SELECT * FROM entry_contents ORDER BY entry_id");
    let expected_bodies: Vec<Map<String, Value>> = table("entry_contents")
        .into_iter()
        .map(|row| row.as_object().unwrap().clone())
        .filter(|row| live.contains(&row["entry_id"].as_i64().unwrap()))
        .map(|mut row| {
            row.remove("feed_id");
            row
        })
        .collect();
    assert_eq!(bodies.len(), expected_bodies.len(), "body count");
    for (expected, actual) in expected_bodies.iter().zip(&bodies) {
        assert_columns("entry_contents", expected, actual);
    }
    let with_body: std::collections::BTreeSet<i64> =
        bodies.iter().map(|body| body["entry_id"].as_i64().unwrap()).collect();
    for (expected, actual) in table("entries").iter().zip(&entries) {
        let mut expected = expected.as_object().unwrap().clone();
        let id = expected["id"].as_i64().unwrap();
        // has_content is derived from the bodies on import.
        let source_has_content = expected.remove("has_content");
        assert_columns("entries", &expected, actual);
        assert_eq!(
            actual["has_content"],
            Value::from(i64::from(with_body.contains(&id))),
            "entries.has_content of {id} (source {source_has_content:?})"
        );
        assert_eq!(
            actual["title_search"].as_str().unwrap(),
            fold_for_search(actual["title"].as_str().unwrap())
        );
    }

    let documents = rows(&conn, "SELECT key, value FROM app_settings ORDER BY key");
    for expected in table("app_settings") {
        let key = expected["key"].as_str().unwrap();
        let imported = documents.iter().find(|row| row["key"] == key);
        let expected_value: Value =
            serde_json::from_str(expected["value"].as_str().unwrap()).unwrap();
        match key {
            "user_settings" => {
                // Compared as settings: the import re-serializes them (an `f32` prints the same,
                // but a JSON `Value` built from the parsed settings widens it to `f64`).
                let expected: UserSettings =
                    serde_json::from_value(expected_value.clone()).unwrap();
                let actual: UserSettings =
                    serde_json::from_str(imported.unwrap()["value"].as_str().unwrap()).unwrap();
                assert_eq!(actual, expected, "user_settings");
            }
            "app_state_v2" => {
                let _: AppStateSnapshot = serde_json::from_value(expected_value.clone()).unwrap();
                let actual: Value =
                    serde_json::from_str(imported.unwrap()["value"].as_str().unwrap()).unwrap();
                assert_eq!(actual, expected_value, "app_state_v2");
            }
            other => panic!("unexpected v0.1 document {other}"),
        }
    }
    drop(conn);

    let store = LibraryStore::open(&target).await.expect("the imported library opens");
    assert_eq!(store.journal_mode(), "wal");
}
