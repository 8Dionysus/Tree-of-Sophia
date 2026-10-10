//! Explicit record/row scope over authenticated, unchanged source members.
//! Catalog addresses are navigation: every selected binding is rechecked
//! against the held source bytes. This model creates no source revision,
//! membership EOF, rights grant, or semantic validation outcome.
use crate::item_rules::ItemRefusal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, RelativePath, canonical_raw_bytes_v1,
    parse_json,
};

pub const SELECTION_SCHEMA_PATH: &str = "ToS/contracts/source-record-closure-selection.schema.json";
pub const SELECTION_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../../ToS/contracts/source-record-closure-selection.schema.json");
pub fn selection_schema_digest() -> Digest256 {
    Digest256::of_bytes(SELECTION_SCHEMA_BYTES)
}

/// Allocation-free physical JSONL rows, including blank rows. LF, CRLF,
/// bare CR and final EOF follow the existing catalog slot delimiter law.
pub struct SourceRows<'a> {
    raw: &'a [u8],
    offset: usize,
    line: u64,
}
pub fn source_rows(raw: &[u8]) -> SourceRows<'_> {
    SourceRows {
        raw,
        offset: 0,
        line: 1,
    }
}
impl<'a> Iterator for SourceRows<'a> {
    type Item = (u64, &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == self.raw.len() {
            return None;
        }
        let start = self.offset;
        while self.offset < self.raw.len() && !matches!(self.raw[self.offset], b'\r' | b'\n') {
            self.offset += 1;
        }
        let end = self.offset;
        if self.offset < self.raw.len() {
            let delimiter = self.raw[self.offset];
            self.offset += 1;
            if delimiter == b'\r' && self.raw.get(self.offset) == Some(&b'\n') {
                self.offset += 1;
            }
        }
        let line = self.line;
        // There is another byte, hence this physical line number is strictly
        // below the representable slice length, including on 64-bit hosts.
        if self.offset < self.raw.len() {
            self.line += 1;
        }
        Some((line, &self.raw[start..end]))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SelectionLimits {
    pub max_manifest_bytes: usize,
    pub max_records: usize,
    pub max_slots: usize,
    pub max_roots: usize,
    pub max_owned_state_bytes: usize,
    pub max_row_bytes: usize,
    pub max_verify_state_bytes: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceRecordRef {
    pub id: String,
    pub version: u64,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectedSourceMember {
    pub source_ref: String,
    pub raw_sha256: String,
    pub raw_bytes: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectedRecordBinding {
    pub source_ref: String,
    pub raw_sha256: String,
    pub raw_bytes: u64,
    pub record_ref: SourceRecordRef,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectedSourceRecord {
    pub record_id: String,
    pub entry: Value,
    pub source: SelectedRecordBinding,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectedSlotBinding {
    pub source_ref: String,
    pub source_line: u64,
    pub byte_offset: u64,
    pub row_bytes: u64,
    pub raw_row_sha256: String,
    pub delimiter: String,
    pub file_sha256: String,
    pub file_bytes: u64,
    pub canonical_sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectedSourceSlot {
    pub source_slot_key: String,
    pub kind: String,
    pub identity: String,
    pub source: SelectedSlotBinding,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    roots: Vec<SourceRecordRef>,
    required_members: Vec<SelectedSourceMember>,
    records: Vec<SelectedSourceRecord>,
    source_slots: Vec<SelectedSourceSlot>,
}
pub struct SourceRecordSelection {
    manifest: Manifest,
    digest: Digest256,
    limits: SelectionLimits,
    charged_state_bytes: usize,
}
fn refuse() -> ItemRefusal {
    ItemRefusal::Source("invalid explicit source record closure selection".into())
}
fn finite(n: usize) -> bool {
    n > 0 && n < usize::MAX
}
fn digest(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identity(v: &str) -> bool {
    if v.len() > 4096 {
        return false;
    }
    let Some((kind, tail)) = v.strip_prefix("tos.").and_then(|v| v.split_once('.')) else {
        return false;
    };
    kind.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !tail.is_empty()
        && tail.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
/// Conservative shared preallocation bound for the strict parse, owned model
/// and bounded public binding serialization. Callers use the same owner law.
pub fn selection_state_upper_bound(raw_bytes: usize) -> Result<usize, ItemRefusal> {
    raw_bytes
        .checked_mul(160)
        .and_then(|n| n.checked_add(8192))
        .ok_or(crate::item_budget_origin!())
}

fn record_ref(v: &SourceRecordRef) -> bool {
    identity(&v.id)
        && v.version > 0
        && v.version <= 9_007_199_254_740_991
        && v.digest.strip_prefix("sha256:").is_some_and(digest)
}
fn source_path(v: &str) -> bool {
    v.len() <= 4096 && v.starts_with("ToS/") && RelativePath::parse(v).is_ok()
}
fn public_record_path(v: &str, suffix: &str) -> bool {
    source_path(v)
        && v.len() <= 1024
        && v.starts_with("ToS/source-witnesses/")
        && v.ends_with(suffix)
        && !v.split('/').any(|p| {
            p.starts_with('.')
                || matches!(
                    p,
                    "catalog" | "payload" | "private" | "owner-local" | "local-content"
                )
        })
}
fn checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "source record selection cancelled".into(),
        ));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}
fn json_limits(bytes: usize) -> Result<JsonLimits, ItemRefusal> {
    JsonLimits::new(
        bytes,
        128,
        bytes.checked_mul(2).ok_or(crate::item_budget_origin!())?,
        4096,
    )
    .map_err(|_| refuse())
}
fn canonical(raw: &[u8], cap: usize) -> Result<Vec<u8>, ItemRefusal> {
    parse_json(raw, JsonMode::PublishedStrict, json_limits(cap)?).map_err(|_| refuse())?;
    canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::SourceRecordDigestV1,
        json_limits(cap)?,
    )
    .map_err(|_| refuse())
}
impl SourceRecordSelection {
    pub fn parse(raw: &[u8], limits: SelectionLimits) -> Result<Self, ItemRefusal> {
        if ![
            limits.max_manifest_bytes,
            limits.max_records,
            limits.max_slots,
            limits.max_roots,
            limits.max_owned_state_bytes,
            limits.max_row_bytes,
            limits.max_verify_state_bytes,
        ]
        .into_iter()
        .all(finite)
            || raw.is_empty()
            || raw.len() > limits.max_manifest_bytes
        {
            return Err(refuse());
        }
        // Covers simultaneous strict JSON + serde decoded trees and retained
        // strings/index headers before allocating either representation.
        let charged_state_bytes = selection_state_upper_bound(raw.len())?;
        if charged_state_bytes > limits.max_owned_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source record selection parse state",
                used: u64::try_from(charged_state_bytes).ok(),
                limit: u64::try_from(limits.max_owned_state_bytes).ok(),
            });
        }
        parse_json(
            raw,
            JsonMode::PublishedStrict,
            json_limits(limits.max_manifest_bytes)?,
        )
        .map_err(|_| refuse())?;
        let mut manifest: Manifest = serde_json::from_slice(raw).map_err(|_| refuse())?;
        if manifest.schema_version != "tos_source_record_closure_selection_v1"
            || manifest.roots.is_empty()
            || manifest.roots.len() > limits.max_roots
            || manifest.records.is_empty()
            || manifest.records.len() > limits.max_records
            || manifest.source_slots.len() > limits.max_slots
            || manifest.required_members.is_empty()
            || manifest.required_members.len()
                > limits
                    .max_records
                    .checked_add(limits.max_slots)
                    .ok_or(crate::item_budget_origin!())?
        {
            return Err(refuse());
        }
        manifest
            .required_members
            .sort_unstable_by(|a, b| a.source_ref.cmp(&b.source_ref));
        manifest
            .records
            .sort_unstable_by(|a, b| a.source.source_ref.cmp(&b.source.source_ref));
        manifest.source_slots.sort_unstable_by(|a, b| {
            (&a.source.source_ref, a.source.source_line)
                .cmp(&(&b.source.source_ref, b.source.source_line))
        });
        manifest.roots.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        let mut ids = BTreeSet::new();
        let mut slot_ids = BTreeSet::new();
        for m in &manifest.required_members {
            if !source_path(&m.source_ref) || !digest(&m.raw_sha256) {
                return Err(refuse());
            }
        }
        if manifest
            .required_members
            .windows(2)
            .any(|w| w[0].source_ref == w[1].source_ref)
            || manifest
                .records
                .windows(2)
                .any(|w| w[0].source.source_ref == w[1].source.source_ref)
            || manifest.roots.windows(2).any(|w| w[0].id == w[1].id)
            || manifest.source_slots.windows(2).any(|w| {
                w[0].source.source_ref == w[1].source.source_ref
                    && w[0].source.source_line == w[1].source.source_line
            })
        {
            return Err(refuse());
        }
        let member = |path: &str| {
            manifest
                .required_members
                .binary_search_by(|m| m.source_ref.as_str().cmp(path))
                .ok()
                .map(|i| &manifest.required_members[i])
        };
        for r in &manifest.records {
            if !identity(&r.record_id)
                || !ids.insert(r.record_id.as_str())
                || !record_ref(&r.source.record_ref)
                || r.record_id != r.source.record_ref.id
                || !public_record_path(&r.source.source_ref, ".json")
                || r.source.raw_bytes == 0
                || r.source.raw_bytes > limits.max_row_bytes as u64
                || !digest(&r.source.raw_sha256)
                || r.entry.get("record_id").and_then(Value::as_str) != Some(&r.record_id)
                || r.entry.get("source_record_ref").and_then(Value::as_str)
                    != Some(&r.source.source_ref)
                || r.entry.get("record_sha256").and_then(Value::as_str)
                    != r.source.record_ref.digest.strip_prefix("sha256:")
            {
                return Err(refuse());
            }
            let m = member(&r.source.source_ref).ok_or_else(refuse)?;
            if m.raw_bytes != r.source.raw_bytes || m.raw_sha256 != r.source.raw_sha256 {
                return Err(refuse());
            }
        }
        for root in &manifest.roots {
            if !record_ref(root)
                || !manifest
                    .records
                    .iter()
                    .any(|r| r.source.record_ref == *root)
            {
                return Err(refuse());
            }
        }
        for s in &manifest.source_slots {
            let b = &s.source;
            if !identity(&s.identity)
                || !slot_ids.insert((s.kind.as_str(), s.identity.as_str()))
                || !matches!(s.kind.as_str(), "claim" | "provenance_event" | "anchor")
                || !public_record_path(&b.source_ref, ".jsonl")
                || b.source_line == 0
                || b.source_line > 9_007_199_254_740_991
                || b.row_bytes == 0
                || b.row_bytes > limits.max_row_bytes as u64
                || b.file_bytes == 0
                || b.file_bytes > 16_777_216
                || !digest(&b.raw_row_sha256)
                || !digest(&b.file_sha256)
                || !digest(&b.canonical_sha256)
                || !matches!(b.delimiter.as_str(), "lf" | "crlf" | "cr" | "eof")
            {
                return Err(refuse());
            }
            let key = serde_json::to_vec(&vec![s.kind.as_str(), s.identity.as_str()])
                .map_err(|_| refuse())?;
            if canonical(&key, limits.max_row_bytes)? != s.source_slot_key.as_bytes() {
                return Err(refuse());
            }
            let m = member(&b.source_ref).ok_or_else(refuse)?;
            if m.raw_bytes != b.file_bytes || m.raw_sha256 != b.file_sha256 {
                return Err(refuse());
            }
        }
        drop(ids);
        drop(slot_ids);
        Ok(Self {
            manifest,
            digest: Digest256::of_bytes(raw),
            limits,
            charged_state_bytes,
        })
    }
    /// Bounded public binding: exact raw manifest, root refs and selected-slot
    /// addresses remain separate from the authenticated physical member EOF.
    /// Temporary serialization fits the already charged manifest parse peak.
    pub fn binding(&self) -> Result<Value, ItemRefusal> {
        let slots = serde_json::to_vec(&self.manifest.source_slots).map_err(|_| refuse())?;
        if slots.len() > self.limits.max_manifest_bytes {
            return Err(crate::item_budget_origin!());
        }
        let canonical_slots = canonical(&slots, self.limits.max_manifest_bytes)?;
        Ok(serde_json::json!({
            "schema_version": "tos_source_record_closure_selection_binding_v1",
            "manifest_sha256": self.digest.to_hex(),
            "software_schema_sha256": selection_schema_digest().to_hex(),
            "roots": self.manifest.roots,
            "selected_slot_bindings_sha256": Digest256::of_bytes(&canonical_slots).to_hex(),
            "record_count": self.record_count(), "slot_count": self.slot_count(),
            "member_count": self.member_count()
        }))
    }
    pub fn verify_file<'s, 'a>(
        &'s self,
        path: &str,
        raw: &'a [u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<VerifiedSelectionFile<'s, 'a>, ItemRefusal> {
        checkpoint(deadline, cancelled)?;
        self.verify_metadata_member(path, raw)?;
        checkpoint(deadline, cancelled)?;
        let member = self
            .manifest
            .required_members
            .binary_search_by(|m| m.source_ref.as_str().cmp(path))
            .map_err(|_| refuse())?;
        Ok(VerifiedSelectionFile {
            selection: self,
            path: &self.manifest.required_members[member].source_ref,
            raw,
        })
    }
    /// The same owner parser profile used for canonical selected-row binding.
    pub fn row_json_limits(&self) -> Result<JsonLimits, ItemRefusal> {
        json_limits(self.limits.max_row_bytes)
    }
    pub fn digest(&self) -> Digest256 {
        self.digest
    }
    pub fn charged_state_bytes(&self) -> usize {
        self.charged_state_bytes
    }
    pub fn state_bytes(&self) -> usize {
        self.charged_state_bytes
    }
    pub fn root_count(&self) -> usize {
        self.manifest.roots.len()
    }
    pub fn record_count(&self) -> usize {
        self.manifest.records.len()
    }
    pub fn slot_count(&self) -> usize {
        self.manifest.source_slots.len()
    }
    pub fn member_count(&self) -> usize {
        self.manifest.required_members.len()
    }
    pub fn roots(&self) -> &[SourceRecordRef] {
        &self.manifest.roots
    }
    pub fn records(&self) -> impl Iterator<Item = &SelectedSourceRecord> {
        self.manifest.records.iter()
    }
    pub fn slots(&self) -> impl Iterator<Item = &SelectedSourceSlot> {
        self.manifest.source_slots.iter()
    }
    pub fn members(&self) -> impl Iterator<Item = &SelectedSourceMember> {
        self.manifest.required_members.iter()
    }
    pub fn record_ids(&self) -> impl Iterator<Item = &str> {
        self.manifest.records.iter().map(|r| r.record_id.as_str())
    }
    pub fn record(&self, path: &str) -> Option<&SelectedSourceRecord> {
        self.manifest
            .records
            .binary_search_by(|r| r.source.source_ref.as_str().cmp(path))
            .ok()
            .map(|i| &self.manifest.records[i])
    }
    pub fn selected_record(&self, path: &str, id: &str) -> bool {
        self.record(path).is_some_and(|r| r.record_id == id)
    }
    pub fn slot(&self, path: &str, line: u64) -> Option<&SelectedSourceSlot> {
        self.manifest
            .source_slots
            .binary_search_by(|s| {
                (s.source.source_ref.as_str(), s.source.source_line).cmp(&(path, line))
            })
            .ok()
            .map(|i| &self.manifest.source_slots[i])
    }
    pub fn selected_row(&self, path: &str, line: u64) -> bool {
        self.slot(path, line).is_some()
    }
    pub fn file_slots(&self, path: &str) -> &[SelectedSourceSlot] {
        let slots = &self.manifest.source_slots;
        let start = slots.partition_point(|slot| slot.source.source_ref.as_str() < path);
        let end = slots.partition_point(|slot| slot.source.source_ref.as_str() <= path);
        &slots[start..end]
    }
    pub fn selects_semantic_member(&self, path: &str) -> bool {
        self.record(path).is_some() || !self.file_slots(path).is_empty()
    }
    pub fn contains_member(&self, path: &str) -> bool {
        self.manifest
            .required_members
            .binary_search_by(|m| m.source_ref.as_str().cmp(path))
            .is_ok()
    }
    pub fn verify_metadata_member(&self, path: &str, raw: &[u8]) -> Result<(), ItemRefusal> {
        let i = self
            .manifest
            .required_members
            .binary_search_by(|m| m.source_ref.as_str().cmp(path))
            .map_err(|_| refuse())?;
        let m = &self.manifest.required_members[i];
        if m.raw_bytes != raw.len() as u64 || m.raw_sha256 != Digest256::of_bytes(raw).to_hex() {
            return Err(refuse());
        }
        Ok(())
    }
    pub fn verify_record(
        &self,
        path: &str,
        raw: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        checkpoint(deadline, cancelled)?;
        let r = self.record(path).ok_or_else(refuse)?;
        self.verify_metadata_member(path, raw)?;
        let peak = raw
            .len()
            .checked_mul(160)
            .and_then(|n| n.checked_add(8192))
            .ok_or(crate::item_budget_origin!())?;
        if peak > self.limits.max_verify_state_bytes {
            return Err(crate::item_budget_origin!());
        }
        let c = canonical(raw, self.limits.max_row_bytes)?;
        if format!("sha256:{}", Digest256::of_bytes(&c).to_hex()) != r.source.record_ref.digest {
            return Err(refuse());
        }
        let v: Value = serde_json::from_slice(raw).map_err(|_| refuse())?;
        if !["record_id", "artifact_id", "composite_id"]
            .into_iter()
            .any(|key| v.get(key).and_then(Value::as_str) == Some(&r.record_id))
        {
            return Err(refuse());
        }
        if v.get("record_version").and_then(Value::as_u64).unwrap_or(1)
            != r.source.record_ref.version
        {
            return Err(refuse());
        }
        checkpoint(deadline, cancelled)
    }
    pub fn verify_slot<'a>(
        &self,
        slot: &SelectedSourceSlot,
        raw: &'a [u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<&'a [u8], ItemRefusal> {
        checkpoint(deadline, cancelled)?;
        if self.slot(&slot.source.source_ref, slot.source.source_line) != Some(slot) {
            return Err(refuse());
        }
        self.verify_metadata_member(&slot.source.source_ref, raw)?;
        let peak = slot.verification_state_upper_bound()?;
        if peak > self.limits.max_verify_state_bytes {
            return Err(crate::item_budget_origin!());
        }
        let row = slot.verify_member(raw, self.limits.max_row_bytes, deadline, cancelled)?;
        checkpoint(deadline, cancelled)?;
        Ok(row)
    }
}
impl SelectedSourceSlot {
    pub fn verification_state_upper_bound(&self) -> Result<usize, ItemRefusal> {
        usize::try_from(self.source.row_bytes)
            .ok()
            .and_then(|n| n.checked_mul(160))
            .and_then(|n| n.checked_add(8192))
            .ok_or(crate::item_budget_origin!())
    }
    pub fn verify_member<'a>(
        &self,
        file: &'a [u8],
        max_row_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<&'a [u8], ItemRefusal> {
        checkpoint(deadline, cancelled)?;
        let b = &self.source;
        if file.len() as u64 != b.file_bytes || Digest256::of_bytes(file).to_hex() != b.file_sha256
        {
            return Err(refuse());
        }
        let offset = usize::try_from(b.byte_offset).map_err(|_| refuse())?;
        let n = usize::try_from(b.row_bytes).map_err(|_| refuse())?;
        let delimiter: &[u8] = match b.delimiter.as_str() {
            "lf" => b"\n",
            "crlf" => b"\r\n",
            "cr" => b"\r",
            "eof" => b"",
            _ => return Err(refuse()),
        };
        let end = offset.checked_add(n).ok_or(crate::item_budget_origin!())?;
        let final_end = end
            .checked_add(delimiter.len())
            .ok_or(crate::item_budget_origin!())?;
        if n == 0
            || n > max_row_bytes
            || final_end > file.len()
            || delimiter.is_empty() && final_end != file.len()
            || offset > 0 && !matches!(file[offset - 1], b'\r' | b'\n')
            || offset > 0 && file[offset - 1] == b'\r' && file.get(offset) == Some(&b'\n')
            || file[end..final_end] != *delimiter
            || delimiter == b"\r" && file.get(final_end) == Some(&b'\n')
        {
            return Err(refuse());
        }
        // Count physical boundaries, including blank lines, without allocation.
        let mut line = 1u64;
        let mut i = 0usize;
        while i < offset {
            checkpoint(deadline, cancelled)?;
            match file[i] {
                b'\r' => {
                    line = line.checked_add(1).ok_or(crate::item_budget_origin!())?;
                    i += 1;
                    if i < offset && file[i] == b'\n' {
                        i += 1;
                    }
                }
                b'\n' => {
                    line = line.checked_add(1).ok_or(crate::item_budget_origin!())?;
                    i += 1;
                }
                _ => i += 1,
            }
        }
        if line != b.source_line {
            return Err(refuse());
        }
        let row = &file[offset..end];
        if row.iter().any(|b| matches!(b, b'\r' | b'\n'))
            || Digest256::of_bytes(row).to_hex() != b.raw_row_sha256
        {
            return Err(refuse());
        }
        self.verify_row_payload(row, max_row_bytes, deadline, cancelled)?;
        Ok(row)
    }
    fn verify_row_payload(
        &self,
        row: &[u8],
        max_row_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        let b = &self.source;
        if row.len() > max_row_bytes || Digest256::of_bytes(row).to_hex() != b.raw_row_sha256 {
            return Err(refuse());
        }
        let c = canonical(row, max_row_bytes)?;
        if Digest256::of_bytes(&c).to_hex() != b.canonical_sha256 {
            return Err(refuse());
        }
        let value: Value = serde_json::from_slice(row).map_err(|_| refuse())?;
        let field = match self.kind.as_str() {
            "claim" => "claim_id",
            "provenance_event" => "event_id",
            "anchor" => "anchor_id",
            _ => return Err(refuse()),
        };
        if value.get(field).and_then(Value::as_str) != Some(&self.identity)
            || self.kind == "claim"
                && !matches!(
                    value.get("visibility").and_then(Value::as_str),
                    Some("public" | "public_metadata_only")
                )
            || self.kind == "provenance_event"
                && value.get("schema_version").and_then(Value::as_str)
                    == Some("tos_provenance_event_v2")
                && !matches!(
                    value
                        .get("rights_and_visibility")
                        .and_then(|v| v.get("content_visibility"))
                        .and_then(Value::as_str),
                    Some("tracked_public_metadata" | "public_content" | "public_synthetic")
                )
        {
            return Err(refuse());
        }
        checkpoint(deadline, cancelled)?;
        Ok(())
    }
}

/// Borrowed semantic selection over one already authenticated full member.
/// It grants neither a filesystem capability nor a physical membership EOF.
pub struct VerifiedSelectionFile<'s, 'a> {
    selection: &'s SourceRecordSelection,
    path: &'s str,
    raw: &'a [u8],
}
impl<'s, 'a> VerifiedSelectionFile<'s, 'a> {
    pub fn row_cursor(&self) -> SelectedRowCursor<'s, 'a> {
        let slots = self.selection.file_slots(self.path);
        SelectedRowCursor {
            rows: source_rows(self.raw),
            raw: self.raw,
            slots,
            next_slot: 0,
            max_row_bytes: self.selection.limits.max_row_bytes,
            max_verify_state_bytes: self.selection.limits.max_verify_state_bytes,
            terminal: false,
        }
    }
}
pub struct SelectedRowCursor<'s, 'a> {
    rows: SourceRows<'a>,
    raw: &'a [u8],
    slots: &'s [SelectedSourceSlot],
    next_slot: usize,
    max_row_bytes: usize,
    max_verify_state_bytes: usize,
    terminal: bool,
}
impl<'s, 'a> SelectedRowCursor<'s, 'a> {
    pub fn next_checked(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<Result<(u64, &'a [u8], &'s SelectedSourceSlot), ItemRefusal>> {
        if self.terminal {
            return None;
        }
        if let Err(e) = checkpoint(deadline, cancelled) {
            self.terminal = true;
            return Some(Err(e));
        }
        while let Some((line, row)) = self.rows.next() {
            if let Err(e) = checkpoint(deadline, cancelled) {
                self.terminal = true;
                return Some(Err(e));
            }
            let Some(slot) = self.slots.get(self.next_slot) else {
                continue;
            };
            if line < slot.source.source_line {
                continue;
            }
            let result = (|| {
                if line != slot.source.source_line {
                    return Err(refuse());
                }
                let b = &slot.source;
                let offset = (row.as_ptr() as usize)
                    .checked_sub(self.raw.as_ptr() as usize)
                    .ok_or_else(refuse)?;
                let end = offset
                    .checked_add(row.len())
                    .ok_or(crate::item_budget_origin!())?;
                let delimiter: &[u8] = match b.delimiter.as_str() {
                    "lf" => b"\n",
                    "crlf" => b"\r\n",
                    "cr" => b"\r",
                    "eof" => b"",
                    _ => return Err(refuse()),
                };
                let final_end = end
                    .checked_add(delimiter.len())
                    .ok_or(crate::item_budget_origin!())?;
                if offset as u64 != b.byte_offset
                    || row.len() as u64 != b.row_bytes
                    || final_end > self.raw.len()
                    || self.raw[end..final_end] != *delimiter
                    || delimiter.is_empty() && end != self.raw.len()
                    || delimiter == b"\r" && self.raw.get(final_end) == Some(&b'\n')
                {
                    return Err(refuse());
                }
                if slot.verification_state_upper_bound()? > self.max_verify_state_bytes {
                    return Err(crate::item_budget_origin!());
                }
                slot.verify_row_payload(row, self.max_row_bytes, deadline, cancelled)?;
                Ok((line, row, slot))
            })();
            self.next_slot += 1;
            if result.is_err() {
                self.terminal = true;
            }
            return Some(result);
        }
        self.terminal = true;
        if self.next_slot != self.slots.len() {
            Some(Err(refuse()))
        } else {
            None
        }
    }
}

impl<'s, 'a> VerifiedSelectionFile<'s, 'a> {
    pub fn selected_rows<'c>(
        &self,
        deadline: Instant,
        cancelled: &'c AtomicBool,
    ) -> SelectedRows<'s, 'a, 'c> {
        SelectedRows {
            cursor: self.row_cursor(),
            deadline,
            cancelled,
        }
    }
}
pub struct SelectedRows<'s, 'a, 'c> {
    cursor: SelectedRowCursor<'s, 'a>,
    deadline: Instant,
    cancelled: &'c AtomicBool,
}
impl<'s, 'a, 'c> Iterator for SelectedRows<'s, 'a, 'c> {
    type Item = Result<(u64, &'a [u8], &'s SelectedSourceSlot), ItemRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        self.cursor.next_checked(self.deadline, self.cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_slot_preserves_blank_crlf_and_rejects_line_or_byte_alias() {
        let raw=br#"{"event_id":"tos.event.selection.test","schema_version":"tos_provenance_event_v1"}"#;
        let mut file = b"\r\n".to_vec();
        file.extend_from_slice(raw);
        file.extend_from_slice(b"\r\n");
        let mut slot = SelectedSourceSlot {
            source_slot_key: "[\"provenance_event\",\"tos.event.selection.test\"]".into(),
            kind: "provenance_event".into(),
            identity: "tos.event.selection.test".into(),
            source: SelectedSlotBinding {
                source_ref: "ToS/source-witnesses/relations/selection/provenance.jsonl".into(),
                source_line: 2,
                byte_offset: 2,
                row_bytes: raw.len() as u64,
                raw_row_sha256: Digest256::of_bytes(raw).to_hex(),
                delimiter: "crlf".into(),
                file_sha256: Digest256::of_bytes(&file).to_hex(),
                file_bytes: file.len() as u64,
                canonical_sha256: Digest256::of_bytes(&canonical(raw, 4096).unwrap()).to_hex(),
            },
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let cancel = AtomicBool::new(false);
        assert_eq!(
            slot.verify_member(&file, 4096, deadline, &cancel).unwrap(),
            raw
        );
        slot.source.source_line = 3;
        assert!(slot.verify_member(&file, 4096, deadline, &cancel).is_err());
        slot.source.source_line = 2;
        file[3] ^= 1;
        assert!(slot.verify_member(&file, 4096, deadline, &cancel).is_err());
    }
}
