//! Domain types for jobs, submissions and verdicts.
//!
//! The core library never touches the filesystem or the network. A submission
//! carries the artifact hashes and a free-form `metadata` object that the
//! provider computed off-box; the verifier only checks that those values
//! satisfy the acceptance criteria declared in the spec. This keeps
//! verification pure and therefore deterministic and reproducible on any host.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single, machine-checkable acceptance condition.
// No `Eq`: numeric bounds are `f64`, which is only `PartialEq`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Criterion {
    /// An artifact at `path` must have exactly this SHA-256 (lowercase hex).
    ArtifactSha256 { id: String, path: String, expected: String },
    /// The value at `pointer` (RFC-6901 JSON pointer) must equal `expected`.
    FieldEquals { id: String, pointer: String, expected: Value },
    /// The numeric value at `pointer` must be >= `min`.
    NumericAtLeast { id: String, pointer: String, min: f64 },
    /// The numeric value at `pointer` must be <= `max`.
    NumericAtMost { id: String, pointer: String, max: f64 },
    /// The string value at `pointer` must contain `needle`.
    StringContains { id: String, pointer: String, needle: String },
}

impl Criterion {
    pub fn id(&self) -> &str {
        match self {
            Criterion::ArtifactSha256 { id, .. }
            | Criterion::FieldEquals { id, .. }
            | Criterion::NumericAtLeast { id, .. }
            | Criterion::NumericAtMost { id, .. }
            | Criterion::StringContains { id, .. } => id,
        }
    }
}

/// A unit of work with its payout and the conditions under which it is met.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobSpec {
    pub id: String,
    pub title: String,
    /// Payout held in escrow, in wei, as a decimal string to avoid f64 loss.
    pub payout_wei: String,
    pub criteria: Vec<Criterion>,
}

/// An artifact the provider submitted, identified by its content hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifact {
    pub path: String,
    /// SHA-256 of the artifact bytes, lowercase hex.
    pub sha256: String,
}

/// What a provider submits for a job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Submission {
    pub job_id: String,
    pub artifacts: Vec<Artifact>,
    /// Arbitrary provider-reported results that criteria can point into.
    #[serde(default)]
    pub metadata: Value,
}

/// Outcome of one criterion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckResult {
    pub id: String,
    pub passed: bool,
    pub detail: String,
}

/// The full, signable result of verifying a submission against a spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verdict {
    pub job_id: String,
    pub passed: bool,
    pub checks: Vec<CheckResult>,
    /// SHA-256 over canonical(spec) ++ canonical(submission), lowercase hex.
    /// The escrow contract releases funds against this commitment, so the same
    /// inputs always yield the same on-chain reference.
    pub commitment: String,
}
