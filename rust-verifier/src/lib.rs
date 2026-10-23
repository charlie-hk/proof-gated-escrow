//! Proof-Gated Escrow verifier.
//!
//! A pure, deterministic engine that decides whether a provider's submission
//! satisfies the acceptance criteria of a job, and produces a `commitment`
//! hash that the Solidity escrow releases funds against.
//!
//! Design rules that make settlement trustworthy:
//! - **No side effects.** The library never reads files or the network, so the
//!   same inputs verify identically on any machine and at any time.
//! - **Canonical hashing.** Spec and submission are canonicalized (sorted keys,
//!   compact form) before hashing, so logically-equal inputs commit equally.
//! - **Typed failures.** Structural problems are `VerifyError`s; a criterion
//!   that simply isn't met is a failed check inside an `Ok(Verdict)`.
//!
//! ```
//! use proof_gated_escrow::{verify, JobSpec, Submission};
//!
//! let spec: JobSpec = serde_json::from_str(r#"{
//!   "id": "job-1", "title": "resize logo", "payout_wei": "1000000000000000000",
//!   "criteria": [{ "kind": "numeric_at_least", "id": "c1", "pointer": "/width", "min": 512.0 }]
//! }"#).unwrap();
//! let sub: Submission = serde_json::from_str(r#"{
//!   "job_id": "job-1", "artifacts": [], "metadata": { "width": 1024 }
//! }"#).unwrap();
//!
//! let verdict = verify(&spec, &sub).unwrap();
//! assert!(verdict.passed);
//! assert_eq!(verdict.commitment.len(), 64);
//! ```

pub mod error;
pub mod hashing;
pub mod spec;
pub mod verification;

pub use error::{Result, VerifyError};
pub use spec::{Artifact, CheckResult, Criterion, JobSpec, Submission, Verdict};
pub use verification::{validate_spec, verify, verify_json};
