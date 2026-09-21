//! Verification bundle, version 2.
//!
//! A bundle is the single artifact handed from the prover to whoever submits
//! the proof on-chain. Version 2 carries what a RISC Zero Groth16 receipt
//! actually needs:
//!
//! - the **seal**: 4-byte selector plus the 256-byte Groth16 proof,
//! - the **journal**: the raw [`JournalV1`] bytes the guest committed,
//! - the **image id**: which guest program produced it,
//! - metadata: prover version, timings and cycle count.
//!
//! The public inputs are not stored separately: they are derived from the
//! journal, so the bundle cannot contradict itself.
//!
//! Version 1 bundles (`proof.data` plus a `public_inputs` struct) never carried
//! a real proof. They can still be read through [`AnyBundle`] so older files do
//! not become unreadable.

use serde::{Deserialize, Serialize};

use crate::journal::{JournalError, JournalV1};
use crate::proof::{PublicInputs, VerificationBundle};
use crate::risc0::SEAL_LEN;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

/// Bundle format version written by this crate.
pub const BUNDLE_VERSION: u16 = 2;

/// Magic prefix of the compact binary encoding.
pub const BUNDLE_MAGIC: [u8; 8] = *b"ZKMLBNDL";

/// Which proof system produced the seal.
///
/// The identifier travels with the bundle so a verifier can dispatch on it
/// instead of assuming one system forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ProofSystemId {
    /// A RISC Zero receipt compressed to Groth16 over BN254.
    Risc0Groth16 = 1,
}

impl ProofSystemId {
    /// Parse from the wire value.
    pub fn from_u16(value: u16) -> Result<Self, BundleError> {
        match value {
            1 => Ok(ProofSystemId::Risc0Groth16),
            other => Err(BundleError::UnknownProofSystem(other)),
        }
    }
}

/// Timings of one proving run, in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProveTimings {
    /// Proving the guest and compressing the receipt.
    ///
    /// `ProverOpts::groth16()` runs execution, segment proving, recursion and
    /// the Groth16 wrap inside one `prove_with_opts` call and does not report
    /// the stages separately, so this is one number rather than three. It is
    /// the honest granularity; splitting it would mean publishing zeros.
    pub prove_and_compress_ms: u64,
    /// Total wall time, including building the executor environment and the
    /// cross-check against native inference.
    pub total_ms: u64,
}

/// Provenance of a bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleMeta {
    /// Version of the crate that produced the bundle.
    pub prover_version: String,
    /// Unix timestamp in seconds, or zero when unknown.
    pub created_at: i64,
    /// Guest cycles executed, or zero when unknown.
    pub cycles: u64,
    /// Timings of the proving run.
    pub timings: ProveTimings,
}

/// Errors from bundle decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    /// The binary buffer is shorter than the field being read.
    Truncated,
    /// The binary magic prefix does not match.
    BadMagic,
    /// The version field is not supported.
    UnsupportedVersion(u16),
    /// The proof system identifier is not known.
    UnknownProofSystem(u16),
    /// The seal is not [`SEAL_LEN`] bytes.
    BadSealLength(usize),
    /// The journal could not be decoded.
    Journal(JournalError),
}

impl core::fmt::Display for BundleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BundleError::Truncated => write!(f, "bundle bytes are truncated"),
            BundleError::BadMagic => write!(f, "bundle magic is not ZKMLBNDL"),
            BundleError::UnsupportedVersion(v) => write!(f, "unsupported bundle version {v}"),
            BundleError::UnknownProofSystem(p) => write!(f, "unknown proof system {p}"),
            BundleError::BadSealLength(len) => {
                write!(f, "seal must be {SEAL_LEN} bytes, got {len}")
            }
            BundleError::Journal(e) => write!(f, "journal: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BundleError {}

impl From<JournalError> for BundleError {
    fn from(value: JournalError) -> Self {
        BundleError::Journal(value)
    }
}

/// A verification bundle carrying a real RISC Zero Groth16 receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationBundleV2 {
    /// Always [`BUNDLE_VERSION`].
    pub version: u16,
    /// Which proof system produced [`Self::seal`].
    pub proof_system: ProofSystemId,
    /// Guest image id, hex encoded in JSON.
    #[serde(with = "hex32")]
    pub image_id: [u8; 32],
    /// Selector plus Groth16 proof, hex encoded in JSON.
    #[serde(with = "hex_bytes")]
    pub seal: Vec<u8>,
    /// Raw journal bytes, hex encoded in JSON.
    #[serde(with = "hex_bytes")]
    pub journal: Vec<u8>,
    /// Provenance and timings.
    pub meta: BundleMeta,
}

impl VerificationBundleV2 {
    /// Build a bundle, checking the seal length and the journal layout.
    pub fn new(
        proof_system: ProofSystemId,
        image_id: [u8; 32],
        seal: Vec<u8>,
        journal: Vec<u8>,
        meta: BundleMeta,
    ) -> Result<Self, BundleError> {
        if seal.len() != SEAL_LEN {
            return Err(BundleError::BadSealLength(seal.len()));
        }
        JournalV1::decode(&journal)?;
        Ok(Self {
            version: BUNDLE_VERSION,
            proof_system,
            image_id,
            seal,
            journal,
            meta,
        })
    }

    /// Decode the journal this bundle carries.
    pub fn journal_v1(&self) -> Result<JournalV1, BundleError> {
        Ok(JournalV1::decode(&self.journal)?)
    }

    /// The public inputs derived from the journal.
    pub fn public_inputs(&self) -> Result<PublicInputs, BundleError> {
        let journal = self.journal_v1()?;
        Ok(PublicInputs {
            model_hash: journal.model_hash,
            input_hash: journal.input_hash,
            output: journal.output.to_le_bytes().to_vec(),
            class_label: journal.class_label,
        })
    }

    /// The 4-byte selector prefix of the seal.
    pub fn selector(&self) -> [u8; 4] {
        [self.seal[0], self.seal[1], self.seal[2], self.seal[3]]
    }

    /// The 256-byte Groth16 proof, without the selector.
    pub fn proof_bytes(&self) -> &[u8] {
        &self.seal[4..]
    }

    /// Compact binary encoding, for transports where JSON is wasteful.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            BUNDLE_MAGIC.len() + 4 + 32 + 4 + self.seal.len() + 4 + self.journal.len() + 64,
        );
        out.extend_from_slice(&BUNDLE_MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(self.proof_system as u16).to_le_bytes());
        out.extend_from_slice(&self.image_id);
        push_bytes(&mut out, &self.seal);
        push_bytes(&mut out, &self.journal);
        push_bytes(&mut out, self.meta.prover_version.as_bytes());
        out.extend_from_slice(&self.meta.created_at.to_le_bytes());
        out.extend_from_slice(&self.meta.cycles.to_le_bytes());
        out.extend_from_slice(&self.meta.timings.prove_and_compress_ms.to_le_bytes());
        out.extend_from_slice(&self.meta.timings.total_ms.to_le_bytes());
        out
    }

    /// Decode the compact binary encoding.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BundleError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(8)? != BUNDLE_MAGIC {
            return Err(BundleError::BadMagic);
        }
        let version = u16::from_le_bytes(as_array2(cursor.take(2)?));
        if version != BUNDLE_VERSION {
            return Err(BundleError::UnsupportedVersion(version));
        }
        let proof_system = ProofSystemId::from_u16(u16::from_le_bytes(as_array2(cursor.take(2)?)))?;
        let image_id = as_array32(cursor.take(32)?);
        let seal = cursor.take_prefixed()?.to_vec();
        let journal = cursor.take_prefixed()?.to_vec();
        let prover_version = String::from_utf8_lossy(cursor.take_prefixed()?).into_owned();
        let created_at = i64::from_le_bytes(as_array8(cursor.take(8)?));
        let cycles = u64::from_le_bytes(as_array8(cursor.take(8)?));
        let timings = ProveTimings {
            prove_and_compress_ms: u64::from_le_bytes(as_array8(cursor.take(8)?)),
            total_ms: u64::from_le_bytes(as_array8(cursor.take(8)?)),
        };

        Self::new(
            proof_system,
            image_id,
            seal,
            journal,
            BundleMeta {
                prover_version,
                created_at,
                cycles,
                timings,
            },
        )
    }
}

/// Either bundle format, so old files keep loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnyBundle {
    /// The current format.
    V2(VerificationBundleV2),
    /// The legacy format, which never carried a real proof.
    V1(VerificationBundle),
}

impl AnyBundle {
    /// `true` when the bundle uses the legacy format.
    pub fn is_legacy(&self) -> bool {
        matches!(self, AnyBundle::V1(_))
    }
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], BundleError> {
        let end = self.offset.checked_add(len).ok_or(BundleError::Truncated)?;
        if end > self.bytes.len() {
            return Err(BundleError::Truncated);
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn take_prefixed(&mut self) -> Result<&'a [u8], BundleError> {
        let len = u32::from_le_bytes(as_array4(self.take(4)?)) as usize;
        self.take(len)
    }
}

fn as_array2(bytes: &[u8]) -> [u8; 2] {
    let mut out = [0u8; 2];
    out.copy_from_slice(bytes);
    out
}

fn as_array4(bytes: &[u8]) -> [u8; 4] {
    let mut out = [0u8; 4];
    out.copy_from_slice(bytes);
    out
}

fn as_array8(bytes: &[u8]) -> [u8; 8] {
    let mut out = [0u8; 8];
    out.copy_from_slice(bytes);
    out
}

fn as_array32(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    out
}

/// Hex serialization for `Vec<u8>` fields.
pub mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec::Vec};

    /// Serialize as a lowercase hex string.
    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::to_hex(bytes))
    }

    /// Deserialize from a lowercase or uppercase hex string.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        super::from_hex(&text).ok_or_else(|| serde::de::Error::custom("invalid hex string"))
    }
}

/// Hex serialization for `[u8; 32]` fields.
pub mod hex32 {
    use serde::{Deserialize, Deserializer, Serializer};

    #[cfg(not(feature = "std"))]
    use alloc::string::String;

    /// Serialize as a 64-character hex string.
    pub fn serialize<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::to_hex(bytes))
    }

    /// Deserialize from a 64-character hex string.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes =
            super::from_hex(&text).ok_or_else(|| serde::de::Error::custom("invalid hex string"))?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("expected 32 bytes"));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Ok(out)
    }
}

/// Lowercase hex encoding.
pub fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Hex decoding; `None` on odd length or invalid characters.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(text.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)? as u8;
        let lo = (bytes[i + 1] as char).to_digit(16)? as u8;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::ModelKind;

    fn journal_bytes() -> Vec<u8> {
        JournalV1 {
            model_kind: ModelKind::LogisticRegression,
            model_hash: [1u8; 32],
            input_hash: [2u8; 32],
            output: 34_865,
            class_label: 1,
        }
        .encode()
        .to_vec()
    }

    fn sample() -> VerificationBundleV2 {
        let mut seal = vec![0u8; SEAL_LEN];
        seal[0..4].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        VerificationBundleV2::new(
            ProofSystemId::Risc0Groth16,
            [3u8; 32],
            seal,
            journal_bytes(),
            BundleMeta {
                prover_version: "0.0.1".into(),
                created_at: 1_700_000_000,
                cycles: 123_456,
                timings: ProveTimings {
                    prove_and_compress_ms: 20,
                    total_ms: 60,
                },
            },
        )
        .unwrap()
    }

    #[test]
    fn rejects_wrong_seal_length() {
        let err = VerificationBundleV2::new(
            ProofSystemId::Risc0Groth16,
            [0u8; 32],
            vec![0u8; 10],
            journal_bytes(),
            BundleMeta::default(),
        )
        .unwrap_err();
        assert_eq!(err, BundleError::BadSealLength(10));
    }

    #[test]
    fn rejects_bad_journal() {
        let err = VerificationBundleV2::new(
            ProofSystemId::Risc0Groth16,
            [0u8; 32],
            vec![0u8; SEAL_LEN],
            vec![0u8; 96],
            BundleMeta::default(),
        )
        .unwrap_err();
        assert!(matches!(err, BundleError::Journal(_)));
    }

    #[test]
    fn binary_round_trips() {
        let bundle = sample();
        let bytes = bundle.to_bytes();
        assert_eq!(&bytes[0..8], b"ZKMLBNDL");
        assert_eq!(VerificationBundleV2::from_bytes(&bytes).unwrap(), bundle);
    }

    #[test]
    fn binary_rejects_truncation() {
        let bytes = sample().to_bytes();
        assert_eq!(
            VerificationBundleV2::from_bytes(&bytes[..bytes.len() - 1]),
            Err(BundleError::Truncated)
        );
    }

    #[test]
    fn json_uses_hex_strings() {
        let bundle = sample();
        let json = serde_json::to_string(&bundle).unwrap();
        assert!(json.contains("\"image_id\":\"0303"));
        assert!(json.contains("\"seal\":\"deadbeef"));
        let back: VerificationBundleV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, bundle);
    }

    #[test]
    fn selector_and_proof_split() {
        let bundle = sample();
        assert_eq!(bundle.selector(), [0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(bundle.proof_bytes().len(), 256);
    }

    #[test]
    fn public_inputs_come_from_the_journal() {
        let bundle = sample();
        let pi = bundle.public_inputs().unwrap();
        assert_eq!(pi.model_hash, [1u8; 32]);
        assert_eq!(pi.class_label, 1);
        assert_eq!(pi.to_bytes().len(), 80);
    }

    #[test]
    fn any_bundle_reads_both_formats() {
        let v2_json = serde_json::to_string(&sample()).unwrap();
        assert!(!serde_json::from_str::<AnyBundle>(&v2_json)
            .unwrap()
            .is_legacy());

        let v1 = VerificationBundle {
            proof: crate::proof::Groth16Proof { data: Vec::new() },
            public_inputs: PublicInputs {
                model_hash: [1u8; 32],
                input_hash: [2u8; 32],
                output: vec![0u8; 8],
                class_label: 0,
            },
        };
        let v1_json = serde_json::to_string(&v1).unwrap();
        assert!(serde_json::from_str::<AnyBundle>(&v1_json)
            .unwrap()
            .is_legacy());
    }

    #[test]
    fn hex_helpers_round_trip() {
        let bytes = vec![0x00, 0x0f, 0xf0, 0xff];
        assert_eq!(to_hex(&bytes), "000ff0ff");
        assert_eq!(from_hex("000ff0ff").unwrap(), bytes);
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }
}
