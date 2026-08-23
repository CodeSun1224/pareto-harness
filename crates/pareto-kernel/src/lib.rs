#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Trusted mechanisms for authoritative Pareto Harness state.
//!
//! Authority-bearing Event Store operations are intentionally not public.
//!
//! ```compile_fail
//! use pareto_kernel::event_store::{AdmittedAppend, AdmittedRead, EventStore};
//! ```

// The authority-bearing API remains crate-private until a later Kernel capability requirement
// supplies its public caller. The complete vertical slice is exercised by in-crate tests.
#[allow(dead_code)]
mod event_store;
