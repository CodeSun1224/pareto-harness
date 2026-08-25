use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use pareto_protocol::RevisionId;

use super::test_support::Fixture;
use super::*;

async fn event_count(store: &EventStore) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn recorded_read_only() {
    let fixture = Fixture::new("run_replay-read-only");
    let store = fixture.open_created().await;
    let before = event_count(&store).await;
    let projection = store
        .recorded_replay(&fixture.projection_registry(), &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(projection.cursor.sequence, "1");
    assert_eq!(event_count(&store).await, before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projection_snapshots")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn simulated_no_effect() {
    let fixture = Fixture::new("run_replay-simulated");
    let store = fixture.open_created().await;
    let effect_calls = Arc::new(AtomicUsize::new(0));
    let request = SimulationRequest {
        source: fixture.projection_target(),
        fixture_revisions: vec![RevisionId::parse("rev_fixture").unwrap()],
    };
    assert_eq!(
        store.simulated_replay(&request).unwrap_err().kind,
        ProjectionErrorKind::SimulationUnavailable
    );
    assert_eq!(effect_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn recorded_determinism() {
    let fixture = Fixture::new("run_replay-determinism");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let first = store
        .recorded_replay(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let second = store
        .recorded_replay(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(canonical(&first).unwrap(), canonical(&second).unwrap());
}

#[tokio::test]
async fn digest_equivalence() {
    let fixture = Fixture::new("run_replay-equivalence");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let full = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap()
        .projection;
    let replay = store
        .recorded_replay(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(
        compare_projections(&fixture.projection_target(), &full, &replay).unwrap(),
        ProjectionComparison::Equal
    );
}

#[tokio::test]
async fn cross_store_not_comparable() {
    let left_fixture = Fixture::new("run_replay-cross-store");
    let right_fixture = Fixture::new("run_replay-cross-store");
    let left_store = left_fixture.open_created().await;
    let right_store = right_fixture.open_created().await;
    let left = left_store
        .recorded_replay(
            &left_fixture.projection_registry(),
            &left_fixture.projection_target(),
        )
        .await
        .unwrap();
    let right = right_store
        .recorded_replay(
            &right_fixture.projection_registry(),
            &right_fixture.projection_target(),
        )
        .await
        .unwrap();
    assert_ne!(left.source_store_id, right.source_store_id);
    assert_eq!(
        compare_projections(&left_fixture.projection_target(), &left, &right).unwrap(),
        ProjectionComparison::NotComparable
    );
}
