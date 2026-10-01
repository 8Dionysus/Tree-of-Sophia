//! One immutable candidate for the maintained native corpus admission call.
//! Full foundation evaluation and index construction precede `publish`.
use super::source_admission::{AdmissionBatch, AdmissionLimits, active, invalid};
use super::source_admission_index::{Index, MAX_EVENT_BYTES};
use super::source_admission_store::AdmissionStore;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    mem::size_of,
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, SourceRevision, canonical_bytes_v1,
    parse_json,
};
use tos_source_store::{ReadLimits, Snapshot};

#[derive(Clone, Copy)]
pub(crate) struct CandidateLimits {
    pub admission: AdmissionLimits,
    pub reader: ReadLimits,
    pub max_state_bytes: usize,
    pub max_history_revisions: usize,
    pub max_history_identities: usize,
    pub max_read_bytes: u64,
    pub max_write_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, fs::File, time::Duration};

    /// Physical candidate preparation, not a substitute for source validation.
    /// The maintained ledger omits target size although custody needs it.
    #[test]
    fn retirement_preparation_keeps_target_size_out_of_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let store = AdmissionStore::create(&tmp.path().join("store"), deadline, &cancel).unwrap();
        let json = JsonLimits::new(16384, 32, 4096, 4300).unwrap();
        let admission = AdmissionLimits {
            max_batch_bytes: 16384,
            max_members: 32,
            max_member_bytes: 1024,
            max_source_bytes: 4096,
            json,
        };
        let reader = ReadLimits {
            max_manifest_bytes: 16384,
            max_manifest_entries: 32,
            max_selected_object_bytes: 4096,
            json,
        };
        let limits = CandidateLimits {
            admission,
            reader,
            max_state_bytes: 8 * 1024 * 1024,
            max_history_revisions: 4,
            max_history_identities: 32,
            max_read_bytes: 65536,
            max_write_bytes: 65536,
        };
        let target = "ToS/source-witnesses/old.md";
        let event = "ToS/source-witnesses/retirements/old.json";
        let old = b"retained target bytes";
        let old_digest = Digest256::of_bytes(old);
        let validator = Digest256::of_bytes(b"selected fixture program");
        let input = tmp.path().join("input");
        fs::create_dir_all(input.join("ToS/source-witnesses/retirements")).unwrap();
        fs::write(input.join(target), old).unwrap();
        store
            .ingest(
                &mut File::open(input.join(target)).unwrap(),
                old.len() as u64,
                old_digest,
                deadline,
                &cancel,
            )
            .unwrap();
        store.sync_objects(deadline, &cancel).unwrap();
        let mut base = json!({"schema_version":"tos_corpus_snapshot_v1","base_revision":null,"validator_sha256":validator.to_hex(),"files":[{"path":target,"sha256":old_digest.to_hex(),"size_bytes":old.len(),"mode":420}],"identities":{},"dependencies":{},"retirements":[]});
        let revision = Digest256::of_bytes(&canonical(&base, json).unwrap());
        base["revision"] = revision.to_hex().into();
        store
            .publish(
                None,
                revision,
                &canonical(&base, json).unwrap(),
                reader,
                deadline,
                &cancel,
                &|_| Ok(()),
            )
            .unwrap();
        let event_raw = b"{}";
        let event_digest = Digest256::of_bytes(event_raw);
        fs::write(input.join(event), event_raw).unwrap();
        let batch_value = json!({"schema_version":"tos_corpus_batch_v1","base_revision":revision.to_hex(),"validator_sha256":validator.to_hex(),"updates":[{"path":event,"sha256":event_digest.to_hex(),"size_bytes":event_raw.len(),"mode":420}],"retirements":[{"path":target,"event_ref":event,"event_sha256":event_digest.to_hex()}]});
        let batch_path = tmp.path().join("batch.json");
        fs::write(&batch_path, canonical(&batch_value, json).unwrap()).unwrap();
        // A rejected candidate state profile must refuse before this new event
        // can become a CAS object or the accepted pointer can move.
        let batch =
            AdmissionBatch::read(&batch_path, &input, admission, deadline, &cancel).unwrap();
        let small = CandidateLimits {
            max_state_bytes: size_of::<Candidate<'_>>() - 1,
            ..limits
        };
        let error = Candidate::prepare(&store, batch, validator, small, deadline, &cancel)
            .err()
            .expect("insufficient retained candidate state must refuse");
        assert!(error.to_string().contains("retained state bound exceeded"));
        assert!(
            !store_path
                .join("objects")
                .join(event_digest.to_hex())
                .exists()
        );
        assert_eq!(
            store.reader(reader).unwrap().select_current().unwrap(),
            Some(SourceRevision(revision))
        );
        let batch = AdmissionBatch::read(
            &batch_path,
            &input,
            limits.bounded_batch_limits().unwrap(),
            deadline,
            &cancel,
        )
        .unwrap();
        let candidate =
            Candidate::prepare(&store, batch, validator, limits, deadline, &cancel).unwrap();
        assert!(candidate.peak_state_bytes() >= candidate.retained_state_bytes());
        assert!(candidate.retained_state_bytes() < limits.max_state_bytes);
        assert!(!candidate.members.contains_key(target));
        assert_eq!(candidate.new_retirements.len(), 1);
        let row = &candidate.new_retirements[0];
        assert_eq!(row.as_object().unwrap().len(), 5);
        assert!(row.get("size_bytes").is_none());
        assert_eq!(row["sha256"], old_digest.to_hex());
        assert_eq!(row["event_size_bytes"], 2);
        // No source-validator invocation or accepted-pointer movement happened.
        assert_eq!(
            store.reader(reader).unwrap().select_current().unwrap(),
            Some(SourceRevision(revision))
        );
    }
}
impl CandidateLimits {
    /// The native caller must use this BEFORE AdmissionBatch::read; preparation
    /// cannot retroactively fence allocations in an already constructed batch.
    pub(crate) fn bounded_batch_limits(self) -> io::Result<AdmissionLimits> {
        let limits = self.validate()?;
        let available = limits
            .max_state_bytes
            .checked_sub(size_of::<Candidate<'_>>())
            .ok_or_else(|| invalid("candidate batch state bound exceeded"))?;
        let mut batch = limits.admission;
        let bytes = (available / 4 / 16).saturating_sub(1);
        let visits = available / 4 / entry::<String, Value>()?;
        batch.max_batch_bytes = batch.max_batch_bytes.min(bytes);
        batch.json.max_bytes = batch.json.max_bytes.min(bytes);
        batch.json.max_visits = batch.json.max_visits.min(visits);
        batch.validate()
    }
    pub(crate) fn validate(self) -> io::Result<Self> {
        self.admission.validate()?;
        self.reader.validate().map_err(invalid)?;
        if self.max_state_bytes == 0
            || self.max_state_bytes == usize::MAX
            || self.max_history_revisions == 0
            || self.max_history_revisions == usize::MAX
            || self.max_history_identities == 0
            || self.max_history_identities == usize::MAX
            || self.max_read_bytes == 0
            || self.max_read_bytes == u64::MAX
            || self.max_write_bytes == 0
            || self.max_write_bytes == u64::MAX
        {
            return Err(invalid("invalid source candidate operation limits"));
        }
        Ok(self)
    }
}

pub(crate) struct Candidate<'a> {
    store: &'a AdmissionStore,
    batch: AdmissionBatch,
    pub(crate) members: BTreeMap<String, Value>,
    pub(crate) base: Option<Value>,
    pub(crate) base_index: Option<Index>,
    pub(crate) new_retirements: Vec<Value>,
    pub(crate) affected: BTreeSet<String>,
    retirements: Vec<Value>,
    changed: BTreeSet<String>,
    read_paths: RefCell<BTreeSet<String>>,
    limits: CandidateLimits,
    reads: Cell<u64>,
    writes: Cell<u64>,
    read_limit: Cell<u64>,
    write_limit: Cell<u64>,
    state: Cell<usize>,
    peak_state: Cell<usize>,
    state_limit: Cell<usize>,
    deadline: Instant,
    cancel: &'a AtomicBool,
}

// Logical allocation reservation, not measured RSS: include owned strings,
// container capacity and a conservative per-entry B-tree node allowance. The
// sixteen slots cover a newly allocated sparse node, links and alignment.
fn sum(a: usize, b: usize) -> io::Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("candidate state overflow"))
}
fn times(a: usize, b: usize) -> io::Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("candidate state overflow"))
}
fn entry<K, V>() -> io::Result<usize> {
    times(16, sum(size_of::<(K, V)>(), size_of::<usize>())?)
}
fn text_state(text: &str) -> io::Result<usize> {
    sum(size_of::<String>(), text.len())
}
fn value_state(value: &Value) -> io::Result<usize> {
    let mut n = size_of::<Value>();
    match value {
        Value::String(s) => n = sum(n, s.capacity())?,
        Value::Array(a) => {
            n = sum(n, times(a.capacity(), size_of::<Value>())?)?;
            for v in a {
                n = sum(n, value_state(v)?)?;
            }
        }
        Value::Object(o) => {
            for (k, v) in o {
                n = sum(
                    n,
                    sum(
                        entry::<String, Value>()?,
                        sum(k.capacity(), value_state(v)?)?,
                    )?,
                )?;
            }
        }
        _ => {}
    }
    Ok(n)
}
fn index_state(index: &Index) -> io::Result<usize> {
    let mut n = size_of::<Index>();
    for (id, path) in &index.identities {
        n = sum(
            n,
            sum(
                entry::<String, String>()?,
                sum(id.capacity(), path.capacity())?,
            )?,
        )?;
    }
    for (path, targets) in &index.dependencies {
        n = sum(n, sum(entry::<String, Vec<String>>()?, path.capacity())?)?;
        n = sum(n, times(targets.capacity(), size_of::<String>())?)?;
        for target in targets {
            n = sum(n, target.capacity())?;
        }
    }
    Ok(n)
}
fn set_state(set: &BTreeSet<String>) -> io::Result<usize> {
    let mut n = size_of::<BTreeSet<String>>();
    for path in set {
        n = sum(n, sum(entry::<String, ()>()?, path.capacity())?)?;
    }
    Ok(n)
}
// Reader owns raw/canonical buffers, typed JSON/body clones, snapshot maps and
// path/index duplicates. Reserve before entering its allocation-owning API.
// Tighten both encoded bytes and visits together, never enlarge its profile.
fn reader_state(limits: ReadLimits) -> io::Result<usize> {
    let bytes = sum(limits.max_manifest_bytes.min(limits.json.max_bytes), 1)?;
    let visits = limits.json.max_visits.min(bytes);
    sum(times(bytes, 16)?, times(visits, entry::<String, Value>()?)?)
}

fn snapshot_state(snapshot: &Snapshot) -> io::Result<usize> {
    let mut n = size_of::<Snapshot>();
    for m in snapshot.members() {
        n = sum(
            n,
            sum(
                entry::<tos_foundation::RelativePath, tos_source_store::MemberMetadata>()?,
                times(m.path.as_str().len(), 2)?,
            )?,
        )?;
        if let Some(targets) = snapshot.indexed_dependencies(&m.path) {
            n = sum(
                n,
                sum(
                    entry::<tos_foundation::RelativePath, Vec<tos_foundation::RelativePath>>()?,
                    m.path.as_str().len(),
                )?,
            )?;
            n = sum(
                n,
                times(targets.len(), 2 * size_of::<tos_foundation::RelativePath>())?,
            )?;
            for target in targets {
                n = sum(n, target.as_str().len())?;
            }
        }
    }
    for (id, path) in snapshot.indexed_identities() {
        n = sum(
            n,
            sum(
                times(entry::<String, tos_foundation::RelativePath>()?, 2)?,
                times(sum(id.len(), path.as_str().len())?, 2)?,
            )?,
        )?;
    }
    for r in snapshot.retirements() {
        n = sum(
            n,
            sum(
                2 * size_of::<tos_source_store::RetirementMetadata>(),
                sum(r.path.as_str().len(), r.event_ref.as_str().len())?,
            )?,
        )?;
    }
    Ok(n)
}
fn conversion_state(snapshot: &Snapshot) -> io::Result<usize> {
    let mut n = times(entry::<String, Value>()?, 8)?;
    for m in snapshot.members() {
        n = sum(
            n,
            sum(
                times(entry::<String, Value>()?, 5)?,
                sum(times(m.path.as_str().len(), 2)?, 256)?,
            )?,
        )?;
        if let Some(targets) = snapshot.indexed_dependencies(&m.path) {
            n = sum(
                n,
                sum(entry::<String, Vec<String>>()?, m.path.as_str().len())?,
            )?;
            for target in targets {
                n = sum(n, times(text_state(target.as_str())?, 2)?)?;
            }
        }
    }
    for (id, path) in snapshot.indexed_identities() {
        n = sum(
            n,
            sum(
                entry::<String, String>()?,
                sum(id.len(), path.as_str().len())?,
            )?,
        )?;
    }
    for r in snapshot.retirements() {
        n = sum(
            n,
            sum(
                times(entry::<String, Value>()?, 6)?,
                sum(sum(r.path.as_str().len(), r.event_ref.as_str().len())?, 256)?,
            )?,
        )?;
    }
    // Metadata/index/ledger and the independently owned base serde tree coexist.
    times(n, 3)
}

fn metadata(snapshot: &Snapshot) -> BTreeMap<String, Value> {
    snapshot.members().map(|m| (m.path.as_str().to_owned(), json!({
        "path":m.path.as_str(),"sha256":m.sha256.to_hex(),"size_bytes":m.size_bytes,"mode":m.mode
    }))).collect()
}
fn snapshot_index(snapshot: &Snapshot) -> Index {
    Index {
        identities: snapshot
            .indexed_identities()
            .map(|(id, path)| (id.to_owned(), path.as_str().to_owned()))
            .collect(),
        dependencies: snapshot
            .members()
            .filter_map(|m| {
                snapshot
                    .indexed_dependencies(&m.path)
                    .filter(|targets| !targets.is_empty())
                    .map(|targets| {
                        (
                            m.path.as_str().to_owned(),
                            targets.iter().map(|p| p.as_str().to_owned()).collect(),
                        )
                    })
            })
            .collect(),
    }
}
fn ledger(snapshot: &Snapshot) -> Vec<Value> {
    snapshot.retirements().iter().map(|r| json!({"path":r.path.as_str(),"sha256":r.sha256.to_hex(),
        "event_ref":r.event_ref.as_str(),"event_sha256":r.event_sha256.to_hex(),"event_size_bytes":r.event_size_bytes})).collect()
}
fn snapshot_value(
    snapshot: &Snapshot,
    members: &BTreeMap<String, Value>,
    index: &Index,
    events: &[Value],
) -> Value {
    json!({"schema_version":"tos_corpus_snapshot_v1","revision":snapshot.revision().0.to_hex(),
        "base_revision":snapshot.base_revision().map(|r|r.0.to_hex()),"validator_sha256":snapshot.validator_sha256().to_hex(),
        "files":members.values().collect::<Vec<_>>(),"identities":index.identities,"dependencies":index.dependencies,"retirements":events})
}
fn binding(member: &Value) -> io::Result<(Digest256, u64)> {
    Ok((
        Digest256::from_hex(
            member["sha256"]
                .as_str()
                .ok_or_else(|| invalid("member digest absent"))?,
        )
        .map_err(invalid)?,
        member["size_bytes"]
            .as_u64()
            .ok_or_else(|| invalid("member size absent"))?,
    ))
}
pub(crate) fn canonical(value: &Value, limits: JsonLimits) -> io::Result<Vec<u8>> {
    struct Encoded {
        bytes: Vec<u8>,
        cap: usize,
    }
    impl Write for Encoded {
        fn write(&mut self, block: &[u8]) -> io::Result<usize> {
            let len = self
                .bytes
                .len()
                .checked_add(block.len())
                .filter(|n| *n <= self.cap)
                .ok_or_else(|| invalid("candidate encoded manifest bound exceeded"))?;
            self.bytes
                .try_reserve(len - self.bytes.len())
                .map_err(invalid)?;
            self.bytes.extend_from_slice(block);
            Ok(block.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut encoded = Encoded {
        bytes: Vec::new(),
        cap: limits.max_bytes,
    };
    serde_json::to_writer(&mut encoded, value).map_err(invalid)?;
    let bytes = encoded.bytes;
    let parsed = parse_json(&bytes, JsonMode::PublishedStrict, limits).map_err(invalid)?;
    canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, limits).map_err(invalid)
}
impl<'a> Candidate<'a> {
    /// `selected_validator` is computed by the native program, never selected
    /// or executed by the batch. Check it before constructing the store too.
    pub(crate) fn prepare(
        store: &'a AdmissionStore,
        batch: AdmissionBatch,
        selected_validator: Digest256,
        limits: CandidateLimits,
        deadline: Instant,
        cancel: &'a AtomicBool,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        active(deadline, cancel)?;
        if batch.validator_sha256 != selected_validator {
            return Err(invalid(
                "batch validator identity differs from selected native validator",
            ));
        }

        let mut result = Self {
            store,
            batch,
            members: BTreeMap::new(),
            base: None,
            base_index: None,
            new_retirements: Vec::new(),
            affected: BTreeSet::new(),
            retirements: Vec::new(),
            changed: BTreeSet::new(),
            read_paths: RefCell::new(BTreeSet::new()),
            limits,
            reads: Cell::new(0),
            writes: Cell::new(0),
            read_limit: Cell::new(limits.max_read_bytes),
            write_limit: Cell::new(limits.max_write_bytes),
            state: Cell::new(0),
            peak_state: Cell::new(0),
            state_limit: Cell::new(limits.max_state_bytes),
            deadline,
            cancel,
        };
        result.refresh_state()?;
        // Load only the selected base. The maintained no-op route never walks
        // ancestors; historical ownership is checked after full validation.
        if let Some(revision) = result.batch.base_revision {
            result.tick()?;
            // The v1 reader bounds each canonical manifest. Precharge its full
            // allowed read before calling it, so every ancestor consumes the
            // same operation budget, including manifests with no identities.
            result.charge_read(limits.reader.max_manifest_bytes as u64)?;
            let reader_limits = result.bounded_reader_limits(0)?;
            result.check_state(reader_state(reader_limits)?)?;
            let reader = store.reader(reader_limits)?;
            let snapshot = reader
                .load_exact(SourceRevision(revision))
                .map_err(invalid)?;
            result.check_state(sum(
                snapshot_state(&snapshot)?,
                conversion_state(&snapshot)?,
            )?)?;
            // Check this actual decoded snapshot plus all owned conversions
            // before the first metadata/index/base allocation.
            result.members = metadata(&snapshot);
            let index = snapshot_index(&snapshot);
            result.retirements = ledger(&snapshot);
            result.base = Some(snapshot_value(
                &snapshot,
                &result.members,
                &index,
                &result.retirements,
            ));
            result.base_index = Some(index);
        }
        result.refresh_state()?;
        // Reserve all proposed rows/sets and retirement work before json!/clones.
        let mut plan_state = 0;
        for path in result.batch.updates.keys() {
            plan_state = sum(
                plan_state,
                sum(
                    times(text_state(path)?, 4)?,
                    times(entry::<String, Value>()?, 8)?,
                )?,
            )?;
        }
        for (path, spec) in &result.batch.retirements {
            plan_state = sum(
                plan_state,
                sum(
                    times(
                        sum(text_state(path)?, text_state(spec.event_ref.as_str())?)?,
                        4,
                    )?,
                    times(entry::<String, Value>()?, 16)?,
                )?,
            )?;
        }
        result.check_state(plan_state)?;
        // Plan the COMPLETE final membership before any update opens/ingests.
        // In particular a rejected aggregate must leave no new CAS objects.
        let mut ingest_reads = 0u64;
        let mut ingest_writes = 0u64;
        for (path, update) in &result.batch.updates {
            result.tick()?;
            ingest_reads = ingest_reads
                .checked_add(
                    update
                        .size_bytes
                        .checked_mul(3)
                        .ok_or_else(|| invalid("source read charge overflow"))?,
                )
                .ok_or_else(|| invalid("source read charge overflow"))?;
            ingest_writes = ingest_writes
                .checked_add(update.size_bytes)
                .ok_or_else(|| invalid("source write charge overflow"))?;
            let row = json!({"path":path,"sha256":update.sha256.to_hex(),"size_bytes":update.size_bytes,"mode":update.mode});
            if result.members.get(path) != Some(&row) {
                result.changed.insert(path.clone());
            }
            result.members.insert(path.clone(), row);
        }
        let mut retirement_reads = 0u64;
        // Retired target lengths belong only to this physical verification
        // work, not to the maintained serialized retirement ledger format.
        let mut retirement_work = Vec::new();
        for (path, spec) in &result.batch.retirements {
            result.tick()?;
            let event_ref = spec.event_ref.as_str();
            if event_ref == path || result.batch.retirements.contains_key(event_ref) {
                return Err(invalid("retirement event is removed by the same batch"));
            }
            let target = result
                .members
                .get(path)
                .ok_or_else(|| invalid("retirement needs an existing source path"))?;
            let event = result
                .members
                .get(event_ref)
                .ok_or_else(|| invalid("retirement event missing from candidate"))?;
            let (event_digest, event_size) = binding(event)?;
            let (target_digest, target_size) = binding(target)?;
            if event_digest != spec.event_sha256 {
                return Err(invalid("retirement event digest differs"));
            }
            retirement_reads = retirement_reads
                .checked_add(target_size)
                .and_then(|n| n.checked_add(event_size))
                .ok_or_else(|| invalid("retirement read charge overflow"))?;
            let row = json!({"path":path,"sha256":target["sha256"],"event_ref":event_ref,
                "event_sha256":event_digest.to_hex(),"event_size_bytes":event_size});
            if result.retirements.iter().any(|old| {
                old["path"] == row["path"]
                    && old["sha256"] == row["sha256"]
                    && old["event_ref"] == row["event_ref"]
                    && old["event_sha256"] == row["event_sha256"]
            }) {
                return Err(invalid("retirement event already exists in history"));
            }
            result.new_retirements.push(row.clone());
            retirement_work.push((target_digest, target_size, event_digest, event_size));
            result.retirements.push(row);
            result.members.remove(path);
            result.changed.insert(path.clone());
        }
        if result.members.len() > limits.admission.max_members {
            return Err(invalid("source candidate member bound exceeded"));
        }
        let mut total = 0u64;
        for row in result.members.values() {
            result.tick()?;
            let (_, size) = binding(row)?;
            if size > limits.admission.max_member_bytes {
                return Err(invalid("source candidate member byte bound exceeded"));
            }
            total = total
                .checked_add(size)
                .filter(|n| *n <= limits.admission.max_source_bytes)
                .ok_or_else(|| invalid("source candidate aggregate byte bound exceeded"))?;
        }
        let planned_reads = ingest_reads
            .checked_add(retirement_reads)
            .ok_or_else(|| invalid("source read charge overflow"))?;
        if result
            .reads
            .get()
            .checked_add(planned_reads)
            .is_none_or(|n| n > limits.max_read_bytes)
            || result
                .writes
                .get()
                .checked_add(ingest_writes)
                .is_none_or(|n| n > limits.max_write_bytes)
        {
            return Err(invalid("source admission cumulative I/O bound exceeded"));
        }
        // Only the validated plan may now acquire immutable object bytes.
        for (path, update) in &result.batch.updates {
            result.tick()?;
            result.charge_read(
                update
                    .size_bytes
                    .checked_mul(3)
                    .ok_or_else(|| invalid("source read charge overflow"))?,
            )?;
            result.charge_write(update.size_bytes)?;
            let mut input = result.batch.open_update(path, deadline, cancel)?;
            store.ingest(
                &mut input,
                update.size_bytes,
                update.sha256,
                deadline,
                cancel,
            )?;
        }
        store.sync_objects(deadline, cancel)?;
        for (digest, size, event_digest, event_size) in retirement_work {
            result.tick()?;
            result.charge_read(size)?;
            store.verify_object(digest, size, deadline, cancel)?;
            result.charge_read(event_size)?;
            store.verify_object(event_digest, event_size, deadline, cancel)?;
        }
        result.refresh_state()?;
        let mut closure_state = set_state(&result.changed)?;
        // Reverse references and pending/affected path copies coexist.
        for path in result.members.keys().chain(result.changed.iter()) {
            closure_state = sum(
                closure_state,
                sum(times(text_state(path)?, 4)?, entry::<String, ()>()?)?,
            )?;
        }
        if let Some(index) = &result.base_index {
            for targets in index.dependencies.values() {
                closure_state = sum(
                    closure_state,
                    times(
                        targets.len(),
                        sum(entry::<&str, Vec<&str>>()?, 2 * size_of::<&str>())?,
                    )?,
                )?;
            }
        }
        result.check_state(closure_state)?;
        result.affected = result.changed.clone();
        // Build the reverse index once. Repeated fixed-point whole-map scans
        // turn a long dependency chain into quadratic preparation.
        if let Some(index) = &result.base_index {
            let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for (path, targets) in &index.dependencies {
                result.tick()?;
                for target in targets {
                    reverse.entry(target).or_default().push(path);
                }
            }
            let mut pending: Vec<String> = result.changed.iter().cloned().collect();
            while let Some(path) = pending.pop() {
                result.tick()?;
                for dependent in reverse.get(path.as_str()).into_iter().flatten() {
                    if result.affected.insert((*dependent).to_owned()) {
                        pending.push((*dependent).to_owned());
                    }
                }
            }
        }
        if result
            .base
            .as_ref()
            .is_none_or(|base| base["validator_sha256"] != selected_validator.to_hex())
        {
            result.affected.extend(result.members.keys().cloned());
        }
        result.refresh_state()?;
        result.tick()?;
        Ok(result)
    }
    /// Candidate-owned retained allocation estimate. Temporary reader/history/
    /// encoding peaks are reported separately and are NOT permanently retained.
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.state.get()
    }
    pub(crate) fn peak_state_bytes(&self) -> usize {
        self.peak_state.get()
    }
    pub(crate) fn restrict_remaining_state(&self, remaining: usize) -> io::Result<()> {
        self.tick()?;
        self.state_limit.set(
            self.state_limit
                .get()
                .min(sum(self.state.get(), remaining)?),
        );
        Ok(())
    }
    fn check_state(&self, temporary: usize) -> io::Result<()> {
        self.tick()?;
        let peak = sum(self.state.get(), temporary)?;
        if peak > self.state_limit.get() {
            return Err(invalid("candidate state bound exceeded"));
        }
        self.peak_state.set(self.peak_state.get().max(peak));
        Ok(())
    }
    fn refresh_state(&self) -> io::Result<()> {
        let mut n = size_of::<Self>();
        for (path, _) in &self.batch.updates {
            n = sum(
                n,
                sum(
                    entry::<String, super::source_admission::SourceUpdate>()?,
                    path.capacity(),
                )?,
            )?;
        }
        for (path, spec) in &self.batch.retirements {
            n = sum(
                n,
                sum(
                    entry::<String, super::source_admission::SourceRetirement>()?,
                    sum(path.capacity(), spec.event_ref.as_str().len())?,
                )?,
            )?;
        }
        for (path, value) in &self.members {
            n = sum(
                n,
                sum(
                    entry::<String, Value>()?,
                    sum(path.capacity(), value_state(value)?)?,
                )?,
            )?;
        }
        if let Some(v) = &self.base {
            n = sum(n, value_state(v)?)?;
        }
        if let Some(v) = &self.base_index {
            n = sum(n, index_state(v)?)?;
        }
        for rows in [&self.retirements, &self.new_retirements] {
            n = sum(n, times(rows.capacity(), size_of::<Value>())?)?;
            for v in rows {
                n = sum(n, value_state(v)?)?;
            }
        }
        for set in [&self.affected, &self.changed, &*self.read_paths.borrow()] {
            n = sum(n, set_state(set)?)?;
        }
        if n > self.state_limit.get() {
            return Err(invalid("candidate retained state bound exceeded"));
        }
        self.state.set(n);
        self.peak_state.set(self.peak_state.get().max(n));
        Ok(())
    }
    fn bounded_reader_limits(&self, other: usize) -> io::Result<ReadLimits> {
        let available = self
            .state_limit
            .get()
            .checked_sub(sum(self.state.get(), other)?)
            .ok_or_else(|| invalid("candidate reader state bound exceeded"))?;
        let mut limits = self.limits.reader;
        // Reserve half for decoded Snapshot conversion/history insertions;
        // each quarter bounds one independently controlled reader dimension; actual returned state is recomputed afterward.
        let bytes = (available / 4 / 16).saturating_sub(1);
        let visits = available / 4 / entry::<String, Value>()?;
        limits.max_manifest_bytes = limits.max_manifest_bytes.min(bytes);
        limits.json.max_bytes = limits.json.max_bytes.min(bytes);
        limits.json.max_visits = limits.json.max_visits.min(visits);
        limits.validate().map_err(invalid)?;
        Ok(limits)
    }
    /// Maximum additional retained read-set state in the index callback. This
    /// is a reservation only; the caller debits the actual retained delta.
    pub(crate) fn unread_path_state_bytes(&self) -> io::Result<usize> {
        let read = self.read_paths.borrow();
        let mut n = 0;
        for path in self.members.keys() {
            self.tick()?;
            if !read.contains(path) {
                n = sum(n, sum(entry::<String, ()>()?, path.len())?)?;
            }
        }
        Ok(n)
    }
    /// Reserve index JSON scratch separately from the growing retained Index.
    /// Sizes come from exact candidate metadata and the index's selected read
    /// routes; no source payload is opened by this cost calculation. The returned
    /// JSON profile cannot increase the caller's profile. Ordinary documents
    /// use the Foundation parser's allocation budget; only retirement records
    /// need the separate conservative serde conversion allowance.
    pub(crate) fn index_scratch_state_bytes(
        &self,
        mut json: JsonLimits,
        available: usize,
    ) -> io::Result<(usize, JsonLimits, usize)> {
        let mut raw_max = 0usize;
        let mut event_present = !self.new_retirements.is_empty();
        for (path, row) in &self.members {
            self.tick()?;
            let size = usize::try_from(binding(row)?.1).map_err(invalid)?;
            let event =
                path.starts_with("ToS/source-witnesses/retirements/") && path.ends_with(".json");
            if event {
                event_present = true;
                if size > MAX_EVENT_BYTES.min(json.max_bytes) {
                    return Err(invalid("index retirement read byte bound exceeded"));
                }
                raw_max = raw_max.max(size);
            }
            // Mirrors build_index's existing structured-JSON read route. Large
            // .json documents are intentionally not selected by that kernel.
            if path.starts_with("ToS/contracts/")
                || path.starts_with("ToS/doctrine/semantic-interchange/")
            {
                continue;
            }
            if (path.ends_with(".json") && size <= 16 * 1024 * 1024) || path.ends_with(".jsonl") {
                if size > json.max_bytes {
                    return Err(invalid("index selected JSON read byte bound exceeded"));
                }
                raw_max = raw_max.max(size);
            }
        }
        let schema_raw = if event_present {
            let row = self
                .members
                .get(super::source_admission_index::RETIREMENT_SCHEMA)
                .ok_or_else(|| invalid("index retirement schema missing"))?;
            let size = usize::try_from(binding(row)?.1).map_err(invalid)?;
            if size > MAX_EVENT_BYTES.min(json.max_bytes) {
                return Err(invalid("index retirement schema byte bound exceeded"));
            }
            raw_max = raw_max.max(size);
            size
        } else {
            0
        };
        // UTF-16 surrogatepass is the largest supported expansion: two raw
        // bytes may become six JSON escape bytes. UTF-32/UTF-8 expand less.
        let decoded = times(raw_max, 3)?.min(json.max_bytes);
        json.max_bytes = json.max_bytes.min(decoded.max(raw_max).max(1));
        json.max_visits = json.max_visits.min(json.max_bytes);
        JsonLimits::new(
            json.max_bytes,
            json.max_depth,
            json.max_visits,
            json.max_integer_digits,
        )
        .map_err(invalid)?;
        // Raw read and encoding Vec/String growth can coexist. The decoder's
        // bounded output may temporarily retain old and new capacity; the
        // parser's owned WTF-16 strings/containers are charged separately.
        let ordinary = sum(
            sum(times(raw_max, 2)?, times(json.max_bytes, 4)?)?,
            64 * 1024,
        )?;
        let retirement = if event_present {
            let bytes = json.max_bytes.min(MAX_EVENT_BYTES);
            sum(
                times(bytes, 16)?,
                times(json.max_visits.min(bytes), entry::<String, Value>()?)?,
            )?
        } else {
            0
        };
        let mut scratch = ordinary.max(retirement);
        scratch = sum(scratch, schema_raw)?;
        // Retirement grouping/targets/provenance comparison clones overlap the
        // raw/schema and parsed event. Charge actual selected ledger metadata.
        for row in &self.new_retirements {
            self.tick()?;
            scratch = sum(
                scratch,
                sum(times(value_state(row)?, 8)?, entry::<&str, Vec<&Value>>()?)?,
            )?;
        }
        // Split the remaining live reservation between one parsed document
        // and the growing Index. These are enforced budgets, not a guess of
        // serde Value size for the Foundation parser's different tree. Each
        // document is dropped before the next parser budget is reused.
        let parser_state = available
            .checked_sub(scratch)
            .map(|remaining| remaining / 2)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| invalid("candidate index parser workspace exceeds state budget"))?;
        scratch = sum(scratch, parser_state)?;
        Ok((scratch, json, parser_state))
    }
    fn mark_read(&self, path: &str) -> io::Result<()> {
        if !self.read_paths.borrow().contains(path) {
            self.check_state(sum(entry::<String, ()>()?, path.len())?)?;
            let retained = sum(self.state.get(), sum(entry::<String, ()>()?, path.len())?)?;
            self.read_paths.borrow_mut().insert(path.to_owned());
            self.state.set(retained);
        }
        Ok(())
    }

    /// Monotonic charged byte totals. Some physical operations precharge a
    /// conservative upper bound; these are not measured RSS or disk usage.
    pub(crate) fn io_usage(&self) -> (u64, u64) {
        (self.reads.get(), self.writes.get())
    }
    pub(crate) fn batch_bytes_read(&self) -> u64 {
        self.batch.bytes_read()
    }
    /// After FND consumes its share of the same invocation, publication may
    /// use only what remains. This can only tighten the original profile.
    pub(crate) fn restrict_remaining_io(&self, read: u64, write: u64) -> io::Result<()> {
        self.tick()?;
        let read = self
            .reads
            .get()
            .checked_add(read)
            .ok_or_else(|| invalid("candidate remaining read overflow"))?;
        let write = self
            .writes
            .get()
            .checked_add(write)
            .ok_or_else(|| invalid("candidate remaining write overflow"))?;
        self.read_limit.set(self.read_limit.get().min(read));
        self.write_limit.set(self.write_limit.get().min(write));
        Ok(())
    }
    pub(crate) fn tick(&self) -> io::Result<()> {
        active(self.deadline, self.cancel)
    }
    fn charge(counter: &Cell<u64>, amount: u64, cap: u64) -> io::Result<()> {
        counter.set(
            counter
                .get()
                .checked_add(amount)
                .filter(|n| *n <= cap)
                .ok_or_else(|| invalid("source admission cumulative I/O bound exceeded"))?,
        );
        Ok(())
    }
    fn charge_read(&self, n: u64) -> io::Result<()> {
        self.tick()?;
        Self::charge(&self.reads, n, self.read_limit.get())
    }
    fn charge_write(&self, n: u64) -> io::Result<()> {
        self.tick()?;
        Self::charge(&self.writes, n, self.write_limit.get())
    }
    fn verify_row(&self, row: &Value) -> io::Result<()> {
        let (digest, size) = binding(row)?;
        self.charge_read(size)?;
        self.store
            .verify_object(digest, size, self.deadline, self.cancel)
    }
    pub(crate) fn read(&self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        let row = self
            .members
            .get(path)
            .ok_or_else(|| invalid("read outside candidate membership"))?;
        let (digest, size) = binding(row)?;
        if size > cap as u64 {
            return Err(invalid("candidate object exceeds selected read bound"));
        }
        self.check_state(sum(
            usize::try_from(size).map_err(invalid)?,
            sum(entry::<String, ()>()?, path.len())?,
        )?)?;
        self.charge_read(size)?;
        let bytes = self
            .store
            .read_object(digest, size, cap, self.deadline, self.cancel)?;
        self.mark_read(path)?;
        Ok(bytes)
    }
    pub(crate) fn verify(&self, path: &str) -> io::Result<()> {
        self.verify_row(
            self.members
                .get(path)
                .ok_or_else(|| invalid("verification outside candidate membership"))?,
        )?;
        self.mark_read(path)?;
        Ok(())
    }
    /// FND privately materializes only selected members through the same exact
    /// streaming custody and cumulative budget as read/verify.
    pub(crate) fn copy(&self, path: &str, sink: &mut dyn Write) -> io::Result<()> {
        let row = self
            .members
            .get(path)
            .ok_or_else(|| invalid("copy outside candidate membership"))?;
        let (digest, size) = binding(row)?;
        self.charge_read(size)?;
        self.store
            .copy_object(digest, size, sink, self.deadline, self.cancel)?;
        self.mark_read(path)?;
        Ok(())
    }
    fn historical_reservations(&self, other_state: usize) -> io::Result<BTreeMap<String, String>> {
        let mut reserved = BTreeMap::<String, String>::new();
        let Some(base) = &self.base else {
            return Ok(reserved);
        };
        let reserved_state = Cell::new(0usize);
        let history_state = Cell::new(0usize);
        self.check_state(other_state)?;
        let live_snapshot = Cell::new(0usize);
        let mut history = BTreeSet::new();
        let mut visits = 0usize;
        let mut add = |id: &str, path: &str| -> io::Result<()> {
            self.tick()?;
            visits = visits
                .checked_add(1)
                .filter(|n| *n <= self.limits.max_history_identities)
                .ok_or_else(|| invalid("source history identity bound exceeded"))?;
            if reserved.get(id).is_some_and(|old| old != path) {
                return Err(invalid("historical identity ownership conflict"));
            }
            // The map remains live across subsequent ancestor loads.
            self.check_state(sum(
                sum(other_state, sum(history_state.get(), live_snapshot.get())?)?,
                sum(
                    reserved_state.get(),
                    sum(entry::<String, String>()?, sum(id.len(), path.len())?)?,
                )?,
            )?)?;
            if !reserved.contains_key(id) {
                reserved_state.set(sum(
                    reserved_state.get(),
                    sum(entry::<String, String>()?, sum(id.len(), path.len())?)?,
                )?);
            }
            reserved.insert(id.to_owned(), path.to_owned());
            Ok(())
        };
        if let Some(revision) = self.batch.base_revision {
            let n = sum(entry::<String, ()>()?, 64)?;
            self.check_state(sum(other_state, sum(reserved_state.get(), n)?)?)?;
            history_state.set(n);
            history.insert(revision.to_hex());
        }
        if let Some(index) = &self.base_index {
            for (id, path) in &index.identities {
                add(id, path)?;
            }
        }
        let mut ancestor = match base.get("base_revision") {
            Some(Value::Null) => None,
            Some(Value::String(s)) => Some(Digest256::from_hex(s).map_err(invalid)?),
            _ => return Err(invalid("base ancestor absent or invalid")),
        };
        while let Some(revision) = ancestor {
            self.tick()?;
            if history.len() >= self.limits.max_history_revisions {
                return Err(invalid("source history bound or cycle"));
            }
            let next_history = sum(history_state.get(), sum(entry::<String, ()>()?, 64)?)?;
            self.check_state(sum(other_state, sum(reserved_state.get(), next_history)?)?)?;
            if !history.insert(revision.to_hex()) {
                return Err(invalid("source history bound or cycle"));
            }
            history_state.set(next_history);
            self.charge_read(self.limits.reader.max_manifest_bytes as u64)?;
            let other = sum(other_state, sum(reserved_state.get(), history_state.get())?)?;
            let limits = self.bounded_reader_limits(other)?;
            let snapshot_bound = reader_state(limits)?;
            self.check_state(sum(other, snapshot_bound)?)?;
            live_snapshot.set(0);
            let reader = self.store.reader(limits)?;
            let snapshot = reader
                .load_exact(SourceRevision(revision))
                .map_err(invalid)?;
            live_snapshot.set(snapshot_state(&snapshot)?);
            for (id, path) in snapshot.indexed_identities() {
                add(id, path.as_str())?;
            }
            ancestor = snapshot.base_revision().map(|r| r.0);
            live_snapshot.set(0);
        }
        self.tick()?;
        Ok(reserved)
    }
    pub(crate) fn unchanged(&self) -> io::Result<Option<Value>> {
        if self.changed.is_empty()
            && self
                .base
                .as_ref()
                .is_some_and(|b| b["validator_sha256"] == self.batch.validator_sha256.to_hex())
        {
            let reader_limits = self.bounded_reader_limits(0)?;
            self.check_state(reader_state(reader_limits)?)?;
            self.store.check_current(
                self.batch.base_revision,
                reader_limits,
                self.deadline,
                self.cancel,
            )?;
            if let Some(base) = &self.base {
                self.check_state(value_state(base)?)?;
            }
            return Ok(self.base.clone());
        }
        Ok(None)
    }
    /// The sole caller must first receive the complete native FND result for
    /// THIS candidate, then construct this index from that same fresh render.
    pub(crate) fn publish(&self, index: Index) -> io::Result<Value> {
        self.tick()?;
        for (id, path) in &index.identities {
            self.tick()?;
            if tos_foundation::python_strip_unicode16_v1(id, self.limits.reader.json.max_bytes)
                .map_err(invalid)?
                .is_empty()
                || !self.members.contains_key(path)
            {
                return Err(invalid("identity index outside candidate"));
            }
        }
        for (path, targets) in &index.dependencies {
            self.tick()?;
            if !self.members.contains_key(path)
                || targets.windows(2).any(|p| p[0] >= p[1])
                || targets
                    .iter()
                    .any(|target| !self.members.contains_key(target))
            {
                return Err(invalid("dependency index outside candidate or unordered"));
            }
        }
        // The FND caller already charged the incoming index in its successful
        // window. Only new history/manifest allocations consume remaining state.
        let reserved = self.historical_reservations(0)?;
        for (id, path) in &index.identities {
            self.tick()?;
            if reserved.get(id).is_some_and(|old| old != path) {
                return Err(invalid(
                    "stable identity is reserved by a historical source path",
                ));
            }
        }
        // Exact maintained verify_reads(additional=changed & files): every
        // read/copied/review member as well as changed surviving members.
        let touched = self.read_paths.borrow();
        for path in touched.union(&self.changed) {
            if let Some(row) = self.members.get(path) {
                self.verify_row(row)?;
            }
        }
        drop(touched);
        drop(reserved);
        // Publication clones all retained rows and index into its result. Both
        // encoded passes also retain an original serde DOM, a typed DOM/body
        // and encoded buffers. Reserve the maximum overlap before json!.
        let mut manifest_state = sum(
            times(index_state(&index)?, 3)?,
            times(entry::<String, Value>()?, 8)?,
        )?;
        for value in self.members.values().chain(self.retirements.iter()) {
            manifest_state = sum(manifest_state, times(value_state(value)?, 2)?)?;
        }
        manifest_state = sum(
            manifest_state,
            times(self.members.len(), size_of::<&Value>())?,
        )?;
        let encoding_state = reader_state(self.bounded_reader_limits(manifest_state)?)?;
        self.check_state(sum(manifest_state, encoding_state)?)?;
        let mut manifest = json!({"schema_version":"tos_corpus_snapshot_v1","base_revision":self.batch.base_revision.map(|d|d.to_hex()),
            "validator_sha256":self.batch.validator_sha256.to_hex(),"files":self.members.values().collect::<Vec<_>>(),
            "identities":index.identities,"dependencies":index.dependencies,"retirements":self.retirements});
        let publication_limits = self.bounded_reader_limits(manifest_state)?;
        let body = canonical(&manifest, publication_limits.json)?;
        self.tick()?;
        let revision = Digest256::of_bytes(&body);
        manifest["revision"] = Value::String(revision.to_hex());
        let bytes = canonical(&manifest, publication_limits.json)?;
        self.tick()?;
        self.charge_write(bytes.len() as u64 + 256)?;
        self.charge_read(
            (bytes.len() as u64)
                .checked_mul(3)
                .ok_or_else(|| invalid("manifest read charge overflow"))?,
        )?;
        self.store.publish(
            self.batch.base_revision,
            revision,
            &bytes,
            publication_limits,
            self.deadline,
            self.cancel,
            &|n| self.charge_read(n),
        )?;
        self.tick()?;
        Ok(manifest)
    }
    /// Charge this returned manifest once in the caller's shared ledger before
    /// receipt(), which fences only its additional allocation. The manifest is
    /// caller-owned; it is never included in retained_state_bytes().
    pub(crate) fn manifest_state_bytes(manifest: &Value) -> io::Result<usize> {
        value_state(manifest)
    }
    pub(crate) fn receipt(&self, manifest: &Value) -> io::Result<Value> {
        let files = manifest["files"]
            .as_array()
            .ok_or_else(|| invalid("snapshot files absent"))?;
        let mut bytes = 0u64;
        for row in files {
            self.tick()?;
            bytes = bytes
                .checked_add(binding(row)?.1)
                .ok_or_else(|| invalid("source byte count overflow"))?;
        }
        let mut receipt_state = times(entry::<String, Value>()?, 10)?;
        for key in ["revision", "base_revision", "validator_sha256"] {
            receipt_state = sum(receipt_state, value_state(&manifest[key])?)?;
        }
        self.check_state(receipt_state)?;
        Ok(
            json!({"schema_version":"tos_corpus_admission_receipt_v1","batch_sha256":self.batch.batch_sha256.to_hex(),
            "revision":manifest["revision"],"base_revision":manifest["base_revision"],"validator_sha256":self.batch.validator_sha256.to_hex(),
            "members":files.len(),"identities":manifest["identities"].as_object().ok_or_else(||invalid("snapshot identities absent"))?.len(),
            "source_bytes":bytes,"semantic_admission":false,"rights_change":false}),
        )
    }
}
