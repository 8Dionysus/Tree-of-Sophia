//! Historical capture bytes supplied to the complete admission validator.
//! Reuses the accepted corpus_archive v1/v2 verifier, never executes captured
//! software, and never grants source, semantic, publication or rights authority.
use super::source_admission::{active, invalid};
use super::source_admission_candidate::Candidate;
use serde_json::Value;
use std::{
    cell::Cell, collections::BTreeSet, io, path::PathBuf, sync::atomic::AtomicBool, time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonMode, RelativePath, canonical_bytes_v1,
    parse_json,
};
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_source_store::{
    CaptureRestoreLimits, ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
    verify_capture_with_usage,
};

pub(crate) struct HistorySelection {
    pub capture: PathBuf,
    pub restored: PathBuf,
}
#[derive(Clone, Copy)]
pub(crate) struct HistoryLimits {
    pub metadata: ReadLimits,
    pub max_packs: usize,
    pub max_read_bytes: u64,
    pub max_state_bytes: usize,
    pub max_member_bytes: usize,
}
struct Pack {
    capture: RouteSources,
    restored: RouteSources,
    manifest_sha: Digest256,
    members_sha: Digest256,
    receipt_sha: Digest256,
    manifest_size: usize,
    members_size: usize,
    receipt_size: usize,
}
struct Evidence {
    row: Value,
    path: String,
    sha256: Digest256,
    size: usize,
    pack: usize,
}
pub(crate) struct HistoryRead {
    pub bytes: Vec<u8>,
    pub read_bytes: u64,
}
pub(crate) struct HistoryEvidence {
    packs: Vec<Pack>,
    entries: Vec<Evidence>,
    deadline: Instant,
    read_bytes: Cell<u64>,
    read_limit: u64,
    retained_state: usize,
    peak_state: usize,
}
fn add(a: u64, b: u64) -> io::Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| invalid("historical evidence byte cost overflow"))
}
fn mul(a: u64, b: u64) -> io::Result<u64> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("historical evidence state cost overflow"))
}
fn text<'a>(v: &'a Value, key: &str) -> io::Result<&'a str> {
    v.as_object()
        .and_then(|o| o.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("historical member lacks {key}")))
}
fn eligible(path: &str) -> bool {
    let parts: Vec<_> = path.split('/').collect();
    if parts.contains(&"owner-local") {
        return false;
    }
    let source_member = path.starts_with("ToS/")
        && !parts
            .iter()
            .any(|p| [".git", "payload", "owner-local"].contains(p))
        && (!(path.starts_with("ToS/derived-exports/")
            || path.starts_with("ToS/source-witnesses/catalog/"))
            || path.ends_with(".md"));
    !source_member
        && (!path.starts_with("ToS/")
            || path.starts_with("ToS/derived-exports/")
            || parts.contains(&"payload"))
}
// The Host reserves the existing stream_regular 32KiB workspace once. Only
// the exact member/control Vec consumes the per-request remaining state here.
fn bounded(
    sources: &mut RouteSources,
    path: &str,
    cap: usize,
    read: &mut usize,
    total: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<Vec<u8>> {
    if cap > total.saturating_sub(*read) {
        return Err(invalid(
            "historical operand exceeds remaining read allowance",
        ));
    }
    let mut bytes = Vec::with_capacity(cap);
    let mut used = *read as u64;
    let result = sources.stream_regular(path, cap as u64, &mut used, total as u64, |chunk| {
        active(deadline, cancel)?;
        bytes.extend_from_slice(chunk);
        Ok(())
    });
    *read = usize::try_from(used).map_err(invalid)?;
    result?.ok_or_else(|| invalid("historical regular operand missing"))?;
    Ok(bytes)
}

impl HistoryEvidence {
    pub(crate) fn select(
        primary: &RouteSources,
        selections: &[HistorySelection],
        limits: HistoryLimits,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        active(deadline, cancel)?;
        let meta = limits.metadata.validate().map_err(invalid)?;
        if deadline > primary.deadline()
            || selections.is_empty()
            || selections.len() > limits.max_packs
            || [
                limits.max_packs,
                limits.max_state_bytes,
                limits.max_member_bytes,
            ]
            .iter()
            .any(|n| *n == 0 || *n == usize::MAX)
            || limits.max_read_bytes == 0
            || limits.max_read_bytes == u64::MAX
        {
            return Err(invalid("invalid historical evidence limits or selection"));
        }
        usize::try_from(limits.max_read_bytes).map_err(invalid)?;
        let mut packs = Vec::new();
        let mut entries = Vec::new();
        let mut seen_captures = BTreeSet::new();
        let mut seen_roots = BTreeSet::new();
        let mut seen_paths = BTreeSet::new();
        let mut direct_read = 0usize;
        let mut external_read = 0u64;
        let mut retained = 1024u64;
        let mut peak = retained;
        for selected in selections {
            active(deadline, cancel)?;
            let total = usize::try_from(
                limits
                    .max_read_bytes
                    .checked_sub(external_read)
                    .ok_or_else(|| invalid("historical read budget exceeded"))?,
            )
            .map_err(invalid)?;
            // Bound related-root/path allocations before opening any pack.
            let roots = add(
                mul(
                    add(
                        selected.capture.as_os_str().len() as u64,
                        selected.restored.as_os_str().len() as u64,
                    )?,
                    8,
                )?,
                32768,
            )?;
            retained = add(retained, roots)?;
            if retained > limits.max_state_bytes as u64 {
                return Err(invalid("historical root retention exceeds state bound"));
            }
            let mut capture =
                RouteSources::new_until_related(&selected.capture, deadline, primary)?;
            let mut restored =
                RouteSources::new_until_related(&selected.restored, deadline, primary)?;
            if !seen_captures.insert(capture.selected_root_path().to_path_buf())
                || !seen_roots.insert(restored.selected_root_path().to_path_buf())
            {
                return Err(invalid(
                    "historical validation evidence overlaps another pack",
                ));
            }
            let mut sizes = Vec::new();
            for path in ["capture.json", "members.jsonl", "restore-receipt.json"] {
                let root = if path == "restore-receipt.json" {
                    &mut restored
                } else {
                    &mut capture
                };
                let m = root
                    .metadata(path)?
                    .ok_or_else(|| invalid("historical control file missing"))?;
                if !m.is_file() || m.len() > meta.max_manifest_bytes as u64 {
                    return Err(invalid(
                        "historical control file exceeds selected metadata bound",
                    ));
                }
                sizes.push(usize::try_from(m.len()).map_err(invalid)?);
            }
            let (manifest_size, members_size, receipt_size) = (sizes[0], sizes[1], sizes[2]);
            let metadata_size = add(
                add(manifest_size as u64, members_size as u64)?,
                receipt_size as u64,
            )?;
            let prep = add(retained, mul(metadata_size, 32)?)?;
            if prep > limits.max_state_bytes as u64 {
                return Err(invalid(
                    "historical metadata preparation exceeds state bound",
                ));
            }
            peak = peak.max(prep);
            let manifest_raw = bounded(
                &mut capture,
                "capture.json",
                manifest_size,
                &mut direct_read,
                total,
                deadline,
                cancel,
            )?;
            let requested: Value = serde_json::from_slice(&manifest_raw).map_err(invalid)?;
            let selection = SoftwareCaptureSelectionV1 {
                source_git_commit: text(&requested, "source_git_commit")?.to_owned(),
                source_git_tree: text(&requested, "source_git_tree")?.to_owned(),
                capture_manifest_sha256: Digest256::of_bytes(&manifest_raw),
            };
            let number = |key: &str| {
                requested
                    .get(key)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid(format!("historical manifest lacks {key}")))
            };
            let archive_size = number("archive_size_bytes")?;
            let source_size = number("source_bytes")?;
            let count = number("member_count")?;
            if count > meta.max_manifest_entries as u64 || archive_size == 0 {
                return Err(invalid("historical manifest totals exceed limits"));
            }
            let members_sha =
                Digest256::from_hex(text(&requested, "members_sha256")?).map_err(invalid)?;
            let members_raw = bounded(
                &mut capture,
                "members.jsonl",
                members_size,
                &mut direct_read,
                total,
                deadline,
                cancel,
            )?;
            if Digest256::of_bytes(&members_raw) != members_sha {
                return Err(invalid("historical capture members changed"));
            }
            let pack = packs.len();
            let mut row_count = 0usize;
            for line in members_raw.split(|b| *b == b'\n').filter(|b| !b.is_empty()) {
                active(deadline, cancel)?;
                row_count = row_count
                    .checked_add(1)
                    .ok_or_else(|| invalid("historical member count overflow"))?;
                if row_count > meta.max_manifest_entries {
                    return Err(invalid("historical member count exceeds bound"));
                }
                let row: Value = serde_json::from_slice(line).map_err(invalid)?;
                let path = text(&row, "path")?;
                RelativePath::parse(path).map_err(invalid)?;
                let size = row
                    .get("size_bytes")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("historical member has invalid length"))?;
                let sha = Digest256::from_hex(text(&row, "sha256")?).map_err(invalid)?;
                if eligible(path) {
                    let charge = add(
                        add(mul(line.len() as u64, 8)?, mul(path.len() as u64, 3)?)?,
                        256,
                    )?;
                    retained = add(retained, charge)?;
                    if add(retained, mul(metadata_size, 32)?)? > limits.max_state_bytes as u64 {
                        return Err(invalid(
                            "historical original-row retention exceeds state bound",
                        ));
                    }
                    if !seen_paths.insert(path.to_owned()) {
                        return Err(invalid("historical evidence packs overlap"));
                    }
                    let size = usize::try_from(size).map_err(invalid)?;
                    if size > limits.max_member_bytes {
                        return Err(invalid("historical evidence member byte bound exceeded"));
                    }
                    entries.push(Evidence {
                        path: path.to_owned(),
                        sha256: sha,
                        size,
                        pack,
                        row,
                    });
                }
            }
            if row_count as u64 != count {
                return Err(invalid("historical member count changed"));
            }
            // Actual fixed metadata/compressed costs, not N times worst caps.
            // The existing tar crate has no separate per-PAX allocation bound:
            // admit its decoded-total transient from the real remaining ledger.
            let fixed = add(
                add(mul(archive_size, 2)?, mul(manifest_size as u64, 3)?)?,
                add(mul(members_size as u64, 3)?, mul(receipt_size as u64, 2)?)?,
            )?;
            let read_left = limits
                .max_read_bytes
                .checked_sub(add(add(external_read, direct_read as u64)?, fixed)?)
                .ok_or_else(|| invalid("historical fixed reads exceed admission budget"))?;
            let workspace = add(
                add(retained, mul(metadata_size, 32)?)?,
                add(mul(count, 1024)?, 131072)?,
            )?;
            let state_left = (limits.max_state_bytes as u64)
                .checked_sub(workspace)
                .ok_or_else(|| invalid("historical verification workspace exceeds state bound"))?;
            let decoded_cap = read_left.min(state_left);
            if decoded_cap == 0 || source_size > decoded_cap {
                return Err(invalid(
                    "historical decoded capture does not fit remaining ledger",
                ));
            }

            peak = peak.max(add(workspace, decoded_cap)?);
            let transport = CaptureRestoreLimits {
                metadata: meta,
                max_archive_bytes: archive_size,
                max_decoded_bytes: decoded_cap,
                max_source_bytes: source_size.max(1),
            };
            let verification = verify_capture_with_usage(
                capture.selected_root_path(),
                &selection,
                transport,
                deadline,
                cancel,
            )
            .map_err(invalid)?;
            external_read = add(
                external_read,
                verification.usage.total_read_bytes().map_err(invalid)?,
            )?;
            let manifest = verification.manifest;
            capture.verify_root()?;
            let reader = SoftwareCaptureReader::open(
                capture.selected_root_path(),
                restored.selected_root_path(),
                selection.clone(),
                meta,
                deadline,
                cancel,
            )
            .map_err(invalid)?;
            if reader.members().count() != row_count {
                return Err(invalid("historical verified member count differs"));
            }
            drop(reader);
            external_read = add(external_read, metadata_size)?;
            let total = usize::try_from(
                limits
                    .max_read_bytes
                    .checked_sub(external_read)
                    .ok_or_else(|| invalid("historical read budget exceeded"))?,
            )
            .map_err(invalid)?;
            let recheck = bounded(
                &mut capture,
                "capture.json",
                manifest_size,
                &mut direct_read,
                total,
                deadline,
                cancel,
            )?;
            if Digest256::of_bytes(&recheck) != selection.capture_manifest_sha256 {
                return Err(invalid("historical capture manifest changed"));
            }
            drop(recheck);
            let members_recheck = bounded(
                &mut capture,
                "members.jsonl",
                members_size,
                &mut direct_read,
                total,
                deadline,
                cancel,
            )?;
            if Digest256::of_bytes(&members_recheck) != members_sha {
                return Err(invalid(
                    "historical capture members changed after verification",
                ));
            }
            drop(members_recheck);
            let receipt_raw = bounded(
                &mut restored,
                "restore-receipt.json",
                receipt_size,
                &mut direct_read,
                total,
                deadline,
                cancel,
            )?;
            let receipt =
                parse_json(&receipt_raw, JsonMode::PublishedStrict, meta.json).map_err(invalid)?;
            let canonical = canonical_bytes_v1(
                receipt.root(),
                CanonicalProfile::CorpusSnapshotV1,
                meta.json,
            )
            .map_err(invalid)?;
            let r = receipt.root();
            if canonical != receipt_raw
                || r.as_object().map(|o| o.len()) != Some(5)
                || r.object_get("schema_version").and_then(|v| v.as_str())
                    != Some("tos_corpus_restore_receipt_v1")
                || r.object_get("source_git_commit").and_then(|v| v.as_str())
                    != Some(selection.source_git_commit.as_str())
                || r.object_get("manifest_sha256").and_then(|v| v.as_str())
                    != Some(selection.capture_manifest_sha256.to_hex().as_str())
                || r.object_get("member_count").and_then(|v| v.as_u64())
                    != manifest.object_get("member_count").and_then(|v| v.as_u64())
                || r.object_get("source_bytes").and_then(|v| v.as_u64())
                    != manifest.object_get("source_bytes").and_then(|v| v.as_u64())
            {
                return Err(invalid(
                    "historical restore receipt does not bind its capture",
                ));
            }
            capture.verify_root()?;
            restored.verify_root()?;
            packs.push(Pack {
                capture,
                restored,
                manifest_sha: selection.capture_manifest_sha256,
                members_sha,
                receipt_sha: Digest256::of_bytes(&receipt_raw),
                manifest_size,
                members_size,
                receipt_size,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Self {
            packs,
            entries,
            deadline,
            read_bytes: Cell::new(add(external_read, direct_read as u64)?),
            read_limit: limits.max_read_bytes,
            retained_state: usize::try_from(retained).map_err(invalid)?,
            peak_state: usize::try_from(peak).map_err(invalid)?,
        })
    }
    pub(crate) fn original_rows(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|e| &e.row)
    }
    /// Successfully consumed transport compressed/decoded/metadata bytes plus
    /// exact reader/helper metadata reads, never configured caps as measurements.
    pub(crate) fn read_bytes(&self) -> u64 {
        self.read_bytes.get()
    }
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.retained_state
    }
    pub(crate) fn state_bytes(&self) -> usize {
        self.peak_state
    }
    /// Historical rows must remain outside the actual current candidate.
    /// Each presence query is fenced by the same input and charged by it.
    pub(crate) fn verify_candidate_input(
        &self,
        input: &dyn tos_validation::record_biblio_cut::SourceCutInput,
        coverage: &tos_validation::record_biblio_cut::SourceCutInputCoverage,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        input
            .verify_current_fence(coverage, self.deadline, cancelled)
            .map_err(|_| invalid("historical candidate fence refused"))?;
        for entry in &self.entries {
            active(self.deadline, cancelled)?;
            if input
                .path_presence(&entry.path, self.deadline, cancelled)
                .map_err(|_| invalid("historical candidate presence refused"))?
                == Some(tos_source_store::SourcePresenceV1::File)
            {
                return Err(invalid("historical validation evidence overlaps source"));
            }
        }
        input
            .verify_current_fence(coverage, self.deadline, cancelled)
            .map_err(|_| invalid("historical candidate fence refused"))?;
        active(self.deadline, cancelled)
    }
    pub(crate) fn verify_candidate(&self, candidate: &Candidate<'_>) -> io::Result<()> {
        for e in &self.entries {
            candidate.tick()?;
            if candidate.members.contains_key(&e.path) {
                return Err(invalid("historical validation evidence overlaps source"));
            }
        }
        Ok(())
    }
    /// Final identity/custody check also runs for identity-only and unchanged
    /// admission paths. Retained rows are already charged by the caller.
    pub(crate) fn recheck(
        &mut self,
        remaining_read: u64,
        remaining_state: usize,
        cancel: &AtomicBool,
    ) -> io::Result<u64> {
        active(self.deadline, cancel)?;
        let cap = usize::try_from(remaining_read).map_err(invalid)?;
        let mut read = 0usize;
        let result = (|| {
            for p in &mut self.packs {
                p.capture.verify_root()?;
                p.restored.verify_root()?;
                for (path, size, sha) in [
                    ("capture.json", p.manifest_size, p.manifest_sha),
                    ("members.jsonl", p.members_size, p.members_sha),
                    ("restore-receipt.json", p.receipt_size, p.receipt_sha),
                ] {
                    let sources = if path == "restore-receipt.json" {
                        &mut p.restored
                    } else {
                        &mut p.capture
                    };
                    active(self.deadline, cancel)?;
                    if size.checked_add(32768).is_none_or(|n| n > remaining_state) {
                        return Err(invalid(
                            "historical identity recheck transient state bound exceeded",
                        ));
                    }
                    let bytes =
                        bounded(sources, path, size, &mut read, cap, self.deadline, cancel)?;
                    if bytes.len() != size || Digest256::of_bytes(&bytes) != sha {
                        return Err(invalid("historical evidence control bytes changed"));
                    }
                }
                p.capture.verify_root()?;
                p.restored.verify_root()?;
            }
            active(self.deadline, cancel)?;
            Ok(read as u64)
        })();
        self.read_bytes
            .set(add(self.read_bytes.get(), read as u64)?);
        result
    }
    pub(crate) fn binding(&self, path: &str) -> Option<&Value> {
        self.entries
            .binary_search_by(|e| e.path.as_str().cmp(path))
            .ok()
            .map(|n| &self.entries[n].row)
    }
    /// Direct readonly evidence for the actual FND source provider. `expected`
    /// is the recorded lookup's digest; None is an exact current-path lookup.
    /// The captured bytes always match their sealed row in either case.
    pub(crate) fn read_recorded(
        &mut self,
        path: &str,
        expected: Option<Digest256>,
        max_member_bytes: usize,
        remaining_read: u64,
        remaining_state: usize,
        cancel: &AtomicBool,
    ) -> io::Result<Option<HistoryRead>> {
        active(self.deadline, cancel)?;
        let Ok(n) = self.entries.binary_search_by(|e| e.path.as_str().cmp(path)) else {
            return Ok(None);
        };
        let e = &self.entries[n];
        if expected.is_some_and(|digest| digest != e.sha256) {
            return Ok(None);
        }
        if e.size > max_member_bytes
            || e.size
                .checked_add(32768)
                .is_none_or(|n| n > remaining_state)
        {
            return Err(invalid(
                "historical member exceeds remaining read/state profile",
            ));
        }
        let cap = usize::try_from(remaining_read).map_err(invalid)?;
        let mut read = 0usize;
        let result = (|| {
            let p = &mut self.packs[e.pack];
            p.capture.verify_root()?;
            p.restored.verify_root()?;
            for (path, size, sha) in [
                ("capture.json", p.manifest_size, p.manifest_sha),
                ("members.jsonl", p.members_size, p.members_sha),
                ("restore-receipt.json", p.receipt_size, p.receipt_sha),
            ] {
                let sources = if path == "restore-receipt.json" {
                    &mut p.restored
                } else {
                    &mut p.capture
                };
                active(self.deadline, cancel)?;
                if size.checked_add(32768).is_none_or(|n| n > remaining_state) {
                    return Err(invalid(
                        "historical control recheck exceeds transient state profile",
                    ));
                }
                let bytes = bounded(sources, path, size, &mut read, cap, self.deadline, cancel)?;
                if bytes.len() != size || Digest256::of_bytes(&bytes) != sha {
                    return Err(invalid("historical evidence control bytes changed"));
                }
            }
            let bytes = bounded(
                &mut p.restored,
                &e.path,
                e.size,
                &mut read,
                cap,
                self.deadline,
                cancel,
            )?;
            if bytes.len() != e.size || Digest256::of_bytes(&bytes) != e.sha256 {
                return Err(invalid(
                    "historical validation evidence is missing or changed",
                ));
            }
            p.capture.verify_root()?;
            p.restored.verify_root()?;
            active(self.deadline, cancel)?;
            Ok(Some(HistoryRead {
                bytes,
                read_bytes: read as u64,
            }))
        })();
        self.read_bytes
            .set(add(self.read_bytes.get(), read as u64)?);
        result
    }
}

impl crate::source_current_cut::foundation_reader::FoundationHistoricalEvidence
    for HistoryEvidence
{
    fn selected(&self, path: &str) -> bool {
        self.binding(path).is_some()
    }
    fn read(
        &mut self,
        path: &str,
        expected_digest: Option<&str>,
        max_bytes: usize,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<Option<Vec<u8>>> {
        active(deadline.min(self.deadline), cancelled)?;
        let expected = match expected_digest {
            Some(d) => match Digest256::from_hex(d) {
                Ok(d) => Some(d),
                Err(_) => return Ok(None),
            },
            None => None,
        };
        let remaining = self
            .read_limit
            .checked_sub(self.read_bytes.get())
            .ok_or_else(|| invalid("historical lifetime read allowance exhausted"))?;
        let original = self.deadline;
        self.deadline = self.deadline.min(deadline);
        let result = self.read_recorded(
            path,
            expected,
            max_bytes,
            remaining,
            max_state_bytes,
            cancelled,
        );
        self.deadline = original;
        let result = result?.map(|read| read.bytes);
        active(deadline.min(self.deadline), cancelled)?;
        Ok(result)
    }
    fn physical(
        &mut self,
        path: &str,
        max_read_bytes: u64,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<Option<tos_validation::source_foundation_discovery::PhysicalPathFacts>> {
        let deadline = deadline.min(self.deadline);
        active(deadline, cancelled)?;
        let Ok(n) = self.entries.binary_search_by(|e| e.path.as_str().cmp(path)) else {
            return Ok(None);
        };
        if max_state_bytes < 32768 {
            return Err(invalid(
                "historical physical stream workspace exceeds remaining state",
            ));
        }
        let e = &self.entries[n];
        let expected = e.sha256;
        let size = e.size;
        let remaining = self
            .read_limit
            .checked_sub(self.read_bytes.get())
            .ok_or_else(|| invalid("historical lifetime read allowance exhausted"))?
            .min(max_read_bytes);
        let mut read = 0u64;
        let mut hash = Digest256Hasher::new();
        let result = self.packs[e.pack].restored.stream_regular(
            path,
            size as u64,
            &mut read,
            remaining,
            |chunk| {
                active(deadline, cancelled)?;
                hash.update(chunk);
                Ok(())
            },
        );
        self.read_bytes.set(add(self.read_bytes.get(), read)?);
        let metadata =
            result?.ok_or_else(|| invalid("historical evidence physical binding disappeared"))?;
        if metadata.len() != size as u64 || hash.finalize() != expected {
            return Err(invalid(
                "historical physical bytes differ from sealed evidence",
            ));
        }
        active(deadline, cancelled)?;
        Ok(Some(
            crate::source_current_cut::foundation_physical::physical_from_metadata(
                &metadata,
                Some(expected.to_hex()),
            ),
        ))
    }
    fn recheck(
        &mut self,
        max_read_bytes: u64,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        active(deadline.min(self.deadline), cancelled)?;
        let remaining = self
            .read_limit
            .checked_sub(self.read_bytes.get())
            .ok_or_else(|| invalid("historical lifetime read allowance exhausted"))?
            .min(max_read_bytes);
        let original = self.deadline;
        self.deadline = self.deadline.min(deadline);
        let result = HistoryEvidence::recheck(self, remaining, max_state_bytes, cancelled);
        self.deadline = original;
        result?;
        active(deadline.min(self.deadline), cancelled)?;
        Ok(())
    }
    fn usage(&self) -> (u64, usize) {
        (self.read_bytes(), self.retained_state_bytes())
    }
}
