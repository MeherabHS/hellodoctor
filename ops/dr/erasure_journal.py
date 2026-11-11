"""Small append-only recovery journal contract for account erasure.

This module intentionally stores only pseudonymous subject references and
retention metadata. Production deployment must place the journal in a separate
protected store and supply its HMAC key through an independent custody path.
"""

from __future__ import annotations

import hashlib
import hmac
import json
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path


@dataclass(frozen=True)
class ErasureJournalEntry:
    event_id: str
    subject_ref: str
    state: str
    retention_class: str
    sequence: int
    recorded_at: str
    previous_mac: str
    key_version: str
    mac: str


def pseudonymous_subject_ref(stable_identity: str, namespace: str) -> str:
    """Return a non-reversible reference; callers must not pass clinical data."""
    return "sha256:" + hashlib.sha256((namespace + ":" + stable_identity).encode()).hexdigest()


class ErasureJournal:
    def __init__(self, path: Path, hmac_key: bytes, *, anchor_key: bytes | None = None, key_version: str = "v1"):
        if len(hmac_key) < 32:
            raise ValueError("erasure journal HMAC key must be at least 32 bytes")
        if anchor_key is None:
            # This compatibility default is only for local fixtures. Production
            # must provide a separately-custodied anchor key.
            anchor_key = hmac.new(hmac_key, b"hellodoctor-erasure-anchor-v1", hashlib.sha256).digest()
        if len(anchor_key) < 32:
            raise ValueError("erasure journal anchor key must be at least 32 bytes")
        if not key_version or len(key_version) > 32:
            raise ValueError("erasure journal key version is invalid")
        self.path = path
        self.key = hmac_key
        self.anchor_key = anchor_key
        self.key_version = key_version
        self.anchor_path = path.with_name(path.name + ".anchor")

    def _mac(self, payload: dict[str, object]) -> str:
        canonical = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
        return hmac.new(self.key, canonical, hashlib.sha256).hexdigest()

    def _anchor_mac(self, payload: dict[str, object]) -> str:
        canonical = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
        return hmac.new(self.anchor_key, canonical, hashlib.sha256).hexdigest()

    def _write_anchor(self, sequence: int, last_mac: str) -> None:
        payload = {"key_version": self.key_version, "sequence": sequence, "last_mac": last_mac}
        row = {**payload, "mac": self._anchor_mac(payload)}
        temporary = self.anchor_path.with_name(self.anchor_path.name + ".tmp")
        temporary.write_text(json.dumps(row, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(self.anchor_path)

    def _read_anchor(self) -> dict[str, object]:
        if not self.anchor_path.exists():
            raise ValueError("erasure journal checkpoint is missing")
        try:
            row = json.loads(self.anchor_path.read_text(encoding="utf-8"))
            payload = {"key_version": row["key_version"], "sequence": row["sequence"], "last_mac": row["last_mac"]}
            mac = row["mac"]
        except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
            raise ValueError("erasure journal checkpoint is invalid") from exc
        if payload["key_version"] != self.key_version or not isinstance(payload["sequence"], int) or not isinstance(payload["last_mac"], str) or not isinstance(mac, str) or not hmac.compare_digest(mac, self._anchor_mac(payload)):
            raise ValueError("erasure journal checkpoint integrity validation failed")
        return payload

    def append(self, event_id: str, subject_ref: str, state: str, retention_class: str) -> ErasureJournalEntry:
        if not subject_ref.startswith("sha256:") or len(subject_ref) != 71:
            raise ValueError("subject_ref must be a pseudonymous sha256 reference")
        existing = self.read_verified()
        sequence = len(existing) + 1
        previous_mac = existing[-1].mac if existing else "0" * 64
        payload: dict[str, object] = {
            "event_id": event_id,
            "subject_ref": subject_ref,
            "state": state,
            "retention_class": retention_class,
            "sequence": sequence,
            "recorded_at": datetime.now(timezone.utc).isoformat(),
            "previous_mac": previous_mac,
            "key_version": self.key_version,
        }
        entry = ErasureJournalEntry(**payload, mac=self._mac(payload))  # type: ignore[arg-type]
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with self.path.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(asdict(entry), sort_keys=True) + "\n")
        self._write_anchor(entry.sequence, entry.mac)
        return entry

    def read_verified(self) -> list[ErasureJournalEntry]:
        if not self.path.exists():
            if self.anchor_path.exists():
                raise ValueError("erasure journal data is missing while checkpoint remains")
            return []
        entries: list[ErasureJournalEntry] = []
        previous_mac = "0" * 64
        for expected_sequence, line in enumerate(self.path.read_text(encoding="utf-8").splitlines(), start=1):
            row = json.loads(line)
            entry = ErasureJournalEntry(**row)
            payload = asdict(entry)
            payload.pop("mac")
            if entry.sequence != expected_sequence or entry.previous_mac != previous_mac or entry.key_version != self.key_version or not hmac.compare_digest(entry.mac, self._mac(payload)):
                raise ValueError("erasure journal integrity or sequence validation failed")
            entries.append(entry)
            previous_mac = entry.mac
        anchor = self._read_anchor()
        if anchor["sequence"] != len(entries) or anchor["last_mac"] != previous_mac:
            raise ValueError("erasure journal checkpoint does not match journal tail")
        return entries

    def reconcile(self, erased_subject_refs: set[str]) -> dict[str, str]:
        """Fail closed on a missing/tampered journal; never activates subjects."""
        if not self.path.exists() or not self.anchor_path.exists():
            raise ValueError("erasure journal and independent checkpoint are required")
        result: dict[str, str] = {}
        for entry in self.read_verified():
            if entry.subject_ref in erased_subject_refs or entry.state == "COMPLETED":
                result[entry.subject_ref] = "ERASED_OR_RETAINED"
            else:
                result[entry.subject_ref] = "REVIEW_REQUIRED"
        return result
