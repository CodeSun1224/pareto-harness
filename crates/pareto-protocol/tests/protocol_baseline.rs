use std::time::Instant;

use pareto_protocol::{
    ProtocolLimitsProfileV1, ProtocolLimitsV1, SchemaSet, canonical_json_bytes, digest_json,
    generate_schema_bundle, parse_bounded,
};
use serde_json::json;

#[test]
#[ignore = "reproducible observation baseline; no pass/fail performance threshold"]
fn protocol_operation_latency_baseline() {
    const ITERATIONS: u32 = 1_000;
    let bundle = generate_schema_bundle().unwrap();
    let set =
        SchemaSet::bootstrap_initial(bundle.manifest, bundle.schemas, &bundle.reference).unwrap();
    let value = serde_json::to_value(ProtocolLimitsV1::profile()).unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    let schema_ref = set.reference().manifest_schema_ref.clone();

    measure("parse", ITERATIONS, || {
        parse_bounded::<ProtocolLimitsProfileV1>(&bytes).unwrap();
    });
    measure("schema_validate", ITERATIONS, || {
        set.parse_record::<ProtocolLimitsProfileV1>(&bytes).unwrap();
    });
    measure("canonicalize", ITERATIONS, || {
        canonical_json_bytes(&value).unwrap();
    });
    measure("digest", ITERATIONS, || {
        digest_json("baseline", &schema_ref, &json!({"limits":value})).unwrap();
    });
    measure("schema_generation", 100, || {
        generate_schema_bundle().unwrap();
    });
}

fn measure(name: &str, iterations: u32, mut operation: impl FnMut()) {
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    let elapsed = started.elapsed();
    println!(
        "{name}: iterations={iterations}, total_ns={}",
        elapsed.as_nanos()
    );
}
