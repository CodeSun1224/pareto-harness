use std::str::FromStr;

use pareto_protocol::{Digest, EventId, RunCreatedPayload, TaskCreatedPayload, TaskId, TaskState};
use sqlx::{Connection, Executor, Row, SqliteConnection, sqlite::SqliteConnectOptions};

use super::test_support::Fixture;
use super::*;

async fn snapshot_count(store: &EventStore) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM projection_snapshots")
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

async fn setup_v1_history(fixture: &Fixture) -> (SqliteConnection, String, Vec<Vec<String>>) {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", fixture.path.display()))
        .unwrap()
        .create_if_missing(true);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection
        .execute(format!("PRAGMA application_id={}", super::super::APPLICATION_ID).as_str())
        .await
        .unwrap();
    connection.execute("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at_explicit TEXT NOT NULL)").await.unwrap();
    sqlx::raw_sql(super::super::EVENTS_DDL)
        .execute(&mut connection)
        .await
        .unwrap();
    connection
        .execute(super::super::UPDATE_TRIGGER)
        .await
        .unwrap();
    connection
        .execute(super::super::DELETE_TRIGGER)
        .await
        .unwrap();
    sqlx::query("INSERT INTO schema_migrations(version,checksum,applied_at_explicit) VALUES(1,?,'2026-08-23T00:00:00.000Z')")
        .bind(fingerprint(super::super::EVENTS_DDL.as_bytes()))
        .execute(&mut connection)
        .await
        .unwrap();
    connection.execute("PRAGMA user_version=1").await.unwrap();

    let stream = super::super::lifecycle::lifecycle_stream_id(&fixture.scope).unwrap();
    let run = super::super::lifecycle::lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_v1-run").unwrap(),
        1,
        "2026-08-25T01:00:00.000Z",
        "corr-v1-run",
        "run-created",
        &RunCreatedPayload {
            manifest: fixture.manifest.clone(),
        },
    )
    .unwrap();
    let task = super::super::lifecycle::lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_v1-task").unwrap(),
        2,
        "2026-08-25T01:00:01.000Z",
        "corr-v1-task",
        "task-created",
        &TaskCreatedPayload {
            task_id: TaskId::parse("task_v1").unwrap(),
            parent_task_id: None,
            initial_state: TaskState::Created,
        },
    )
    .unwrap();
    for event in [&run, &task] {
        let prepared =
            super::super::PreparedEvent::new(event, &fixture.set, &fixture.limits).unwrap();
        let envelope = &prepared.envelope;
        let (present, user) = user_key(&envelope.scope);
        sqlx::query("INSERT INTO events(event_id,envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,causation_id,correlation_id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(envelope.event_id.as_str())
            .bind(&prepared.envelope_json)
            .bind(&prepared.envelope_fingerprint)
            .bind(&prepared.schema_set_json)
            .bind(&prepared.schema_set_fingerprint)
            .bind(&prepared.limits_json)
            .bind(&prepared.limits_fingerprint)
            .bind(envelope.scope.tenant_id.as_str())
            .bind(present)
            .bind(user)
            .bind(envelope.scope.workspace_id.as_str())
            .bind(envelope.scope.run_id.as_str())
            .bind(envelope.scope.agent_id.as_str())
            .bind(envelope.stream_id.as_str())
            .bind(prepared.sequence)
            .bind(envelope.causation_id.as_ref().map(EventId::as_str))
            .bind(&envelope.correlation_id)
            .execute(&mut connection)
            .await
            .unwrap();
    }
    let store_id: String =
        sqlx::query_scalar("SELECT store_id FROM store_metadata WHERE singleton=1")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    let bytes = v1_event_bytes(&mut connection).await;
    (connection, store_id, bytes)
}

async fn v1_event_bytes(connection: &mut SqliteConnection) -> Vec<Vec<String>> {
    sqlx::query("SELECT append_ordinal,event_id,envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,causation_id,correlation_id FROM events ORDER BY append_ordinal")
        .fetch_all(connection)
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            vec![
                row.get::<i64, _>(0).to_string(),
                row.get::<String, _>(1),
                row.get::<String, _>(2),
                row.get::<String, _>(3),
                row.get::<String, _>(4),
                row.get::<String, _>(5),
                row.get::<String, _>(6),
                row.get::<String, _>(7),
                row.get::<String, _>(8),
                row.get::<i64, _>(9).to_string(),
                row.get::<String, _>(10),
                row.get::<String, _>(11),
                row.get::<String, _>(12),
                row.get::<String, _>(13),
                row.get::<String, _>(14),
                row.get::<i64, _>(15).to_string(),
                row.get::<Option<String>, _>(16)
                    .unwrap_or_else(|| "<null>".to_owned()),
                row.get::<String, _>(17),
            ]
        })
        .collect()
}

async fn assert_snapshot_table_drift_rejected(ddl: String) {
    let fixture = Fixture::new("run_snapshot-table-drift");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let store_id = store.store_id.clone();
    drop(store);
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", fixture.path.display())).unwrap();
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection
        .execute("DROP TABLE projection_snapshots")
        .await
        .unwrap();
    connection.execute(ddl.as_str()).await.unwrap();
    connection
        .execute(super::super::SNAPSHOT_INDEX_DDL)
        .await
        .unwrap();
    connection
        .execute(super::super::SNAPSHOT_UPDATE_TRIGGER)
        .await
        .unwrap();
    connection
        .execute(super::super::SNAPSHOT_DELETE_TRIGGER)
        .await
        .unwrap();
    connection.close().await.unwrap();
    assert_eq!(
        EventStore::open_pinned(&fixture.path, &store_id)
            .await
            .unwrap_err()
            .kind,
        super::super::ErrorKind::Migration
    );
}

async fn temporarily_mutate_snapshot(store: &EventStore, sql: &str) {
    super::test_support::mutate_snapshot_rows(store, sql).await;
}

async fn assert_candidate_mutation(case: usize, mutation: &str, expected: SnapshotDisposition) {
    let fixture = Fixture::new(&format!("run_candidate-{case}"));
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    temporarily_mutate_snapshot(&store, mutation).await;
    let load = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(load.snapshot_disposition, expected, "case {case}");
    assert_eq!(load.projection.cursor.sequence, "1");
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
    let mut connection = store.pool.acquire().await.unwrap();
    connection.execute("BEGIN EXCLUSIVE").await.unwrap();
    connection
        .execute("DROP TRIGGER projection_snapshots_no_update")
        .await
        .unwrap();
    sqlx::query(
        "UPDATE projection_snapshots SET output_schema_set_json=?,output_schema_set_fingerprint=?",
    )
    .bind(wrong_json)
    .bind(wrong_fingerprint)
    .execute(&mut *connection)
    .await
    .unwrap();
    connection
        .execute(super::super::SNAPSHOT_UPDATE_TRIGGER)
        .await
        .unwrap();
    connection.execute("COMMIT").await.unwrap();
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
    let mut connection = store.pool.acquire().await.unwrap();
    connection.execute("BEGIN EXCLUSIVE").await.unwrap();
    connection
        .execute("DROP TRIGGER projection_snapshots_no_update")
        .await
        .unwrap();
    sqlx::query("UPDATE projection_snapshots SET snapshot_json=?,snapshot_fingerprint=?,projection_digest=?,snapshot_digest=?")
        .bind(&forged_json)
        .bind(fingerprint(forged_json.as_bytes()))
        .bind(forged.projection_digest.as_str())
        .bind(forged.snapshot_digest.as_str())
        .execute(&mut *connection)
        .await
        .unwrap();
    connection
        .execute(super::super::SNAPSHOT_UPDATE_TRIGGER)
        .await
        .unwrap();
    connection.execute("COMMIT").await.unwrap();
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
    super::test_support::mutate_event_rows(
        &store,
        "UPDATE events SET envelope_fingerprint='sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' WHERE sequence_i64=1",
    )
    .await;
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
async fn candidate_failure_matrix() {
    let invalid_json = "{";
    assert_candidate_mutation(
        1,
        &format!(
            "UPDATE projection_snapshots SET snapshot_json='{{',snapshot_fingerprint='{}'",
            fingerprint(invalid_json.as_bytes())
        ),
        SnapshotDisposition::RejectedIntegrity,
    )
    .await;

    let mut wrong_output = retained_output_reference().unwrap();
    wrong_output.manifest_digest = Digest::parse(format!("sha256:{}", "1".repeat(64))).unwrap();
    let wrong_output_json = canonical(&wrong_output).unwrap();
    assert_candidate_mutation(
        2,
        &format!(
            "UPDATE projection_snapshots SET output_schema_set_json='{}',output_schema_set_fingerprint='{}'",
            wrong_output_json,
            fingerprint(wrong_output_json.as_bytes())
        ),
        SnapshotDisposition::RejectedIncompatible,
    )
    .await;

    let wrong_limits = pareto_protocol::ProtocolLimitsRef {
        profile: "protocol-limits-v1".to_owned(),
        digest: Digest::parse(format!("sha256:{}", "2".repeat(64))).unwrap(),
    };
    let wrong_limits_json = canonical(&wrong_limits).unwrap();
    assert_candidate_mutation(
        3,
        &format!(
            "UPDATE projection_snapshots SET output_limits_json='{}',output_limits_fingerprint='{}'",
            wrong_limits_json,
            fingerprint(wrong_limits_json.as_bytes())
        ),
        SnapshotDisposition::RejectedIncompatible,
    )
    .await;

    let mut wrong_source = retained_output_reference().unwrap();
    wrong_source.manifest_digest = Digest::parse(format!("sha256:{}", "3".repeat(64))).unwrap();
    let wrong_source_json = canonical(&wrong_source).unwrap();
    assert_candidate_mutation(
        4,
        &format!(
            "UPDATE projection_snapshots SET source_schema_set_json='{}',source_schema_set_fingerprint='{}'",
            wrong_source_json,
            fingerprint(wrong_source_json.as_bytes())
        ),
        SnapshotDisposition::RejectedIncompatible,
    )
    .await;

    let wrong_reducer = ProjectionReducerRef {
        descriptor_schema_ref: retained_schema_ref(
            "projection-reducer-descriptor",
            "sha256:d70b46a8148d3cbb3856c4665bf34824c245258b2a6a5d2cb150a6d82618f1aa",
        )
        .unwrap(),
        contract_digest: Digest::parse(format!("sha256:{}", "4".repeat(64))).unwrap(),
    };
    let wrong_reducer_json = canonical(&wrong_reducer).unwrap();
    assert_candidate_mutation(
        5,
        &format!(
            "UPDATE projection_snapshots SET reducer_ref_json='{}',reducer_ref_fingerprint='{}'",
            wrong_reducer_json,
            fingerprint(wrong_reducer_json.as_bytes())
        ),
        SnapshotDisposition::RejectedIncompatible,
    )
    .await;

    let wrong_source_limits = pareto_protocol::ProtocolLimitsRef {
        profile: "protocol-limits-v1".to_owned(),
        digest: Digest::parse(format!("sha256:{}", "5".repeat(64))).unwrap(),
    };
    let wrong_source_limits_json = canonical(&wrong_source_limits).unwrap();
    assert_candidate_mutation(
        6,
        &format!(
            "UPDATE projection_snapshots SET source_limits_json='{}',source_limits_fingerprint='{}'",
            wrong_source_limits_json,
            fingerprint(wrong_source_limits_json.as_bytes())
        ),
        SnapshotDisposition::RejectedIncompatible,
    )
    .await;

    assert_candidate_mutation(
        7,
        "UPDATE projection_snapshots SET cursor_event_id='event_wrong-cursor'",
        SnapshotDisposition::RejectedCursor,
    )
    .await;
    assert_candidate_mutation(
        8,
        "UPDATE projection_snapshots SET projection_digest='sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'",
        SnapshotDisposition::RejectedIntegrity,
    )
    .await;
    assert_candidate_mutation(
        9,
        "UPDATE projection_snapshots SET snapshot_digest='sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'",
        SnapshotDisposition::RejectedIntegrity,
    )
    .await;

    let fixture = Fixture::new("run_candidate-version");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let json: String = sqlx::query_scalar("SELECT snapshot_json FROM projection_snapshots LIMIT 1")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let mut versioned: serde_json::Value = serde_json::from_str(&json).unwrap();
    versioned["schema_ref"]["major"] = serde_json::json!(2);
    let versioned = pareto_protocol::canonical_json(&versioned).unwrap();
    temporarily_mutate_snapshot(
        &store,
        &format!(
            "UPDATE projection_snapshots SET snapshot_json='{}',snapshot_fingerprint='{}'",
            versioned.replace('\'', "''"),
            fingerprint(versioned.as_bytes())
        ),
    )
    .await;
    assert_eq!(
        store
            .project_snapshot_assisted(&registry, &fixture.projection_target())
            .await
            .unwrap()
            .snapshot_disposition,
        SnapshotDisposition::RejectedIntegrity
    );
}

#[tokio::test]
async fn snapshot_lookup_isolation_matrix() {
    for (case, mutation) in [
        (1, "source_store_id='ffffffffffffffffffffffffffffffff'"),
        (2, "tenant_id='tenant_other'"),
        (3, "user_present=0,user_id=''"),
        (4, "user_id='user_other'"),
        (5, "workspace_id='workspace_other'"),
        (6, "run_id='run_other'"),
        (7, "agent_id='agent_other'"),
        (8, "owner_actor='agent_other'"),
        (9, "stream_id='stream_other'"),
    ] {
        assert_candidate_mutation(
            100 + case,
            &format!("UPDATE projection_snapshots SET {mutation}"),
            SnapshotDisposition::Missing,
        )
        .await;
    }
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
    let (connection, store_id, before) = setup_v1_history(&fixture).await;
    connection.close().await.unwrap();
    let store = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let mut read_connection = store.pool.acquire().await.unwrap();
    let after = v1_event_bytes(&mut read_connection).await;
    drop(read_connection);
    assert_eq!(after, before);
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
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events WHERE writer_epoch=1")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        2
    );
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_v2-task", 2, "task_v2"),
        )
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT writer_epoch FROM events WHERE event_id='event_v2-task'"
        )
        .fetch_one(&store.pool)
        .await
        .unwrap(),
        2
    );
    let projection = store
        .project_full(&fixture.projection_registry(), &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(projection.projection.cursor.sequence, "3");
    assert_eq!(projection.projection.manifest, fixture.manifest);
}

#[tokio::test]
async fn migration_rolls_back_each_v2_ddl_stage_with_history_intact() {
    for failed_step in 1..=6 {
        let fixture = Fixture::new(&format!("run_v2-rollback-{failed_step}"));
        let (mut connection, store_id, before) = setup_v1_history(&fixture).await;
        assert_eq!(
            EventStore::migrate_with_v2_failure(&mut connection, Some(failed_step))
                .await
                .unwrap_err()
                .kind,
            super::super::ErrorKind::Migration
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM pragma_table_info('events') WHERE name='writer_epoch'"
            )
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='projection_snapshots'")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM schema_migrations WHERE version=2")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            0
        );
        assert_eq!(v1_event_bytes(&mut connection).await, before);
        connection.close().await.unwrap();
        let migrated = EventStore::open_pinned(&fixture.path, &store_id)
            .await
            .unwrap();
        let mut migrated_connection = migrated.pool.acquire().await.unwrap();
        assert_eq!(v1_event_bytes(&mut migrated_connection).await, before);
    }
}

#[tokio::test]
async fn snapshot_actual_ddl_drift_is_rejected() {
    assert_snapshot_table_drift_rejected(
        super::super::SNAPSHOT_TABLE_DDL.replace(" CHECK(cursor_sequence > 0)", ""),
    )
    .await;
    assert_snapshot_table_drift_rejected(super::super::SNAPSHOT_TABLE_DDL.replace(
        " snapshot_digest TEXT NOT NULL,\n UNIQUE(source_store_id,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,owner_actor,stream_id,cursor_sequence,reducer_ref_fingerprint)",
        " snapshot_digest TEXT NOT NULL",
    ))
    .await;
    assert_snapshot_table_drift_rejected(super::super::SNAPSHOT_TABLE_DDL.replace(
        " snapshot_digest TEXT NOT NULL",
        " snapshot_digest BLOB NOT NULL",
    ))
    .await;

    let fixture = Fixture::new("run_snapshot-index-drift");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let store_id = store.store_id.clone();
    drop(store);
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", fixture.path.display())).unwrap();
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection
        .execute("DROP INDEX projection_snapshots_lookup")
        .await
        .unwrap();
    connection.execute("CREATE INDEX projection_snapshots_lookup ON projection_snapshots(source_store_id,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,owner_actor,stream_id,snapshot_ordinal DESC,cursor_sequence DESC)").await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(
        EventStore::open_pinned(&fixture.path, &store_id)
            .await
            .unwrap_err()
            .kind,
        super::super::ErrorKind::Migration
    );
}

#[tokio::test]
async fn already_open_v1_writer() {
    let fixture = Fixture::new("run_old-writer");
    let (setup, store_id, before) = setup_v1_history(&fixture).await;
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", fixture.path.display())).unwrap();
    let mut old_writer = SqliteConnection::connect_with(&options).await.unwrap();
    setup.close().await.unwrap();
    let migrated = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let stream = super::super::lifecycle::lifecycle_stream_id(&fixture.scope).unwrap();
    let event = super::super::lifecycle::lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_old-writer").unwrap(),
        3,
        "2026-08-25T01:00:02.000Z",
        "corr-old-writer",
        "task-created",
        &TaskCreatedPayload {
            task_id: TaskId::parse("task_old-writer").unwrap(),
            parent_task_id: None,
            initial_state: TaskState::Created,
        },
    )
    .unwrap();
    let prepared = super::super::PreparedEvent::new(&event, &fixture.set, &fixture.limits).unwrap();
    let envelope = &prepared.envelope;
    let (present, user) = user_key(&envelope.scope);
    let error = sqlx::query("INSERT INTO events(event_id,envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,causation_id,correlation_id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(envelope.event_id.as_str())
        .bind(&prepared.envelope_json)
        .bind(&prepared.envelope_fingerprint)
        .bind(&prepared.schema_set_json)
        .bind(&prepared.schema_set_fingerprint)
        .bind(&prepared.limits_json)
        .bind(&prepared.limits_fingerprint)
        .bind(envelope.scope.tenant_id.as_str())
        .bind(present)
        .bind(user)
        .bind(envelope.scope.workspace_id.as_str())
        .bind(envelope.scope.run_id.as_str())
        .bind(envelope.scope.agent_id.as_str())
        .bind(envelope.stream_id.as_str())
        .bind(prepared.sequence)
        .bind(envelope.causation_id.as_ref().map(EventId::as_str))
        .bind(&envelope.correlation_id)
        .execute(&mut old_writer)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("writer_epoch_conflict"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events")
            .fetch_one(&migrated.pool)
            .await
            .unwrap(),
        2
    );
    let mut read_connection = migrated.pool.acquire().await.unwrap();
    assert_eq!(v1_event_bytes(&mut read_connection).await, before);
    drop(read_connection);
    migrated
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_v2-writer", 2, "task_v2-writer"),
        )
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT writer_epoch FROM events WHERE event_id='event_v2-writer'"
        )
        .fetch_one(&migrated.pool)
        .await
        .unwrap(),
        2
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
