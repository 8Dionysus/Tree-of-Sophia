//! Public Evidence Lens projection. Source, review, rights and canon retain authority.
//! Only requested identities are retained while selected graph collections stream.
use crate::{
    Error, PublicCaptureLimits, Result,
    d1_public_capture::{CreationState, PublicCapture, compact, json, source_digest},
    safe_open,
};
use serde_json::{Value, json as value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Component, Path},
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, canonical_bytes_v1,
};
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};

pub const SOURCE_REF: &str = "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json";
pub const SCHEMA_REF: &str = "ToS/contracts/epistemic-evidence-projection.schema.json";
pub const PROJECTION_REF: &str = "ToS/derived-exports/epistemic_evidence_projection.min.json";
pub const CANON_REF: &str =
    "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv";
const CAP: usize = 1024 * 1024;

type FileStamp = (u64, u64, u64, i64, i64, i64, i64);

struct BuiltEvidence {
    raw: Vec<u8>,
    capture: PublicCapture,
    opened: BTreeMap<String, Digest256>,
    projection_path: std::path::PathBuf,
}

/// The exact checked projection file and every input used to validate it.
/// Construction stays inside this maintained owner; consumers can only borrow
/// its actual file bytes while the independent source closure is current.
pub struct CompletedEvidenceProjection {
    root: std::path::PathBuf,
    main_binding: Option<(
        std::path::PathBuf,
        (u64, u64, u64, i64, i64, i64, i64),
        String,
    )>,
    source_revision: String,
    capture: PublicCapture,
    limits: PublicCaptureLimits,
    deadline: Instant,
    opened: OpenedMembership,
    projection_stamp: FileStamp,
    projection_sha256: Digest256,
    projection_path: std::path::PathBuf,
    raw: Vec<u8>,
}

// Construction still uses its maintained ordered map. Query delivery consumes
// that map into one sorted Vec, moving keys/digests without cloning payloads.
enum OpenedMembership {
    Build(BTreeMap<String, Digest256>),
    Delivery(Vec<(String, Digest256)>),
}
enum OpenedMembershipIter<'a> {
    Build(std::collections::btree_map::Iter<'a, String, Digest256>),
    Delivery(std::slice::Iter<'a, (String, Digest256)>),
}
impl<'a> Iterator for OpenedMembershipIter<'a> {
    type Item = (&'a String, &'a Digest256);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Build(iter) => iter.next(),
            Self::Delivery(iter) => iter.next().map(|(key, value)| (key, value)),
        }
    }
}
impl<'a> IntoIterator for &'a OpenedMembership {
    type Item = (&'a String, &'a Digest256);
    type IntoIter = OpenedMembershipIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            OpenedMembership::Build(map) => OpenedMembershipIter::Build(map.iter()),
            OpenedMembership::Delivery(values) => OpenedMembershipIter::Delivery(values.iter()),
        }
    }
}

fn file_stamp(metadata: &fs::Metadata) -> Result<FileStamp> {
    if !metadata.file_type().is_file() {
        return Err(Error::Invalid(
            "Evidence Lens projection is not a regular file",
        ));
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}

fn verify_projection_file(
    capture: &PublicCapture,
    projection_path: &Path,
    expected_stamp: Option<FileStamp>,
    expected_sha256: Option<Digest256>,
    deadline: Instant,
) -> Result<(Vec<u8>, FileStamp, Digest256)> {
    guard(deadline)?;
    let before = file_stamp(&fs::symlink_metadata(projection_path)?)?;
    if before.2 == 0 || before.2 > CAP as u64 || expected_stamp.is_some_and(|stamp| stamp != before)
    {
        return Err(Error::Budget(
            "Evidence Lens checked projection bytes or identity",
        ));
    }
    let mut file = safe_open::open_regular(projection_path, CAP as u64)?;
    if file_stamp(&file.metadata()?)? != before {
        return Err(Error::Invalid(
            "Evidence Lens projection changed while opening",
        ));
    }
    capture.charge_work(before.2)?;
    let mut raw = Vec::with_capacity(before.2 as usize);
    file.by_ref().take(CAP as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 != before.2 || raw.len() > CAP {
        return Err(Error::Invalid(
            "Evidence Lens projection changed while reading",
        ));
    }
    let after_handle = file_stamp(&file.metadata()?)?;
    let after_path = file_stamp(&fs::symlink_metadata(projection_path)?)?;
    if before != after_handle || before != after_path {
        return Err(Error::Invalid(
            "Evidence Lens projection changed while checking",
        ));
    }
    let digest = Digest256::of_bytes(&raw);
    if expected_sha256.is_some_and(|expected| expected != digest) {
        return Err(Error::Invalid(
            "Evidence Lens checked projection bytes changed",
        ));
    }
    let payload = decode(&raw)?;
    if payload["schema_version"] != "tos_epistemic_evidence_projection_v1" {
        return Err(Error::Invalid("Evidence Lens checked projection schema"));
    }
    guard(deadline)?;
    Ok((raw, before, digest))
}

fn verify_projection_file_owned(
    capture: &PublicCapture,
    path: &Path,
    deadline: Instant,
    state: &CreationState<'_>,
) -> Result<(Vec<u8>, FileStamp, Digest256)> {
    state.active()?;
    capture.check_custody()?;
    let before = file_stamp(&fs::symlink_metadata(path)?)?;
    if before.2 == 0 || before.2 > CAP as u64 {
        return Err(Error::Budget(
            "Evidence Lens checked projection bytes or identity",
        ));
    }
    let raw = read_owned_path(path, deadline, state)?;
    state.charge_work(raw.len())?;
    let digest = Digest256::of_bytes(&raw);
    let limits = JsonLimits::new(CAP, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("Evidence projection JSON"))?;
    state.with_serde_owned_with_limits(&raw, limits, |payload| {
        if payload["schema_version"] != "tos_epistemic_evidence_projection_v1" {
            return Err(Error::Invalid("Evidence Lens checked projection schema"));
        }
        Ok(())
    })?;
    if before != file_stamp(&fs::symlink_metadata(path)?)? {
        return Err(Error::Invalid("Evidence projection changed while checking"));
    }
    capture.check_custody()?;
    state.active()?;
    Ok((raw, before, digest))
}

impl CompletedEvidenceProjection {
    /// Logical retained builder records plus owned buffers before delivery.
    /// Builder-map node bookkeeping belongs to the original OS allocation
    /// guard; the actual record slots and keys are counted here. Query delivery
    /// additionally reserves the complete prospective Vec before moving records.
    pub fn query_transition_state_upper_bound(&self) -> Result<usize> {
        use tos_foundation::{OwnedState, checked_state_add};
        let membership = match &self.opened {
            OpenedMembership::Delivery(values) => values
                .owned_heap_bytes()
                .map_err(|_| Error::Budget("Evidence membership retained state"))?,
            OpenedMembership::Build(map) => {
                let mut bytes = map
                    .len()
                    .checked_mul(std::mem::size_of::<(String, Digest256)>())
                    .ok_or(Error::Budget("Evidence builder record state overflow"))?;
                for key in map.keys() {
                    bytes = checked_state_add(bytes, key.capacity())
                        .map_err(|_| Error::Budget("Evidence builder key state overflow"))?;
                }
                bytes
            }
        };
        let mut bytes = std::mem::size_of::<Self>();
        for amount in [
            self.root.capacity(),
            self.projection_path.capacity(),
            self.source_revision.capacity(),
            self.raw.capacity(),
            membership,
            self.capture
                .retained_state_upper_bound()?
                .checked_sub(std::mem::size_of::<PublicCapture>())
                .ok_or(Error::Budget("Evidence capture inline state"))?,
        ] {
            bytes = checked_state_add(bytes, amount)
                .map_err(|_| Error::Budget("Evidence transition state overflow"))?;
        }
        if let Some((path, _, revision)) = &self.main_binding {
            bytes = checked_state_add(bytes, path.capacity())
                .and_then(|n| checked_state_add(n, revision.capacity()))
                .map_err(|_| Error::Budget("Evidence transition main binding state"))?;
        }
        Ok(bytes)
    }

    /// Prospective contiguous membership slots while the original builder map
    /// is consumed. The caller subtracts this amount from the SAME original
    /// whole-state allowance before asking this owner to allocate the Vec.
    pub fn query_delivery_workspace_bytes(&self) -> Result<usize> {
        let count = match &self.opened {
            OpenedMembership::Build(map) => map.len(),
            OpenedMembership::Delivery(_) => return Ok(0),
        };
        count
            .checked_mul(std::mem::size_of::<(String, Digest256)>())
            .ok_or(Error::Budget(
                "Evidence query membership workspace overflow",
            ))
    }

    pub fn into_reference_query_delivery(mut self, reserved_workspace: usize) -> Result<Self> {
        let needed = self.query_delivery_workspace_bytes()?;
        if needed > reserved_workspace {
            return Err(Error::Budget("Evidence query membership workspace"));
        }
        let old = std::mem::replace(&mut self.opened, OpenedMembership::Delivery(Vec::new()));
        self.opened = match old {
            OpenedMembership::Delivery(values) => OpenedMembership::Delivery(values),
            OpenedMembership::Build(map) => {
                let mut values = Vec::with_capacity(map.len());
                let actual = values
                    .capacity()
                    .checked_mul(std::mem::size_of::<(String, Digest256)>())
                    .ok_or(Error::Budget("Evidence query membership capacity overflow"))?;
                if actual > reserved_workspace {
                    return Err(Error::Budget("Evidence query membership capacity"));
                }
                values.extend(map.into_iter());
                OpenedMembership::Delivery(values)
            }
        };
        Ok(self)
    }

    /// All privately owned buffers of this checked Evidence holder, including
    /// its distinct capture. Borrowed evidence views carry no second charge.
    pub fn retained_query_state_upper_bound(&self) -> Result<usize> {
        use tos_foundation::{OwnedState, checked_state_add};
        let OpenedMembership::Delivery(opened) = &self.opened else {
            return Err(Error::Invalid("Evidence query membership not transitioned"));
        };
        let mut bytes = std::mem::size_of::<Self>();
        for amount in [
            self.root.capacity(),
            self.projection_path.capacity(),
            self.source_revision.capacity(),
            self.raw.capacity(),
            opened
                .owned_heap_bytes()
                .map_err(|_| Error::Budget("Evidence membership state"))?,
            self.capture
                .retained_state_upper_bound()?
                .checked_sub(std::mem::size_of::<PublicCapture>())
                .ok_or(Error::Budget("Evidence capture inline state"))?,
        ] {
            bytes = checked_state_add(bytes, amount)
                .map_err(|_| Error::Budget("Evidence retained state overflow"))?;
        }
        if let Some((path, _, revision)) = &self.main_binding {
            bytes = checked_state_add(bytes, path.capacity())
                .and_then(|n| checked_state_add(n, revision.capacity()))
                .map_err(|_| Error::Budget("Evidence main binding state"))?;
        }
        Ok(bytes)
    }

    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub(crate) fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub(crate) fn charge_work(&self, bytes: u64) -> Result<()> {
        self.capture.charge_work(bytes)
    }

    pub(crate) fn verify_binding(
        &self,
        main_capture: &PublicCapture,
        source_revision: &str,
    ) -> Result<()> {
        let Some((main_root, main_identity, main_revision)) = &self.main_binding else {
            return Err(Error::Invalid(
                "Evidence Lens has no completed graph binding",
            ));
        };
        if main_root != main_capture.root()
            || *main_identity != main_capture.capture_identity()?
            || main_revision != source_revision
            || self.source_revision != source_revision
        {
            return Err(Error::Invalid("Evidence Lens completed snapshot binding"));
        }
        self.verify_current()
    }

    pub(crate) fn verify_current(&self) -> Result<()> {
        self.capture.verify_inputs(self.limits)?;
        for (reference, expected) in &self.opened {
            let actual = route_digest(
                &self.root,
                reference,
                &self.capture,
                self.limits,
                self.deadline,
            )?;
            if actual != *expected {
                return Err(Error::Invalid("Evidence Lens checked input changed"));
            }
        }
        let (_, stamp, digest) = verify_projection_file(
            &self.capture,
            &self.projection_path,
            Some(self.projection_stamp),
            Some(self.projection_sha256),
            self.deadline,
        )?;
        if stamp != self.projection_stamp || digest != self.projection_sha256 {
            return Err(Error::Invalid("Evidence Lens checked projection changed"));
        }
        Ok(())
    }

    /// Lend this actual checked file only while its own selected input closure
    /// and projection output stay current. Whole-snapshot callers additionally
    /// use `verify_binding` against the completed graph capture.
    pub fn with_current<T>(
        &self,
        consume: impl for<'view> FnOnce(
            &crate::native_snapshot::CompletedEvidenceProjectionView<'view>,
        ) -> Result<T>,
    ) -> Result<T> {
        self.verify_current()?;
        let view = crate::native_snapshot::CompletedEvidenceProjectionView { evidence: self };
        let result = consume(&view);
        let current = self.verify_current();
        match current {
            Err(error) => Err(error),
            Ok(()) => result,
        }
    }
}

fn err(message: impl Into<String>) -> Error {
    Error::Source(message.into())
}
fn decode(raw: &[u8]) -> Result<Value> {
    let parsed = json(raw, CAP)?;
    serde_json::from_slice(&compact(&parsed, CAP)?).map_err(|e| err(e.to_string()))
}
fn array<'a>(v: &'a Value, field: &str) -> Result<&'a [Value]> {
    match v.get(field) {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        _ => Err(err(format!("Evidence Lens {field} must be an array"))),
    }
}
fn string(v: &Value) -> Result<&str> {
    v.as_str().ok_or(Error::Invalid(
        "Evidence Lens identity/ref must be a string",
    ))
}
fn guard(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::Budget("Evidence Lens deadline"))
    } else {
        Ok(())
    }
}
fn reference_path(root: &Path, reference: &str) -> Result<std::path::PathBuf> {
    let p = Path::new(reference);
    if reference.is_empty()
        || p.is_absolute()
        || reference.contains("/payload/")
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(err(format!(
            "unsafe Evidence Lens route ref: {reference:?}"
        )));
    }
    Ok(root.join(p))
}
fn read(root: &Path, reference: &str, deadline: Instant) -> Result<Vec<u8>> {
    guard(deadline)?;
    let mut file = safe_open::open_regular(&reference_path(root, reference)?, CAP as u64)?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)?;
    guard(deadline)?;
    Ok(raw)
}
/// Controlled read keeps the exact frozen envelope: no growing read-to-end
/// allocation and no read past the initial payload other than one EOF probe.
fn read_owned(
    root: &Path,
    reference: &str,
    deadline: Instant,
    state: &CreationState<'_>,
) -> Result<Vec<u8>> {
    state.active()?;
    guard(deadline)?;
    let path_bytes = root
        .as_os_str()
        .len()
        .checked_add(reference.len())
        .and_then(|n| n.checked_add(2))
        .ok_or(Error::Budget("Evidence owned path geometry"))?;
    // Path join and safe-open native pathname coexist with this owner.
    let _path_hold = state.hold(
        path_bytes
            .checked_mul(4)
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<std::path::PathBuf>()
                        + std::mem::size_of::<fs::File>()
                        + 2 * std::mem::size_of::<fs::Metadata>(),
                )
            })
            .ok_or(Error::Budget("Evidence owned path workspace"))?,
    )?;
    state.charge_work(path_bytes)?;
    let path = reference_path(root, reference)?;
    read_owned_path(&path, deadline, state)
}
fn read_owned_path(path: &Path, deadline: Instant, state: &CreationState<'_>) -> Result<Vec<u8>> {
    state.active()?;
    guard(deadline)?;
    let _path_hold = state.hold(
        path.as_os_str()
            .len()
            .checked_mul(3)
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<fs::File>() + 3 * std::mem::size_of::<fs::Metadata>(),
                )
            })
            .ok_or(Error::Budget("Evidence owned selected path workspace"))?,
    )?;
    let mut file = safe_open::open_regular(path, CAP as u64)?;
    let before = file_stamp(&file.metadata()?)?;
    let count = usize::try_from(before.2).map_err(|_| Error::Budget("Evidence owned read size"))?;
    if count > CAP {
        return Err(Error::Budget("Evidence owned read cap"));
    }
    state.retain(count)?;
    state.charge_work(
        count
            .checked_add(1)
            .ok_or(Error::Budget("Evidence owned read work"))?,
    )?;
    let mut raw = Vec::with_capacity(count);
    // Exact capacity is initialized before the read; every chunk checks the
    // same original cutoff/cancel state. EOF probes cannot grow the Vec.
    raw.resize(count, 0);
    let mut at = 0;
    while at < count {
        state.active()?;
        guard(deadline)?;
        let end = at.checked_add(65536).unwrap_or(count).min(count);
        let n = file.read(&mut raw[at..end])?;
        if n == 0 {
            return Err(Error::Invalid("Evidence source shrank during read"));
        }
        at += n;
    }
    state.active()?;
    let mut eof = [0u8; 1];
    if file.read(&mut eof)? != 0
        || before != file_stamp(&file.metadata()?)?
        || before != file_stamp(&fs::symlink_metadata(path)?)?
    {
        return Err(Error::Invalid("Evidence source changed during read"));
    }
    guard(deadline)?;
    state.active()?;
    Ok(raw)
}

/// Same CorpusSnapshotV1 spelling as the maintained renderer, with original
/// state/work/visits and cooperative checks through both writer passes.
fn rendered_owned(v: &Value, deadline: Instant, state: &CreationState<'_>) -> Result<Vec<u8>> {
    guard(deadline)?;
    state.with_json_encoded(v, CAP, |raw| {
        let document = state.json(raw, CAP)?;
        let visits = state.remaining_json_visits()?;
        let limits = JsonLimits::new(CAP, 96, visits.min(1_000_000), 4096)
            .map_err(|_| Error::Budget("Evidence owned JSON output"))?;
        state.charge_work(
            limits
                .max_bytes
                .checked_mul(2)
                .and_then(|n| n.checked_add(limits.max_visits))
                .ok_or(Error::Budget("Evidence owned canonical work"))?,
        )?;
        let available = state.remaining(0)?;
        let mut check = || {
            state.active().and_then(|_| guard(deadline)).map_err(|_| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "Evidence owned canonical cutoff",
                )
            })
        };
        let result = tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_check(
            &document,
            CanonicalProfile::CorpusSnapshotV1,
            limits,
            available,
            &mut check,
        );
        let used = result
            .as_ref()
            .map(|(_, used)| *used)
            .unwrap_or(limits.max_visits);
        state.debit_json_visits(used)?;
        let (encoded, _) = result.map_err(|_| Error::Budget("Evidence owned canonical output"))?;
        state.retain(encoded.capacity())?;
        state.active()?;
        Ok(encoded)
    })
}

/// Routes contribute only byte identity; never retain or parse their body.
fn route_digest(
    root: &Path,
    reference: &str,
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<Digest256> {
    guard(deadline)?;
    let mut file =
        safe_open::open_regular(&reference_path(root, reference)?, limits.max_input_bytes)?;
    let (digest, _) = source_digest(&mut file, limits.max_input_bytes, |bytes| {
        capture.charge_work(bytes as u64)
    })?;
    guard(deadline)?;
    Ok(digest)
}
fn rendered(v: &Value, deadline: Instant) -> Result<Vec<u8>> {
    guard(deadline)?;
    let raw = serde_json::to_vec(v).map_err(|e| err(e.to_string()))?;
    // The donor sorts every object, independently of serde's unified map feature.
    // This existing byte profile supplies Python compact spelling and the final LF.
    let result = canonical_bytes_v1(
        &json(&raw, CAP)?,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(CAP, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Evidence Lens JSON output"))?,
    )
    .map_err(|e| err(e.to_string()))?;
    guard(deadline)?;
    Ok(result)
}

pub fn validate_payload(root: &Path, payload: &Value, deadline: Instant) -> Result<()> {
    guard(deadline)?;
    let raw = read(root, SCHEMA_REF, deadline)?;
    let schema = decode(&raw)?;
    let uri = string(&schema["$id"])?;
    let backend = SchemaBackendProbe::new(
        [SchemaResource {
            uri: uri.to_owned(),
            raw,
        }],
        FormatProfile::LegacyPythonObserved20260923,
    )
    .map_err(|e| err(format!("Evidence Lens schema: {e:?}")))?;
    if !backend
        .is_valid_raw(uri, &rendered(payload, deadline)?)
        .map_err(|e| err(format!("Evidence Lens schema: {e:?}")))?
    {
        return Err(Error::Invalid("Evidence Lens schema violation"));
    }
    guard(deadline)?;
    let mut ids = BTreeSet::new();
    for scene in array(payload, "scenes")? {
        if scene["posture"] == "contested-pre-canon" && scene["conclusion"]["can_conclude"] == true
        {
            return Err(Error::Invalid(
                "contested pre-canon scenes cannot be conclusive",
            ));
        }
        if scene["conclusion"]["claim_evidence_closed"] == true {
            return Err(Error::Invalid(
                "v1 Evidence Lens scenes must not assert claim/evidence closure",
            ));
        }
        for id in array(scene, "selection_ids")? {
            if !ids.insert(string(id)?) {
                return Err(Error::Invalid("selection IDs must bind to one scene only"));
            }
        }
    }
    guard(deadline)
}

struct EvidenceSchemaControl<'a, 'budget>(&'a CreationState<'budget>);
impl tos_validation::SchemaProbeControl for EvidenceSchemaControl<'_, '_> {
    fn remaining(
        &self,
        prospective: usize,
    ) -> std::result::Result<usize, tos_validation::SchemaProbeError> {
        self.0
            .remaining(prospective)
            .map_err(|_| tos_validation::SchemaProbeError::BudgetExceeded)
    }
    fn check(&self) -> std::result::Result<(), tos_validation::SchemaProbeError> {
        self.0
            .active()
            .map_err(|_| tos_validation::SchemaProbeError::BudgetExceeded)
    }
    fn charge_work(&self, bytes: u64) -> std::result::Result<(), tos_validation::SchemaProbeError> {
        usize::try_from(bytes)
            .map_err(|_| tos_validation::SchemaProbeError::BudgetExceeded)
            .and_then(|n| {
                self.0
                    .charge_work(n)
                    .map_err(|_| tos_validation::SchemaProbeError::BudgetExceeded)
            })
    }
    fn remaining_json_visits(
        &self,
    ) -> std::result::Result<usize, tos_validation::SchemaProbeError> {
        self.0
            .remaining_json_visits()
            .map_err(|_| tos_validation::SchemaProbeError::BudgetExceeded)
    }
    fn debit_json_visits(
        &self,
        used: usize,
    ) -> std::result::Result<(), tos_validation::SchemaProbeError> {
        self.0
            .debit_json_visits(used)
            .map_err(|_| tos_validation::SchemaProbeError::BudgetExceeded)
    }
}

fn validate_payload_owned(
    root: &Path,
    payload: &Value,
    deadline: Instant,
    state: &CreationState<'_>,
) -> Result<()> {
    state.active()?;
    let raw = read_owned(root, SCHEMA_REF, deadline, state)?;
    let limits = JsonLimits::new(CAP, 96, state.remaining_json_visits()?.min(1_000_000), 4096)
        .map_err(|_| Error::Budget("Evidence controlled schema limits"))?;
    let (schema, schema_hold) = state.serde_scoped_with_limits(&raw, limits)?;
    let uri = string(&schema["$id"])?;
    state.charge_work(uri.len())?;
    let uri_hold = state.hold(
        uri.len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<String>()))
            .ok_or(Error::Budget("Evidence controlled schema URI state"))?,
    )?;
    let uri = uri.to_owned();
    // Construction and validation stay in the genuine Validation owner. Its
    // supported-schema check refuses anything outside its explicit grammar;
    // no digest match or check-before/after stand-in issues this proof.
    let control = EvidenceSchemaControl(state);
    let backend = SchemaBackendProbe::new_controlled(
        SchemaResource {
            uri: uri.clone(),
            raw,
        },
        FormatProfile::LegacyPythonObserved20260923,
        limits,
        &control,
    )
    .map_err(|error| match error {
        tos_validation::SchemaProbeError::BudgetExceeded => {
            Error::Budget("Evidence controlled schema construction")
        }
        tos_validation::SchemaProbeError::UnsupportedControlledSchema => {
            Error::PreparedUnsupported("Evidence controlled schema keyword/shape")
        }
        _ => Error::Invalid("Evidence controlled schema construction"),
    })?;
    let backend_hold = state.hold(
        backend
            .retained_state_upper_bound()
            .map_err(|_| Error::Budget("Evidence controlled schema retained state"))?,
    )?;
    let rendered = rendered_owned(payload, deadline, state)?;
    if !backend
        .is_valid_raw(&uri, &rendered, limits, &control)
        .map_err(|error| match error {
            tos_validation::SchemaProbeError::BudgetExceeded => {
                Error::Budget("Evidence controlled schema evaluation")
            }
            tos_validation::SchemaProbeError::UnsupportedControlledSchema => {
                Error::PreparedUnsupported("Evidence controlled schema keyword/shape")
            }
            _ => Error::Invalid("Evidence controlled schema evaluation"),
        })?
    {
        return Err(Error::Invalid("Evidence Lens schema violation"));
    }
    drop(rendered);
    drop(backend);
    drop(backend_hold);
    drop(schema);
    drop(schema_hold);
    drop(uri);
    drop(uri_hold);
    let scenes = array(payload, "scenes")?;
    let mut selected = 0usize;
    for scene in scenes {
        state.active()?;
        selected = selected
            .checked_add(array(scene, "selection_ids")?.len())
            .ok_or(Error::Budget("Evidence controlled selection count"))?;
    }
    let node = 11 * std::mem::size_of::<&str>() + 16 * std::mem::size_of::<usize>();
    let _ids_hold = state.hold(
        selected
            .checked_mul(node)
            .ok_or(Error::Budget("Evidence controlled selection state"))?,
    )?;
    let mut ids = BTreeSet::new();
    for scene in scenes {
        state.active()?;
        if scene["posture"] == "contested-pre-canon" && scene["conclusion"]["can_conclude"] == true
        {
            return Err(Error::Invalid(
                "contested pre-canon scenes cannot be conclusive",
            ));
        }
        if scene["conclusion"]["claim_evidence_closed"] == true {
            return Err(Error::Invalid(
                "v1 Evidence Lens scenes must not assert claim/evidence closure",
            ));
        }
        for id in array(scene, "selection_ids")? {
            let id = string(id)?;
            state.charge_work(id.len())?;
            if !ids.insert(id) {
                return Err(Error::Invalid("selection IDs must bind to one scene only"));
            }
        }
    }
    state.active()
}

/// Captures selected collections to caller-owned fresh staging, then rechecks all
/// opened inputs before returning bytes. Never writes a source or public output.
fn evidence_string_owned(value: &Value, state: &CreationState<'_>) -> Result<String> {
    let literal = match value {
        Value::Null => Some("None"),
        Value::Bool(true) => Some("True"),
        Value::Bool(false) => Some("False"),
        Value::String(text) => Some(text.as_str()),
        _ => None,
    };
    if let Some(text) = literal {
        evidence_retain_strings(state, &[text], std::mem::size_of::<String>())?;
        return Ok(text.to_owned());
    }
    if let Value::Number(number) = value {
        let document = state.json(number.as_str().as_bytes(), 4096)?;
        let limits = JsonLimits::new(
            4096,
            96,
            state.remaining_json_visits()?.min(1_000_000),
            4096,
        )
        .map_err(|_| Error::Budget("Evidence anchor number limits"))?;
        return state.with_foundation_canonical_bytes(&document, limits, |raw| {
            state.retain(raw.len())?;
            state.charge_work(raw.len())?;
            String::from_utf8(raw.to_vec())
                .map_err(|_| Error::Invalid("Evidence anchor numeric UTF8"))
        });
    }
    // Existing non-scalar spelling is serde's compact insertion order.
    String::from_utf8(state.encode_json(value, CAP)?)
        .map_err(|_| Error::Invalid("Evidence anchor UTF8"))
}
fn route_digest_dispatch(
    root: &Path,
    reference: &str,
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
    deadline: Instant,
    state: Option<&CreationState<'_>>,
) -> Result<Digest256> {
    let _hold = if let Some(state) = state {
        let path = root
            .as_os_str()
            .len()
            .checked_add(reference.len())
            .and_then(|n| n.checked_add(2))
            .ok_or(Error::Budget("Evidence route path"))?;
        state.charge_work(path)?;
        Some(
            state.hold(
                path.checked_mul(4)
                    .and_then(|n| {
                        n.checked_add(
                            65536
                                + std::mem::size_of::<fs::File>()
                                + std::mem::size_of::<std::path::PathBuf>(),
                        )
                    })
                    .ok_or(Error::Budget("Evidence route workspace"))?,
            )?,
        )
    } else {
        None
    };
    route_digest(root, reference, capture, limits, deadline)
}
fn evidence_retain_strings(
    state: &CreationState<'_>,
    strings: &[&str],
    containers: usize,
) -> Result<()> {
    let mut bytes = containers;
    for text in strings {
        state.charge_work(text.len())?;
        bytes = bytes
            .checked_add(text.len())
            .ok_or(Error::Budget("Evidence retained string geometry"))?;
    }
    state.retain(bytes)
}
fn csv_records_dispatch<F: FnMut(Vec<String>) -> Result<()>>(
    raw: &[u8],
    limits: crate::knowledge_canon_source::CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    state: Option<&CreationState<'_>>,
    emit: F,
) -> Result<()> {
    match state {
        Some(state) => crate::knowledge_canon_source::csv_records_with_owned_state(
            raw, limits, deadline, cancelled, state, emit,
        ),
        None => crate::knowledge_canon_source::csv_records(raw, limits, deadline, cancelled, emit),
    }
}
fn build_checked(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
    selected: Option<&crate::d1_public_capture::PublicCaptureInputPaths>,
    cancelled: std::sync::Arc<AtomicBool>,
) -> Result<BuiltEvidence> {
    build_checked_with_state(
        root, staging, limits, deadline, deadline, selected, cancelled, None,
    )
}
fn build_checked_with_state(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
    owner_deadline: Instant,
    selected: Option<&crate::d1_public_capture::PublicCaptureInputPaths>,
    cancelled: Arc<AtomicBool>,
    state: Option<&CreationState<'_>>,
) -> Result<BuiltEvidence> {
    guard(deadline)?;
    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Err(Error::Budget("Evidence Lens cancelled"));
    }
    let source_raw = match state {
        Some(state) => read_owned(root, SOURCE_REF, deadline, state)?,
        None => read(root, SOURCE_REF, deadline)?,
    };
    let source = match state {
        Some(state) => state.serde_owned(&source_raw, CAP)?,
        None => decode(&source_raw)?,
    };
    guard(deadline)?;
    let scenes = array(&source, "scenes")?;
    let mut requested: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for scene in scenes {
        if !scene.is_object() {
            return Err(Error::Invalid("Evidence Lens scenes must be objects"));
        }
        for selection in array(scene, "selections")? {
            if !selection.is_object() {
                return Err(Error::Invalid("Evidence Lens selections must be objects"));
            }
            if let Some(state) = state {
                let mode = string(&selection["mode"])?;
                let view = string(&selection["view_id"])?;
                evidence_retain_strings(
                    state,
                    &[mode, view],
                    11 * std::mem::size_of::<((String, String), BTreeSet<String>)>()
                        + 16 * std::mem::size_of::<usize>(),
                )?;
            }
            let key = (
                string(&selection["mode"])?.to_owned(),
                string(&selection["view_id"])?.to_owned(),
            );
            for id in array(selection, "item_ids")? {
                if let Some(state) = state {
                    evidence_retain_strings(
                        state,
                        &[&key.0, &key.1, string(id)?],
                        11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>(),
                    )?;
                }
                requested
                    .entry(key.clone())
                    .or_default()
                    .insert(string(id)?.to_owned());
            }
            requested.entry(key).or_default();
        }
    }
    let capture = match selected {
        Some(paths) if state.is_some() => PublicCapture::create_evidence_selected_with_owned_state(
            root,
            paths,
            staging,
            limits,
            owner_deadline,
            state.unwrap(),
        )?,
        Some(paths) => PublicCapture::create_evidence_selected(
            root,
            paths,
            staging,
            limits,
            deadline,
            Arc::clone(&cancelled),
        )?,
        None if state.is_some() => {
            return Err(Error::Invalid(
                "owned Evidence requires actual selected paths",
            ));
        }
        None => PublicCapture::create_evidence(root, staging, limits, deadline)?,
    };
    let mut found: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    capture.visit_rows("evidence-philosophy", "views", |_, raw| {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::Budget("Evidence Lens cancelled"));
        }
        let (view, _view_hold) = match state {
            Some(state) => {
                let limits = JsonLimits::new(CAP, 96, 1_000_000, 4096)
                    .map_err(|_| Error::Budget("Evidence row JSON limits"))?;
                let (v, h) = state.serde_scoped_with_limits(raw, limits)?;
                (v, Some(h))
            }
            None => (decode(raw)?, None),
        };
        let Some(id) = view["view_id"].as_str() else {
            return Ok(());
        };
        let _key_hold = if let Some(state) = state {
            state.charge_work(id.len() + 10)?;
            Some(state.hold(id.len() + 10 + 2 * std::mem::size_of::<String>())?)
        } else {
            None
        };
        let key = ("philosophy".into(), id.to_owned());
        let Some(wanted) = requested.get(&key) else {
            return Ok(());
        };
        // Later duplicate view IDs replace the earlier membership, matching donor.
        let mut matched = BTreeSet::new();
        for field in ["node_ids", "edge_ids"] {
            for item in array(&view, field)? {
                if let Some(id) = item.as_str() {
                    if wanted.contains(id) {
                        if let Some(state) = state {
                            evidence_retain_strings(
                                state,
                                &[id],
                                11 * std::mem::size_of::<String>()
                                    + 16 * std::mem::size_of::<usize>(),
                            )?;
                        }
                        matched.insert(id.to_owned());
                    }
                }
            }
        }
        for field in ["nodes", "edges"] {
            for item in array(&view, field)? {
                let identity = if crate::source_philosophy_support::truth(&item["node_id"]) {
                    &item["node_id"]
                } else {
                    &item["edge_id"]
                };
                let id = identity.as_str();
                if let Some(id) = id {
                    if wanted.contains(id) {
                        if let Some(state) = state {
                            evidence_retain_strings(
                                state,
                                &[id],
                                11 * std::mem::size_of::<String>()
                                    + 16 * std::mem::size_of::<usize>(),
                            )?;
                        }
                        matched.insert(id.to_owned());
                    }
                }
            }
        }
        if let Some(state) = state {
            evidence_retain_strings(
                state,
                &[&key.0, &key.1],
                11 * std::mem::size_of::<((String, String), BTreeSet<String>)>()
                    + 16 * std::mem::size_of::<usize>(),
            )?;
        }
        found.insert(key, matched);
        Ok(())
    })?;
    if let Some(state) = state {
        evidence_retain_strings(
            state,
            &["corpus", "route-graph", "corpus", "route-graph"],
            11 * std::mem::size_of::<((String, String), BTreeSet<String>)>()
                + 16 * std::mem::size_of::<usize>(),
        )?;
    }
    let corpus_key = ("corpus".into(), "route-graph".into());
    found.insert(corpus_key.clone(), BTreeSet::new());
    for collection in ["nodes", "relation_edges"] {
        capture.visit_rows("evidence-corpus", collection, |_, raw| {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Err(Error::Budget("Evidence Lens cancelled"));
            }
            let (row, _row_hold) = match state {
                Some(state) => {
                    let limits = JsonLimits::new(CAP, 96, 1_000_000, 4096)
                        .map_err(|_| Error::Budget("Evidence row JSON limits"))?;
                    let (v, h) = state.serde_scoped_with_limits(raw, limits)?;
                    (v, Some(h))
                }
                None => (decode(raw)?, None),
            };
            if collection == "relation_edges" && row["owner_branch"] != "ToS/canon" {
                return Ok(());
            }
            let field = if collection == "nodes" {
                "node_id"
            } else {
                "edge_id"
            };
            if let (Some(id), Some(wanted)) = (row[field].as_str(), requested.get(&corpus_key)) {
                if wanted.contains(id) {
                    if let Some(state) = state {
                        evidence_retain_strings(
                            state,
                            &[id],
                            11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>(),
                        )?;
                    }
                    found.get_mut(&corpus_key).unwrap().insert(id.to_owned());
                }
            }
            Ok(())
        })?;
    }
    for (key, wanted) in &requested {
        let ids = found.get(key).ok_or_else(|| match state {
            Some(_) => Error::Invalid("unknown Evidence Lens view"),
            None => err(format!("unknown Evidence Lens view: {}/{}", key.0, key.1)),
        })?;
        if state.is_some() && wanted.difference(ids).next().is_some() {
            return Err(Error::Invalid(
                "Evidence Lens selection IDs absent from requested view",
            ));
        }
        let missing: Vec<_> = wanted.difference(ids).collect();
        if !missing.is_empty() {
            return Err(err(format!(
                "Evidence Lens selection IDs not in {key:?}: {missing:?}"
            )));
        }
    }
    let canon_raw = match state {
        Some(state) => read_owned(root, CANON_REF, deadline, state)?,
        None => read(root, CANON_REF, deadline)?,
    };
    let mut header = Vec::new();
    let mut anchors = BTreeMap::new();
    let l = crate::knowledge_canon_source::CanonSourceLimits {
        max_manifest_members: 1,
        max_selected_members: 1,
        max_nodes: 1,
        max_packs: 1,
        max_edges: 65536,
        max_source_bytes: CAP,
        max_raw_row_bytes: CAP,
        max_csv_fields: 1024,
        max_csv_record_bytes: CAP,
        max_forms: 1,
        max_forms_output_bytes: 1,
        max_page_rows: 1,
        max_page_bytes: 1,
        max_work_bytes: limits.max_work_bytes,
    };
    csv_records_dispatch(
        &canon_raw,
        l,
        deadline,
        cancelled.as_ref(),
        state,
        |cells| {
            if cells.is_empty() {
                return Ok(());
            }
            if header.is_empty() {
                if let Some(state) = state {
                    let bytes = cells.iter().try_fold(
                        cells.capacity() * std::mem::size_of::<String>(),
                        |n, v| {
                            n.checked_add(v.capacity())
                                .ok_or(Error::Budget("Evidence CSV header retained state"))
                        },
                    )?;
                    state.retain(bytes)?;
                }
                header = cells;
                return Ok(());
            }
            if let Some(state) = state {
                state.retain(cells.iter().try_fold(0usize, |n, c| {
                    n.checked_add(c.capacity() - c.len())
                        .ok_or(Error::Budget("Evidence moved CSV cell capacity"))
                })?)?;
                for (key, cell) in header.iter().zip(&cells) {
                    evidence_retain_strings(
                        state,
                        &[key, cell],
                        11 * std::mem::size_of::<(String, String)>()
                            + 16 * std::mem::size_of::<usize>(),
                    )?;
                }
            }
            let row: BTreeMap<_, _> = header.iter().cloned().zip(cells).collect();
            let id = row
                .get("edge_id")
                .ok_or(Error::Invalid("canonical CSV edge_id header"))?;
            if let Some(state) = state {
                evidence_retain_strings(
                    state,
                    &[id],
                    11 * std::mem::size_of::<(String, BTreeMap<String, String>)>()
                        + 16 * std::mem::size_of::<usize>(),
                )?;
            }
            anchors.insert(id.clone(), row);
            Ok(())
        },
    )?;
    if let Some(state) = state {
        evidence_retain_strings(
            state,
            &[SOURCE_REF, CANON_REF, SCHEMA_REF],
            3 * (11 * std::mem::size_of::<(String, Digest256)>()
                + 16 * std::mem::size_of::<usize>()),
        )?;
    }
    let mut opened = BTreeMap::from([
        (SOURCE_REF.to_owned(), Digest256::of_bytes(&source_raw)),
        (CANON_REF.to_owned(), Digest256::of_bytes(&canon_raw)),
        (
            SCHEMA_REF.to_owned(),
            Digest256::of_bytes(&match state {
                Some(state) => read_owned(root, SCHEMA_REF, deadline, state)?,
                None => read(root, SCHEMA_REF, deadline)?,
            }),
        ),
    ]);
    capture.charge_work((source_raw.len() + canon_raw.len()) as u64)?;
    if let Some(state) = state {
        state.retain(
            scenes
                .len()
                .checked_mul(std::mem::size_of::<Value>())
                .ok_or(Error::Budget("Evidence scene slots"))?,
        )?;
    }
    let mut output = if state.is_some() {
        Vec::with_capacity(scenes.len())
    } else {
        Vec::new()
    };
    for raw_scene in scenes {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::Budget("Evidence Lens cancelled"));
        }
        let mut scene = if let Some(state) = state {
            match state.clone_value(raw_scene)? {
                Value::Object(map) => map,
                _ => return Err(Error::Invalid("Evidence Lens scenes must be objects")),
            }
        } else {
            raw_scene.as_object().unwrap().clone()
        };
        if let Some(state) = state {
            state.retain(
                crate::knowledge_normalization::serde_object_slots_upper(scene.len() + 4)?
                    + "selection_idssource_anchorsroutessource_refs".len(),
            )?;
        }
        scene.remove("anchor_edge_ids");
        let selections = array(raw_scene, "selections")?;
        let selection_count = selections.iter().try_fold(0usize, |n, v| {
            n.checked_add(array(v, "item_ids")?.len())
                .ok_or(Error::Budget("Evidence selection slots"))
        })?;
        if let Some(state) = state {
            state.retain(
                selection_count
                    .checked_mul(std::mem::size_of::<Value>())
                    .ok_or(Error::Budget("Evidence selection vector"))?,
            )?;
        }
        let mut selection_ids = if state.is_some() {
            Vec::with_capacity(selection_count)
        } else {
            Vec::new()
        };
        let mut seen = BTreeSet::new();
        for selection in array(raw_scene, "selections")? {
            for id in array(selection, "item_ids")? {
                if let Some(state) = state {
                    evidence_retain_strings(
                        state,
                        &[string(id)?],
                        11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>(),
                    )?;
                }
                if seen.insert(string(id)?.to_owned()) {
                    selection_ids.push(match state {
                        Some(state) => state.clone_value(id)?,
                        None => id.clone(),
                    });
                }
            }
        }
        let raw_routes = array(raw_scene, "routes")?;
        if let Some(state) = state {
            state.retain(
                raw_routes
                    .len()
                    .checked_mul(std::mem::size_of::<Value>())
                    .ok_or(Error::Budget("Evidence route slots"))?,
            )?;
            evidence_retain_strings(
                state,
                &[SOURCE_REF],
                11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>(),
            )?;
        }
        let mut routes = if state.is_some() {
            Vec::with_capacity(raw_routes.len())
        } else {
            Vec::new()
        };
        let mut refs = BTreeSet::from([SOURCE_REF.to_owned()]);
        for raw_route in array(raw_scene, "routes")? {
            let mut route = if let Some(state) = state {
                match state.clone_value(raw_route)? {
                    Value::Object(map) => map,
                    _ => return Err(Error::Invalid("Evidence Lens routes must be objects")),
                }
            } else {
                raw_route
                    .as_object()
                    .ok_or(Error::Invalid("Evidence Lens routes must be objects"))?
                    .clone()
            };
            if let Some(state) = state {
                state.retain(
                    crate::knowledge_normalization::serde_object_slots_upper(route.len() + 2)?
                        + "existssha256".len()
                        + 64,
                )?;
            }
            let reference = string(&raw_route["ref"])?;
            let digest = route_digest_dispatch(root, reference, &capture, limits, deadline, state)?;
            route.insert("exists".into(), Value::Bool(true));
            route.insert("sha256".into(), Value::String(digest.to_hex()));
            if let Some(state) = state {
                evidence_retain_strings(
                    state,
                    &[reference, reference],
                    11 * std::mem::size_of::<(String, Digest256)>()
                        + 11 * std::mem::size_of::<String>()
                        + 32 * std::mem::size_of::<usize>(),
                )?;
            }
            if let Some(old) = opened.insert(reference.to_owned(), digest) {
                if old != digest {
                    return Err(Error::Invalid("Evidence Lens source changed during build"));
                }
            }
            refs.insert(reference.to_owned());
            routes.push(Value::Object(route));
        }
        let anchor_ids = array(raw_scene, "anchor_edge_ids")?;
        if let Some(state) = state {
            state.retain(
                anchor_ids
                    .len()
                    .checked_mul(std::mem::size_of::<Value>())
                    .ok_or(Error::Budget("Evidence anchor slots"))?,
            )?;
        }
        let mut source_anchors = if state.is_some() {
            Vec::with_capacity(anchor_ids.len())
        } else {
            Vec::new()
        };
        for id in array(raw_scene, "anchor_edge_ids")? {
            let edge = match state {
                Some(state) => evidence_string_owned(id, state)?,
                None => crate::source_philosophy_support::string(id),
            };
            let row = anchors.get(&edge).ok_or_else(|| match state {
                Some(_) => Error::Invalid("unknown canonical anchor edge"),
                None => err(format!("unknown canonical anchor edge: {edge}")),
            })?;
            if let Some(state) = state {
                let segment_raw = row
                    .get("anchor_segment_ids")
                    .map(String::as_str)
                    .unwrap_or("");
                state.charge_work(segment_raw.len())?;
                let count = segment_raw.split('|').filter(|v| !v.is_empty()).count();
                let pointer_capacity = if count == 0 {
                    0
                } else {
                    count
                        .checked_next_power_of_two()
                        .unwrap_or(usize::MAX)
                        .max(4)
                };
                state.retain(
                    pointer_capacity
                        .checked_mul(std::mem::size_of::<&str>())
                        .and_then(|n| n.checked_add(count * std::mem::size_of::<Value>()))
                        .and_then(|n| {
                            n.checked_add(
                                crate::knowledge_normalization::serde_object_slots_upper(4).ok()?,
                            )
                        })
                        .and_then(|n| {
                            n.checked_add(
                                "edge_idanchor_segment_idswitness_scoperelation_ref".len()
                                    + edge.len()
                                    + segment_raw.len()
                                    + CANON_REF.len()
                                    + row.get("witness_scope").map(|v| v.len()).unwrap_or(0),
                            )
                        })
                        .ok_or(Error::Budget("Evidence segment geometry"))?,
                )?;
            }
            let segments: Vec<_> = row
                .get("anchor_segment_ids")
                .map(String::as_str)
                .unwrap_or("")
                .split('|')
                .filter(|v| !v.is_empty())
                .collect();
            if segments.is_empty() {
                return Err(err(format!(
                    "canonical edge has no segment anchors: {edge}"
                )));
            }
            source_anchors.push(value!({"edge_id":edge,"anchor_segment_ids":segments,"witness_scope":row.get("witness_scope").map(String::as_str).unwrap_or(""),"relation_ref":CANON_REF}));
        }
        scene.insert("selection_ids".into(), Value::Array(selection_ids));
        scene.insert("source_anchors".into(), Value::Array(source_anchors));
        scene.insert("routes".into(), Value::Array(routes));
        if let Some(state) = state {
            state.retain(
                refs.len()
                    .checked_mul(std::mem::size_of::<Value>())
                    .ok_or(Error::Budget("Evidence source ref slots"))?,
            )?;
            for reference in &refs {
                evidence_retain_strings(state, &[reference], 0)?;
            }
        }
        scene.insert("source_refs".into(), value!(refs));
        output.push(Value::Object(scene));
    }
    if let Some(state) = state {
        state.retain(crate::knowledge_normalization::serde_object_slots_upper(7)?
        +crate::knowledge_normalization::serde_object_slots_upper(5)?
        +"schema_versionowner_reposurface_kindsource_definition_refsource_definition_sha256scenesauthority_boundary".len()
        +"tos_epistemic_evidence_projection_v1Tree-of-Sophiaderived_public_evidence_navigation".len()+SOURCE_REF.len()+128
        +"is_sourceis_canonis_semantic_truthis_rights_clearancenote".len()
        +"This projection joins explicit owner routes for inspection. The referenced source, review, canon, and rights surfaces retain authority.".len())?;
    }
    let mut payload = value!({"schema_version":"tos_epistemic_evidence_projection_v1","owner_repo":"Tree-of-Sophia","surface_kind":"derived_public_evidence_navigation","source_definition_ref":SOURCE_REF,"source_definition_sha256":opened[SOURCE_REF].to_hex(),"authority_boundary":{"is_source":false,"is_canon":false,"is_semantic_truth":false,"is_rights_clearance":false,"note":"This projection joins explicit owner routes for inspection. The referenced source, review, canon, and rights surfaces retain authority."}});
    payload
        .as_object_mut()
        .unwrap()
        .insert("scenes".into(), Value::Array(output));
    match state {
        Some(state) => validate_payload_owned(root, &payload, deadline, state)?,
        None => validate_payload(root, &payload, deadline)?,
    };
    capture.verify_inputs(limits)?;
    let raw = match state {
        Some(state) => rendered_owned(&payload, deadline, state)?,
        None => rendered(&payload, deadline)?,
    };
    capture.verify_inputs(limits)?;
    for (reference, digest) in &opened {
        if route_digest_dispatch(root, reference, &capture, limits, deadline, state)? != *digest {
            return Err(Error::Invalid("Evidence Lens source changed during build"));
        }
    }
    guard(deadline)?;
    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Err(Error::Budget("Evidence Lens cancelled"));
    }
    let projection_path = match selected {
        Some(paths) => {
            if let Some(state) = state {
                state.retain(
                    paths.evidence_projection_path.as_os_str().len()
                        + std::mem::size_of::<std::path::PathBuf>(),
                )?;
            }
            paths.evidence_projection_path.clone()
        }
        None => reference_path(root, PROJECTION_REF)?,
    };
    Ok(BuiltEvidence {
        raw,
        capture,
        opened,
        projection_path,
    })
}

pub fn build(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<Vec<u8>> {
    Ok(build_checked(
        root,
        staging,
        limits,
        deadline,
        None,
        Arc::new(AtomicBool::new(false)),
    )?
    .raw)
}

/// Read-only parity check shared by maintained check and validator commands.
pub fn check(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<()> {
    let expected = build_checked(
        root,
        staging,
        limits,
        deadline,
        None,
        Arc::new(AtomicBool::new(false)),
    )?
    .raw;
    if read(root, PROJECTION_REF, deadline)? != expected {
        return Err(err(format!("{PROJECTION_REF} is out of date")));
    }
    guard(deadline)
}

fn evidence_input_revision(built: &BuiltEvidence) -> Result<String> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-epistemic-evidence-input-cut-v1\0");
    for (path, digest, bytes) in built.capture.retained_input_members()? {
        hash.update(&(path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        hash.update(digest.as_bytes());
        hash.update(&bytes.to_be_bytes());
    }
    for (path, digest) in &built.opened {
        hash.update(&(path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        hash.update(digest.as_bytes());
    }
    hash.update(&(built.raw.len() as u64).to_be_bytes());
    hash.update(Digest256::of_bytes(&built.raw).as_bytes());
    Ok(hash.finalize().to_hex())
}

fn completed_from_built(
    root: &Path,
    built: BuiltEvidence,
    limits: PublicCaptureLimits,
    deadline: Instant,
    source_revision: String,
    main_binding: Option<(
        std::path::PathBuf,
        (u64, u64, u64, i64, i64, i64, i64),
        String,
    )>,
) -> Result<CompletedEvidenceProjection> {
    let (raw, projection_stamp, projection_sha256) =
        verify_projection_file(&built.capture, &built.projection_path, None, None, deadline)?;
    if raw != built.raw {
        return Err(err(format!("{PROJECTION_REF} is out of date")));
    }
    built.capture.verify_inputs(limits)?;
    for (reference, digest) in &built.opened {
        if route_digest(root, reference, &built.capture, limits, deadline)? != *digest {
            return Err(Error::Invalid("Evidence Lens checked input changed"));
        }
    }
    Ok(CompletedEvidenceProjection {
        root: root.to_owned(),
        main_binding,
        source_revision,
        capture: built.capture,
        limits,
        deadline,
        opened: OpenedMembership::Build(built.opened),
        projection_stamp,
        projection_sha256,
        projection_path: built.projection_path,
        raw,
    })
}

/// Run the maintained Evidence Lens check while retaining only its actual
/// checked output, independent raw capture, and complete used route closure.
/// Every selected projection source must be the same byte member of the
/// already-completed graph capture; this binds the holder to that graph cut.
pub(crate) fn check_completed(
    main_capture: &PublicCapture,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<CompletedEvidenceProjection> {
    main_capture.verify_captured_inputs()?;
    let main_root = main_capture.root().to_owned();
    let main_capture_identity = main_capture.capture_identity()?;
    let source_revision = main_capture.core_source_revision()?;
    let mut main_members = BTreeMap::new();
    for (path, digest, bytes) in main_capture.retained_input_members()? {
        main_members.insert(path, (digest, bytes));
    }
    let selected = main_capture.runtime_input_paths()?;
    let built = build_checked(
        &main_root,
        staging,
        limits,
        deadline,
        Some(&selected),
        main_capture.cancellation_handle(),
    )?;
    for (path, digest, bytes) in built.capture.retained_input_members()? {
        if main_members.get(&path) != Some(&(digest, bytes)) {
            return Err(Error::Invalid(
                "Evidence Lens source differs from completed graph capture",
            ));
        }
    }
    let holder = completed_from_built(
        main_capture.root(),
        built,
        limits,
        deadline,
        source_revision.clone(),
        Some((main_root, main_capture_identity, source_revision)),
    )?;
    main_capture.verify_captured_inputs()?;
    if main_capture.capture_identity()? != main_capture_identity {
        return Err(Error::Invalid(
            "main source capture changed during Evidence Lens check",
        ));
    }
    Ok(holder)
}

/// Same complete Evidence owner, borrowing the already-held Whole ledger.
/// A construction cutoff does not shorten the completed holder's lifetime.
pub(crate) fn check_completed_owned(
    main_capture: &PublicCapture,
    staging: &Path,
    limits: PublicCaptureLimits,
    owner_deadline: Instant,
    state: &CreationState<'_>,
) -> Result<CompletedEvidenceProjection> {
    if owner_deadline != main_capture.deadline() {
        return Err(Error::Invalid("Evidence owner lifetime changed"));
    }
    let deadline = state.operation_deadline();
    if main_capture.max_work_bytes() > limits.max_work_bytes
        || state.sql_vm_limit() > limits.max_sql_vm_steps
    {
        return Err(Error::Budget("Evidence original phase intersection"));
    }
    let mut limits = limits;
    limits.max_work_bytes = main_capture.max_work_bytes();
    limits.max_sql_vm_steps = state.sql_vm_limit();
    state.active()?;
    main_capture.verify_captured_inputs()?;
    state.retain(
        main_capture.root().as_os_str().len() + std::mem::size_of::<std::path::PathBuf>(),
    )?;
    let main_root = main_capture.root().to_owned();
    let main_capture_identity = main_capture.capture_identity()?;
    state.retain(2 * 64 + 2 * std::mem::size_of::<String>())?;
    let source_revision = main_capture.core_source_revision()?;
    let members = main_capture.retained_input_members_owned(state)?;
    state.retain(
        members
            .len()
            .checked_mul(
                11 * std::mem::size_of::<(String, (Digest256, u64))>()
                    + 16 * std::mem::size_of::<usize>(),
            )
            .ok_or(Error::Budget("Evidence main membership nodes"))?,
    )?;
    let mut main_members = BTreeMap::new();
    for (path, digest, bytes) in members {
        state.charge_work(path.len() + 40)?;
        main_members.insert(path, (digest, bytes));
    }
    let selected = main_capture.runtime_input_paths_owned(state)?;
    let built = build_checked_with_state(
        &main_root,
        staging,
        limits,
        deadline,
        owner_deadline,
        Some(&selected),
        main_capture.cancellation_handle(),
        Some(state),
    )?;
    for (path, digest, bytes) in built.capture.retained_input_members_owned(state)? {
        state.charge_work(path.len() + 40)?;
        if main_members.get(&path) != Some(&(digest, bytes)) {
            return Err(Error::Invalid(
                "Evidence Lens source differs from completed graph capture",
            ));
        }
    }
    let (raw, projection_stamp, projection_sha256) =
        verify_projection_file_owned(&built.capture, &built.projection_path, deadline, state)?;
    if raw != built.raw {
        return Err(Error::Invalid(
            "Evidence Lens checked projection is out of date",
        ));
    }
    built.capture.verify_inputs(limits)?;
    for (reference, digest) in &built.opened {
        if route_digest_dispatch(
            &main_root,
            reference,
            &built.capture,
            limits,
            deadline,
            Some(state),
        )? != *digest
        {
            return Err(Error::Invalid("Evidence Lens checked input changed"));
        }
    }
    state
        .retain(main_root.as_os_str().len() + std::mem::size_of::<CompletedEvidenceProjection>())?;
    let holder = CompletedEvidenceProjection {
        root: main_root.clone(),
        main_binding: Some((main_root, main_capture_identity, source_revision.clone())),
        source_revision,
        capture: built.capture,
        limits,
        deadline: owner_deadline,
        opened: OpenedMembership::Build(built.opened),
        projection_stamp,
        projection_sha256,
        projection_path: built.projection_path,
        raw,
    };
    main_capture.verify_captured_inputs()?;
    if main_capture.capture_identity()? != main_capture_identity {
        return Err(Error::Invalid(
            "main source capture changed during Evidence Lens check",
        ));
    }
    state.active()?;
    Ok(holder)
}

/// Isolated checked projection route for `evidence_projection()` on a partial
/// root. This binds its complete corpus/philosophy/canon/route cut without
/// requiring the optional biblio registry or building the whole graph.
pub fn check_isolated(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<CompletedEvidenceProjection> {
    let built = build_checked(
        root,
        staging,
        limits,
        deadline,
        None,
        Arc::new(AtomicBool::new(false)),
    )?;
    let source_revision = evidence_input_revision(&built)?;
    completed_from_built(root, built, limits, deadline, source_revision, None)
}

/// Isolated checked projection for the exact Evidence Lens path selected by
/// a Reference Core constructor. Corpus/philosophy data and the output path
/// remain the caller's selected paths; source definition, canon, schema, and
/// route references retain their root-relative owner semantics.
pub fn check_isolated_selected(
    root: &Path,
    selected: &crate::d1_public_capture::PublicCaptureInputPaths,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
) -> Result<CompletedEvidenceProjection> {
    let built = build_checked(root, staging, limits, deadline, Some(selected), cancelled)?;
    let source_revision = evidence_input_revision(&built)?;
    completed_from_built(root, built, limits, deadline, source_revision, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn donor_bytes_sort_nested_objects_independently_of_map_features() {
        let payload: Value =
            serde_json::from_str(r#"{"z":{"я":"Ω","a":1.0},"a":[{"z":false,"a":"é"}]}"#).unwrap();
        assert_eq!(
            rendered(&payload, Instant::now() + std::time::Duration::from_secs(1)).unwrap(),
            "{\"a\":[{\"a\":\"é\",\"z\":false}],\"z\":{\"a\":1.0,\"я\":\"Ω\"}}\n".as_bytes()
        );
    }
    fn put(root: &Path, reference: &str, raw: &[u8]) {
        let path = root.join(reference);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    fn limits() -> PublicCaptureLimits {
        PublicCaptureLimits {
            max_input_bytes: 16 * 1024 * 1024,
            max_rows: 1000,
            max_staging_bytes: 16 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
            max_sql_vm_steps: 1_000_000,
            sqlite_cache_kib: 256,
        }
    }
    fn fixture(root: &Path) -> Value {
        put(
            root,
            SCHEMA_REF,
            include_bytes!("../../../../ToS/contracts/epistemic-evidence-projection.schema.json"),
        );
        put(
            root,
            CANON_REF,
            b"edge_id,anchor_segment_ids,witness_scope\r\ne,seg-a|seg-b,\"scope, quoted\"\r\n",
        );
        let mut witness = "Источник Ω\n".as_bytes().to_vec();
        witness.resize(1_598_518, b'x');
        put(root, "docs/witness.md", &witness);
        let corpus = value!({"schema_version":"tos_corpus_index_v1","nodes":[{"node_id":"n"},{"node_id":"n2"}],"relation_edges":[{"edge_id":"e","owner_branch":"ToS/canon"}],"source_navigation":"x".repeat(3*1024*1024)});
        put(
            root,
            "ToS/derived-exports/tos_corpus_index.min.json",
            &serde_json::to_vec(&corpus).unwrap(),
        );
        put(root,"ToS/derived-exports/philosophy_graph_projection.min.json",br#"{"schema_version":"tos_philosophy_graph_projection_v2","views":[{"view_id":"10","node_ids":["other"]},{"view_id":"20","node_ids":["other"]},{"view_id":"100","node_ids":["p"]}]}"#);
        value!({"scenes":[{"scene_id":"one","selections":[{"mode":"corpus","view_id":"route-graph","item_ids":["n","e"]},{"mode":"philosophy","view_id":"100","item_ids":["p"]}],"posture":"contested-pre-canon","finding":"open","finding_ru":"Открыто Ω","conclusion":{"can_conclude":false,"canon_membership":false,"claim_evidence_closed":false,"allowed":["inspect"],"not_allowed":["closure"]},"anchor_edge_ids":["e"],"routes":[{"route_kind":"witness","ref":"docs/witness.md","status":"open"}],"gaps":[],"gaps_ru":[]}]})
    }
    fn write_source(root: &Path, source: &Value) {
        put(root, SOURCE_REF, &serde_json::to_vec(source).unwrap());
    }
    fn candidate(root: &Path, name: &str) -> Result<Vec<u8>> {
        build(
            root,
            &root.join(name),
            limits(),
            Instant::now() + std::time::Duration::from_secs(20),
        )
    }
    #[test]
    fn scene_authority_identity_anchor_and_route_fences() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut source = fixture(root);
        // The retired oracle's contrasting canon/open scenes remain distinct.
        let mut canon = source["scenes"][0].clone();
        canon["scene_id"] = value!("canon-route");
        canon["posture"] = value!("canon-retained-evidence-open");
        canon["conclusion"]["canon_membership"] = value!(true);
        canon["selections"] =
            value!([{ "mode":"corpus", "view_id":"route-graph", "item_ids":["n2"] }]);
        canon["anchor_edge_ids"] = value!([]);
        source["scenes"].as_array_mut().unwrap().push(canon);
        write_source(root, &source);
        let raw = candidate(root, "positive.sqlite").unwrap();
        let payload: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(
            payload["scenes"][0]["selection_ids"],
            value!(["n", "e", "p"])
        );
        assert_eq!(
            payload["scenes"][0]["source_anchors"][0]["anchor_segment_ids"],
            value!(["seg-a", "seg-b"])
        );
        assert_eq!(
            payload["scenes"][0]["source_anchors"][0]["witness_scope"],
            "scope, quoted"
        );
        assert_eq!(payload["scenes"][0]["finding_ru"], "Открыто Ω");
        assert_eq!(payload["authority_boundary"]["is_source"], false);
        assert_eq!(payload["scenes"].as_array().unwrap().len(), 2);
        assert_eq!(
            payload["scenes"][0]["conclusion"]["canon_membership"],
            false
        );
        assert_eq!(payload["scenes"][1]["conclusion"]["canon_membership"], true);
        for scene in payload["scenes"].as_array().unwrap() {
            assert_eq!(scene["conclusion"]["claim_evidence_closed"], false);
            assert!(
                scene["routes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|route| route["exists"] == true)
            );
        }
        assert!(!String::from_utf8_lossy(&raw).contains("/payload/"));
        assert!(!String::from_utf8_lossy(&raw).contains("file://"));
        assert!(!String::from_utf8_lossy(&raw).contains("/srv/"));
        put(root, PROJECTION_REF, &raw);
        check(
            root,
            &root.join("check.sqlite"),
            limits(),
            Instant::now() + std::time::Duration::from_secs(20),
        )
        .unwrap();
        source["scenes"][0]["conclusion"]["can_conclude"] = Value::Bool(true);
        write_source(root, &source);
        assert!(candidate(root, "conclusive.sqlite").is_err());
        source["scenes"][0]["conclusion"]["can_conclude"] = Value::Bool(false);
        source["scenes"][0]["routes"][0]["ref"] =
            value!("ToS/source-witnesses/items/x/payload/private.txt");
        write_source(root, &source);
        assert!(candidate(root, "private.sqlite").is_err());
        source["scenes"][0]["routes"][0]["ref"] = value!("../outside.txt");
        write_source(root, &source);
        assert!(candidate(root, "escape.sqlite").is_err());
        source["scenes"][0]["routes"][0]["ref"] = value!("docs/witness.md");
        source["scenes"][0]["selections"][0]["item_ids"] = value!(["absent"]);
        write_source(root, &source);
        assert!(candidate(root, "unknown.sqlite").is_err());
        assert!(build(root, &root.join("expired.sqlite"), limits(), Instant::now()).is_err());
        assert!(!root.join("expired.sqlite").exists());
    }
}
