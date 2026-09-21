//! Fixed-layout journal committed by the zkVM guest.
//!
//! The guest commits raw bytes instead of a serde-encoded struct so that the
//! on-chain verifier can parse the journal without a serde implementation and
//! so the layout is a stable wire-format contract.
//!
//! # Layout (96 bytes, little-endian)
//!
//! | Offset | Size | Field         | Notes                                   |
//! | ------ | ---- | ------------- | --------------------------------------- |
//! | 0      | 4    | `magic`       | ASCII `ZKML`                            |
//! | 4      | 2    | `version`     | `1`                                     |
//! | 6      | 1    | `model_kind`  | 0 tree, 1 logistic regression, 2 MLP    |
//! | 7      | 1    | `reserved0`   | zero                                    |
//! | 8      | 32   | `model_hash`  | Poseidon commitment over the parameters |
//! | 40     | 32   | `input_hash`  | Poseidon commitment over the inputs     |
//! | 72     | 8    | `output`      | `i64`, raw Q16.16 value                 |
//! | 80     | 8    | `class_label` | `i64`                                   |
//! | 88     | 8    | `reserved1`   | zero                                    |
//!
//! Any change to this layout is a breaking change for the guest, the prover and
//! the verifier contract at once.

use crate::models::Model;
use crate::Commitment;

/// Byte length of an encoded [`JournalV1`].
pub const JOURNAL_V1_LEN: usize = 96;

/// Magic prefix that identifies a zkml journal.
pub const JOURNAL_MAGIC: [u8; 4] = *b"ZKML";

/// Journal version encoded in the header.
pub const JOURNAL_VERSION: u16 = 1;

/// Model family committed in the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelKind {
    /// [`Model::DecisionTree`].
    DecisionTree = 0,
    /// [`Model::LogisticRegression`].
    LogisticRegression = 1,
    /// [`Model::TinyMLP`].
    TinyMlp = 2,
}

impl ModelKind {
    /// The kind of a model value.
    pub fn of(model: &Model) -> Self {
        match model {
            Model::DecisionTree(_) => ModelKind::DecisionTree,
            Model::LogisticRegression(_) => ModelKind::LogisticRegression,
            Model::TinyMLP(_) => ModelKind::TinyMlp,
        }
    }

    /// Parse a kind from its wire byte.
    pub fn from_byte(byte: u8) -> Result<Self, JournalError> {
        match byte {
            0 => Ok(ModelKind::DecisionTree),
            1 => Ok(ModelKind::LogisticRegression),
            2 => Ok(ModelKind::TinyMlp),
            other => Err(JournalError::UnknownModelKind(other)),
        }
    }
}

/// Errors returned while decoding a journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    /// The buffer is not exactly [`JOURNAL_V1_LEN`] bytes.
    InvalidLength(usize),
    /// The magic prefix does not match [`JOURNAL_MAGIC`].
    BadMagic,
    /// The version field is not supported by this build.
    UnsupportedVersion(u16),
    /// The model kind byte is not a known [`ModelKind`].
    UnknownModelKind(u8),
}

impl core::fmt::Display for JournalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            JournalError::InvalidLength(len) => {
                write!(f, "journal must be {JOURNAL_V1_LEN} bytes, got {len}")
            }
            JournalError::BadMagic => write!(f, "journal magic is not ZKML"),
            JournalError::UnsupportedVersion(v) => write!(f, "unsupported journal version {v}"),
            JournalError::UnknownModelKind(k) => write!(f, "unknown model kind {k}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JournalError {}

/// The public result of one inference, as committed by the guest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalV1 {
    /// Which model family produced the result.
    pub model_kind: ModelKind,
    /// Commitment to the model parameters.
    pub model_hash: Commitment,
    /// Commitment to the input features.
    pub input_hash: Commitment,
    /// Raw Q16.16 inference output.
    pub output: i64,
    /// Decision label: threshold class or argmax index.
    pub class_label: i64,
}

impl JournalV1 {
    /// Encode to the fixed 96-byte layout.
    pub fn encode(&self) -> [u8; JOURNAL_V1_LEN] {
        let mut out = [0u8; JOURNAL_V1_LEN];
        out[0..4].copy_from_slice(&JOURNAL_MAGIC);
        out[4..6].copy_from_slice(&JOURNAL_VERSION.to_le_bytes());
        out[6] = self.model_kind as u8;
        // out[7] stays zero (reserved)
        out[8..40].copy_from_slice(&self.model_hash);
        out[40..72].copy_from_slice(&self.input_hash);
        out[72..80].copy_from_slice(&self.output.to_le_bytes());
        out[80..88].copy_from_slice(&self.class_label.to_le_bytes());
        // out[88..96] stays zero (reserved)
        out
    }

    /// Decode from the fixed 96-byte layout.
    pub fn decode(bytes: &[u8]) -> Result<Self, JournalError> {
        if bytes.len() != JOURNAL_V1_LEN {
            return Err(JournalError::InvalidLength(bytes.len()));
        }
        if bytes[0..4] != JOURNAL_MAGIC {
            return Err(JournalError::BadMagic);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != JOURNAL_VERSION {
            return Err(JournalError::UnsupportedVersion(version));
        }
        let model_kind = ModelKind::from_byte(bytes[6])?;

        let mut model_hash = [0u8; 32];
        model_hash.copy_from_slice(&bytes[8..40]);
        let mut input_hash = [0u8; 32];
        input_hash.copy_from_slice(&bytes[40..72]);

        let mut output_bytes = [0u8; 8];
        output_bytes.copy_from_slice(&bytes[72..80]);
        let mut label_bytes = [0u8; 8];
        label_bytes.copy_from_slice(&bytes[80..88]);

        Ok(JournalV1 {
            model_kind,
            model_hash,
            input_hash,
            output: i64::from_le_bytes(output_bytes),
            class_label: i64::from_le_bytes(label_bytes),
        })
    }

    /// The 80-byte public input blob the verifier contract expects:
    /// `model_hash (32) || input_hash (32) || output (8) || class_label (8)`.
    pub fn public_inputs_bytes(&self) -> [u8; 80] {
        let mut out = [0u8; 80];
        out[0..32].copy_from_slice(&self.model_hash);
        out[32..64].copy_from_slice(&self.input_hash);
        out[64..72].copy_from_slice(&self.output.to_le_bytes());
        out[72..80].copy_from_slice(&self.class_label.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> JournalV1 {
        JournalV1 {
            model_kind: ModelKind::LogisticRegression,
            model_hash: [7u8; 32],
            input_hash: [9u8; 32],
            output: -34_865,
            class_label: 1,
        }
    }

    #[test]
    fn round_trips() {
        let journal = sample();
        let bytes = journal.encode();
        assert_eq!(bytes.len(), JOURNAL_V1_LEN);
        assert_eq!(JournalV1::decode(&bytes).unwrap(), journal);
    }

    #[test]
    fn header_is_stable() {
        let bytes = sample().encode();
        assert_eq!(&bytes[0..4], b"ZKML");
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1);
        assert_eq!(bytes[6], ModelKind::LogisticRegression as u8);
        assert_eq!(bytes[7], 0);
        assert_eq!(&bytes[88..96], &[0u8; 8]);
    }

    #[test]
    fn rejects_bad_length() {
        assert_eq!(
            JournalV1::decode(&[0u8; 10]),
            Err(JournalError::InvalidLength(10))
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample().encode();
        bytes[0] = b'X';
        assert_eq!(JournalV1::decode(&bytes), Err(JournalError::BadMagic));
    }

    #[test]
    fn rejects_unknown_version() {
        let mut bytes = sample().encode();
        bytes[4..6].copy_from_slice(&9u16.to_le_bytes());
        assert_eq!(
            JournalV1::decode(&bytes),
            Err(JournalError::UnsupportedVersion(9))
        );
    }

    #[test]
    fn rejects_unknown_model_kind() {
        let mut bytes = sample().encode();
        bytes[6] = 42;
        assert_eq!(
            JournalV1::decode(&bytes),
            Err(JournalError::UnknownModelKind(42))
        );
    }

    #[test]
    fn public_inputs_match_contract_layout() {
        let journal = sample();
        let pi = journal.public_inputs_bytes();
        assert_eq!(pi.len(), 80);
        assert_eq!(&pi[0..32], &journal.model_hash);
        assert_eq!(&pi[32..64], &journal.input_hash);
        assert_eq!(&pi[64..72], &journal.output.to_le_bytes());
        assert_eq!(&pi[72..80], &journal.class_label.to_le_bytes());
    }
}
