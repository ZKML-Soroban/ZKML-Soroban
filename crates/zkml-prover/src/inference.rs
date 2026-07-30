//! Model inference engine.
//!
//! Re-exports the shared implementation from `zkml-common` so existing
//! `zkml_prover::inference` call sites keep working. The proven guest path
//! uses the same `zkml_common::inference` module directly.

pub use zkml_common::inference::*;
