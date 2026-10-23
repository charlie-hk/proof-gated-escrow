//! Canonicalization and hashing.
//!
//! Two JSON documents that differ only in key order or insignificant whitespace
//! are the *same* document for settlement purposes, so they must hash
//! identically. We canonicalize by recursively sorting object keys and
//! re-serializing with no superfluous whitespace, then SHA-256 the bytes.

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Recursively rewrite a `Value` into canonical form: every object's keys are
/// sorted lexicographically; arrays keep their order (order is significant).
pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), canonicalize(&map[k]));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// Canonical UTF-8 bytes of any serializable value.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let as_value = serde_json::to_value(value)?;
    let canonical = canonicalize(&as_value);
    // `to_string` on a Value emits compact, key-stable JSON here.
    Ok(serde_json::to_string(&canonical)?.into_bytes())
}

/// Lowercase-hex SHA-256 of a byte slice.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Commitment binding a spec to a submission: sha256(canon(spec) ++ 0x1f ++ canon(submission)).
/// The 0x1f unit separator stops a crafted spec/submission pair from colliding
/// with a different split of the same concatenated bytes.
pub fn commitment<S: Serialize, U: Serialize>(
    spec: &S,
    submission: &U,
) -> Result<String, serde_json::Error> {
    let mut buf = canonical_bytes(spec)?;
    buf.push(0x1f);
    buf.extend_from_slice(&canonical_bytes(submission)?);
    Ok(sha256_hex(&buf))
}
