//! Native source-validator identity over the explicitly selected grammar root.
//! Software hashes are actual executing native/worker hashes supplied by the
//! program's held executable custody, never historical Python substitutions.
//! This grammar closure is not full source validation or an admission grant.
//! Historical identity input must come from the authentic capture verifier.
//! Binding its rows here does not verify or supply those historical bytes.
use super::source_admission::{active, invalid};
use super::source_admission_candidate::Candidate;
use crate::source_current_cut::foundation_cli::{
    VALIDATION_PROFILE_DECLARATION, ValidationProfile,
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json,
};
use tos_ops_mechanics_plan::route_cards::{RouteRootCustody, RouteSources};

const PREFIXES: [&str; 2] = ["ToS/contracts", "ToS/doctrine/semantic-interchange"];
const DISCOVERY_SCRATCH: usize = 8192;
#[derive(Clone, Copy)]
pub(crate) struct IdentityLimits {
    pub max_read_bytes: u64,
    pub max_member_bytes: usize,
    pub max_members: usize,
    pub max_state_bytes: usize,
    pub max_discovery_entries: usize,
}
impl IdentityLimits {
    fn validate(self) -> io::Result<Self> {
        if self.max_read_bytes == 0
            || self.max_read_bytes == u64::MAX
            || usize::try_from(self.max_read_bytes).is_err()
            || self.max_member_bytes == 0
            || self.max_member_bytes == usize::MAX
            || self.max_members == 0
            || self.max_members == usize::MAX
            || self.max_state_bytes < DISCOVERY_SCRATCH
            || self.max_state_bytes == usize::MAX
            || self.max_discovery_entries == 0
            || self.max_discovery_entries == usize::MAX
        {
            return Err(invalid("invalid native grammar identity limits"));
        }
        Ok(self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GrammarBinding {
    pub path: String,
    pub sha256: Digest256,
    pub size_bytes: u64,
}
pub(crate) struct GrammarIdentity {
    bindings: Vec<GrammarBinding>,
    digest: Digest256,
    limits: IdentityLimits,
    deadline: Instant,
    read_bytes: u64,
    state_bytes: usize,
    retained_state_bytes: usize,
    root_custody: RouteRootCustody,
}
fn grammar(path: &str) -> bool {
    path != tos_validation::source_record_selection::SELECTION_SCHEMA_PATH
        && path != VALIDATION_PROFILE_DECLARATION
        && path.ends_with(".json")
        && PREFIXES.iter().any(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|tail| tail.starts_with('/'))
        })
}
fn reserve(state: &mut usize, amount: usize, cap: usize) -> io::Result<()> {
    *state = state
        .checked_add(amount)
        .filter(|n| *n <= cap)
        .ok_or_else(|| invalid("native grammar identity state bound exceeded"))?;
    Ok(())
}
/// Existing RouteSources allocates at most one validated 4096-byte path before
/// its eligibility callback. That fixed scratch is reserved first. Callback
/// precharges every encountered name before children/set/vector retention;
/// budget/cancellation refusal is terminal, never a partial/pruned identity.
fn discover(
    sources: &mut RouteSources,
    limits: IdentityLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<(Vec<String>, usize)> {
    active(deadline, cancel)?;
    sources.verify_root()?;
    let state = Cell::new(DISCOVERY_SCRATCH);
    let entries = Cell::new(0usize);
    let failed = Cell::new(false);
    let selected = |path: &str, _directory: bool| {
        if failed.get() {
            return false;
        }
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            failed.set(true);
            return false;
        }
        let entry = entries.get().checked_add(1);
        let amount = path.len().checked_mul(3).and_then(|n| n.checked_add(256));
        let next = amount.and_then(|n| state.get().checked_add(n));
        if entry.is_none_or(|n| n > limits.max_discovery_entries)
            || next.is_none_or(|n| n > limits.max_state_bytes)
        {
            failed.set(true);
            return false;
        }
        entries.set(entry.unwrap_or(usize::MAX));
        state.set(next.unwrap_or(usize::MAX));
        true
    };
    let mut paths = Vec::new();
    for prefix in PREFIXES {
        // Select all names only inside these two grammar districts, so linked
        // directories/non-JSON names cannot hide an unsupported grammar home.
        let discovered = sources.selected_paths(prefix, &selected)?;
        active(deadline, cancel)?;
        if failed.get() {
            return Err(invalid(
                "native grammar identity discovery/state bound exceeded",
            ));
        }
        for path in discovered {
            if grammar(&path) {
                if paths.len() >= limits.max_members {
                    return Err(invalid("native grammar identity member bound exceeded"));
                }
                paths.push(path);
            }
        }
    }
    sources.verify_root()?;
    active(deadline, cancel)?;
    paths.sort();
    if paths.windows(2).any(|v| v[0] == v[1]) {
        return Err(invalid("duplicate native grammar path"));
    }
    if !paths.iter().any(|p| p.starts_with("ToS/contracts/")) {
        return Err(invalid("source snapshot has no schema contracts"));
    }
    Ok((paths, state.get()))
}
impl GrammarIdentity {
    /// Borrow the SAME held primary RouteSources used by the full FND hook.
    /// Do not construct/reset a source operation allowance for identity work.
    pub(crate) fn select(
        sources: &mut RouteSources,
        executable: Digest256,
        worker: Digest256,
        limits: IdentityLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        profile: ValidationProfile,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        if deadline > sources.deadline() {
            return Err(invalid(
                "native grammar identity extends primary route deadline",
            ));
        }
        let (paths, mut state) = discover(sources, limits, deadline, cancel)?;
        let mut peak_state = state;
        let mut read_bytes = 0usize;
        let mut bindings = Vec::new();
        let mut retained_state = 256usize;
        for path in paths {
            active(deadline, cancel)?;
            reserve(
                &mut state,
                path.len()
                    .checked_add(128)
                    .ok_or_else(|| invalid("identity state overflow"))?,
                limits.max_state_bytes,
            )?;
            reserve(
                &mut retained_state,
                path.len()
                    .checked_add(128)
                    .ok_or_else(|| invalid("identity state overflow"))?,
                limits.max_state_bytes,
            )?;
            let metadata = sources
                .metadata(&path)?
                .ok_or_else(|| invalid("grammar source disappeared during identity selection"))?;
            if !metadata.is_file() {
                return Err(invalid("source grammar JSON is not a regular file"));
            }
            let size = usize::try_from(metadata.len()).map_err(invalid)?;
            if size > limits.max_member_bytes {
                return Err(invalid("grammar source exceeds member byte bound"));
            }
            let mut raw_state = state;
            reserve(&mut raw_state, size, limits.max_state_bytes)?;
            peak_state = peak_state.max(raw_state);
            let raw = sources.bounded_bytes(
                &path,
                size,
                &mut read_bytes,
                limits.max_read_bytes as usize,
            )?;
            active(deadline, cancel)?;
            bindings.push(GrammarBinding {
                path,
                sha256: Digest256::of_bytes(&raw),
                size_bytes: raw.len() as u64,
            });
            drop(raw);
        }
        sources.verify_root()?;
        // Charge the native identity Value, UTF-8 encoding, FND JSON tree and
        // canonical buffer BEFORE constructing them. This is conservative
        // logical payload accounting; actual allocator/descriptor RSS is extra.
        let mut serialization_state = state;
        reserve(&mut serialization_state, 2048, limits.max_state_bytes)?;
        // Additional software-profile fields coexist in the serde Value/raw,
        // FND UTF-16/UTF-8 tree and canonical buffer just like grammar strings.
        let profile_state = profile
            .id
            .len()
            .checked_add(64)
            .and_then(|n| n.checked_mul(24))
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| invalid("profile identity serialization state overflow"))?;
        reserve(
            &mut serialization_state,
            profile_state,
            limits.max_state_bytes,
        )?;

        for binding in &bindings {
            reserve(
                &mut serialization_state,
                binding
                    .path
                    .len()
                    .checked_mul(24)
                    .and_then(|n| n.checked_add(4096))
                    .ok_or_else(|| invalid("identity serialization state overflow"))?,
                limits.max_state_bytes,
            )?;
        }
        peak_state = peak_state.max(serialization_state);
        let value = json!({"domain":"tos_native_source_validator_v1","executable_sha256":executable.to_hex(),"worker_sha256":worker.to_hex(),"validation_profile_id":profile.id,"validation_profile_declaration_sha256":profile.declaration_sha256.to_hex(),"grammar":bindings.iter().map(|b|json!({"path":b.path,"sha256":b.sha256.to_hex(),"size_bytes":b.size_bytes})).collect::<Vec<_>>()});
        let raw = serde_json::to_vec(&value).map_err(invalid)?;
        let json_limits = JsonLimits::new(
            limits.max_state_bytes,
            16,
            bindings
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(64))
                .ok_or_else(|| invalid("identity JSON visit overflow"))?,
            20,
        )
        .map_err(invalid)?;
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, json_limits).map_err(invalid)?;
        let canonical = canonical_bytes_v1(
            parsed.root(),
            CanonicalProfile::CorpusSnapshotV1,
            json_limits,
        )
        .map_err(invalid)?;
        active(deadline, cancel)?;
        let result = Self {
            bindings,
            digest: Digest256::of_bytes(&canonical),
            limits,
            deadline,
            read_bytes: read_bytes as u64,
            state_bytes: peak_state,
            retained_state_bytes: retained_state,
            root_custody: sources.root_custody(),
        };
        sources.verify_root()?;
        Ok(result)
    }
    /// Bind the owner's exact record/slot selection separately from physical
    /// grammar membership and its EOF proof. Existing profiles keep their digest.
    pub(crate) fn bind_record_selection(
        &mut self,
        binding: &Value,
        remaining_state: usize,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        self.bind_selection_output(
            "record_selection",
            remaining_state,
            cancel,
            |writer, _cap| serde_json::to_writer(writer, binding).map_err(invalid),
        )
        .map(|_| ())
    }

    pub(crate) fn bind_generated_input(
        &mut self,
        declaration: &crate::source_admission_indexed_input::HeldIndexedInputDeclarationV1,
        remaining_state: usize,
        cancel: &AtomicBool,
    ) -> io::Result<Vec<u8>> {
        let deadline = self.deadline;
        self.bind_selection_output(
            "generated_input",
            remaining_state,
            cancel,
            |mut writer, cap| {
                declaration
                    .write_identity_binding_v1(&mut writer, cap, deadline, cancel)
                    .map(|_| ())
            },
        )
    }

    fn bind_selection_output(
        &mut self,
        key: &'static str,
        remaining_state: usize,
        cancel: &AtomicBool,
        mut write_binding: impl FnMut(&mut dyn io::Write, usize) -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        struct Count<'a> {
            bytes: usize,
            cap: usize,
            deadline: Instant,
            cancel: &'a AtomicBool,
        }
        impl io::Write for Count<'_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                active(self.deadline, self.cancel)?;
                self.bytes = self
                    .bytes
                    .checked_add(bytes.len())
                    .filter(|n| *n <= self.cap)
                    .ok_or_else(|| invalid("record selection identity state bound"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                active(self.deadline, self.cancel)
            }
        }
        let cap = remaining_state
            .checked_sub(2048)
            .ok_or_else(|| invalid("record selection identity state bound"))?
            / 24;
        let mut count = Count {
            bytes: 0,
            cap,
            deadline: self.deadline,
            cancel,
        };
        let digest = self.digest.to_hex();
        let mut emit = |writer: &mut dyn io::Write| -> io::Result<()> {
            writer.write_all(b"{\"grammar_validator\":\"")?;
            writer.write_all(digest.as_bytes())?;
            writer.write_all(b"\",")?;
            serde_json::to_writer(&mut *writer, key).map_err(invalid)?;
            writer.write_all(b":")?;
            write_binding(writer, cap)?;
            writer.write_all(b"}")
        };
        emit(&mut count)?;
        // CorpusSnapshotV1 owns a terminal LF beyond serde's compact envelope.
        // Keep that byte inside the same caller-derived serialization cap.
        let canonical_bytes = count
            .bytes
            .checked_add(1)
            .filter(|bytes| *bytes <= cap)
            .ok_or_else(|| invalid("record selection identity state bound"))?;
        let limits = JsonLimits::new(
            canonical_bytes,
            32,
            count
                .bytes
                .checked_mul(2)
                .ok_or_else(|| invalid("record selection identity visit bound"))?,
            20,
        )
        .map_err(invalid)?;
        let mut raw = Vec::new();
        raw.try_reserve_exact(count.bytes).map_err(invalid)?;
        emit(&mut raw)?;
        if raw.len() != count.bytes {
            return Err(invalid("selection identity serialization differs"));
        }
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(invalid)?;
        let canonical =
            canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, limits)
                .map_err(invalid)?;
        active(self.deadline, cancel)?;
        self.digest = Digest256::of_bytes(&canonical);
        Ok(canonical)
    }

    /// Preserve the maintained canonical envelope while streaming one exact
    /// authenticated member row at a time. No full history Value/Vec is cloned.
    pub(crate) fn bind_history<'r>(
        &self,
        rows: impl Iterator<Item = &'r Value>,
        remaining_state: usize,
        cancel: &AtomicBool,
    ) -> io::Result<Digest256> {
        active(self.deadline, cancel)?;
        let mut rows = rows.peekable();
        if rows.peek().is_none() {
            return Ok(self.digest);
        }
        let mut hash = Digest256Hasher::new();
        hash.update(b"{\"grammar_validator\":\"");
        hash.update(self.digest.to_hex().as_bytes());
        hash.update(b"\",\"historical_evidence\":[");
        let mut last: Option<&str> = None;
        for row in rows {
            active(self.deadline, cancel)?;
            let object = row
                .as_object()
                .ok_or_else(|| invalid("historical identity requires exact member rows"))?;
            let keys = ["path", "git_blob_oid", "size_bytes", "sha256", "mode"];
            if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
                return Err(invalid("historical identity member fields differ"));
            }
            let path = row["path"]
                .as_str()
                .ok_or_else(|| invalid("historical identity path must be text"))?;
            if last.is_some_and(|previous| previous >= path) {
                return Err(invalid(
                    "historical identity paths are not unique and sorted",
                ));
            }
            // At most six JSON bytes per input UTF-8 byte, plus scalar syntax.
            // Flat capture rows only: recursive or arbitrary JSON is refused.
            let mut encoded_bound = 128usize;
            for (key, value) in object {
                let scalar_bytes = match value {
                    Value::String(text) => text.len(),
                    Value::Number(number) if number.as_u64().is_some() => 20,
                    _ => return Err(invalid("historical identity member scalar differs")),
                };
                encoded_bound = encoded_bound
                    .checked_add(key.len())
                    .and_then(|n| n.checked_add(scalar_bytes.checked_mul(6)?))
                    .ok_or_else(|| invalid("historical identity row cost overflow"))?;
            }
            let workspace = encoded_bound
                .checked_mul(16)
                .and_then(|n| n.checked_add(2048))
                .ok_or_else(|| invalid("historical identity state overflow"))?;
            if workspace > remaining_state {
                return Err(invalid("historical identity row exceeds remaining state"));
            }
            let raw = serde_json::to_vec(row).map_err(invalid)?;
            let limits = JsonLimits::new(encoded_bound, 4, 32, 20).map_err(invalid)?;
            let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(invalid)?;
            let canonical =
                canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, limits)
                    .map_err(invalid)?;
            let canonical = canonical
                .strip_suffix(b"\n")
                .ok_or_else(|| invalid("historical canonical row lacks terminal newline"))?;
            if last.is_some() {
                hash.update(b",");
            }
            hash.update(canonical);
            last = Some(path);
        }
        hash.update(b"]}\n");
        active(self.deadline, cancel)?;
        Ok(hash.finalize())
    }
    pub(crate) fn digest(&self) -> Digest256 {
        self.digest
    }
    pub(crate) fn read_bytes(&self) -> u64 {
        self.read_bytes
    }
    /// Conservatively charged peak payload, not measured process RSS.
    pub(crate) fn state_bytes(&self) -> usize {
        self.state_bytes
    }
    /// Permanent binding payload charged while Foundation retains this identity.
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }
    pub(crate) fn bindings(&self) -> &[GrammarBinding] {
        &self.bindings
    }
    /// Recheck original grammar membership and bytes using the SAME primary
    /// operation allowance. `remaining_read` is the caller's real remainder,
    /// not a reset of the whole admission read budget. `remaining_state_bytes`
    /// excludes these retained bindings, already charged by the caller.
    pub(crate) fn recheck(
        &self,
        sources: &mut RouteSources,
        remaining_read: u64,
        remaining_state_bytes: usize,
        cancel: &AtomicBool,
    ) -> io::Result<u64> {
        active(self.deadline, cancel)?;
        sources.verify_custody(&self.root_custody)?;
        let cap =
            usize::try_from(remaining_read.min(self.limits.max_read_bytes)).map_err(invalid)?;
        let discovery_limits = IdentityLimits {
            max_state_bytes: self.limits.max_state_bytes.min(remaining_state_bytes),
            ..self.limits
        }
        .validate()?;
        let (paths, state) = discover(sources, discovery_limits, self.deadline, cancel)?;
        if paths.len() != self.bindings.len()
            || paths.iter().zip(&self.bindings).any(|(p, b)| p != &b.path)
        {
            return Err(invalid(
                "source grammar membership changed during native validation",
            ));
        }
        let mut read_bytes = 0usize;
        for binding in &self.bindings {
            active(self.deadline, cancel)?;
            // The caller has already charged retained rows. Only this discovery
            // set and one bounded raw file consume its remaining state quota.
            let size = usize::try_from(binding.size_bytes).map_err(invalid)?;
            if state
                .checked_add(size)
                .is_none_or(|n| n > discovery_limits.max_state_bytes)
            {
                return Err(invalid("grammar recheck state bound exceeded"));
            }
            let raw = sources.bounded_bytes(&binding.path, size, &mut read_bytes, cap)?;
            if raw.len() as u64 != binding.size_bytes || Digest256::of_bytes(&raw) != binding.sha256
            {
                return Err(invalid(
                    "source grammar bytes changed during native validation",
                ));
            }
        }
        sources.verify_custody(&self.root_custody)?;
        active(self.deadline, cancel)?;
        Ok(read_bytes as u64)
    }
    /// Verify the same selected grammar against the actual borrowed candidate
    /// bytes. The input owns its read budget and fence; no revision is minted.
    pub(crate) fn verify_candidate_input(
        &self,
        input: &dyn tos_validation::record_biblio_cut::SourceCutInput,
        cancelled: &AtomicBool,
    ) -> io::Result<tos_validation::record_biblio_cut::SourceCutInputCoverage> {
        active(self.deadline, cancelled)?;
        let mut count = 0usize;
        let mut failure = None;
        let coverage = input.for_each_current_member(self.deadline, cancelled, &mut |meta, raw| {
            let result = (|| {
                active(self.deadline, cancelled)?;
                if !grammar(meta.path) {
                    return Ok(());
                }
                // The authentic candidate traverses paths in byte order. Match
                // the next selected binding, so duplicate callbacks cannot
                // replace an omitted path while preserving the total count.
                let binding = self
                    .bindings
                    .get(count)
                    .filter(|binding| binding.path.as_str() == meta.path)
                    .ok_or_else(|| {
                        invalid("candidate grammar differs from selected native validator grammar")
                    })?;
                if raw.len() > self.limits.max_member_bytes
                    || meta.size_bytes != raw.len() as u64
                    || meta.size_bytes != binding.size_bytes
                    || Digest256::of_bytes(raw) != binding.sha256
                {
                    return Err(invalid(
                        "candidate grammar bytes differ from selected native validator grammar",
                    ));
                }
                count = count
                    .checked_add(1)
                    .ok_or_else(|| invalid("candidate grammar count overflow"))?;
                Ok(())
            })();
            result.map_err(|error| {
                failure = Some(error);
                tos_validation::item_rules::ItemRefusal::Source(
                    "native candidate grammar refused".into(),
                )
            })
        });
        if let Some(error) = failure {
            return Err(error);
        }
        let coverage = coverage.map_err(|_| invalid("native candidate grammar input refused"))?;
        if count != self.bindings.len() {
            return Err(invalid(
                "candidate grammar differs from selected native validator grammar",
            ));
        }
        input
            .verify_current_fence(&coverage, self.deadline, cancelled)
            .map_err(|_| invalid("native candidate grammar fence refused"))?;
        active(self.deadline, cancelled)?;
        Ok(coverage)
    }
    /// Candidate reads charge the actual admission ledger and become touched
    /// paths, therefore publication re-verifies them before CAS as well.
    pub(crate) fn verify_candidate(&self, candidate: &Candidate<'_>) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            return Err(invalid("native grammar identity deadline exceeded"));
        }
        candidate.tick()?;
        let mut count = 0usize;
        for path in candidate.members.keys() {
            candidate.tick()?;
            if grammar(path) {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| invalid("candidate grammar count overflow"))?;
                if self
                    .bindings
                    .binary_search_by(|b| b.path.as_str().cmp(path.as_str()))
                    .is_err()
                {
                    return Err(invalid(
                        "candidate grammar differs from selected native validator grammar",
                    ));
                }
            }
        }
        if count != self.bindings.len() {
            return Err(invalid(
                "candidate grammar differs from selected native validator grammar",
            ));
        }
        for binding in &self.bindings {
            candidate.tick()?;
            let raw = candidate.read(&binding.path, self.limits.max_member_bytes)?;
            if raw.len() as u64 != binding.size_bytes || Digest256::of_bytes(&raw) != binding.sha256
            {
                return Err(invalid(
                    "candidate grammar bytes differ from selected native validator grammar",
                ));
            }
        }
        candidate.tick()?;
        if Instant::now() >= self.deadline {
            return Err(invalid("native grammar identity deadline exceeded"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::Duration};
    #[test]
    fn exact_native_grammar_selector_and_recheck_refusals() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("ToS/contracts/nested")).unwrap();
        fs::create_dir_all(tmp.path().join("ToS/doctrine/semantic-interchange")).unwrap();
        fs::create_dir_all(tmp.path().join("unselected")).unwrap();
        fs::write(tmp.path().join("ToS/contracts/a.json"), b"{}\n").unwrap();
        fs::write(
            tmp.path().join("ToS/contracts/nested/b.json"),
            b"{\"type\":\"object\"}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("ToS/contracts/ignored.txt"),
            b"ignored grammar bytes",
        )
        .unwrap();
        fs::write(
            tmp.path()
                .join("ToS/doctrine/semantic-interchange/registry.json"),
            b"[]\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("unselected/outside.json"),
            b"outside selected districts",
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let limits = IdentityLimits {
            max_read_bytes: 65536,
            max_member_bytes: 4096,
            max_members: 16,
            max_state_bytes: 262144,
            max_discovery_entries: 64,
        };
        let executable = Digest256::of_bytes(b"native executable");
        let worker = Digest256::of_bytes(b"native schema worker");
        let mut sources = RouteSources::new_until(tmp.path(), deadline).unwrap();
        let identity = GrammarIdentity::select(
            &mut sources,
            executable,
            worker,
            limits,
            deadline,
            &cancel,
            crate::source_current_cut::foundation_cli::select_validation_profile(None).unwrap(),
        )
        .unwrap();
        assert_eq!(
            identity
                .bindings()
                .iter()
                .map(|b| b.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ToS/contracts/a.json",
                "ToS/contracts/nested/b.json",
                "ToS/doctrine/semantic-interchange/registry.json"
            ]
        );
        assert_eq!(identity.read_bytes(), 3 + 18 + 3);
        assert!(identity.state_bytes() <= limits.max_state_bytes);
        // The streamed historical identity preserves the maintained canonical
        // envelope, including Unicode, key ordering and its one terminal LF.
        let rows = [
            json!({"path":"scripts/a.py","git_blob_oid":"1".repeat(40),
                "size_bytes":3,"sha256":"2".repeat(64),"mode":33188}),
            json!({"path":"scripts/файл.py","git_blob_oid":"3".repeat(40),
                "size_bytes":7,"sha256":"4".repeat(64),"mode":33261}),
        ];
        let envelope = json!({"grammar_validator":identity.digest().to_hex(),
            "historical_evidence":rows});
        let encoded = serde_json::to_vec(&envelope).unwrap();
        let json_limits = JsonLimits::default();
        let parsed = parse_json(&encoded, JsonMode::PublishedStrict, json_limits).unwrap();
        let canonical = canonical_bytes_v1(
            parsed.root(),
            CanonicalProfile::CorpusSnapshotV1,
            json_limits,
        )
        .unwrap();
        assert_eq!(
            identity
                .bind_history(rows.iter(), limits.max_state_bytes, &cancel)
                .unwrap(),
            Digest256::of_bytes(&canonical)
        );
        assert_eq!(
            identity
                .bind_history(std::iter::empty(), 1, &cancel)
                .unwrap(),
            identity.digest()
        );
        assert!(identity.bind_history(rows.iter(), 1, &cancel).is_err());
        assert!(
            identity
                .bind_history(rows.iter().rev(), limits.max_state_bytes, &cancel)
                .is_err()
        );

        let mut replacement = RouteSources::new_until(tmp.path(), deadline).unwrap();
        assert!(
            identity
                .recheck(
                    &mut replacement,
                    limits.max_read_bytes,
                    limits.max_state_bytes,
                    &cancel
                )
                .is_err()
        );
        assert_eq!(
            identity
                .recheck(
                    &mut sources,
                    limits.max_read_bytes,
                    limits.max_state_bytes,
                    &cancel
                )
                .unwrap(),
            identity.read_bytes()
        );
        let mut selected_profile = GrammarIdentity::select(
            &mut sources,
            executable,
            worker,
            limits,
            deadline,
            &cancel,
            crate::source_current_cut::foundation_cli::select_validation_profile(Some(
                "selected-source-closure",
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(identity.bindings(), selected_profile.bindings());
        assert_ne!(identity.digest(), selected_profile.digest());
        let before_selection = selected_profile.digest();
        assert!(
            selected_profile
                .bind_record_selection(&json!("selected-record-a"), 64, &cancel)
                .is_err()
        );
        assert_eq!(before_selection, selected_profile.digest());
        selected_profile
            .bind_record_selection(&json!("selected-record-a"), limits.max_state_bytes, &cancel)
            .unwrap();
        assert_ne!(before_selection, selected_profile.digest());
        assert!(!grammar(
            tos_validation::source_record_selection::SELECTION_SCHEMA_PATH
        ));

        // The compiled declaration is not required at the selected immutable source root.
        assert!(!tmp.path().join(VALIDATION_PROFILE_DECLARATION).exists());
        let other = GrammarIdentity::select(
            &mut sources,
            Digest256::of_bytes(b"different executing ELF"),
            worker,
            limits,
            deadline,
            &cancel,
            crate::source_current_cut::foundation_cli::select_validation_profile(None).unwrap(),
        )
        .unwrap();
        assert_ne!(identity.digest(), other.digest());
        fs::write(tmp.path().join("ToS/contracts/a.json"), b"[]\n").unwrap();
        assert!(
            identity
                .recheck(
                    &mut sources,
                    limits.max_read_bytes,
                    limits.max_state_bytes,
                    &cancel
                )
                .is_err()
        );
        fs::write(tmp.path().join("ToS/contracts/a.json"), b"{}\n").unwrap();
        fs::write(tmp.path().join("ToS/contracts/new.json"), b"{}\n").unwrap();
        assert!(
            identity
                .recheck(
                    &mut sources,
                    limits.max_read_bytes,
                    limits.max_state_bytes,
                    &cancel
                )
                .is_err()
        );
        fs::remove_file(tmp.path().join("ToS/contracts/new.json")).unwrap();
        assert!(
            GrammarIdentity::select(
                &mut sources,
                executable,
                worker,
                IdentityLimits {
                    max_discovery_entries: 1,
                    ..limits
                },
                deadline,
                &cancel,
                crate::source_current_cut::foundation_cli::select_validation_profile(None).unwrap()
            )
            .is_err()
        );
        std::os::unix::fs::symlink(
            tmp.path().join("unselected"),
            tmp.path().join("ToS/contracts/linked"),
        )
        .unwrap();
        assert!(
            GrammarIdentity::select(
                &mut sources,
                executable,
                worker,
                limits,
                deadline,
                &cancel,
                crate::source_current_cut::foundation_cli::select_validation_profile(None).unwrap()
            )
            .is_err()
        );
    }
}
