//! # zkml-common
//!
//! Shared types, data structures, and utilities used across the
//! `zkml-prover` and `zkml-verifier` crates.

pub mod error;
pub mod fixed_point;
pub mod models;
pub mod proof;

pub use error::ZkmlError;
pub mod activation;
pub mod commitment;
pub mod merkle;
pub mod tensor;
