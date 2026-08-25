use std::str::FromStr;

use pareto_protocol::{Digest, EventId};
use sqlx::{Connection, Executor, sqlite::SqliteConnectOptions};

use super::test_support::Fixture;
use super::*;

async fn snapshot_count(store: &EventStore) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM projection_snapshots")
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

async fn temporarily_mutate_snapshot(store: &EventStore, sql: &str) {
    sqlx::query("DROP TRIGGER projection_snapshots_no_update")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query(sql).execute(&store.pool).await.unwrap();
    sqlx::query(super::super::SNAPSHOT_UPDATE_TRIGGER)
        .execute(&store.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn creation() {
    let fixture = Fixture::new("run_snapshot-creation");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let first = store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let second = store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(snapshot_count(&store).await, 1);
    assert_eq!(first.projection.cursor.sequence, "1");
    assert!(
        sqlx::query("UPDATE projection_snapshots SET cursor_event_id='event_changed'")
            .execute(&store.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM projection_snapshots")
            .execute(&store.pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn atomicity() {
    let fixture = Fixture::new("run_snapshot-atomicity");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let snapshot = store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let mut uncommitted = snapshot.clone();
    uncommitted.projection.cursor.sequence = "2".to_owned();
    uncommitted.projection.cursor.event_id = EventId::parse("event_uncommitted").unwrap();
    let mut transaction = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    insert_snapshot(&mut transaction, &uncommitted)
        .await
        .unwrap();
    drop(transaction);
    assert_eq!(snapshot_count(&store).await, 1);
}

#[tokio::test]
async fn incremental() {
    let fixture = Fixture::new("run_snapshot-incremental");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_snapshot-suffix", 1, "task_suffix"),
        )
        .await
        .unwrap();
    let assisted = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let full = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(assisted.snapshot_disposition, SnapshotDisposition::Used);
    assert_eq!(assisted.projection, full.projection);
}

#[tokio::test]
async fn output_reader() {
    let fixture = Fixture::new("run_snapshot-output-reader");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let mut wrong = fixture.set.reference().clone();
    wrong.manifest_digest = Digest::parse(format!("sha256:{}", "f".repeat(64))).unwrap();
    let wrong_json = canonical(&wrong).unwrap();
    let wrong_fingerprint = fingerprint(wrong_json.as_bytes());
    sqlx::query("DROP TRIGGER projection_snapshots_no_update")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE projection_snapshots SET output_schema_set_json=?,output_schema_set_fingerprint=?",
    )
    .bind(wrong_json)
    .bind(wrong_fingerprint)
    .execute(&store.pool)
    .await
    .unwrap();
    sqlx::query(super::super::SNAPSHOT_UPDATE_TRIGGER)
        .execute(&store.pool)
        .await
        .unwrap();
    let load = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(
        load.snapshot_disposition,
        SnapshotDisposition::RejectedIncompatible
    );
}

#[tokio::test]
async fn prefix_validation() {
    let fixture = Fixture::new("run_snapshot-prefix-validation");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_prefix-suffix", 1, "task_prefix"),
        )
        .await
        .unwrap();
    let assisted = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(assisted.snapshot_disposition, SnapshotDisposition::Used);
    assert_eq!(assisted.projection.cursor.sequence, "2");

    let mut forged =
        sqlx::query_scalar::<_, String>("SELECT snapshot_json FROM projection_snapshots LIMIT 1")
            .fetch_one(&store.pool)
            .await
            .map(|bytes| serde_json::from_str::<RunTaskProjectionSnapshot>(&bytes).unwrap())
            .unwrap();
    let reducer = registry.resolve_reducer(&fixture.set).unwrap();
    forged.projection.history_chain_state =
        Digest::parse(format!("sha256:{}", "e".repeat(64))).unwrap();
    forged.projection.projection_digest =
        compute_projection_digest(&forged.projection, reducer).unwrap();
    forged.projection_digest = forged.projection.projection_digest.clone();
    forged.snapshot_digest = compute_snapshot_digest(&forged, reducer).unwrap();
    let forged_json = canonical(&forged).unwrap();
    sqlx::query("DROP TRIGGER projection_snapshots_no_update")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE projection_snapshots SET snapshot_json=?,snapshot_fingerprint=?,projection_digest=?,snapshot_digest=?")
        .bind(&forged_json)
        .bind(fingerprint(forged_json.as_bytes()))
        .bind(forged.projection_digest.as_str())
        .bind(forged.snapshot_digest.as_str())
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query(super::super::SNAPSHOT_UPDATE_TRIGGER)
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .project_snapshot_assisted(&registry, &fixture.projection_target())
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::HistoryMismatch
    );
}

#[tokio::test]
async fn prefix_corruption() {
    let fixture = Fixture::new("run_snapshot-prefix-corruption");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER events_no_update")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET envelope_fingerprint='sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' WHERE sequence_i64=1")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query(super::super::UPDATE_TRIGGER)
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .project_snapshot_assisted(&registry, &fixture.projection_target())
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::AggregateCorrupt
    );
}

#[tokio::test]
async fn fallbacks() {
    let fixture = Fixture::new("run_snapshot-fallbacks");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let snapshot = store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    temporarily_mutate_snapshot(
        &store,
        "UPDATE projection_snapshots SET cursor_event_id='event_wrong-cursor'",
    )
    .await;
    let cursor_load = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(
        cursor_load.snapshot_disposition,
        SnapshotDisposition::RejectedCursor
    );
    let restore = format!(
        "UPDATE projection_snapshots SET cursor_event_id='{}'",
        snapshot.projection.cursor.event_id.as_str()
    );
    temporarily_mutate_snapshot(&store, &restore).await;
    temporarily_mutate_snapshot(
        &store,
        "UPDATE projection_snapshots SET snapshot_fingerprint='sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'",
    )
    .await;
    let load = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(
        load.snapshot_disposition,
        SnapshotDisposition::RejectedIntegrity
    );
    assert_eq!(load.projection.cursor.sequence, "1");
}

#[tokio::test]
async fn isolation() {
    let fixture = Fixture::new("run_snapshot-isolation");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let mut target = fixture.projection_target();
    target.actor = AgentId::parse("agent_intruder").unwrap();
    assert_eq!(
        store
            .project_snapshot_assisted(&registry, &target)
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::Unauthorized
    );
}

#[tokio::test]
async fn concurrency() {
    let fixture = Fixture::new("run_snapshot-concurrency");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let projection_target = fixture.projection_target();
    let source_registry = fixture.source_registry();
    let lifecycle_target = fixture.lifecycle_target();
    let command = fixture.create_task("event_snapshot-concurrent", 1, "task_concurrent");
    let snapshot_future = store.create_projection_snapshot(&registry, &projection_target);
    let append_future = store.create_task(&source_registry, &lifecycle_target, &command);
    let (snapshot_result, append_result) = tokio::join!(snapshot_future, append_future);
    snapshot_result.unwrap();
    append_result.unwrap();
    let assisted = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let full = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(assisted.projection, full.projection);
}

#[tokio::test]
async fn recovery() {
    let fixture = Fixture::new("run_snapshot-recovery");
    let store = fixture.open_created().await;
    let store_id = store.store_id.clone();
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    drop(store);
    let reopened = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let load = reopened
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(load.snapshot_disposition, SnapshotDisposition::Used);
}

#[tokio::test]
async fn migration() {
    let fixture = Fixture::new("run_snapshot-migration");
    let store = fixture.open_created().await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='projection_snapshots'"
        )
        .fetch_one(&store.pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT writer_epoch FROM events WHERE sequence_i64=1")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn already_open_v1_writer() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("old-writer.sqlite3");
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .unwrap()
        .create_if_missing(true);
    let mut setup = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    setup
        .execute(format!("PRAGMA application_id={}", super::super::APPLICATION_ID).as_str())
        .await
        .unwrap();
    setup.execute("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at_explicit TEXT NOT NULL)").await.unwrap();
    sqlx::raw_sql(super::super::EVENTS_DDL)
        .execute(&mut setup)
        .await
        .unwrap();
    setup.execute(super::super::UPDATE_TRIGGER).await.unwrap();
    setup.execute(super::super::DELETE_TRIGGER).await.unwrap();
    sqlx::query("INSERT INTO schema_migrations(version,checksum,applied_at_explicit) VALUES(1,?,'2026-08-23T00:00:00.000Z')")
        .bind(fingerprint(super::super::EVENTS_DDL.as_bytes()))
        .execute(&mut setup)
        .await
        .unwrap();
    setup.execute("PRAGMA user_version=1").await.unwrap();
    let store_id: String =
        sqlx::query_scalar("SELECT store_id FROM store_metadata WHERE singleton=1")
            .fetch_one(&mut setup)
            .await
            .unwrap();
    let mut old_writer = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    setup.close().await.unwrap();
    let migrated = EventStore::open_pinned(&path, &store_id).await.unwrap();
    let error = sqlx::query("INSERT INTO events(event_id,envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,causation_id,correlation_id) VALUES('event_old','{}','x','{}','x','{}','x','tenant_local',0,'','workspace_repo','run_old','agent_owner','stream_old',1,NULL,'corr')")
        .execute(&mut old_writer)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("writer_epoch_conflict"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events")
            .fetch_one(&migrated.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn compatibility() {
    let fixture = Fixture::new("run_snapshot-compatibility");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let mut missing_reducer = registry.clone();
    missing_reducer.reducers.clear();
    assert_eq!(
        store
            .project_snapshot_assisted(&missing_reducer, &fixture.projection_target())
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::ReducerUnavailable
    );
}
