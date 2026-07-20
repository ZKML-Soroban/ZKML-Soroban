//! # zkml-common
//!
//! Shared types, data structures, and utilities used across the
//! `zkml-prover` and `zkml-verifier` crates.
//!
//! This crate deliberately depends on neither Soroban nor RISC Zero so the
//! same code compiles for native host tests and for the zkVM guest.

pub mod activation;
pub mod commitment;
pub mod error;
pub mod fixed_point;
pub mod inference;
pub mod merkle;
pub mod models;
pub mod proof;
pub mod tensor;

pub use error::ZkmlError;
