//! Typed error surface for the verifier.
//!
//! Every failure a caller can act on is a distinct variant, so the Node.js
//! layer can map them to stable API error codes instead of string-matching.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VerifyError {
    #[error("spec is invalid: {0}")]
    InvalidSpec(String),

    #[error("submission is invalid: {0}")]
    InvalidSubmission(String),

    #[error("job id mismatch: spec declares `{spec}` but submission targets `{submission}`")]
    JobIdMismatch { spec: String, submission: String },

    #[error("criterion `{id}` references JSON pointer `{pointer}` which is not present in the submission metadata")]
    MissingField { id: String, pointer: String },

    #[error("criterion `{id}` expected a numeric value at `{pointer}` but found a non-number")]
    NotANumber { id: String, pointer: String },

    #[error("malformed hex in criterion `{id}`: {reason}")]
    MalformedHex { id: String, reason: String },

    #[error("could not parse JSON: {0}")]
    Json(String),
}

/// `serde_json::Error` is not `PartialEq`, so we flatten it to a message to keep
/// `VerifyError` comparable in tests while preserving the original text.
impl From<serde_json::Error> for VerifyError {
    fn from(e: serde_json::Error) -> Self {
        VerifyError::Json(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VerifyError>;
