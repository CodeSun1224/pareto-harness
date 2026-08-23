use super::*;
use pareto_protocol::{
    AgentId, Digest, EventTypeBinding, EventVariantDecoder, ProtocolLimitsV1,
    SchemaAdmissionAuthorizer, SchemaDocument, SchemaRef, SchemaSetRef, ValidationError,
    digest_json, digest_schema, generate_schema_bundle,
};
use serde_json::json;
use sqlx::Executor;
use std::{any::Any, collections::BTreeMap};
use tempfile::TempDir;
use tokio::sync::Barrier;

struct Allow;
impl SchemaAdmissionAuthorizer for Allow {
    fn authorize(&self, _: Option<&SchemaSetRef>, _: &SchemaSetRef) -> Result<(), ValidationError> {
        Ok(())
    }
}

struct Decoder(SchemaRef);
impl EventVariantDecoder for Decoder {
    fn variant_id(&self) -> &str {
        "kernel-test-v1"
    }
    fn payload_schema_ref(&self) -> &SchemaRef {
        &self.0
    }
    fn decode(
        &self,
        payload: &serde_json::Value,
    ) -> Result<Box<dyn Any + Send + Sync>, ValidationError> {
        serde_json::from_value::<BTreeMap<String, String>>(payload.clone())
            .map(|value| Box::new(value) as Box<dyn Any + Send + Sync>)
            .map_err(|_| ValidationError {
                code: pareto_protocol::ErrorCode::SchemaMismatch,
                path: "/payload".into(),
                contract: "kernel_test".into(),
                detail: "decode failed".into(),
            })
    }
}

struct Fixture {
    _temp: TempDir,
    path: std::path::PathBuf,
    set: Arc<SchemaSet>,
    envelope_schema: SchemaRef,
    payload_schema: SchemaRef,
    limits: ProtocolLimitsRef,
}

impl Fixture {
    fn new() -> Self {
        let initial_bundle = generate_schema_bundle().unwrap();
        let initial = SchemaSet::bootstrap_initial(
            initial_bundle.manifest,
            initial_bundle.schemas,
            &initial_bundle.reference,
        )
        .unwrap();
        let mut bundle = generate_schema_bundle().unwrap();
        let document = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"urn:pareto-harness:schema:kernel-test:1.0","type":"object","properties":{"message":{"type":"string"}},"required":["message"],"unevaluatedProperties":false});
        let payload_schema = SchemaRef {
            r#type: "kernel-test".into(),
            major: 1,
            minor: 0,
            schema_digest: digest_schema("urn:pareto-harness:schema:kernel-test:1.0", &document)
                .unwrap(),
        };
        bundle.manifest.schemas.push(payload_schema.clone());
        bundle.manifest.schemas.sort();
        bundle.manifest.event_bindings.push(EventTypeBinding {
            event_type: "kernel-test".into(),
            major: 1,
            minor: 0,
            payload_schema_ref: payload_schema.clone(),
            variant_id: "kernel-test-v1".into(),
        });
        bundle.schemas.push(SchemaDocument {
            filename: "kernel-test-v1.0.schema.json".into(),
            document,
        });
        let reference = SchemaSetRef {
            manifest_schema_ref: bundle.reference.manifest_schema_ref.clone(),
            manifest_digest: digest_json(
                "schema-set",
                &bundle.reference.manifest_schema_ref,
                &serde_json::to_value(&bundle.manifest).unwrap(),
            )
            .unwrap(),
        };
        let envelope_schema = bundle.manifest.event_envelope_schema_ref.clone();
        let set = SchemaSet::admit_with(
            &Allow,
            Some(&initial),
            bundle.manifest,
            bundle.schemas,
            &reference,
            vec![Arc::new(Decoder(payload_schema.clone()))],
        )
        .unwrap();
        let limits = ProtocolLimitsRef {
            profile: "protocol-limits-v1".into(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("events.sqlite3");
        Self {
            _temp: temp,
            path,
            set: Arc::new(set),
            envelope_schema,
            payload_schema,
            limits,
        }
    }

    fn scope(&self) -> IsolationScope {
        IsolationScope {
            tenant_id: pareto_protocol::TenantId::parse("tenant_local").unwrap(),
            user_id: None,
            workspace_id: pareto_protocol::WorkspaceId::parse("workspace_repo").unwrap(),
            run_id: pareto_protocol::RunId::parse("run_one").unwrap(),
            agent_id: AgentId::parse("agent_primary").unwrap(),
        }
    }

    fn event(&self, event: &str, stream: &str, sequence: i64) -> EventEnvelope {
        let payload = json!({"message":event});
        EventEnvelope {
            schema_ref: self.envelope_schema.clone(),
            scope: self.scope(),
            event_id: EventId::parse(event).unwrap(),
            stream_id: StreamId::parse(stream).unwrap(),
            run_id: self.scope().run_id,
            sequence: sequence.to_string(),
            causation_id: None,
            correlation_id: "corr-test".into(),
            event_type: "kernel-test".into(),
            event_major: 1,
            event_minor: 0,
            occurred_at: "2026-08-23T00:00:00.000Z".into(),
            actor: self.scope().agent_id,
            payload_schema_ref: self.payload_schema.clone(),
            payload_digest: digest_json("event-payload", &self.payload_schema, &payload).unwrap(),
            payload,
        }
    }

    fn admit(&self, event: EventEnvelope) -> AdmittedAppend {
        let validated = self
            .set
            .validate_event_at_boundary(
                event.clone(),
                event.scope.clone(),
                event.actor.clone(),
                event.stream_id.clone(),
                self.limits.clone(),
            )
            .unwrap();
        AdmittedAppend::admit(validated, self.set.clone(), self.limits.clone()).unwrap()
    }

    fn stream_read(&self, stream: &str) -> AdmittedRead {
        AdmittedRead {
            scope: self.scope(),
            stream_id: Some(StreamId::parse(stream).unwrap()),
            schema_set: self.set.clone(),
            limits: self.limits.clone(),
        }
    }

    fn run_read(&self) -> AdmittedRead {
        AdmittedRead {
            scope: self.scope(),
            stream_id: None,
            schema_set: self.set.clone(),
            limits: self.limits.clone(),
        }
    }
}

#[tokio::test]
async fn migration_is_atomic_versioned_and_detects_trigger_drift() {
    let fixture = Fixture::new();
    let store = EventStore::open(&fixture.path).await.unwrap();
    drop(store);
    EventStore::open(&fixture.path).await.unwrap();
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", fixture.path.display())).unwrap();
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection.execute("PRAGMA user_version=2").await.unwrap();
    assert_eq!(
        EventStore::open(&fixture.path).await.unwrap_err().kind,
        ErrorKind::Migration
    );
    connection.execute("PRAGMA user_version=1").await.unwrap();
    connection
        .execute("DROP TRIGGER events_no_delete")
        .await
        .unwrap();
    assert_eq!(
        EventStore::open(&fixture.path).await.unwrap_err().kind,
        ErrorKind::Migration
    );
}

#[tokio::test]
async fn append_round_trip_idempotency_sequence_and_restart() {
    let fixture = Fixture::new();
    let store = EventStore::open(&fixture.path).await.unwrap();
    let first = fixture.event("event_one", "stream_main", 1);
    assert!(matches!(
        store.append(fixture.admit(first.clone())).await.unwrap(),
        AppendResult::Appended { sequence: 1, .. }
    ));
    assert!(matches!(
        store.append(fixture.admit(first.clone())).await.unwrap(),
        AppendResult::AlreadyCommitted { sequence: 1, .. }
    ));
    let mut changed = first;
    changed.correlation_id = "corr-changed".into();
    assert_eq!(
        store.append(fixture.admit(changed)).await.unwrap_err().kind,
        ErrorKind::IdempotencyConflict
    );
    assert_eq!(
        store
            .append(fixture.admit(fixture.event("event_gap", "stream_main", 3)))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::SequenceConflict
    );
    drop(store);
    let store = EventStore::open(&fixture.path).await.unwrap();
    store
        .append(fixture.admit(fixture.event("event_two", "stream_main", 2)))
        .await
        .unwrap();
    let page = store
        .read(&fixture.stream_read("stream_main"), None, 10)
        .await
        .unwrap();
    assert_eq!(page.events.len(), 2);
    assert_eq!(page.events[1].envelope().event_id.as_str(), "event_two");
}

#[tokio::test]
async fn append_only_triggers_and_row_drift_fail_closed() {
    let fixture = Fixture::new();
    let store = EventStore::open(&fixture.path).await.unwrap();
    store
        .append(fixture.admit(fixture.event("event_one", "stream_main", 1)))
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE events SET correlation_id='bad'")
            .execute(&store.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM events")
            .execute(&store.pool)
            .await
            .is_err()
    );
    drop(store);
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", fixture.path.display())).unwrap();
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection
        .execute("DROP TRIGGER events_no_update")
        .await
        .unwrap();
    connection
        .execute("UPDATE events SET stream_id='stream_wrong'")
        .await
        .unwrap();
    connection.execute(UPDATE_TRIGGER).await.unwrap();
    assert_eq!(
        EventStore::open(&fixture.path).await.unwrap_err().kind,
        ErrorKind::DatabaseCorrupt
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_same_sequence_has_at_most_one_commit() {
    let fixture = Fixture::new();
    let store = Arc::new(EventStore::open(&fixture.path).await.unwrap());
    let barrier = Arc::new(Barrier::new(3));
    let mut tasks = Vec::new();
    for name in ["event_left", "event_right"] {
        let store = store.clone();
        let barrier = barrier.clone();
        let admitted = fixture.admit(fixture.event(name, "stream_race", 1));
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store.append(admitted).await
        }));
    }
    barrier.wait().await;
    let results = futures_for_tests(tasks).await;
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(AppendResult::Appended { .. })))
            .count(),
        1
    );
}

async fn futures_for_tests(
    tasks: Vec<tokio::task::JoinHandle<Result<AppendResult, EventStoreError>>>,
) -> Vec<Result<AppendResult, EventStoreError>> {
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    results
}

#[tokio::test]
async fn run_cursor_uses_fixed_horizon_and_rejects_scope_mixing() {
    let fixture = Fixture::new();
    let store = EventStore::open(&fixture.path).await.unwrap();
    store
        .append(fixture.admit(fixture.event("event_z-one", "stream_z", 1)))
        .await
        .unwrap();
    store
        .append(fixture.admit(fixture.event("event_z-two", "stream_z", 2)))
        .await
        .unwrap();
    let first = store.read(&fixture.run_read(), None, 1).await.unwrap();
    let cursor = first.next.unwrap();
    store
        .append(fixture.admit(fixture.event("event_a-one", "stream_a", 1)))
        .await
        .unwrap();
    drop(store);
    let store = EventStore::open(&fixture.path).await.unwrap();
    sqlx::query("VACUUM").execute(&store.pool).await.unwrap();
    let second = store
        .read(&fixture.run_read(), Some(&cursor), 10)
        .await
        .unwrap();
    assert_eq!(second.events.len(), 1);
    assert_eq!(second.events[0].envelope().event_id.as_str(), "event_z-two");
    let mut wrong = cursor;
    wrong.binding = "tampered".into();
    assert_eq!(
        store
            .read(&fixture.run_read(), Some(&wrong), 10)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::IsolationConflict
    );
}

#[tokio::test]
async fn performance_observation_uses_real_sqlite() {
    let fixture = Fixture::new();
    let store = EventStore::open(&fixture.path).await.unwrap();
    let append_start = std::time::Instant::now();
    for sequence in 1..=50 {
        store
            .append(fixture.admit(fixture.event(
                &format!("event_perf-{sequence}"),
                "stream_perf",
                sequence,
            )))
            .await
            .unwrap();
    }
    let append_elapsed = append_start.elapsed();
    let read_start = std::time::Instant::now();
    let page = store
        .read(&fixture.stream_read("stream_perf"), None, 50)
        .await
        .unwrap();
    let read_elapsed = read_start.elapsed();
    assert_eq!(page.events.len(), 50);
    eprintln!(
        "REQ-0004 observation: 50 FULL/WAL appends={append_elapsed:?}, 50 validated reads={read_elapsed:?}"
    );
}

#[tokio::test]
async fn failed_causation_is_atomic_and_exact_limits_are_bound() {
    let fixture = Fixture::new();
    let store = EventStore::open(&fixture.path).await.unwrap();
    let mut caused = fixture.event("event_caused", "stream_main", 1);
    caused.causation_id = Some(EventId::parse("event_missing").unwrap());
    assert_eq!(
        store.append(fixture.admit(caused)).await.unwrap_err().kind,
        ErrorKind::CausationConflict
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    store
        .append(fixture.admit(fixture.event("event_one", "stream_main", 1)))
        .await
        .unwrap();

    let event = fixture.event("event_two", "stream_main", 2);
    let validated = fixture
        .set
        .validate_event_at_boundary(
            event.clone(),
            event.scope.clone(),
            event.actor.clone(),
            event.stream_id.clone(),
            fixture.limits.clone(),
        )
        .unwrap();
    let wrong_limits = ProtocolLimitsRef {
        profile: "protocol-limits-v1".into(),
        digest: Digest::parse(format!("sha256:{}", "0".repeat(64))).unwrap(),
    };
    assert!(matches!(
        AdmittedAppend::admit(validated, fixture.set.clone(), wrong_limits),
        Err(EventStoreError {
            kind: ErrorKind::ProtocolInvalid
        })
    ));
}
