//! Private selected-source current assessment for the maintained Sign consumer.
//! This module reads owned judgments; it never creates one or issues source admission.
use crate::source_assessment_journal::{AssessmentJournalFence, ProtectedAssessmentJournal};
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
    SourceFile,
};
use crate::source_creation_store::{active, inode, protected_configuration_parents, raw};
use crate::source_sign_native::{
    NativeReadKind, NativeReadScope, SignNativeRead, resolve_assessment, resolve_binding,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::CorpusCutReader;
use tos_validation::assessment::{
    AssessmentLimits, AssessmentReadInput, AssessmentRecordInput, AssessmentRefusal,
    AssessmentSourceRoute, evaluate_current_assessment,
};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";

pub(crate) fn require_assessment_profile(worker: &CutWorkerSchemaExecutor) -> Result<()> {
    if worker.execution_binding().schema_profile
        != tos_validation::FormatProfile::AssertedSourceCandidateV1
    {
        return Err(Error::Unsupported(
            "Sign assessment requires asserted source grammar formats",
        ));
    }
    Ok(())
}

pub(crate) fn finish_worker(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    worker
        .finish(deadline, cancelled)
        .map_err(|error| match error {
            tos_validation::item_rules::ItemRefusal::Deadline => {
                Error::Denied("Sign schema operation expired")
            }
            tos_validation::item_rules::ItemRefusal::Budget
            | tos_validation::item_rules::ItemRefusal::BudgetCheck { .. } => {
                Error::Invalid("Sign schema operation budget")
            }
            tos_validation::item_rules::ItemRefusal::Source(_) => {
                Error::Invalid("Sign schema operation failed")
            }
            tos_validation::item_rules::ItemRefusal::Unsupported(_) => {
                Error::Unsupported("Sign schema operation incomplete")
            }
        })
}

fn protected(fd: &File, uid: u32, directory: bool) -> Result<()> {
    let m = fd
        .metadata()
        .map_err(|_| Error::Invalid("Sign selected metadata"))?;
    if ![0, uid].contains(&m.uid())
        || m.mode() & 0o022 != 0
        || m.is_dir() != directory
        || !directory && !m.is_file()
    {
        return Err(Error::Denied("Sign selected owner boundary"));
    }
    Ok(())
}

/// Only the protected v2/v3 owner configuration supplies this root. Original
/// content has its own explicit read scope; it never enters the authored cut.
struct SignSourceReader<'a> {
    root_path: PathBuf,
    root: File,
    identity: (u64, u64),
    uid: u32,
    cut: &'a CorpusCutReader,
    observed: BTreeMap<String, Vec<u8>>,
    native_reads: BTreeSet<String>,
    native_bytes: usize,
    publication: Option<Vec<u8>>,
    source_reads: BTreeSet<String>,
    source_bytes: usize,
    source_carriers: Vec<String>,
    source_dependencies: BTreeSet<String>,
    identity_snapshot: Option<String>,
    profile_native_inputs: BTreeMap<(String, &'static str), Digest256>,
}
impl<'a> SignSourceReader<'a> {
    fn select(
        root_path: &Path,
        cut: &'a CorpusCutReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        active(deadline, cancelled)?;
        let uid = rustix::process::getuid().as_raw();
        if uid != rustix::process::geteuid().as_raw() || root_path == Path::new("/") {
            return Err(Error::Denied("Sign dedicated nonsetuid root"));
        }
        protected_configuration_parents(root_path, uid)?;
        let root = tos_fd_open::open_absolute_directory(root_path)
            .map_err(|_| Error::Denied("Sign selected root descriptor"))?;
        protected(&root, uid, true)?;
        let identity = inode(
            &root
                .metadata()
                .map_err(|_| Error::Invalid("Sign root identity"))?,
        );
        let mut reader = Self {
            root_path: root_path.to_owned(),
            root,
            identity,
            uid,
            cut,
            observed: BTreeMap::new(),
            native_reads: BTreeSet::new(),
            native_bytes: 0,
            publication: None,
            source_reads: BTreeSet::new(),
            source_bytes: 0,
            source_carriers: Vec::new(),
            source_dependencies: BTreeSet::new(),
            identity_snapshot: None,
            profile_native_inputs: BTreeMap::new(),
        };
        reader.publication = reader.publication_current(deadline, cancelled)?;
        Ok(reader)
    }
    fn open_optional(&self, name: &str) -> Result<Option<File>> {
        let path = RelativePath::parse(name)
            .map_err(|_| Error::Denied("Sign exact relative source reference"))?;
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        let mut parent = tos_fd_open::reopen_directory(&self.root)
            .map_err(|_| Error::Denied("Sign source root reopen"))?;
        for part in &parts[..parts.len() - 1] {
            parent = tos_fd_open::open_directory_at(&parent, Path::new(part))
                .map_err(|_| Error::Denied("Sign selected ancestor"))?;
            protected(&parent, self.uid, true)?;
        }
        let file = match tos_fd_open::open_regular_at(&parent, Path::new(parts.last().unwrap())) {
            Ok(file) => file,
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(None);
            }
            Err(_) => return Err(Error::Denied("Sign selected regular input")),
        };
        protected(&file, self.uid, false)?;
        Ok(Some(file))
    }
    fn current_raw(
        &self,
        name: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        let mut file = self
            .open_optional(name)?
            .ok_or(Error::Conflict("Sign selected input absent"))?;
        raw(&mut file, cap, deadline, cancelled)
    }
    fn authored(
        &mut self,
        name: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        if let Some(raw) = self.observed.get(name) {
            if raw.len() > cap {
                return Err(Error::Invalid("Sign selected input budget"));
            }
            return Ok(raw.clone());
        }
        let path = RelativePath::parse(name)
            .map_err(|_| Error::Invalid("Sign source member reference"))?;
        let selected = self
            .cut
            .current()
            .member(&path)
            .ok_or(Error::Conflict("Sign selected authored member absent"))?;
        if selected.size_bytes > cap as u64 {
            return Err(Error::Invalid("Sign selected member byte budget"));
        }
        let raw = self.current_raw(name, cap, deadline, cancelled)?;
        if Digest256::of_bytes(&raw) != selected.sha256 || raw.len() as u64 != selected.size_bytes {
            return Err(Error::Conflict(
                "Sign current authored bytes differ from selected cut",
            ));
        }
        self.observed.insert(name.to_owned(), raw.clone());
        Ok(raw)
    }
    fn source(
        &mut self,
        name: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        let limit = if self.source_reads.contains(name) {
            cap
        } else {
            cap.min(8 * 1_048_576 - self.source_bytes)
        };
        let raw = self.authored(name, limit, deadline, cancelled)?;
        if self.source_reads.insert(name.to_owned()) {
            self.source_bytes = self
                .source_bytes
                .checked_add(raw.len())
                .filter(|n| *n <= 8 * 1_048_576)
                .ok_or(Error::Invalid(
                    "Sign selected source/contract snapshot budget",
                ))?;
        }
        Ok(raw)
    }
    fn publication_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<u8>>> {
        let path = RelativePath::parse(CONTROL).unwrap();
        let current = self
            .open_optional(CONTROL)?
            .map(|mut file| raw(&mut file, 8192, deadline, cancelled))
            .transpose()?;
        if current.is_none() && self.cut.current().member(&path).is_some() {
            return Err(Error::Conflict(
                "Sign selected publication state disappeared",
            ));
        }
        if let Some(raw) = &current {
            let value = cmd::parse(raw)?;
            crate::source_revisions::state(&value)?;
            if cmd::text(&value, "phase")? != "ready" {
                return Err(Error::Conflict(
                    "Sign participating source publication pending",
                ));
            }
            let selected = self.cut.current().member(&path).ok_or(Error::Conflict(
                "Sign publication state absent from selected cut",
            ))?;
            if Digest256::of_bytes(raw) != selected.sha256 {
                return Err(Error::Conflict("Sign selected publication epoch differs"));
            }
        }
        Ok(current)
    }
    fn context(&self, base: &CommandContext) -> CommandContext {
        let mut ctx = base.clone();
        ctx.files = self
            .observed
            .iter()
            .filter(|(name, _)| {
                self.cut
                    .current()
                    .member(&RelativePath::parse(name).unwrap())
                    .is_some()
            })
            .map(|(name, raw)| SourceFile {
                path: RelativePath::parse(name).unwrap(),
                raw: raw.clone(),
            })
            .collect();
        ctx
    }
}
impl SignNativeRead for SignSourceReader<'_> {
    fn read(
        &mut self,
        name: &str,
        kind: NativeReadKind,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        let path =
            RelativePath::parse(name).map_err(|_| Error::Denied("Sign native reference path"))?;
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        let content = matches!(kind, NativeReadKind::Content);
        let prefix = match kind {
            NativeReadKind::Schema => "ToS/contracts/",
            NativeReadKind::Support => "ToS/",
            _ => "ToS/source-witnesses/",
        };
        if !name.starts_with(prefix)
            || name.starts_with("ToS/source-witnesses/owner-local/")
            || parts.contains(&"catalog")
            || !content
                && parts
                    .iter()
                    .any(|p| matches!(*p, "payload" | "local-content"))
        {
            return Err(Error::Denied("Sign native selected owner namespace"));
        }
        let existing = self.native_reads.contains(name);
        if !existing && self.native_reads.len() >= 128 {
            return Err(Error::Invalid("Sign native shared dependency count"));
        }
        let limit = if existing {
            cap
        } else {
            cap.min(16 * 1_048_576 - self.native_bytes)
        };
        let bytes = if content {
            match self.observed.get(name) {
                Some(raw) => {
                    if raw.len() > limit {
                        return Err(Error::Invalid("Sign native content budget"));
                    }
                    raw.clone()
                }
                None => {
                    let bytes = self.current_raw(name, limit, deadline, cancelled)?;
                    self.observed.insert(name.to_owned(), bytes.clone());
                    bytes
                }
            }
        } else {
            self.authored(name, limit.min(1_048_576), deadline, cancelled)?
        };
        if !existing {
            self.native_bytes = self
                .native_bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= 16 * 1_048_576)
                .ok_or(Error::Invalid("Sign native shared input budget"))?;
            self.native_reads.insert(name.to_owned());
        }
        Ok(bytes)
    }
    fn verify_current(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        active(deadline, cancelled)?;
        if rustix::process::getuid().as_raw() != self.uid
            || rustix::process::geteuid().as_raw() != self.uid
        {
            return Err(Error::Denied("Sign source account changed"));
        }
        protected_configuration_parents(&self.root_path, self.uid)?;
        let root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| Error::Conflict("Sign source root unavailable"))?;
        protected(&root, self.uid, true)?;
        if inode(
            &root
                .metadata()
                .map_err(|_| Error::Invalid("Sign root current identity"))?,
        ) != self.identity
        {
            return Err(Error::Conflict("Sign selected root replaced"));
        }
        if self.publication_current(deadline, cancelled)? != self.publication {
            return Err(Error::Conflict("Sign source publication epoch changed"));
        }
        for (name, bytes) in &self.observed {
            if self.current_raw(name, bytes.len(), deadline, cancelled)? != *bytes {
                return Err(Error::Conflict("Sign selected dependency changed"));
            }
        }
        Ok(())
    }
    fn owner_local(&self, _name: &str) -> Result<bool> {
        Ok(false)
    }
}

fn envelope(
    payload: &JsonValue,
    identity: &str,
    version: &str,
    origin: &JsonValue,
) -> Result<JsonValue> {
    Ok(cmd::object(vec![
        ("id", cmd::field(payload, identity)?.clone()),
        ("version", cmd::field(payload, version)?.clone()),
        ("payload", payload.clone()),
        ("origin_id", origin.clone()),
    ]))
}
fn envelope_ref(row: &JsonValue) -> Result<JsonValue> {
    Ok(cmd::object(vec![
        ("id", cmd::field(row, "id")?.clone()),
        ("version", cmd::field(row, "version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(cmd::field(row, "payload")?)?.to_prefixed()),
        ),
    ]))
}
fn record_input(row: &JsonValue) -> Result<AssessmentRecordInput> {
    Ok(AssessmentRecordInput {
        envelope: cmd::canonical(row)?,
    })
}
fn decoded(value: &serde_json::Value) -> Result<JsonValue> {
    cmd::parse(
        &serde_json::to_vec(value)
            .map_err(|_| Error::Invalid("Sign assessment output serialization"))?,
    )
}

fn schema(
    reader: &mut SignSourceReader<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    name: &str,
    value: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    reader.source_dependencies.insert(name.to_owned());
    let raw = reader.source(name, 1_048_576, deadline, cancelled)?;
    if worker.source_revision() != reader.cut.current().revision()
        || worker.contract_digest(name) != Some(Digest256::of_bytes(&raw))
    {
        return Err(Error::Conflict("Sign selected schema worker binding"));
    }
    if !worker
        .check(
            "Sign selected source",
            &cmd::canonical(value)?,
            name,
            deadline,
            cancelled,
        )
        .map_err(|reason| Error::SchemaExecution {
            path: "Sign selected source".to_owned(),
            root: name.to_owned(),
            reason,
        })?
    {
        return Err(Error::Invalid("Sign selected source violates schema"));
    }
    Ok(())
}
fn local_claim(
    reader: &mut SignSourceReader<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    claim: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let raw = cmd::canonical(claim)?;
    let report = tos_validation::record_rules::validate_source_claim_from_cut(
        reader.cut,
        &raw,
        worker,
        tos_validation::item_rules::ItemLimits {
            max_member_bytes: 1_048_576,
            max_total_bytes: 8 * 1_048_576,
            max_state_bytes: 8 * 1_048_576,
            max_issues: 128,
            deadline,
        },
        cancelled,
    )
    .map_err(|_| Error::Unsupported("Sign source Claim local profile execution"))?;
    if report.source_revision != reader.cut.current().revision()
        || report.source_input_sha256 != Digest256::of_bytes(&raw)
        || report.dependency_digests.len() > 128
        || !report.issues.is_empty()
    {
        return Err(Error::Invalid("Sign selected Claim local profile"));
    }
    for (name, digest) in report.dependency_digests {
        reader.source_dependencies.insert(name.clone());
        let raw = reader.source(&name, 1_048_576, deadline, cancelled)?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(Error::Conflict("Sign Claim selected resource digest"));
        }
    }
    Ok(())
}
fn source_envelopes(
    reader: &mut SignSourceReader<'_>,
    base: &CommandContext,
    config: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<JsonValue>> {
    let bindings = cmd::array(config, "source_records")?;
    if bindings.len() > 1024 {
        return Err(Error::Invalid("Sign configured source selection count"));
    }
    let mut selected = BTreeSet::new();
    let mut files = BTreeMap::<String, BTreeMap<String, JsonValue>>::new();
    let mut rows = Vec::new();
    for binding in bindings {
        active(deadline, cancelled)?;
        cmd::exact_keys(binding, &["path", "record_id", "origin_id"])?;
        let id = cmd::text(binding, "record_id")?;
        if id.is_empty() || !selected.insert(id) {
            return Err(Error::Invalid("Sign distinct selected source identity"));
        }
        let name = cmd::text(binding, "path")?;
        let path =
            RelativePath::parse(name).map_err(|_| Error::Denied("Sign selected source path"))?;
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        if !name.starts_with("ToS/source-witnesses/")
            || name.starts_with("ToS/source-witnesses/owner-local/")
            || parts
                .iter()
                .any(|p| matches!(*p, "payload" | "local-content" | "catalog"))
            || !name.ends_with(".json") && !name.ends_with(".jsonl")
        {
            return Err(Error::Denied("Sign public selected metadata carrier"));
        }
        if !files.contains_key(name) {
            reader.source_carriers.push(name.to_owned());
            let raw = reader.source(name, 8 * 1_048_576, deadline, cancelled)?;
            let mut indexed = BTreeMap::new();
            let requested = bindings
                .iter()
                .filter(|b| b.object_get("path").and_then(JsonValue::as_str) == Some(name))
                .filter_map(|b| b.object_get("record_id").and_then(JsonValue::as_str))
                .collect::<BTreeSet<_>>();
            let mut seen_ids = BTreeSet::new();
            let values: Box<dyn Iterator<Item = Result<JsonValue>> + '_> = if name
                .ends_with(".jsonl")
            {
                Box::new(
                    raw.split(|byte| *byte == b'\n')
                        .filter(|line| {
                            line.iter()
                                .any(|byte| !matches!(*byte, b' ' | b'\t' | b'\r' | b'\n'))
                        })
                        .map(|line| {
                            let end = line
                                .iter()
                                .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\r' | b'\n'))
                                .map_or(0, |i| i + 1);
                            let trimmed = &line[..end];
                            if trimmed.len() > 1_048_576 {
                                return Err(Error::Invalid("Sign selected source row byte budget"));
                            }
                            cmd::parse(trimmed)
                        }),
                )
            } else {
                let value = cmd::parse(&raw)?;
                if value
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                    == Some("tos_human_form_set_v1")
                {
                    let forms = cmd::array(&value, "forms")?;
                    if forms.len() > 1024 {
                        return Err(Error::Invalid("Sign selected source record count"));
                    }
                    Box::new(forms.to_vec().into_iter().map(Ok))
                } else {
                    Box::new(std::iter::once(Ok(value)))
                }
            };
            for value in values {
                active(deadline, cancelled)?;
                let value = value?;
                let identity = match value
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                {
                    Some("tos_claim_packet_v1" | "tos_historical_claim_v1") => {
                        Some(("claim_id", "claim_version"))
                    }
                    Some("tos_corpus_record_v1" | "tos_historical_record_v1") => {
                        Some(("record_id", "record_version"))
                    }
                    Some("tos_human_form_v1") => Some(("form_id", "form_version")),
                    _ if parts.last() == Some(&"source-claims.jsonl")
                        && value.object_get("claim_id").is_some() =>
                    {
                        Some(("claim_id", "claim_version"))
                    }
                    _ if value
                        .object_get("record_type")
                        .and_then(JsonValue::as_str)
                        .is_some_and(|kind| {
                            parts
                                .last()
                                .is_some_and(|leaf| **leaf == format!("{kind}.json"))
                        }) =>
                    {
                        Some(("record_id", "record_version"))
                    }
                    _ => None,
                };
                let Some((key, version)) = identity else {
                    continue;
                };
                if value.object_get("claim_id").is_some() {
                    // Sign consumes the shared declared source-claims carrier. Historical
                    // claims keep their separately captured owner route.
                    if parts.last() == Some(&"source-claims.jsonl") {
                        reader.source(RELATIONS, 1_048_576, deadline, cancelled)?;
                        let relations = cmd::parse(&reader.observed[RELATIONS])?;
                        let known = cmd::array(&relations, "relations")?.iter().any(|relation| {
                            relation
                                .object_get("source_mappings")
                                .and_then(JsonValue::as_array)
                                .is_some_and(|mappings| {
                                    mappings.iter().any(|mapping| {
                                        mapping
                                            .object_get("source_graph")
                                            .and_then(JsonValue::as_str)
                                            == Some("source-claims")
                                            && mapping
                                                .object_get("scope")
                                                .and_then(JsonValue::as_str)
                                                == Some("claim-predicate")
                                            && mapping.object_get("source_predicate_id")
                                                == value.object_get("predicate")
                                    })
                                })
                                && relation
                                    .object_get("source_claim_profile")
                                    .and_then(|profile| profile.object_get("schemas"))
                                    .and_then(JsonValue::as_array)
                                    .is_some_and(|routes| {
                                        routes.iter().any(|route| {
                                            route.object_get("schema_version")
                                                == value.object_get("schema_version")
                                        })
                                    })
                        });
                        if !known {
                            continue;
                        }
                        local_claim(reader, worker, &value, deadline, cancelled)?;
                    } else if cmd::text(&value, "schema_version")? == "tos_historical_claim_v1" {
                        schema(
                            reader,
                            worker,
                            "ToS/contracts/historical-claim.schema.json",
                            &value,
                            deadline,
                            cancelled,
                        )?;
                    } else if cmd::text(&value, "schema_version")? == "tos_claim_packet_v1" {
                        // The generic envelope is not a declared source-claims
                        // profile; its selected Claim remains an opaque body.
                    } else {
                        continue;
                    }
                } else if value.object_get("form_id").is_some() {
                    schema(
                        reader,
                        worker,
                        "ToS/contracts/human-form.schema.json",
                        &value,
                        deadline,
                        cancelled,
                    )?;
                } else if cmd::text(&value, "schema_version")? == "tos_corpus_record_v1" {
                    schema(
                        reader,
                        worker,
                        "ToS/contracts/corpus-record.schema.json",
                        &value,
                        deadline,
                        cancelled,
                    )?;
                } else if cmd::text(&value, "schema_version")? == "tos_historical_record_v1" {
                    schema(
                        reader,
                        worker,
                        "ToS/contracts/historical-record.schema.json",
                        &value,
                        deadline,
                        cancelled,
                    )?;
                } else {
                    validate_source_profile(
                        reader, base, worker, name, &value, deadline, cancelled,
                    )?;
                }
                let identity = cmd::text(&value, key)?.to_owned();
                if tos_foundation::python_strip_unicode16_v1(&identity, 1_048_576)
                    .map_err(|_| Error::Invalid("Sign source identity budget"))?
                    .is_empty()
                    || cmd::canonical(&value)?.len() > 1_048_576
                    || cmd::integer(&value, version)? == 0
                    || !seen_ids.insert(identity.clone())
                {
                    return Err(Error::Invalid(
                        "Sign current source identity duplicate/version",
                    ));
                }
                if !name.ends_with(".jsonl") || requested.contains(identity.as_str()) {
                    indexed.insert(identity, value);
                }
            }
            files.insert(name.to_owned(), indexed);
        }
        let body = files[name]
            .get(id)
            .ok_or(Error::Invalid("Sign source record absent or unsupported"))?;
        if body.object_get("claim_id").is_some() || body.object_get("visibility").is_some() {
            if !matches!(
                body.object_get("visibility").and_then(JsonValue::as_str),
                Some("public" | "public_metadata_only")
            ) {
                return Err(Error::Denied("Sign source metadata visibility"));
            }
        }
        let (key, version) = if body.object_get("claim_id").is_some() {
            ("claim_id", "claim_version")
        } else if body.object_get("form_id").is_some() {
            ("form_id", "form_version")
        } else {
            ("record_id", "record_version")
        };
        rows.push(envelope(
            body,
            key,
            version,
            cmd::field(binding, "origin_id")?,
        )?);
    }
    Ok(rows)
}
fn validate_source_profile(
    reader: &mut SignSourceReader<'_>,
    base: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    name: &str,
    body: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    for name in [
        ENTITIES,
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/corpus-record.schema.json",
    ] {
        reader.source_dependencies.insert(name.to_owned());
        reader.source(name, 1_048_576, deadline, cancelled)?;
    }
    let registry = cmd::parse(reader.observed.get(ENTITIES).unwrap())?;
    let kind = cmd::text(body, "record_type")?;
    let entries = cmd::array(&registry, "types")?
        .iter()
        .filter(|entry| {
            entry
                .object_get("source_record_profile")
                .and_then(|p| p.object_get("record_type"))
                .and_then(JsonValue::as_str)
                == Some(kind)
        })
        .collect::<Vec<_>>();
    if entries.len() != 1 {
        return Err(Error::Unsupported("Sign selected metadata profile route"));
    }
    let entry = entries[0];
    let profile = cmd::field(entry, "source_record_profile")?;
    let route = cmd::array(profile, "schemas")?
        .iter()
        .find(|route| route.object_get("schema_version") == body.object_get("schema_version"))
        .ok_or(Error::Unsupported("Sign selected metadata schema route"))?;
    for dependency in cmd::array(route, "schema_dependencies")? {
        reader.source(
            dependency
                .as_str()
                .ok_or(Error::Invalid("Sign profile schema dependency"))?,
            1_048_576,
            deadline,
            cancelled,
        )?;
    }
    reader.source(
        cmd::text(route, "schema_ref")?,
        1_048_576,
        deadline,
        cancelled,
    )?;
    if let Some(binding) = body.object_get("native_text_binding") {
        // This checks the exact profile's real metadata route. It does not populate
        // native_records: only independently configured content reads do that.
        let resolved = resolve_binding(
            reader,
            worker,
            binding,
            NativeReadScope::MetadataOnly,
            deadline,
            cancelled,
        )?;
        for input in resolved.inputs {
            let raw = reader.source(&input.reference, 1_048_576, deadline, cancelled)?;
            if Digest256::of_bytes(&raw) != input.raw_sha256 {
                return Err(Error::Conflict("Sign profile native snapshot bytes differ"));
            }
            let key = (input.reference, input.category);
            if reader
                .profile_native_inputs
                .insert(key, input.raw_sha256)
                .is_some_and(|old| old != input.raw_sha256)
            {
                return Err(Error::Conflict("Sign profile native dependency changed"));
            }
        }
        reader
            .source_dependencies
            .extend(resolved.schema_digests.into_keys());
    }
    // Reuse the existing complete-cut native identity reservation, over its
    // actual packet partition. There is no filesystem/corpus traversal here.
    if ["occurrence", "lexeme", "sense", "sign", "concept"].contains(&kind) {
        let names = reader
            .cut
            .current()
            .members()
            .filter_map(|member| {
                let name = member.path.as_str();
                let leaf = name.rsplit('/').next().unwrap_or(name);
                (name.starts_with("ToS/source-witnesses/")
                    && leaf.starts_with("semantic-annotation")
                    && leaf.ends_with(".json")
                    && !name
                        .split('/')
                        .any(|p| matches!(p, "payload" | "local-content" | "catalog")))
                .then(|| name.to_owned())
            })
            .collect::<Vec<_>>();
        let used = !names.is_empty();
        for name in names {
            reader.source(&name, 1_048_576, deadline, cancelled)?;
        }
        if used {
            reader.source(
                "ToS/contracts/semantic-annotation-packet-v2.schema.json",
                1_048_576,
                deadline,
                cancelled,
            )?;
        }
    }
    let derived = cmd::object(vec![
        ("profile_type_id", cmd::field(entry, "type_id")?.clone()),
        ("source_path", cmd::string(name)),
        ("record_id", cmd::field(body, "record_id")?.clone()),
    ]);
    let (_, resources, identities, _) = crate::source_revisions::public_profile(
        Some(reader.cut),
        worker,
        deadline,
        cancelled,
        &reader.context(base),
        &derived,
        body,
    )?;
    reader.source_dependencies.extend(resources);
    if let Some(identities) = identities {
        if reader
            .identity_snapshot
            .as_ref()
            .is_some_and(|old| old != &identities)
        {
            return Err(Error::Conflict("Sign native identity reservation changed"));
        }
        reader.identity_snapshot = Some(identities);
    }
    Ok(())
}

fn claim_ground_refs(
    reader: &mut SignSourceReader<'_>,
    rows: &[JsonValue],
    config: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<BTreeMap<String, Vec<JsonValue>>> {
    reader.source(ENTITIES, 1_048_576, deadline, cancelled)?;
    reader.source(RELATIONS, 1_048_576, deadline, cancelled)?;
    reader
        .source_dependencies
        .extend([ENTITIES.to_owned(), RELATIONS.to_owned()]);
    let entities = cmd::parse(reader.observed.get(ENTITIES).unwrap())?;
    let relations = cmd::parse(reader.observed.get(RELATIONS).unwrap())?;
    let types = cmd::array(&entities, "types")?;
    let mut grounded: BTreeMap<String, Vec<JsonValue>> = BTreeMap::new();
    let records = rows
        .iter()
        .map(|row| Ok((cmd::text(row, "id")?, row)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    for row in rows {
        let body = cmd::field(row, "payload")?;
        if body.object_get("claim_id").is_none() {
            continue;
        }
        let path = cmd::array(config, "source_records")?
            .iter()
            .find(|binding| binding.object_get("record_id") == row.object_get("id"))
            .ok_or(Error::Invalid("Sign selected Claim source locator"))?;
        let name = cmd::text(path, "path")?;
        if name.ends_with("/historical-claims.jsonl") {
            return Err(Error::Unsupported(
                "Sign historical Claim grounding requires its historical owner adapter",
            ));
        }
        if !name.ends_with("/source-claims.jsonl") {
            continue;
        }
        let predicate = cmd::text(body, "predicate")?;
        let owners = cmd::array(&relations, "relations")?
            .iter()
            .filter(|relation| {
                relation.object_get("source_claim_profile").is_some()
                    && relation
                        .object_get("source_mappings")
                        .and_then(JsonValue::as_array)
                        .is_some_and(|mappings| {
                            mappings.iter().any(|mapping| {
                                mapping
                                    .object_get("source_graph")
                                    .and_then(JsonValue::as_str)
                                    == Some("source-claims")
                                    && mapping.object_get("scope").and_then(JsonValue::as_str)
                                        == Some("claim-predicate")
                                    && mapping
                                        .object_get("source_predicate_id")
                                        .and_then(JsonValue::as_str)
                                        == Some(predicate)
                            })
                        })
            })
            .collect::<Vec<_>>();
        if owners.len() != 1 {
            return Err(Error::Invalid("Sign Claim declared relation owner"));
        }
        let relation = owners[0];
        let profile = cmd::field(relation, "source_claim_profile")?;
        let route = cmd::text(profile, "reader")?;
        let mut endpoints = vec![(
            cmd::text(body, "subject_ref")?.to_owned(),
            cmd::field(relation, "domain_type_ids")?.clone(),
        )];
        let object = cmd::field(body, "object")?;
        let mut exact = Vec::new();
        if route.starts_with("identity-transition-") {
            endpoints.clear();
            for section in ["predecessors", "successors"] {
                for reference in cmd::array(object, section)? {
                    exact.push(reference.clone());
                    endpoints.push((
                        cmd::text(reference, "id")?.to_owned(),
                        cmd::field(relation, "domain_type_ids")?.clone(),
                    ));
                }
            }
            if let Some(prior) = object
                .object_get("supersedes_proposal")
                .filter(|v| !v.is_null())
            {
                exact.push(prior.clone());
            }
            for related in cmd::array(object, "unresolved_links")? {
                exact.push(cmd::field(related, "claim")?.clone());
            }
        } else if route == "structured-reference-value-v1" {
            let allowed = cmd::field(
                cmd::field(profile, "object_reference_set")?,
                "member_type_ids",
            )?;
            for member in cmd::array(object, "members")? {
                endpoints.push((
                    member
                        .as_str()
                        .ok_or(Error::Invalid("Sign Claim member identity"))?
                        .to_owned(),
                    allowed.clone(),
                ));
            }
        } else if route == "historical-temporal-v1" || route == "document-catalogue-temporal-v1" {
            if let Some(anchor) = object
                .object_get("relative")
                .and_then(|r| r.object_get("anchor_ref"))
                .and_then(JsonValue::as_str)
            {
                endpoints.push((
                    anchor.to_owned(),
                    JsonValue::Array(vec![cmd::string("tos.entity.historical-situation")]),
                ));
            }
        } else if route != "structured-value-v1" {
            endpoints.push((
                object
                    .as_str()
                    .ok_or(Error::Invalid("Sign Claim exact endpoint"))?
                    .to_owned(),
                cmd::field(relation, "range_type_ids")?.clone(),
            ));
        }
        let mut refs = BTreeMap::new();
        for (id, allowed) in endpoints {
            let endpoint = records.get(id.as_str()).ok_or(Error::Invalid(
                "Sign endpoint absent from independently selected sources",
            ))?;
            let payload = cmd::field(endpoint, "payload")?;
            let kind = cmd::text(payload, "record_type")?;
            let mapped = types
                .iter()
                .filter(|entry| {
                    entry
                        .object_get("source_mappings")
                        .and_then(JsonValue::as_array)
                        .is_some_and(|mappings| {
                            mappings.iter().any(|m| {
                                m.object_get("source_graph").and_then(JsonValue::as_str)
                                    == Some("source-claims")
                                    && m.object_get("source_kind_id").and_then(JsonValue::as_str)
                                        == Some(kind)
                            })
                        })
                })
                .collect::<Vec<_>>();
            if mapped.len() != 1 {
                return Err(Error::Invalid("Sign endpoint source-kind owner"));
            }
            let entry = mapped[0];
            let ancestry = crate::source_claims::ancestry(types, cmd::text(entry, "type_id")?)?;
            if !allowed
                .as_array()
                .ok_or(Error::Invalid("Sign endpoint allowed types"))?
                .iter()
                .any(|id| id.as_str().is_some_and(|id| ancestry.contains(id)))
            {
                return Err(Error::Invalid("Sign Claim endpoint domain/range"));
            }
            if route.starts_with("identity-transition-") {
                // The actual proposal route has no abstract ancestry fallback: the
                // selected native or explicitly declared semantic role owns eligibility.
                let role = cmd::text(entry, "object_role")?;
                if entry.object_get("abstract") != Some(&JsonValue::Bool(false))
                    || !matches!(role, "identity" | "semantic")
                    || route == "identity-transition-v1" && role != "identity"
                {
                    return Err(Error::Invalid("Sign proposal participant concrete role"));
                }
            }
            refs.insert(id, envelope_ref(endpoint)?);
        }
        for reference in exact {
            let id = cmd::text(&reference, "id")?;
            let row = records
                .get(id)
                .ok_or(Error::Conflict("Sign frozen proposal source absent"))?;
            if !cmd::same(&envelope_ref(row)?, &reference)? {
                return Err(Error::Conflict("Sign frozen proposal exact source differs"));
            }
            refs.insert(id.to_owned(), reference);
        }
        grounded.insert(
            cmd::text(row, "id")?.to_owned(),
            refs.into_values().collect(),
        );
    }
    for row in rows {
        let body = cmd::field(row, "payload")?;
        if body
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            != Some("tos_human_form_v1")
        {
            continue;
        }
        let subject = cmd::field(body, "subject")?;
        let id = cmd::text(subject, "id")?;
        if !id.starts_with("tos.claim.") {
            continue;
        }
        let parent = records.get(id).ok_or(Error::Invalid(
            "Sign source form needs source-selected Claim",
        ))?;
        let dependencies = grounded.get(id).ok_or(Error::Invalid(
            "Sign source form needs declared Claim grounding",
        ))?;
        if !cmd::same(subject, &envelope_ref(parent)?)? {
            return Err(Error::Conflict("Sign source form current Claim differs"));
        }
        let mut refs = vec![subject.clone()];
        refs.extend(dependencies.clone());
        grounded.insert(cmd::text(row, "id")?.to_owned(), refs);
    }
    let _ = worker;
    Ok(grounded)
}
fn owner_envelope(value: &JsonValue) -> Result<AssessmentRecordInput> {
    let mut fields = vec!["id", "version", "payload"];
    if value.object_get("origin_id").is_some() {
        fields.push("origin_id");
    }
    cmd::exact_keys(value, &fields)?;
    record_input(&cmd::object(vec![
        ("id", cmd::field(value, "id")?.clone()),
        ("version", cmd::field(value, "version")?.clone()),
        ("payload", cmd::field(value, "payload")?.clone()),
        (
            "origin_id",
            value
                .object_get("origin_id")
                .cloned()
                .unwrap_or(JsonValue::Null),
        ),
    ]))
}
fn owner_envelopes(config: &JsonValue, key: &str) -> Result<Vec<AssessmentRecordInput>> {
    let rows = cmd::array(config, key)?;
    if rows.len() > 1024 {
        return Err(Error::Invalid("Sign configured owner records count"));
    }
    rows.iter().map(owner_envelope).collect()
}

/// The final current view is computed only from this protected selection while
/// the real subject flock is held. No caller can submit a ready report/basis.
pub(crate) struct SignPromotionRead<'a> {
    owner: ProtectedAssessmentJournal,
    reader: SignSourceReader<'a>,
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    configuration: JsonValue,
    prepared_sources: Option<SignSourceAssembly>,
}

struct NativeSelection {
    rows: Vec<JsonValue>,
    summaries: Vec<JsonValue>,
    snapshots: Vec<JsonValue>,
}

fn native_envelopes(
    reader: &mut SignSourceReader<'_>,
    config: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<NativeSelection> {
    let Some(selections) = config.object_get("native_text_units") else {
        return Ok(NativeSelection {
            rows: vec![],
            summaries: vec![],
            snapshots: vec![],
        });
    };
    let selections = selections
        .as_array()
        .ok_or(Error::Invalid("Sign native selections list"))?;
    if selections.len() > 64 {
        return Err(Error::Invalid("Sign native selection budget"));
    }
    let subjects = cmd::field(config, "subjects")?;
    let mut selected = BTreeSet::new();
    // All access grants are checked before the first content read.
    for selection in selections {
        cmd::exact_keys(selection, &["binding", "origin_id", "read_scope"])?;
        let id = cmd::text(cmd::field(selection, "binding")?, "unit_id")?;
        if !selected.insert(id) {
            return Err(Error::Invalid("Sign repeated native selected unit"));
        }
        if !matches!(
            cmd::text(selection, "read_scope")?,
            "metadata_only" | "exact_public" | "exact_owner_local"
        ) {
            return Err(Error::Invalid("Sign native read scope"));
        }
        let scope = cmd::field(subjects, id)?;
        cmd::exact_keys(
            scope,
            &[
                "record",
                "assertion_layer",
                "risk",
                "languages",
                "maker_id",
                "requested_use",
                "access_allowed",
            ],
        )?;
        if scope.object_get("access_allowed") != Some(&JsonValue::Bool(true)) {
            return Err(Error::Denied("Sign native unit owner access"));
        }
    }
    let mut rows = Vec::<JsonValue>::new();
    let mut summaries = Vec::new();
    let mut snapshots = Vec::new();
    for selection in selections {
        let binding = cmd::field(selection, "binding")?;
        let scope = match cmd::text(selection, "read_scope")? {
            "metadata_only" => NativeReadScope::MetadataOnly,
            "exact_public" => NativeReadScope::PublicContent,
            "exact_owner_local" => NativeReadScope::ExactOwnerLocal,
            _ => unreachable!(),
        };
        let adapted = resolve_assessment(
            reader,
            worker,
            binding,
            cmd::text(selection, "origin_id")?,
            scope,
            deadline,
            cancelled,
        )?;
        let unit = &adapted.records[0];
        let layer = &adapted.records[1];
        let packet = cmd::field(cmd::field(unit, "payload")?, "packet")?;
        let segment = cmd::array(packet, "segmentations")?
            .iter()
            .find(|s| s.object_get("segmentation_id") == binding.object_get("segmentation_id"))
            .ok_or(Error::Invalid("Sign selected native segmentation"))?;
        let configured = cmd::field(subjects, cmd::text(unit, "id")?)?;
        if !matches!(
            cmd::text(configured, "assertion_layer")?,
            "textual_observation" | "linguistic_analysis"
        ) || cmd::text(configured, "maker_id")?
            != cmd::text(cmd::field(segment, "maker")?, "agent_ref")?
            || !cmd::same(
                cmd::field(configured, "languages")?,
                &JsonValue::Array(vec![cmd::field(&adapted.summary, "language")?.clone()]),
            )?
        {
            return Err(Error::Denied(
                "Sign native configured unit scope differs from actual source",
            ));
        }
        let mut summary = adapted.summary;
        cmd::set(&mut summary, "record", envelope_ref(unit)?)?;
        cmd::set(&mut summary, "evidence_record", envelope_ref(layer)?)?;
        cmd::set(
            &mut summary,
            "read_scope",
            cmd::field(selection, "read_scope")?.clone(),
        )?;
        for row in adapted.records {
            if let Some(prior) = rows
                .iter()
                .find(|prior| prior.object_get("id") == row.object_get("id"))
            {
                if !cmd::same(prior, &row)? {
                    return Err(Error::Invalid("Sign native identity/body/origin differs"));
                }
            } else {
                rows.push(row);
            }
        }
        summaries.push(summary);
        snapshots.push(cmd::string(&adapted.input_snapshot));
    }
    Ok(NativeSelection {
        rows,
        summaries,
        snapshots,
    })
}

fn required_sources(
    subject: &str,
    dependencies: &BTreeMap<String, Vec<JsonValue>>,
    rows: &[JsonValue],
    summaries: &[JsonValue],
) -> Result<Vec<JsonValue>> {
    let records = rows
        .iter()
        .map(|row| Ok((cmd::text(row, "id")?, row)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut selected = BTreeMap::new();
    let mut add = |reference: &JsonValue| -> Result<()> {
        let id = cmd::text(reference, "id")?;
        let row = records
            .get(id)
            .ok_or(Error::Conflict("Sign required exact source absent"))?;
        if id == subject || !cmd::same(&envelope_ref(row)?, reference)? {
            return Err(Error::Conflict("Sign required exact source differs"));
        }
        selected.insert(id.to_owned(), *row);
        Ok(())
    };
    for reference in dependencies.get(subject).into_iter().flatten() {
        add(reference)?;
    }
    // Only actual selected endpoint bindings may add native unit/layer returns.
    for reference in dependencies.get(subject).into_iter().flatten() {
        let row = records[cmd::text(reference, "id")?];
        let Some(binding) = cmd::field(row, "payload")?.object_get("native_text_binding") else {
            continue;
        };
        for summary in summaries {
            let unit_ref = cmd::field(summary, "record")?;
            let evidence_ref = cmd::field(summary, "evidence_record")?;
            let Some(unit) = records.get(cmd::text(unit_ref, "id")?) else {
                continue;
            };
            if cmd::same(&envelope_ref(unit)?, unit_ref)?
                && cmd::field(unit, "payload")?
                    .object_get("native_binding")
                    .is_some_and(|native| cmd::same(native, binding).unwrap_or(false))
            {
                add(unit_ref)?;
                add(evidence_ref)?;
            }
        }
    }
    selected.into_values().map(envelope_ref).collect()
}

impl<'a> SignPromotionRead<'a> {
    pub(crate) fn select(
        configuration_path: &Path,
        base: &CommandContext,
        cut: &'a CorpusCutReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        if base.base_revision != cut.current().revision() {
            return Err(Error::Conflict("Sign command source revision differs"));
        }
        let uid = rustix::process::getuid().as_raw();
        protected_configuration_parents(configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(configuration_path, 1_048_576)
            .map_err(|_| Error::Denied("Sign protected command configuration"))?;
        protected(&file, uid, false)?;
        if file
            .metadata()
            .map_err(|_| Error::Invalid("Sign configuration mode"))?
            .mode()
            & 0o077
            != 0
        {
            return Err(Error::Denied("Sign private command configuration"));
        }
        let configuration_raw = raw(&mut file, 1_048_576, deadline, cancelled)?;
        if configuration_raw != base.configuration_raw {
            return Err(Error::Conflict(
                "Sign selected command configuration differs",
            ));
        }
        let configuration = cmd::parse(&configuration_raw)?;
        if cmd::text(&configuration, "schema_version")? != "tos_local_sign_promote_owner_v1" {
            return Err(Error::Denied(
                "Sign actual promotion configuration required",
            ));
        }
        let root_path = Path::new(cmd::text(&configuration, "source_root")?);
        let owner = ProtectedAssessmentJournal::select(
            Path::new(cmd::text(
                &configuration,
                "promotion_assessment_owner_config",
            )?),
            root_path,
            deadline,
            cancelled,
        )?;
        let reader = SignSourceReader::select(root_path, cut, deadline, cancelled)?;
        Ok(Self {
            owner,
            reader,
            configuration_path: configuration_path.to_owned(),
            configuration_raw,
            configuration,
            prepared_sources: None,
        })
    }

    /// Derive actual source/content observations before the corpus and subject
    /// fences. Only this private selected reader may construct the assembly;
    /// no readiness/admission result is retained.
    pub(crate) fn prepare_sources(
        &mut self,
        base: &CommandContext,
        local_worker: &mut CutWorkerSchemaExecutor,
        limits: AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        self.configuration_current(limits.deadline, cancelled)?;
        self.prepared_sources = Some(assemble_sign_sources(
            &mut self.reader,
            &self.owner,
            &self.configuration,
            base,
            local_worker,
            limits,
            cancelled,
        )?);
        self.configuration_current(limits.deadline, cancelled)?;
        Ok(())
    }

    fn configuration_current(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        configuration_current(
            &self.configuration_path,
            &self.configuration_raw,
            &self.configuration,
            self.reader.uid,
            deadline,
            cancelled,
        )
    }

    /// The only recipient is the Sign creation handler. Its final filesystem
    /// edge must call this again after acquiring the corpus mutex, retaining
    /// the journal fence across NOREPLACE publication.
    pub(crate) fn current_basis(
        &mut self,
        base: &CommandContext,
        local_worker: &mut CutWorkerSchemaExecutor,
        assessment_worker: &mut CutWorkerSchemaExecutor,
        limits: AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> Result<JsonValue> {
        self.with_current_basis(
            base,
            local_worker,
            assessment_worker,
            limits,
            cancelled,
            |basis, _| Ok(basis.clone()),
        )
    }

    /// Diagnostic read preserves the same selected subject and current journal
    /// fences, while an ineligible result grants no promotion or issuance.
    pub(crate) fn describe_promotion(
        &mut self,
        base: &CommandContext,
        local_worker: &mut CutWorkerSchemaExecutor,
        assessment_worker: &mut CutWorkerSchemaExecutor,
        limits: AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> Result<JsonValue> {
        require_assessment_profile(assessment_worker)?;
        let subject = cmd::text(&self.configuration, "promotion_candidate_id")?.to_owned();
        self.configuration_current(limits.deadline, cancelled)?;
        self.prepare_sources(base, local_worker, limits, cancelled)?;
        let assembly = self
            .prepared_sources
            .take()
            .ok_or(Error::Invalid("Sign source preparation absent"))?;
        let fence =
            self.owner
                .lock_subjects(std::slice::from_ref(&subject), limits.deadline, cancelled)?;
        let history = fence.read(
            &subject,
            &self.reader.context(base),
            assessment_worker,
            limits.deadline,
            cancelled,
        )?;
        let view = current_promotion(
            &mut self.reader,
            &self.owner,
            &fence,
            &self.configuration,
            &assembly,
            &history,
            assessment_worker,
            limits,
            cancelled,
            false,
        )?;
        finish_worker(assessment_worker, limits.deadline, cancelled)?;
        self.configuration_current(limits.deadline, cancelled)?;
        self.owner.verify_current(limits.deadline, cancelled)?;
        self.reader.verify_current(limits.deadline, cancelled)?;
        fence.verify_current(limits.deadline, cancelled)?;
        if fence.head(&subject, limits.deadline, cancelled)? != history.head {
            return Err(Error::Conflict("Sign diagnostic journal head changed"));
        }
        Ok(view)
    }

    pub(crate) fn with_current_basis<T>(
        &mut self,
        base: &CommandContext,
        local_worker: &mut CutWorkerSchemaExecutor,
        assessment_worker: &mut CutWorkerSchemaExecutor,
        limits: AssessmentLimits,
        cancelled: &AtomicBool,
        sign_publication: impl FnOnce(&JsonValue, &mut dyn FnMut() -> Result<()>) -> Result<T>,
    ) -> Result<T> {
        require_assessment_profile(assessment_worker)?;
        let subject = cmd::text(&self.configuration, "promotion_candidate_id")?.to_owned();
        self.configuration_current(limits.deadline, cancelled)?;
        if self.prepared_sources.is_none() {
            self.prepare_sources(base, local_worker, limits, cancelled)?;
        }
        let assembly = self
            .prepared_sources
            .take()
            .ok_or(Error::Invalid("Sign private source preparation absent"))?;
        let fence =
            self.owner
                .lock_subjects(std::slice::from_ref(&subject), limits.deadline, cancelled)?;
        self.configuration_current(limits.deadline, cancelled)?;
        let history = fence.read(
            &subject,
            &self.reader.context(base),
            assessment_worker,
            limits.deadline,
            cancelled,
        )?;
        let basis = current_basis(
            &mut self.reader,
            &self.owner,
            &fence,
            &self.configuration,
            &assembly,
            &history,
            assessment_worker,
            limits,
            cancelled,
        )?;
        self.configuration_current(limits.deadline, cancelled)?;
        fence.verify_current(limits.deadline, cancelled)?;
        // This private Sign callback runs before the subject locks drop. The
        // creation handler still owns its corpus mutex, target/current config,
        // selected software and publication transaction at this edge.
        let path = &self.configuration_path;
        let raw = &self.configuration_raw;
        let config = &self.configuration;
        let owner = &self.owner;
        let reader = &mut self.reader;
        let mut finalized = false;
        let mut final_read = || {
            configuration_current(path, raw, config, reader.uid, limits.deadline, cancelled)?;
            let current = current_basis(
                reader,
                owner,
                &fence,
                config,
                &assembly,
                &history,
                assessment_worker,
                limits,
                cancelled,
            )?;
            if !cmd::same(&basis, &current)? {
                return Err(Error::Conflict(
                    "Sign current basis changed at publication edge",
                ));
            }
            configuration_current(path, raw, config, reader.uid, limits.deadline, cancelled)?;
            // FINAL/EOF/resource acceptance must precede the actual rename,
            // not become an error reported after successful publication.
            finish_worker(assessment_worker, limits.deadline, cancelled)?;
            finalized = true;
            configuration_current(path, raw, config, reader.uid, limits.deadline, cancelled)?;
            owner.verify_current(limits.deadline, cancelled)?;
            reader.verify_current(limits.deadline, cancelled)?;
            fence.verify_current(limits.deadline, cancelled)?;
            if fence.head(&subject, limits.deadline, cancelled)? != history.head {
                return Err(Error::Conflict("Sign held journal head changed"));
            }
            Ok(())
        };
        let result = sign_publication(&basis, &mut final_read);
        drop(final_read);
        // Preparation has no publication edge. Its private basis still may
        // return only after the operation child has completed successfully.
        if result.is_ok() && !finalized {
            finish_worker(assessment_worker, limits.deadline, cancelled)?;
        }
        result
    }
}

fn configuration_current(
    path: &Path,
    selected_raw: &[u8],
    configuration: &JsonValue,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    protected_configuration_parents(path, uid)?;
    let mut file = tos_fd_open::open_absolute_regular(path, 1_048_576)
        .map_err(|_| Error::Conflict("Sign command configuration unavailable"))?;
    protected(&file, uid, false)?;
    if file
        .metadata()
        .map_err(|_| Error::Invalid("Sign configuration mode"))?
        .mode()
        & 0o077
        != 0
        || raw(&mut file, 1_048_576, deadline, cancelled)? != selected_raw
    {
        return Err(Error::Conflict("Sign command configuration changed"));
    }
    cmd::validate_expiry(
        cmd::text(configuration, "expires_at")?,
        &crate::source_serialization::instant()?,
    )
}

struct SignSourceAssembly {
    candidate_ref: JsonValue,
    source_records: Vec<AssessmentRecordInput>,
    native_records: Vec<AssessmentRecordInput>,
    scope_raw: Vec<u8>,
    required: Vec<Vec<u8>>,
    owner_snapshot: Digest256,
}

fn assemble_sign_sources(
    reader: &mut SignSourceReader<'_>,
    owner: &ProtectedAssessmentJournal,
    promotion: &JsonValue,
    base: &CommandContext,
    local_worker: &mut CutWorkerSchemaExecutor,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
) -> Result<SignSourceAssembly> {
    // Expensive selected-source/schema/content preparation stays outside the
    // subject fence. These private observations survive only this operation;
    // every raw input is freshly rechecked before use under the held fence.
    reader.verify_current(limits.deadline, cancelled)?;
    reader.source_carriers.clear();
    reader.source_dependencies.clear();
    reader.identity_snapshot = None;
    reader.profile_native_inputs.clear();
    let config = owner.configuration();
    let subject = cmd::text(promotion, "promotion_candidate_id")?;
    if cmd::array(config, "source_records")?
        .iter()
        .filter(|r| r.object_get("record_id").and_then(JsonValue::as_str) == Some(subject))
        .count()
        != 1
    {
        return Err(Error::Denied(
            "Sign candidate must be selected authored Claim",
        ));
    }
    let mut sourced = source_envelopes(
        reader,
        base,
        config,
        local_worker,
        limits.deadline,
        cancelled,
    )?;
    let dependencies = claim_ground_refs(
        reader,
        &sourced,
        config,
        local_worker,
        limits.deadline,
        cancelled,
    )?;
    let candidate = sourced
        .iter()
        .find(|r| r.object_get("id").and_then(JsonValue::as_str) == Some(subject))
        .ok_or(Error::Denied("Sign selected candidate absent"))?;
    let body = cmd::field(candidate, "payload")?;
    if cmd::text(body, "schema_version")? != "tos_source_occurrence_motif_claim_v1"
        || cmd::text(body, "predicate")? != "occurrence_motif_proposal"
        || cmd::text(body, "assertion_layer")? != "semantic_interpretation"
        || !matches!(
            cmd::text(body, "visibility")?,
            "public" | "public_metadata_only"
        )
    {
        return Err(Error::Denied(
            "Sign exact qualified public motif Claim required",
        ));
    }
    let candidate_ref = envelope_ref(candidate)?;
    let native = native_envelopes(reader, config, local_worker, limits.deadline, cancelled)?;
    for row in &native.rows {
        if sourced
            .iter()
            .any(|prior| prior.object_get("id") == row.object_get("id"))
        {
            return Err(Error::Invalid(
                "Sign native evidence shadows selected source",
            ));
        }
        sourced.push(row.clone());
    }
    let required = required_sources(subject, &dependencies, &sourced, &native.summaries)?;
    let scope = cmd::field(cmd::field(config, "subjects")?, subject)?;
    if cmd::text(scope, "requested_use")? != "sign-promotion"
        || cmd::text(scope, "assertion_layer")? != "semantic_interpretation"
        || !matches!(cmd::text(scope, "risk")?, "moderate" | "high")
    {
        return Err(Error::Denied("Sign promotion purpose/layer/risk required"));
    }
    // Journal grammar is selected from the same cut, independently of all
    // owner configuration and retained submissions outside that cut.
    reader.source(
        "ToS/contracts/knowledge-assessment-batch.schema.json",
        1_048_576,
        limits.deadline,
        cancelled,
    )?;
    let source_records = sourced[..sourced.len() - native.rows.len()]
        .iter()
        .map(record_input)
        .collect::<Result<_>>()?;
    let native_records = native
        .rows
        .iter()
        .map(record_input)
        .collect::<Result<_>>()?;
    let scope_raw = cmd::canonical(scope)?;
    let required = required.iter().map(cmd::canonical).collect::<Result<_>>()?;
    let mut fixity = Vec::new();
    for name in reader
        .source_carriers
        .iter()
        .chain(reader.source_dependencies.iter())
    {
        let raw = reader
            .observed
            .get(name)
            .ok_or(Error::Conflict("Sign source snapshot dependency absent"))?;
        fixity.push(cmd::object(vec![
            ("path", cmd::string(name)),
            (
                "digest",
                cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
            ),
        ]));
    }
    let mut snapshot = cmd::object(vec![
        ("configuration", config.clone()),
        ("source_files", JsonValue::Array(fixity)),
        ("resolved_records", JsonValue::Array(sourced)),
        ("native_text_snapshots", JsonValue::Array(native.snapshots)),
    ]);
    if !dependencies.is_empty() {
        cmd::set(
            &mut snapshot,
            "public_claim_dependencies",
            cmd::object(
                dependencies
                    .iter()
                    .map(|(id, refs)| (id.as_str(), JsonValue::Array(refs.clone())))
                    .collect(),
            ),
        )?;
    }
    if let Some(identity) = &reader.identity_snapshot {
        cmd::set(
            &mut snapshot,
            "native_semantic_identity_snapshot",
            cmd::string(identity),
        )?;
    }
    if !reader.profile_native_inputs.is_empty() {
        let tuples = JsonValue::Array(
            reader
                .profile_native_inputs
                .iter()
                .map(|((name, category), digest)| {
                    JsonValue::Array(vec![
                        cmd::string(name),
                        cmd::string(category),
                        cmd::string(&digest.to_hex()),
                    ])
                })
                .collect(),
        );
        cmd::set(
            &mut snapshot,
            "native_text_binding_snapshot",
            cmd::string(&crate::source_revisions::python_ascii_digest(&tuples)?),
        )?;
    }
    owner.verify_current(limits.deadline, cancelled)?;
    reader.verify_current(limits.deadline, cancelled)?;
    Ok(SignSourceAssembly {
        candidate_ref,
        source_records,
        native_records,
        scope_raw,
        required,
        owner_snapshot: cmd::record_digest(&snapshot)?,
    })
}

fn current_basis(
    reader: &mut SignSourceReader<'_>,
    owner: &ProtectedAssessmentJournal,
    fence: &AssessmentJournalFence<'_>,
    promotion: &JsonValue,
    assembly: &SignSourceAssembly,
    history: &crate::source_assessment_journal::AssessmentHistory,
    assessment_worker: &mut CutWorkerSchemaExecutor,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
) -> Result<JsonValue> {
    let view = current_promotion(
        reader,
        owner,
        fence,
        promotion,
        assembly,
        history,
        assessment_worker,
        limits,
        cancelled,
        true,
    )?;
    Ok(cmd::field(&view, "basis")?.clone())
}

fn current_promotion(
    reader: &mut SignSourceReader<'_>,
    owner: &ProtectedAssessmentJournal,
    fence: &AssessmentJournalFence<'_>,
    promotion: &JsonValue,
    assembly: &SignSourceAssembly,
    history: &crate::source_assessment_journal::AssessmentHistory,
    assessment_worker: &mut CutWorkerSchemaExecutor,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
    require_ready: bool,
) -> Result<JsonValue> {
    owner.verify_current(limits.deadline, cancelled)?;
    reader.verify_current(limits.deadline, cancelled)?;
    fence.verify_current(limits.deadline, cancelled)?;
    let config = owner.configuration();
    let subject = cmd::text(promotion, "promotion_candidate_id")?;
    let candidate_ref = &assembly.candidate_ref;
    let input = AssessmentReadInput {
        source_revision: reader.cut.current().revision(),
        policy: owner_envelope(cmd::field(config, "policy")?)?,
        authorities: owner_envelopes(config, "authorities")?,
        competencies: owner_envelopes(config, "competencies")?,
        records: owner_envelopes(config, "records")?,
        source_records: assembly.source_records.clone(),
        native_records: assembly.native_records.clone(),
        source_route: AssessmentSourceRoute::SourceBoundClaim,
        subject_id: subject.to_owned(),
        configured_scope: assembly.scope_raw.clone(),
        required_source_refs: assembly.required.clone(),
        required_admission_bases: vec![],
        reviews: history.submissions.clone(),
        trusted_history: history.submissions.clone(),
        observed_now: crate::source_serialization::instant()?,
    };
    let report = evaluate_current_assessment(&input, assessment_worker, limits, cancelled)
        .map_err(|error| match error {
            AssessmentRefusal::InvalidInput(_) => {
                Error::Invalid("Sign native current assessment input")
            }
            AssessmentRefusal::Budget => Error::Invalid("Sign native current assessment budget"),
            AssessmentRefusal::Cancelled | AssessmentRefusal::Deadline => {
                Error::Denied("Sign native current assessment cancelled or expired")
            }
            AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Source(_)) => {
                Error::Invalid("Sign native current assessment schema")
            }
            AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Budget)
            | AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::BudgetCheck {
                ..
            }) => Error::Invalid("Sign native current assessment schema budget"),
            AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Deadline) => {
                Error::Denied("Sign native current assessment schema expired")
            }
            AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Unsupported(_))
            | AssessmentRefusal::Unsupported(_) => {
                Error::Unsupported("Sign native current assessment evaluation")
            }
        })?;
    let admission = decoded(report.current_admission())?;
    if !cmd::same(cmd::field(&admission, "subject")?, candidate_ref)? {
        return Err(Error::Conflict("Sign evaluated subject differs"));
    }
    let ready = !(cmd::text(&admission, "use")? != "sign-promotion"
        || admission.object_get("can_use") != Some(&JsonValue::Bool(true))
        || !matches!(
            cmd::text(&admission, "status")?,
            "admitted" | "admitted-with-limits"
        )
        || cmd::array(&admission, "assessment_refs")?.is_empty()
        || history.head.is_none()
        || report.required_sources().is_empty()
        || !report.source_read_required()
        || !report.source_read_ready());
    if require_ready && !ready {
        return Err(Error::Denied(
            "Sign current qualified assessment and exact native source reading required",
        ));
    }
    owner.verify_current(limits.deadline, cancelled)?;
    reader.verify_current(limits.deadline, cancelled)?;
    fence.verify_current(limits.deadline, cancelled)?;
    if fence.head(subject, limits.deadline, cancelled)? != history.head {
        return Err(Error::Conflict("Sign held journal head changed"));
    }
    let basis = if ready {
        cmd::object(vec![
            ("schema_version", cmd::string("tos_sign_promotion_basis_v1")),
            ("candidate", candidate_ref.clone()),
            ("policy", cmd::field(&admission, "policy")?.clone()),
            (
                "required_sources",
                JsonValue::Array(
                    report
                        .required_sources()
                        .iter()
                        .map(decoded)
                        .collect::<Result<_>>()?,
                ),
            ),
            (
                "assessment_refs",
                cmd::field(&admission, "assessment_refs")?.clone(),
            ),
            (
                "owner_snapshot",
                cmd::string(&assembly.owner_snapshot.to_prefixed()),
            ),
            (
                "journal_revision",
                cmd::string(history.head.as_ref().unwrap()),
            ),
            ("status", cmd::field(&admission, "status")?.clone()),
            ("use", cmd::string("sign-promotion")),
            ("limits", cmd::field(&admission, "limits")?.clone()),
            ("grants_current_use", JsonValue::Bool(false)),
        ])
    } else {
        JsonValue::Null
    };
    Ok(cmd::object(vec![
        ("eligible", JsonValue::Bool(ready)),
        ("basis", basis),
        ("current_admission", admission),
        ("grants_issuance_authority", JsonValue::Bool(false)),
    ]))
}
