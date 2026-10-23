//! Behavioural tests for the verifier. These double as executable docs for the
//! acceptance semantics the escrow relies on.

use proof_gated_escrow::hashing::{canonicalize, commitment};
use proof_gated_escrow::{verify, Artifact, JobSpec, Submission, VerifyError};
use serde_json::json;

fn spec_with(criteria: serde_json::Value) -> JobSpec {
    serde_json::from_value(json!({
        "id": "job-42",
        "title": "sample",
        "payout_wei": "1000000000000000000",
        "criteria": criteria
    }))
    .unwrap()
}

fn submission_with(metadata: serde_json::Value, artifacts: serde_json::Value) -> Submission {
    serde_json::from_value(json!({
        "job_id": "job-42",
        "artifacts": artifacts,
        "metadata": metadata
    }))
    .unwrap()
}

#[test]
fn all_criteria_met_passes() {
    let spec = spec_with(json!([
        { "kind": "numeric_at_least", "id": "res", "pointer": "/width", "min": 512.0 },
        { "kind": "field_equals", "id": "fmt", "pointer": "/format", "expected": "png" },
        { "kind": "string_contains", "id": "note", "pointer": "/log", "needle": "done" }
    ]));
    let sub = submission_with(
        json!({ "width": 1024, "format": "png", "log": "job done ok" }),
        json!([]),
    );
    let v = verify(&spec, &sub).unwrap();
    assert!(v.passed);
    assert_eq!(v.checks.len(), 3);
    assert!(v.checks.iter().all(|c| c.passed));
    assert_eq!(v.commitment.len(), 64);
}

#[test]
fn one_failed_criterion_fails_overall_but_is_not_an_error() {
    let spec = spec_with(json!([
        { "kind": "numeric_at_least", "id": "res", "pointer": "/width", "min": 512.0 }
    ]));
    let sub = submission_with(json!({ "width": 128 }), json!([]));
    let v = verify(&spec, &sub).unwrap();
    assert!(!v.passed);
    assert!(!v.checks[0].passed);
}

#[test]
fn artifact_hash_match_and_mismatch() {
    let hash = "a".repeat(64);
    let spec = spec_with(json!([
        { "kind": "artifact_sha256", "id": "bin", "path": "out/app", "expected": hash }
    ]));

    let ok = submission_with(
        json!({}),
        json!([{ "path": "out/app", "sha256": "a".repeat(64) }]),
    );
    assert!(verify(&spec, &ok).unwrap().passed);

    let bad = submission_with(
        json!({}),
        json!([{ "path": "out/app", "sha256": "b".repeat(64) }]),
    );
    assert!(!verify(&spec, &bad).unwrap().passed);
}

#[test]
fn missing_pointer_is_a_structural_error() {
    let spec = spec_with(json!([
        { "kind": "numeric_at_least", "id": "res", "pointer": "/width", "min": 512.0 }
    ]));
    let sub = submission_with(json!({ "height": 100 }), json!([]));
    let err = verify(&spec, &sub).unwrap_err();
    assert!(matches!(err, VerifyError::MissingField { .. }));
}

#[test]
fn job_id_mismatch_rejected() {
    let spec = spec_with(json!([
        { "kind": "field_equals", "id": "f", "pointer": "/x", "expected": 1 }
    ]));
    let mut sub = submission_with(json!({ "x": 1 }), json!([]));
    sub.job_id = "other".into();
    assert!(matches!(
        verify(&spec, &sub).unwrap_err(),
        VerifyError::JobIdMismatch { .. }
    ));
}

#[test]
fn empty_criteria_spec_rejected() {
    let spec = spec_with(json!([]));
    let sub = submission_with(json!({}), json!([]));
    assert!(matches!(
        verify(&spec, &sub).unwrap_err(),
        VerifyError::InvalidSpec(_)
    ));
}

#[test]
fn non_integer_payout_rejected() {
    let mut spec = spec_with(json!([
        { "kind": "field_equals", "id": "f", "pointer": "/x", "expected": 1 }
    ]));
    spec.payout_wei = "1.5".into();
    let sub = submission_with(json!({ "x": 1 }), json!([]));
    assert!(matches!(
        verify(&spec, &sub).unwrap_err(),
        VerifyError::InvalidSpec(_)
    ));
}

#[test]
fn uppercase_expected_hash_rejected_early() {
    let spec = spec_with(json!([
        { "kind": "artifact_sha256", "id": "bin", "path": "p", "expected": "A".repeat(64) }
    ]));
    let sub = submission_with(json!({}), json!([]));
    assert!(matches!(
        verify(&spec, &sub).unwrap_err(),
        VerifyError::MalformedHex { .. }
    ));
}

#[test]
fn duplicate_criterion_id_rejected() {
    let spec = spec_with(json!([
        { "kind": "field_equals", "id": "dup", "pointer": "/x", "expected": 1 },
        { "kind": "field_equals", "id": "dup", "pointer": "/y", "expected": 2 }
    ]));
    let sub = submission_with(json!({ "x": 1, "y": 2 }), json!([]));
    assert!(matches!(
        verify(&spec, &sub).unwrap_err(),
        VerifyError::InvalidSpec(_)
    ));
}

#[test]
fn commitment_is_deterministic_and_order_independent() {
    let spec = spec_with(json!([
        { "kind": "field_equals", "id": "f", "pointer": "/a", "expected": 1 }
    ]));
    // Same logical submission, keys in different source order.
    let sub_a: Submission = serde_json::from_value(json!({
        "job_id": "job-42", "artifacts": [], "metadata": { "a": 1, "b": 2 }
    }))
    .unwrap();
    let sub_b: Submission = serde_json::from_value(json!({
        "job_id": "job-42", "artifacts": [], "metadata": { "b": 2, "a": 1 }
    }))
    .unwrap();

    let ca = verify(&spec, &sub_a).unwrap().commitment;
    let cb = verify(&spec, &sub_b).unwrap().commitment;
    assert_eq!(ca, cb, "key order must not change the commitment");
}

#[test]
fn canonicalize_sorts_nested_keys() {
    let v = json!({ "z": { "b": 1, "a": 2 }, "a": [3, 2, 1] });
    let c = canonicalize(&v);
    let s = serde_json::to_string(&c).unwrap();
    // object keys sorted; array order preserved
    assert_eq!(s, r#"{"a":[3,2,1],"z":{"a":2,"b":1}}"#);
}

#[test]
fn commitment_changes_when_payout_changes() {
    let base = spec_with(json!([
        { "kind": "field_equals", "id": "f", "pointer": "/x", "expected": 1 }
    ]));
    let mut hi = base.clone();
    hi.payout_wei = "2000000000000000000".into();
    let sub = submission_with(json!({ "x": 1 }), json!([]));

    let c1 = commitment(&base, &sub).unwrap();
    let c2 = commitment(&hi, &sub).unwrap();
    assert_ne!(c1, c2);
}

#[test]
fn artifacts_field_order_differences_do_not_matter_for_pass() {
    let hash = "c".repeat(64);
    let spec = spec_with(json!([
        { "kind": "artifact_sha256", "id": "a1", "path": "x", "expected": hash }
    ]));
    let sub = submission_with(json!({}), json!([{ "sha256": "c".repeat(64), "path": "x" }]));
    assert!(verify(&spec, &sub).unwrap().passed);
    let _ = Artifact { path: "x".into(), sha256: "c".repeat(64) }; // type is public API
}
