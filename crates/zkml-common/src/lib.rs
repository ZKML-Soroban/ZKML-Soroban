//! # zkml-common
//!
//! Shared types, data structures, and utilities used across the
//! `zkml-prover` and `zkml-verifier` crates.
//!
//! This crate deliberately depends on neither Soroban nor RISC Zero so the
//! same code compiles for native host tests and for the zkVM guest.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod activation;
pub mod bundle;
#[cfg(feature = "poseidon")]
pub mod commitment;
pub mod error;
pub mod fixed_point;
pub mod inference;
pub mod journal;
#[cfg(feature = "poseidon")]
pub mod merkle;
pub mod models;
pub mod proof;
pub mod risc0;
pub mod tensor;

/// A 32-byte commitment.
///
/// Defined here rather than in [`commitment`] because the journal and the
/// verifier need the type without the Poseidon implementation, which pulls in
/// arkworks and does not build for `wasm32v1-none`.
pub type Commitment = [u8; 32];

pub use error::ZkmlError;
