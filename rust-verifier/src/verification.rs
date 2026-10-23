//! The verification engine: pure, deterministic, side-effect free.

use crate::error::{Result, VerifyError};
use crate::hashing::commitment;
use crate::spec::{CheckResult, Criterion, JobSpec, Submission, Verdict};

/// Validate a spec in isolation before it is ever matched against a submission.
pub fn validate_spec(spec: &JobSpec) -> Result<()> {
    if spec.id.trim().is_empty() {
        return Err(VerifyError::InvalidSpec("job id is empty".into()));
    }
    if spec.criteria.is_empty() {
        return Err(VerifyError::InvalidSpec(
            "a job must declare at least one acceptance criterion".into(),
        ));
    }
    // payout is wei: a non-negative integer with no sign, decimal point or spaces.
    if spec.payout_wei.is_empty() || !spec.payout_wei.bytes().all(|b| b.is_ascii_digit()) {
        return Err(VerifyError::InvalidSpec(format!(
            "payout_wei must be a decimal integer string, got `{}`",
            spec.payout_wei
        )));
    }
    // Criterion ids must be unique so check results map back unambiguously.
    let mut seen = std::collections::BTreeSet::new();
    for c in &spec.criteria {
        if c.id().trim().is_empty() {
            return Err(VerifyError::InvalidSpec("a criterion has an empty id".into()));
        }
        if !seen.insert(c.id()) {
            return Err(VerifyError::InvalidSpec(format!(
                "duplicate criterion id `{}`",
                c.id()
            )));
        }
        // Expected hashes must be valid 32-byte lowercase hex up front.
        if let Criterion::ArtifactSha256 { id, expected, .. } = c {
            validate_sha256_hex(id, expected)?;
        }
    }
    Ok(())
}

fn validate_sha256_hex(id: &str, s: &str) -> Result<()> {
    if s.len() != 64 {
        return Err(VerifyError::MalformedHex {
            id: id.to_string(),
            reason: format!("expected 64 hex chars, got {}", s.len()),
        });
    }
    if !s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
        return Err(VerifyError::MalformedHex {
            id: id.to_string(),
            reason: "expected lowercase hex only".into(),
        });
    }
    Ok(())
}

fn validate_submission(sub: &Submission) -> Result<()> {
    if sub.job_id.trim().is_empty() {
        return Err(VerifyError::InvalidSubmission("job id is empty".into()));
    }
    for a in &sub.artifacts {
        if a.path.trim().is_empty() {
            return Err(VerifyError::InvalidSubmission("an artifact has an empty path".into()));
        }
    }
    Ok(())
}

fn number_at(sub: &Submission, id: &str, pointer: &str) -> Result<f64> {
    let v = sub
        .metadata
        .pointer(pointer)
        .ok_or_else(|| VerifyError::MissingField {
            id: id.to_string(),
            pointer: pointer.to_string(),
        })?;
    v.as_f64().ok_or_else(|| VerifyError::NotANumber {
        id: id.to_string(),
        pointer: pointer.to_string(),
    })
}

/// Evaluate one criterion. A `false` outcome is a *failed check*, not an error;
/// an `Err` means the submission was structurally unusable for that criterion.
fn evaluate(sub: &Submission, c: &Criterion) -> Result<CheckResult> {
    let id = c.id().to_string();
    match c {
        Criterion::ArtifactSha256 { path, expected, .. } => {
            match sub.artifacts.iter().find(|a| &a.path == path) {
                Some(a) if &a.sha256 == expected => Ok(CheckResult {
                    id,
                    passed: true,
                    detail: format!("artifact `{path}` matches expected hash"),
                }),
                Some(a) => Ok(CheckResult {
                    id,
                    passed: false,
                    detail: format!("artifact `{path}` hash {} != expected {expected}", a.sha256),
                }),
                None => Ok(CheckResult {
                    id,
                    passed: false,
                    detail: format!("no artifact submitted for path `{path}`"),
                }),
            }
        }
        Criterion::FieldEquals { pointer, expected, .. } => {
            let actual = sub.metadata.pointer(pointer).ok_or_else(|| {
                VerifyError::MissingField { id: id.clone(), pointer: pointer.clone() }
            })?;
            let passed = actual == expected;
            Ok(CheckResult {
                id,
                passed,
                detail: if passed {
                    format!("`{pointer}` equals expected value")
                } else {
                    format!("`{pointer}` is {actual}, expected {expected}")
                },
            })
        }
        Criterion::NumericAtLeast { pointer, min, .. } => {
            let actual = number_at(sub, &id, pointer)?;
            let passed = actual >= *min;
            Ok(CheckResult {
                id,
                passed,
                detail: format!("`{pointer}` = {actual}, required >= {min}"),
            })
        }
        Criterion::NumericAtMost { pointer, max, .. } => {
            let actual = number_at(sub, &id, pointer)?;
            let passed = actual <= *max;
            Ok(CheckResult {
                id,
                passed,
                detail: format!("`{pointer}` = {actual}, required <= {max}"),
            })
        }
        Criterion::StringContains { pointer, needle, .. } => {
            let v = sub.metadata.pointer(pointer).ok_or_else(|| {
                VerifyError::MissingField { id: id.clone(), pointer: pointer.clone() }
            })?;
            let hay = v.as_str().unwrap_or("");
            let passed = hay.contains(needle);
            Ok(CheckResult {
                id,
                passed,
                detail: if passed {
                    format!("`{pointer}` contains `{needle}`")
                } else {
                    format!("`{pointer}` does not contain `{needle}`")
                },
            })
        }
    }
}

/// Verify a submission against a spec and produce a signable verdict.
///
/// This is the single public entry point. It is deterministic: for identical
/// `spec` and `submission` inputs it always returns an identical `Verdict`,
/// including the `commitment` hash.
pub fn verify(spec: &JobSpec, submission: &Submission) -> Result<Verdict> {
    validate_spec(spec)?;
    validate_submission(submission)?;

    if spec.id != submission.job_id {
        return Err(VerifyError::JobIdMismatch {
            spec: spec.id.clone(),
            submission: submission.job_id.clone(),
        });
    }

    let mut checks = Vec::with_capacity(spec.criteria.len());
    for c in &spec.criteria {
        checks.push(evaluate(submission, c)?);
    }
    let passed = checks.iter().all(|c| c.passed);
    let commitment = commitment(spec, submission)?;

    Ok(Verdict {
        job_id: spec.id.clone(),
        passed,
        checks,
        commitment,
    })
}

/// Convenience: verify from JSON strings (used by the CLI and FFI boundaries).
pub fn verify_json(spec_json: &str, submission_json: &str) -> Result<Verdict> {
    let spec: JobSpec = serde_json::from_str(spec_json)?;
    let submission: Submission = serde_json::from_str(submission_json)?;
    verify(&spec, &submission)
}
