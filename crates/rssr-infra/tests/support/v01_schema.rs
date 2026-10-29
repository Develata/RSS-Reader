//! v0.1.x databases built from the frozen v0.1.x migration files (the importer's reference).
#![allow(dead_code)]

use rusqlite::Connection;

/// Index migrations 0001-0009 (0008 and 0009 were never released).
pub const INDEX: [&str; 9] = [
    include_str!("../../../../migrations/0001_initial.sql"),
    include_str!("../../../../migrations/0002_split_entry_content.sql"),
    include_str!("../../../../migrations/0003_entry_sort_key_indexes.sql"),
    include_str!("../../../../migrations/0004_drop_unused_entry_sort_index.sql"),
    include_str!("../../../../migrations/0005_restore_entry_sort_index.sql"),
    include_str!("../../../../migrations/0006_feed_generation.sql"),
    include_str!("../../../../migrations/0007_content_gc_queue.sql"),
    include_str!("../../../../migrations/0008_entry_title_search.sql"),
    include_str!("../../../../migrations/0009_feed_custom_title.sql"),
];

/// Body database migration 0001.
pub const CONTENT: &str = include_str!("../../../../migrations_content/0001_initial.sql");

/// Applies index migrations 1..=`version` to `conn`.
pub fn apply_index(conn: &Connection, version: usize) {
    for sql in &INDEX[..version] {
        conn.execute_batch(sql).expect("apply v0.1 index migration");
    }
}

/// Applies the body migration to `conn`.
pub fn apply_content(conn: &Connection) {
    conn.execute_batch(CONTENT).expect("apply v0.1 content migration");
}
