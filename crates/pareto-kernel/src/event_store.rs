//! SQLite-backed append-only event log. All authority-bearing types and entry points stay private.

use pareto_protocol::{
    AgentId, EventEnvelope, EventId, IsolationScope, ProtocolLimitsRef, SchemaSet, SchemaSetRef,
    StreamId, ValidatedEvent, canonical_json,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use sqlx::{Connection, Executor, Row, SqliteConnection, SqlitePool, sqlite::SqliteConnectOptions};
use std::{path::Path, str::FromStr, sync::Arc, time::Duration};

const APPLICATION_ID: i32 = 0x5041_5245;
const DB_VERSION: i64 = 2;
const BUSY_MILLIS: u64 = 750;
const UPDATE_TRIGGER: &str = "CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT, 'append_only'); END";
const DELETE_TRIGGER: &str = "CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT, 'append_only'); END";
const WRITER_EPOCH_TRIGGER: &str = "CREATE TRIGGER events_writer_epoch_v2 BEFORE INSERT ON events WHEN NEW.writer_epoch != 2 BEGIN SELECT RAISE(ABORT, 'writer_epoch_conflict'); END";
const SNAPSHOT_UPDATE_TRIGGER: &str = "CREATE TRIGGER projection_snapshots_no_update BEFORE UPDATE ON projection_snapshots BEGIN SELECT RAISE(ABORT, 'snapshot_immutable'); END";
const SNAPSHOT_DELETE_TRIGGER: &str = "CREATE TRIGGER projection_snapshots_no_delete BEFORE DELETE ON projection_snapshots BEGIN SELECT RAISE(ABORT, 'snapshot_immutable'); END";

const EVENTS_DDL: &str = r#"
CREATE TABLE events (
 append_ordinal INTEGER PRIMARY KEY AUTOINCREMENT,
 event_id TEXT NOT NULL UNIQUE,
 envelope_json TEXT NOT NULL,
 envelope_fingerprint TEXT NOT NULL,
 schema_set_json TEXT NOT NULL,
 schema_set_fingerprint TEXT NOT NULL,
 limits_json TEXT NOT NULL,
 limits_fingerprint TEXT NOT NULL,
 tenant_id TEXT NOT NULL,
 user_present INTEGER NOT NULL CHECK(user_present IN (0,1)),
 user_id TEXT NOT NULL CHECK((user_present=0 AND user_id='') OR (user_present=1 AND user_id LIKE 'user_%')),
 workspace_id TEXT NOT NULL,
 run_id TEXT NOT NULL,
 agent_id TEXT NOT NULL,
 stream_id TEXT NOT NULL,
 sequence_i64 INTEGER NOT NULL CHECK(sequence_i64 > 0),
 causation_id TEXT,
 correlation_id TEXT NOT NULL,
 UNIQUE(tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64)
);
CREATE INDEX events_stream_scan ON events(tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id);
CREATE INDEX events_run_scan ON events(tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id);
CREATE TABLE store_metadata(singleton INTEGER PRIMARY KEY CHECK(singleton=1), store_id TEXT NOT NULL CHECK(length(store_id)=32));
INSERT INTO store_metadata(singleton,store_id) VALUES(1,lower(hex(randomblob(16))));
"#;

const WRITER_EPOCH_COLUMN_DDL: &str = "ALTER TABLE events ADD COLUMN writer_epoch INTEGER NOT NULL DEFAULT 1 CHECK(writer_epoch IN (1,2))";
const SNAPSHOT_TABLE_DDL: &str = r#"CREATE TABLE projection_snapshots (
 snapshot_ordinal INTEGER PRIMARY KEY AUTOINCREMENT,
 snapshot_json TEXT NOT NULL,
 snapshot_fingerprint TEXT NOT NULL,
 output_schema_set_json TEXT NOT NULL,
 output_schema_set_fingerprint TEXT NOT NULL,
 output_limits_json TEXT NOT NULL,
 output_limits_fingerprint TEXT NOT NULL,
 source_schema_set_json TEXT NOT NULL,
 source_schema_set_fingerprint TEXT NOT NULL,
 source_limits_json TEXT NOT NULL,
 source_limits_fingerprint TEXT NOT NULL,
 reducer_ref_json TEXT NOT NULL,
 reducer_ref_fingerprint TEXT NOT NULL,
 source_store_id TEXT NOT NULL,
 tenant_id TEXT NOT NULL,
 user_present INTEGER NOT NULL CHECK(user_present IN (0,1)),
 user_id TEXT NOT NULL CHECK((user_present=0 AND user_id='') OR (user_present=1 AND user_id LIKE 'user_%')),
 workspace_id TEXT NOT NULL,
 run_id TEXT NOT NULL,
 agent_id TEXT NOT NULL,
 owner_actor TEXT NOT NULL,
 stream_id TEXT NOT NULL,
 cursor_sequence INTEGER NOT NULL CHECK(cursor_sequence > 0),
 cursor_event_id TEXT NOT NULL,
 projection_digest TEXT NOT NULL,
 snapshot_digest TEXT NOT NULL,
 UNIQUE(source_store_id,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,owner_actor,stream_id,cursor_sequence,reducer_ref_fingerprint)
)"#;
const SNAPSHOT_INDEX_DDL: &str = "CREATE INDEX projection_snapshots_lookup ON projection_snapshots(source_store_id,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,owner_actor,stream_id,cursor_sequence DESC,snapshot_ordinal DESC)";
// Frozen checksum of the originally published v2 table/index migration bundle. Actual table,
// index, column and trigger SQL is independently checked below; retaining this value keeps
// databases created by the initial v2 writer openable after validator hardening.
const V2_MIGRATION_CHECKSUM: &str =
    "sha256:fe118a4cd78deb4abe730e545d3eb565a52673a3c9e5ad41c1d9adbcc14f600e";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ErrorKind {
    Migration,
    DatabaseCorrupt,
    ProtocolInvalid,
    IsolationConflict,
    IdempotencyConflict,
    SequenceConflict,
    CausationConflict,
    WriterEpochConflict,
    Busy,
    Io,
}

#[derive(Debug)]
pub(super) struct EventStoreError {
    pub(super) kind: ErrorKind,
}

impl EventStoreError {
    fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }
}

impl From<sqlx::Error> for EventStoreError {
    fn from(error: sqlx::Error) -> Self {
        let kind = match &error {
            sqlx::Error::Database(database)
                if database.message().contains("writer_epoch_conflict") =>
            {
                ErrorKind::WriterEpochConflict
            }
            sqlx::Error::Database(database)
                if matches!(database.code().as_deref(), Some("5" | "6")) =>
            {
                ErrorKind::Busy
            }
            sqlx::Error::Database(database) if database.code().as_deref() == Some("14") => {
                ErrorKind::Io
            }
            sqlx::Error::Io(_) => ErrorKind::Io,
            _ => ErrorKind::DatabaseCorrupt,
        };
        Self::new(kind)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum AppendResult {
    Appended { event_id: EventId, sequence: i64 },
    AlreadyCommitted { event_id: EventId, sequence: i64 },
}

struct AdmittedAppend {
    event: ValidatedEvent,
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
}

impl AdmittedAppend {
    fn admit(
        authority: &KernelAuthority,
        event: ValidatedEvent,
        schema_set: Arc<SchemaSet>,
        limits: ProtocolLimitsRef,
    ) -> Result<Self, EventStoreError> {
        if schema_set.reference() != &authority.schema_set_ref || limits != authority.limits {
            return Err(EventStoreError::new(ErrorKind::ProtocolInvalid));
        }
        let envelope = event.envelope().clone();
        let target_stream = authority
            .target_stream
            .clone()
            .ok_or_else(|| EventStoreError::new(ErrorKind::IsolationConflict))?;
        let event = schema_set
            .validate_event_at_boundary(
                envelope.clone(),
                authority.scope.clone(),
                authority.actor.clone(),
                target_stream,
                limits.clone(),
            )
            .map_err(|_| EventStoreError::new(ErrorKind::ProtocolInvalid))?;
        Ok(Self {
            event,
            schema_set,
            limits,
        })
    }
}

struct KernelAuthority {
    scope: IsolationScope,
    actor: AgentId,
    target_stream: Option<StreamId>,
    schema_set_ref: SchemaSetRef,
    limits: ProtocolLimitsRef,
}

impl KernelAuthority {
    fn authenticated(
        scope: IsolationScope,
        actor: AgentId,
        target_stream: Option<StreamId>,
        schema_set_ref: SchemaSetRef,
        limits: ProtocolLimitsRef,
    ) -> Self {
        Self {
            scope,
            actor,
            target_stream,
            schema_set_ref,
            limits,
        }
    }
}

struct AdmittedRead {
    scope: IsolationScope,
    stream_id: Option<StreamId>,
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
}

impl AdmittedRead {
    fn admit(
        authority: &KernelAuthority,
        registry: &SchemaRegistry,
    ) -> Result<Self, EventStoreError> {
        Ok(Self {
            scope: authority.scope.clone(),
            stream_id: authority.target_stream.clone(),
            schema_set: registry.resolve(&authority.schema_set_ref)?,
            limits: authority.limits.clone(),
        })
    }
}

#[derive(Clone)]
struct SchemaRegistry(Vec<Arc<SchemaSet>>);

impl SchemaRegistry {
    fn resolve(&self, reference: &SchemaSetRef) -> Result<Arc<SchemaSet>, EventStoreError> {
        self.0
            .iter()
            .find(|set| set.reference() == reference)
            .cloned()
            .ok_or_else(|| EventStoreError::new(ErrorKind::ProtocolInvalid))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cursor {
    kind: &'static str,
    binding: String,
    horizon: i64,
    last_stream: String,
    last_sequence: i64,
    last_event: String,
    seal: String,
}

#[derive(Debug)]
struct Page {
    events: Vec<ValidatedEvent>,
    next: Option<Cursor>,
}

#[derive(Debug)]
struct EventStore {
    pool: SqlitePool,
    store_id: String,
}

pub(super) struct PreparedEvent {
    envelope: EventEnvelope,
    envelope_json: String,
    envelope_fingerprint: String,
    schema_set_json: String,
    schema_set_fingerprint: String,
    limits_json: String,
    limits_fingerprint: String,
    sequence: i64,
}

impl PreparedEvent {
    pub(super) fn new(
        event: &ValidatedEvent,
        schema_set: &SchemaSet,
        limits: &ProtocolLimitsRef,
    ) -> Result<Self, EventStoreError> {
        let envelope = event.envelope().clone();
        let sequence = envelope
            .sequence
            .parse::<i64>()
            .map_err(|_| EventStoreError::new(ErrorKind::SequenceConflict))?;
        let envelope_json = canonical(&envelope)?;
        let schema_set_json = canonical(schema_set.reference())?;
        let limits_json = canonical(limits)?;
        Ok(Self {
            envelope,
            envelope_fingerprint: fingerprint(envelope_json.as_bytes()),
            schema_set_fingerprint: fingerprint(schema_set_json.as_bytes()),
            limits_fingerprint: fingerprint(limits_json.as_bytes()),
            envelope_json,
            schema_set_json,
            limits_json,
            sequence,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AtomicPairFault {
    None,
    AfterFirstInsert,
    BeforeCommit,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct AtomicPairResult {
    pub(super) first: AppendResult,
    pub(super) second: AppendResult,
    pub(super) already_committed: bool,
}

pub(super) async fn append_atomic_pair(
    connection: &mut SqliteConnection,
    first: &PreparedEvent,
    second: &PreparedEvent,
    fault: AtomicPairFault,
) -> Result<AtomicPairResult, EventStoreError> {
    let first_existing = check_prepared_idempotency(connection, first).await?;
    let second_existing = check_prepared_idempotency(connection, second).await?;
    match (first_existing, second_existing) {
        (Some(first), Some(second)) => Ok(AtomicPairResult {
            first,
            second,
            already_committed: true,
        }),
        (Some(_), None) | (None, Some(_)) => Err(EventStoreError::new(ErrorKind::DatabaseCorrupt)),
        (None, None) => {
            let first = insert_prepared(connection, first).await?;
            if fault == AtomicPairFault::AfterFirstInsert {
                return Err(EventStoreError::new(ErrorKind::Io));
            }
            let second = insert_prepared(connection, second).await?;
            if fault == AtomicPairFault::BeforeCommit {
                return Err(EventStoreError::new(ErrorKind::Io));
            }
            Ok(AtomicPairResult {
                first,
                second,
                already_committed: false,
            })
        }
    }
}

impl EventStore {
    async fn open(path: &Path) -> Result<Self, EventStoreError> {
        if path.exists() {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
        Self::open_inner(path, None).await
    }

    async fn open_pinned(path: &Path, expected_store_id: &str) -> Result<Self, EventStoreError> {
        Self::open_inner(path, Some(expected_store_id)).await
    }

    async fn open_inner(
        path: &Path,
        expected_store_id: Option<&str>,
    ) -> Result<Self, EventStoreError> {
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .map_err(|_| EventStoreError::new(ErrorKind::Io))?
            .create_if_missing(true)
            .busy_timeout(Duration::from_millis(BUSY_MILLIS));
        let mut connection = SqliteConnection::connect_with(&options).await?;
        let store_id = Self::migrate(&mut connection).await?;
        if expected_store_id.is_some_and(|expected| expected != store_id) {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
        connection.close().await?;
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .after_connect(|connection, _| {
                Box::pin(async move {
                    connection.execute("PRAGMA foreign_keys=ON").await?;
                    connection.execute("PRAGMA trusted_schema=OFF").await?;
                    connection.execute("PRAGMA busy_timeout=750").await?;
                    connection.execute("PRAGMA synchronous=FULL").await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await?;
        Ok(Self { pool, store_id })
    }

    async fn migrate(connection: &mut SqliteConnection) -> Result<String, EventStoreError> {
        Self::migrate_with_v2_failure(connection, None).await
    }

    async fn migrate_with_v2_failure(
        connection: &mut SqliteConnection,
        fail_after_v2_step: Option<usize>,
    ) -> Result<String, EventStoreError> {
        connection.execute("PRAGMA journal_mode=WAL").await?;
        connection.execute("PRAGMA synchronous=FULL").await?;
        connection.execute("PRAGMA foreign_keys=ON").await?;
        connection.execute("PRAGMA trusted_schema=OFF").await?;
        connection.execute("BEGIN EXCLUSIVE").await?;
        let result = Self::migrate_locked(connection, fail_after_v2_step).await;
        match result {
            Ok(store_id) => connection
                .execute("COMMIT")
                .await
                .map(|_| store_id)
                .map_err(Into::into),
            Err(error) => {
                let _ = connection.execute("ROLLBACK").await;
                Err(error)
            }
        }
    }

    async fn migrate_locked(
        connection: &mut SqliteConnection,
        fail_after_v2_step: Option<usize>,
    ) -> Result<String, EventStoreError> {
        let application_id: i64 = sqlx::query_scalar("PRAGMA application_id")
            .fetch_one(&mut *connection)
            .await?;
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *connection)
            .await?;
        if version > DB_VERSION || (version > 0 && application_id != i64::from(APPLICATION_ID)) {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
        if version == 0 {
            connection
                .execute(format!("PRAGMA application_id={APPLICATION_ID}").as_str())
                .await?;
            connection.execute("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at_explicit TEXT NOT NULL)").await?;
            sqlx::raw_sql(EVENTS_DDL).execute(&mut *connection).await?;
            connection.execute(UPDATE_TRIGGER).await?;
            connection.execute(DELETE_TRIGGER).await?;
            let checksum = fingerprint(EVENTS_DDL.as_bytes());
            sqlx::query("INSERT INTO schema_migrations(version,checksum,applied_at_explicit) VALUES(1,?,'2026-08-23T00:00:00.000Z')")
                .bind(checksum).execute(&mut *connection).await?;
            connection.execute("PRAGMA user_version=1").await?;
        }
        validate_v1_contract(connection).await?;
        if version <= 1 {
            connection.execute(WRITER_EPOCH_COLUMN_DDL).await?;
            fail_v2_migration_after(fail_after_v2_step, 1)?;
            connection.execute(SNAPSHOT_TABLE_DDL).await?;
            fail_v2_migration_after(fail_after_v2_step, 2)?;
            connection.execute(SNAPSHOT_INDEX_DDL).await?;
            fail_v2_migration_after(fail_after_v2_step, 3)?;
            connection.execute(WRITER_EPOCH_TRIGGER).await?;
            fail_v2_migration_after(fail_after_v2_step, 4)?;
            connection.execute(SNAPSHOT_UPDATE_TRIGGER).await?;
            fail_v2_migration_after(fail_after_v2_step, 5)?;
            connection.execute(SNAPSHOT_DELETE_TRIGGER).await?;
            fail_v2_migration_after(fail_after_v2_step, 6)?;
            sqlx::query("INSERT INTO schema_migrations(version,checksum,applied_at_explicit) VALUES(2,?,'2026-08-25T00:00:00.000Z')")
                .bind(v2_migration_checksum())
                .execute(&mut *connection)
                .await?;
            connection.execute("PRAGMA user_version=2").await?;
        }
        let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
            .fetch_one(&mut *connection)
            .await?;
        if integrity != "ok" {
            return Err(EventStoreError::new(ErrorKind::DatabaseCorrupt));
        }
        let v2_checksum: String =
            sqlx::query_scalar("SELECT checksum FROM schema_migrations WHERE version=2")
                .fetch_optional(&mut *connection)
                .await?
                .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
        if v2_checksum != v2_migration_checksum() {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
        let writer_epoch_contract: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('events') WHERE name='writer_epoch' AND type='INTEGER' AND \"notnull\"=1 AND dflt_value='1'",
        )
        .fetch_one(&mut *connection)
        .await?;
        let events_table_sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='events'",
        )
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
        if writer_epoch_contract != 1
            || !events_table_sql
                .contains("writer_epoch INTEGER NOT NULL DEFAULT 1 CHECK(writer_epoch IN (1,2))")
        {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
        for (object_type, name, expected) in [
            ("table", "projection_snapshots", SNAPSHOT_TABLE_DDL),
            ("index", "projection_snapshots_lookup", SNAPSHOT_INDEX_DDL),
        ] {
            let actual: String = sqlx::query_scalar(
                "SELECT sql FROM sqlite_master WHERE type=? AND name=? AND tbl_name='projection_snapshots'",
            )
            .bind(object_type)
            .bind(name)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
            if actual != expected {
                return Err(EventStoreError::new(ErrorKind::Migration));
            }
        }
        let store_id: String = sqlx::query_scalar(
            "SELECT store_id FROM store_metadata WHERE singleton=1 AND length(store_id)=32",
        )
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
        if !store_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
        for (name, expected) in [
            ("events_no_update", UPDATE_TRIGGER),
            ("events_no_delete", DELETE_TRIGGER),
            ("events_writer_epoch_v2", WRITER_EPOCH_TRIGGER),
            ("projection_snapshots_no_update", SNAPSHOT_UPDATE_TRIGGER),
            ("projection_snapshots_no_delete", SNAPSHOT_DELETE_TRIGGER),
        ] {
            let actual: String =
                sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?")
                    .bind(name)
                    .fetch_optional(&mut *connection)
                    .await?
                    .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
            if actual != expected {
                return Err(EventStoreError::new(ErrorKind::Migration));
            }
        }
        let drift: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM events WHERE
            json_extract(envelope_json,'$.event_id') != event_id OR
            json_extract(envelope_json,'$.scope.tenant_id') != tenant_id OR
            json_extract(envelope_json,'$.scope.workspace_id') != workspace_id OR
            json_extract(envelope_json,'$.scope.run_id') != run_id OR
            json_extract(envelope_json,'$.run_id') != run_id OR
            json_extract(envelope_json,'$.scope.agent_id') != agent_id OR
            json_extract(envelope_json,'$.stream_id') != stream_id OR
            CAST(json_extract(envelope_json,'$.sequence') AS INTEGER) != sequence_i64 OR
            NOT (json_extract(envelope_json,'$.causation_id') IS causation_id) OR
            json_extract(envelope_json,'$.correlation_id') != correlation_id OR
            (user_present=0 AND json_type(envelope_json,'$.scope.user_id') IS NOT NULL) OR
            (user_present=1 AND json_extract(envelope_json,'$.scope.user_id') != user_id)"#,
        )
        .fetch_one(&mut *connection)
        .await?;
        if drift != 0 {
            return Err(EventStoreError::new(ErrorKind::DatabaseCorrupt));
        }
        let invalid_epoch: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE writer_epoch NOT IN (1,2)")
                .fetch_one(&mut *connection)
                .await?;
        if invalid_epoch != 0 {
            return Err(EventStoreError::new(ErrorKind::DatabaseCorrupt));
        }
        validate_all_stored_bytes(connection).await?;
        Ok(store_id)
    }

    async fn append(&self, admitted: AdmittedAppend) -> Result<AppendResult, EventStoreError> {
        let prepared = PreparedEvent::new(&admitted.event, &admitted.schema_set, &admitted.limits)?;
        let mut connection = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = async {
            if let Some(result) = check_prepared_idempotency(&mut connection, &prepared).await? {
                return Ok(result);
            }
            insert_prepared(&mut connection, &prepared).await
        }
        .await;
        match result {
            Ok(value) => {
                connection.commit().await?;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }

    async fn read(
        &self,
        admitted: &AdmittedRead,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page, EventStoreError> {
        if limit <= 0 {
            return Err(EventStoreError::new(ErrorKind::ProtocolInvalid));
        }
        let kind = if admitted.stream_id.is_some() {
            "stream"
        } else {
            "run"
        };
        let binding = read_binding(admitted, kind)?;
        let mut connection = self.pool.begin().await?;
        let result = async {
            let horizon = match cursor {
                Some(cursor)
                    if cursor.kind == kind
                        && cursor.binding == binding
                        && cursor.seal == cursor_seal(cursor) => cursor.horizon,
                Some(_) => return Err(EventStoreError::new(ErrorKind::IsolationConflict)),
                None => sqlx::query_scalar("SELECT COALESCE(MAX(append_ordinal),0) FROM events").fetch_one(&mut *connection).await?,
            };
            let previous_stream = cursor.map_or("", |item| item.last_stream.as_str());
            let previous_sequence = cursor.map_or(0, |item| item.last_sequence);
            let previous_event = cursor.map_or("", |item| item.last_event.as_str());
            let (present, user) = user_key(&admitted.scope);
            let rows = if let Some(stream) = &admitted.stream_id {
                sqlx::query("SELECT envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? AND append_ordinal<=? AND (sequence_i64>? OR (sequence_i64=? AND event_id>?)) ORDER BY sequence_i64,event_id LIMIT ?")
                    .bind(admitted.scope.tenant_id.as_str()).bind(present).bind(user).bind(admitted.scope.workspace_id.as_str()).bind(admitted.scope.run_id.as_str()).bind(admitted.scope.agent_id.as_str()).bind(stream.as_str()).bind(horizon).bind(previous_sequence).bind(previous_sequence).bind(previous_event).bind(limit + 1).fetch_all(&mut *connection).await?
            } else {
                sqlx::query("SELECT envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND append_ordinal<=? AND (stream_id>? OR (stream_id=? AND sequence_i64>?) OR (stream_id=? AND sequence_i64=? AND event_id>?)) ORDER BY stream_id,sequence_i64,event_id LIMIT ?")
                    .bind(admitted.scope.tenant_id.as_str()).bind(present).bind(user).bind(admitted.scope.workspace_id.as_str()).bind(admitted.scope.run_id.as_str()).bind(admitted.scope.agent_id.as_str()).bind(horizon).bind(previous_stream).bind(previous_stream).bind(previous_sequence).bind(previous_stream).bind(previous_sequence).bind(previous_event).bind(limit + 1).fetch_all(&mut *connection).await?
            };
            let has_more = rows.len() > limit as usize;
            let mut events = Vec::new();
            let mut last = None;
            for row in rows.into_iter().take(limit as usize) {
                let event = validate_row(&row, admitted)?;
                last = Some((event.envelope().stream_id.as_str().to_owned(), event.envelope().sequence.parse().unwrap_or(0), event.envelope().event_id.as_str().to_owned()));
                events.push(event);
            }
            let next = if has_more { last.map(|(stream, sequence, event)| {
                let mut cursor = Cursor { kind, binding, horizon, last_stream: stream, last_sequence: sequence, last_event: event, seal: String::new() };
                cursor.seal = cursor_seal(&cursor);
                cursor
            }) } else { None };
            Ok(Page { events, next })
        }.await;
        let _ = connection.rollback().await;
        result
    }
}

async fn check_prepared_idempotency(
    connection: &mut SqliteConnection,
    prepared: &PreparedEvent,
) -> Result<Option<AppendResult>, EventStoreError> {
    let row = sqlx::query("SELECT envelope_fingerprint,schema_set_fingerprint,limits_fingerprint,sequence_i64 FROM events WHERE event_id=?")
        .bind(prepared.envelope.event_id.as_str())
        .fetch_optional(&mut *connection)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.get::<String, _>(0) == prepared.envelope_fingerprint
        && row.get::<String, _>(1) == prepared.schema_set_fingerprint
        && row.get::<String, _>(2) == prepared.limits_fingerprint
    {
        Ok(Some(AppendResult::AlreadyCommitted {
            event_id: prepared.envelope.event_id.clone(),
            sequence: row.get(3),
        }))
    } else {
        Err(EventStoreError::new(ErrorKind::IdempotencyConflict))
    }
}

async fn insert_prepared(
    connection: &mut SqliteConnection,
    prepared: &PreparedEvent,
) -> Result<AppendResult, EventStoreError> {
    let envelope = &prepared.envelope;
    let (user_present, user_id) = user_key(&envelope.scope);
    let next: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(sequence_i64),0)+1 FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=?")
        .bind(envelope.scope.tenant_id.as_str()).bind(user_present).bind(user_id)
        .bind(envelope.scope.workspace_id.as_str()).bind(envelope.scope.run_id.as_str())
        .bind(envelope.scope.agent_id.as_str()).bind(envelope.stream_id.as_str())
        .fetch_one(&mut *connection).await?;
    if prepared.sequence != next {
        return Err(EventStoreError::new(ErrorKind::SequenceConflict));
    }
    if let Some(cause) = &envelope.causation_id {
        let found: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE event_id=? AND tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=?")
            .bind(cause.as_str()).bind(envelope.scope.tenant_id.as_str()).bind(user_present).bind(user_id)
            .bind(envelope.scope.workspace_id.as_str()).bind(envelope.scope.run_id.as_str())
            .bind(envelope.scope.agent_id.as_str()).bind(envelope.stream_id.as_str()).fetch_one(&mut *connection).await?;
        if found != 1 {
            return Err(EventStoreError::new(ErrorKind::CausationConflict));
        }
    }
    sqlx::query("INSERT INTO events(event_id,envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,causation_id,correlation_id,writer_epoch) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,2)")
        .bind(envelope.event_id.as_str()).bind(&prepared.envelope_json).bind(&prepared.envelope_fingerprint)
        .bind(&prepared.schema_set_json).bind(&prepared.schema_set_fingerprint)
        .bind(&prepared.limits_json).bind(&prepared.limits_fingerprint)
        .bind(envelope.scope.tenant_id.as_str()).bind(user_present).bind(user_id)
        .bind(envelope.scope.workspace_id.as_str()).bind(envelope.scope.run_id.as_str())
        .bind(envelope.scope.agent_id.as_str()).bind(envelope.stream_id.as_str())
        .bind(prepared.sequence).bind(envelope.causation_id.as_ref().map(EventId::as_str))
        .bind(&envelope.correlation_id).execute(&mut *connection).await?;
    Ok(AppendResult::Appended {
        event_id: envelope.event_id.clone(),
        sequence: prepared.sequence,
    })
}

async fn validate_v1_contract(connection: &mut SqliteConnection) -> Result<(), EventStoreError> {
    let checksum: String =
        sqlx::query_scalar("SELECT checksum FROM schema_migrations WHERE version=1")
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
    if checksum != fingerprint(EVENTS_DDL.as_bytes()) {
        return Err(EventStoreError::new(ErrorKind::Migration));
    }
    for (name, expected) in [
        ("events_no_update", UPDATE_TRIGGER),
        ("events_no_delete", DELETE_TRIGGER),
    ] {
        let actual: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?")
                .bind(name)
                .fetch_optional(&mut *connection)
                .await?
                .ok_or_else(|| EventStoreError::new(ErrorKind::Migration))?;
        if actual != expected {
            return Err(EventStoreError::new(ErrorKind::Migration));
        }
    }
    validate_all_stored_bytes(connection).await
}

fn fail_v2_migration_after(
    configured_step: Option<usize>,
    completed_step: usize,
) -> Result<(), EventStoreError> {
    if configured_step == Some(completed_step) {
        Err(EventStoreError::new(ErrorKind::Migration))
    } else {
        Ok(())
    }
}

fn v2_migration_checksum() -> String {
    V2_MIGRATION_CHECKSUM.to_owned()
}

fn validate_row(
    row: &sqlx::sqlite::SqliteRow,
    admitted: &AdmittedRead,
) -> Result<ValidatedEvent, EventStoreError> {
    let envelope_json: String = row.get(0);
    if fingerprint(envelope_json.as_bytes()) != row.get::<String, _>(1) {
        return Err(EventStoreError::new(ErrorKind::DatabaseCorrupt));
    }
    let schema_json = canonical(admitted.schema_set.reference())?;
    let limits_json = canonical(&admitted.limits)?;
    if schema_json != row.get::<String, _>(2)
        || fingerprint(schema_json.as_bytes()) != row.get::<String, _>(3)
        || limits_json != row.get::<String, _>(4)
        || fingerprint(limits_json.as_bytes()) != row.get::<String, _>(5)
    {
        return Err(EventStoreError::new(ErrorKind::ProtocolInvalid));
    }
    let envelope: EventEnvelope = serde_json::from_str(&envelope_json)
        .map_err(|_| EventStoreError::new(ErrorKind::ProtocolInvalid))?;
    let (present, user) = user_key(&envelope.scope);
    let consistent = envelope.scope.tenant_id.as_str() == row.get::<String, _>(6)
        && present == row.get::<i64, _>(7)
        && user == row.get::<String, _>(8)
        && envelope.scope.workspace_id.as_str() == row.get::<String, _>(9)
        && envelope.scope.run_id.as_str() == row.get::<String, _>(10)
        && envelope.scope.agent_id.as_str() == row.get::<String, _>(11)
        && envelope.stream_id.as_str() == row.get::<String, _>(12)
        && envelope.sequence.parse::<i64>().ok() == Some(row.get(13))
        && envelope.event_id.as_str() == row.get::<String, _>(14)
        && envelope.causation_id.as_ref().map(EventId::as_str)
            == row.get::<Option<String>, _>(15).as_deref()
        && envelope.correlation_id == row.get::<String, _>(16);
    if !consistent {
        return Err(EventStoreError::new(ErrorKind::IsolationConflict));
    }
    admitted
        .schema_set
        .validate_event_at_boundary(
            envelope,
            admitted.scope.clone(),
            admitted.scope.agent_id.clone(),
            admitted.stream_id.clone().unwrap_or_else(|| {
                StreamId::parse(row.get::<String, _>(12)).expect("stored validated stream")
            }),
            admitted.limits.clone(),
        )
        .map_err(|_| EventStoreError::new(ErrorKind::ProtocolInvalid))
}

fn read_binding(read: &AdmittedRead, kind: &str) -> Result<String, EventStoreError> {
    canonical(&json!({"kind":kind,"scope":read.scope,"stream":read.stream_id,"schema_set":read.schema_set.reference(),"limits":read.limits})).map(|value| fingerprint(value.as_bytes()))
}

fn cursor_seal(cursor: &Cursor) -> String {
    fingerprint(
        format!(
            "{}\0{}\0{}\0{}\0{}\0{}",
            cursor.kind,
            cursor.binding,
            cursor.horizon,
            cursor.last_stream,
            cursor.last_sequence,
            cursor.last_event
        )
        .as_bytes(),
    )
}

async fn validate_all_stored_bytes(
    connection: &mut SqliteConnection,
) -> Result<(), EventStoreError> {
    let rows = sqlx::query("SELECT envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint FROM events")
        .fetch_all(&mut *connection).await?;
    for row in rows {
        let envelope_json: String = row.get(0);
        let schema_json: String = row.get(2);
        let limits_json: String = row.get(4);
        if fingerprint(envelope_json.as_bytes()) != row.get::<String, _>(1)
            || fingerprint(schema_json.as_bytes()) != row.get::<String, _>(3)
            || fingerprint(limits_json.as_bytes()) != row.get::<String, _>(5)
        {
            return Err(EventStoreError::new(ErrorKind::DatabaseCorrupt));
        }
        let envelope: EventEnvelope = serde_json::from_str(&envelope_json)
            .map_err(|_| EventStoreError::new(ErrorKind::DatabaseCorrupt))?;
        let schema: SchemaSetRef = serde_json::from_str(&schema_json)
            .map_err(|_| EventStoreError::new(ErrorKind::DatabaseCorrupt))?;
        let limits: ProtocolLimitsRef = serde_json::from_str(&limits_json)
            .map_err(|_| EventStoreError::new(ErrorKind::DatabaseCorrupt))?;
        if canonical(&envelope)? != envelope_json
            || canonical(&schema)? != schema_json
            || canonical(&limits)? != limits_json
        {
            return Err(EventStoreError::new(ErrorKind::DatabaseCorrupt));
        }
    }
    Ok(())
}

fn canonical<T: Serialize>(value: &T) -> Result<String, EventStoreError> {
    let value = serde_json::to_value(value)
        .map_err(|_| EventStoreError::new(ErrorKind::ProtocolInvalid))?;
    canonical_json(&value).map_err(|_| EventStoreError::new(ErrorKind::ProtocolInvalid))
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn user_key(scope: &IsolationScope) -> (i64, &str) {
    scope
        .user_id
        .as_ref()
        .map_or((0, ""), |user| (1, user.as_str()))
}

#[cfg(test)]
mod tests;

mod lifecycle;

mod hook_runtime;

mod projection;

mod runtime_control;
