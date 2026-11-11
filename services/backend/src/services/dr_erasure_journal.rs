//! Remediation E1: Rust-native reader/writer for the externally-custodied,
//! HMAC-chained erasure journal. This intentionally uses the exact wire
//! format `ops/dr/erasure_journal.py` defines (append-only JSON Lines, a
//! separate `.anchor` checkpoint file, HMAC-SHA256 over canonical JSON with
//! sorted keys and `(",", ":")` separators) so an operator's Python tooling
//! during a real incident can read/verify a journal this binary wrote, and
//! vice versa.
//!
//! This module intentionally never stores raw clinical data or unnecessary
//! PII — only a pseudonymous `subject_ref`.
//!
//! LIMITATION (see Remediation E1 report): this module only writes to a
//! filesystem path. A local file is not an independently-protected store —
//! restoring the same disk/volume as Postgres rolls it back too. Production
//! MUST point `EXTERNAL_ERASURE_JOURNAL_PATH` at a location outside any
//! Postgres backup domain (today, a separately-backed-up volume; the
//! intended end state is Azure immutable Blob Storage with its own
//! retention policy). This code does not implement Azure immutability and
//! must never be treated as doing so.

use crate::crypto::constant_time_eq;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub event_id: String,
    pub subject_ref: String,
    pub state: String,
    pub retention_class: String,
    pub sequence: i64,
    pub recorded_at: String,
    pub previous_mac: String,
    pub key_version: String,
    pub mac: String,
}

#[derive(Debug)]
pub enum JournalError {
    Io(String),
    Integrity(&'static str),
    Malformed(&'static str),
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournalError::Io(e) => write!(f, "erasure journal io error: {e}"),
            JournalError::Integrity(m) => write!(f, "erasure journal integrity error: {m}"),
            JournalError::Malformed(m) => write!(f, "erasure journal malformed: {m}"),
        }
    }
}
impl std::error::Error for JournalError {}
impl From<std::io::Error> for JournalError {
    fn from(e: std::io::Error) -> Self {
        JournalError::Io(e.to_string())
    }
}

/// Mirrors `ops/dr/erasure_journal.py::pseudonymous_subject_ref` exactly:
/// `"sha256:" + sha256(namespace + ":" + stable_identity)`.
pub fn pseudonymous_subject_ref(stable_identity: &str, namespace: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b":");
    hasher.update(stable_identity.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Matches Python's `json.dumps` escaping for the ASCII-only, control-
/// character-free content this module ever produces (UUIDs, hex digests,
/// ISO-8601 timestamps, fixed literal states) — serde_json's default string
/// escaping is byte-identical to Python's for such input. This is a
/// documented assumption, not a general-purpose JSON-interop guarantee.
fn json_escape(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

#[allow(clippy::too_many_arguments)]
fn canonical_entry_payload(
    event_id: &str,
    subject_ref: &str,
    state: &str,
    retention_class: &str,
    sequence: i64,
    recorded_at: &str,
    previous_mac: &str,
    key_version: &str,
) -> String {
    // Keys alphabetical, matching Python's `sort_keys=True`.
    let mut out = String::new();
    let _ = write!(out, "{{\"event_id\":{},", json_escape(event_id));
    let _ = write!(out, "\"key_version\":{},", json_escape(key_version));
    let _ = write!(out, "\"previous_mac\":{},", json_escape(previous_mac));
    let _ = write!(out, "\"recorded_at\":{},", json_escape(recorded_at));
    let _ = write!(out, "\"retention_class\":{},", json_escape(retention_class));
    let _ = write!(out, "\"sequence\":{sequence},");
    let _ = write!(out, "\"state\":{},", json_escape(state));
    let _ = write!(out, "\"subject_ref\":{}}}", json_escape(subject_ref));
    out
}

fn canonical_anchor_payload(key_version: &str, sequence: i64, last_mac: &str) -> String {
    let mut out = String::new();
    let _ = write!(out, "{{\"key_version\":{},", json_escape(key_version));
    let _ = write!(out, "\"last_mac\":{},", json_escape(last_mac));
    let _ = write!(out, "\"sequence\":{sequence}}}");
    out
}

fn hmac_hex(key: &[u8], message: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    hex::encode(mac.finalize().into_bytes())
}

struct AnchorFile {
    sequence: i64,
    last_mac: String,
}

fn anchor_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".anchor");
    path.with_file_name(name)
}

fn read_anchor(
    path: &Path,
    anchor_key: &[u8],
    key_version: &str,
) -> Result<AnchorFile, JournalError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|_| JournalError::Malformed("erasure journal checkpoint is missing"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|_| JournalError::Malformed("erasure journal checkpoint is invalid"))?;
    let obj = value.as_object().ok_or(JournalError::Malformed(
        "erasure journal checkpoint is invalid",
    ))?;
    let got_key_version =
        obj.get("key_version")
            .and_then(|v| v.as_str())
            .ok_or(JournalError::Malformed(
                "erasure journal checkpoint is invalid",
            ))?;
    let sequence = obj
        .get("sequence")
        .and_then(|v| v.as_i64())
        .ok_or(JournalError::Malformed(
            "erasure journal checkpoint is invalid",
        ))?;
    let last_mac = obj
        .get("last_mac")
        .and_then(|v| v.as_str())
        .ok_or(JournalError::Malformed(
            "erasure journal checkpoint is invalid",
        ))?;
    let mac = obj
        .get("mac")
        .and_then(|v| v.as_str())
        .ok_or(JournalError::Malformed(
            "erasure journal checkpoint is invalid",
        ))?;
    if got_key_version != key_version {
        return Err(JournalError::Integrity(
            "erasure journal checkpoint integrity validation failed",
        ));
    }
    let expected = hmac_hex(
        anchor_key,
        canonical_anchor_payload(got_key_version, sequence, last_mac).as_bytes(),
    );
    if !constant_time_eq(&expected, mac) {
        return Err(JournalError::Integrity(
            "erasure journal checkpoint integrity validation failed",
        ));
    }
    Ok(AnchorFile {
        sequence,
        last_mac: last_mac.to_string(),
    })
}

fn write_anchor(
    path: &Path,
    anchor_key: &[u8],
    key_version: &str,
    sequence: i64,
    last_mac: &str,
) -> Result<(), JournalError> {
    let payload = canonical_anchor_payload(key_version, sequence, last_mac);
    let mac = hmac_hex(anchor_key, payload.as_bytes());
    let row = format!(
        "{{\"key_version\":{},\"last_mac\":{},\"mac\":{},\"sequence\":{sequence}}}\n",
        json_escape(key_version),
        json_escape(last_mac),
        json_escape(&mac),
    );
    let tmp_name = format!(
        "{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, row)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Read and fully verify every entry, matching
/// `ops/dr/erasure_journal.py::read_verified` exactly: sequence continuity,
/// `previous_mac` chaining, per-entry HMAC, and the anchor-vs-tail check.
pub fn read_verified(
    path: &Path,
    hmac_key: &[u8],
    anchor_key: &[u8],
    key_version: &str,
) -> Result<Vec<JournalEntry>, JournalError> {
    let anchor_file = anchor_path(path);
    if !path.exists() {
        if anchor_file.exists() {
            return Err(JournalError::Integrity(
                "erasure journal data is missing while checkpoint remains",
            ));
        }
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut entries = Vec::new();
    let mut previous_mac = "0".repeat(64);
    let mut expected_sequence: i64 = 0;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        expected_sequence += 1;
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|_| JournalError::Malformed("erasure journal entry is not valid JSON"))?;
        let obj = value.as_object().ok_or(JournalError::Malformed(
            "erasure journal entry is not an object",
        ))?;
        let get_str = |k: &str| -> Result<String, JournalError> {
            obj.get(k)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or(JournalError::Malformed(
                    "erasure journal entry is missing a required field",
                ))
        };
        let event_id = get_str("event_id")?;
        let subject_ref = get_str("subject_ref")?;
        let state = get_str("state")?;
        let retention_class = get_str("retention_class")?;
        let recorded_at = get_str("recorded_at")?;
        let previous_mac_field = get_str("previous_mac")?;
        let key_version_field = get_str("key_version")?;
        let mac = get_str("mac")?;
        let sequence =
            obj.get("sequence")
                .and_then(|v| v.as_i64())
                .ok_or(JournalError::Malformed(
                    "erasure journal entry sequence is invalid",
                ))?;
        let payload = canonical_entry_payload(
            &event_id,
            &subject_ref,
            &state,
            &retention_class,
            sequence,
            &recorded_at,
            &previous_mac_field,
            &key_version_field,
        );
        let expected_mac = hmac_hex(hmac_key, payload.as_bytes());
        if sequence != expected_sequence
            || previous_mac_field != previous_mac
            || key_version_field != key_version
            || !constant_time_eq(&expected_mac, &mac)
        {
            return Err(JournalError::Integrity(
                "erasure journal integrity or sequence validation failed",
            ));
        }
        previous_mac = mac.clone();
        entries.push(JournalEntry {
            event_id,
            subject_ref,
            state,
            retention_class,
            sequence,
            recorded_at,
            previous_mac: previous_mac_field,
            key_version: key_version_field,
            mac,
        });
    }
    let anchor = read_anchor(&anchor_file, anchor_key, key_version)?;
    if anchor.sequence != entries.len() as i64 || anchor.last_mac != previous_mac {
        return Err(JournalError::Integrity(
            "erasure journal checkpoint does not match journal tail",
        ));
    }
    Ok(entries)
}

/// Append one entry, idempotent on `event_id`: if an entry with this
/// `event_id` already exists and matches `subject_ref`/`state`, this is a
/// no-op returning the existing entry rather than appending a duplicate or
/// erroring. This lets the outbox-sync caller safely retry the same event
/// after a crash between the file write and marking the outbox row synced.
#[allow(clippy::too_many_arguments)]
pub fn append_idempotent(
    path: &Path,
    hmac_key: &[u8],
    anchor_key: &[u8],
    key_version: &str,
    event_id: &str,
    subject_ref: &str,
    state: &str,
    retention_class: &str,
) -> Result<JournalEntry, JournalError> {
    if !subject_ref.starts_with("sha256:") || subject_ref.len() != 71 {
        return Err(JournalError::Malformed(
            "subject_ref must be a pseudonymous sha256 reference",
        ));
    }
    let existing = read_verified(path, hmac_key, anchor_key, key_version)?;
    if let Some(found) = existing.iter().find(|e| e.event_id == event_id) {
        if found.subject_ref != subject_ref || found.state != state {
            return Err(JournalError::Integrity(
                "event_id reused with different content",
            ));
        }
        return Ok(found.clone());
    }
    let sequence = existing.len() as i64 + 1;
    let previous_mac = existing
        .last()
        .map(|e| e.mac.clone())
        .unwrap_or_else(|| "0".repeat(64));
    let recorded_at = chrono::Utc::now().to_rfc3339();
    let payload = canonical_entry_payload(
        event_id,
        subject_ref,
        state,
        retention_class,
        sequence,
        &recorded_at,
        &previous_mac,
        key_version,
    );
    let mac = hmac_hex(hmac_key, payload.as_bytes());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = format!(
        "{{\"event_id\":{},\"key_version\":{},\"mac\":{},\"previous_mac\":{},\"recorded_at\":{},\"retention_class\":{},\"sequence\":{sequence},\"state\":{},\"subject_ref\":{}}}\n",
        json_escape(event_id),
        json_escape(key_version),
        json_escape(&mac),
        json_escape(&previous_mac),
        json_escape(&recorded_at),
        json_escape(retention_class),
        json_escape(state),
        json_escape(subject_ref),
    );
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.sync_all()?;
    write_anchor(&anchor_path(path), anchor_key, key_version, sequence, &mac)?;
    Ok(JournalEntry {
        event_id: event_id.to_string(),
        subject_ref: subject_ref.to_string(),
        state: state.to_string(),
        retention_class: retention_class.to_string(),
        sequence,
        recorded_at,
        previous_mac,
        key_version: key_version.to_string(),
        mac,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subj() -> String {
        pseudonymous_subject_ref("11111111-1111-1111-1111-111111111111", "hellodoctor-user")
    }

    #[test]
    fn pseudonymous_subject_ref_matches_python_construction() {
        let got = pseudonymous_subject_ref("synthetic-user", "hellodoctor-test");
        assert!(got.starts_with("sha256:"));
        assert_eq!(got.len(), 71);
        // Deterministic for the same inputs.
        assert_eq!(
            got,
            pseudonymous_subject_ref("synthetic-user", "hellodoctor-test")
        );
        assert_ne!(
            got,
            pseudonymous_subject_ref("other-user", "hellodoctor-test")
        );
    }

    #[test]
    fn append_then_read_round_trips_and_chains() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        let e1 = append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        assert_eq!(e1.sequence, 1);
        assert_eq!(e1.previous_mac, "0".repeat(64));
        let e2 = append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-2",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        assert_eq!(e2.sequence, 2);
        assert_eq!(e2.previous_mac, e1.mac);
        let entries = read_verified(&path, &key, &anchor_key, "v1").unwrap();
        assert_eq!(entries.len(), 2);
        cleanup(&dir);
    }

    #[test]
    fn append_is_idempotent_on_duplicate_event_id() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        let first = append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        let second = append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        assert_eq!(first, second);
        let entries = read_verified(&path, &key, &anchor_key, "v1").unwrap();
        assert_eq!(
            entries.len(),
            1,
            "duplicate event_id must not append a second line"
        );
        cleanup(&dir);
    }

    #[test]
    fn hmac_key_mismatch_is_rejected() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        let wrong_key = b"w".repeat(32);
        let result = read_verified(&path, &wrong_key, &anchor_key, "v1");
        assert!(matches!(result, Err(JournalError::Integrity(_))));
        cleanup(&dir);
    }

    #[test]
    fn truncation_is_detected_via_anchor_mismatch() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-2",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        // Truncate the data file back to just its first line without updating the anchor.
        let original = std::fs::read_to_string(&path).unwrap();
        let first_line = original.lines().next().unwrap();
        std::fs::write(&path, format!("{first_line}\n")).unwrap();
        let result = read_verified(&path, &key, &anchor_key, "v1");
        assert!(matches!(result, Err(JournalError::Integrity(_))));
        cleanup(&dir);
    }

    #[test]
    fn missing_anchor_with_present_data_fails_closed() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        std::fs::remove_file(anchor_path(&path)).unwrap();
        let result = read_verified(&path, &key, &anchor_key, "v1");
        // A missing checkpoint alongside present journal data must fail
        // closed. It surfaces as `Malformed` (the checkpoint file itself
        // cannot be read) rather than `Integrity` (a mismatch between two
        // present-but-disagreeing pieces of evidence) — the exact variant is
        // not the property under test; refusing to treat the journal as
        // verified is.
        assert!(result.is_err());
        cleanup(&dir);
    }

    #[test]
    fn missing_journal_with_no_prior_entries_is_empty_not_an_error() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        let entries = read_verified(&path, &key, &anchor_key, "v1").unwrap();
        assert!(entries.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn reused_event_id_with_different_subject_is_rejected() {
        let dir = tempdir();
        let path = dir.join("erasure.journal");
        let key = b"k".repeat(32);
        let anchor_key = b"a".repeat(32);
        append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &subj(),
            "COMPLETED",
            "clinical-retention",
        )
        .unwrap();
        let other_subject = pseudonymous_subject_ref("different-user", "hellodoctor-user");
        let result = append_idempotent(
            &path,
            &key,
            &anchor_key,
            "v1",
            "event-1",
            &other_subject,
            "COMPLETED",
            "clinical-retention",
        );
        assert!(matches!(result, Err(JournalError::Integrity(_))));
        cleanup(&dir);
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "hellodoctor-erasure-journal-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
