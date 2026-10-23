//! Fixed native caller for the seven maintained record-revision owner schemas.
use super::{absolute, capped, digest, exact, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{self as owner, CreationFilesystem};
use crate::source_revisions::ReadonlyRecordFiles;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle};
use tos_validation::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutSchemaDiagnosticsLimits, CutSchemaExecutor,
    CutWorkerLimits, CutWorkerSchemaExecutor,
};
use tos_validation::{FormatProfile, SchemaResource};

struct V1RecordFiles<'a> {
    cut: &'a CorpusCutReader,
}

impl ReadonlyRecordFiles for V1RecordFiles<'_> {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let relative = tos_foundation::RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("record revision selected path"))?;
        self.cut
            .read_member(
                self.cut.current().revision(),
                &relative,
                u64::try_from(max_bytes)
                    .map_err(|_| SourceCommandError::Invalid("record revision read cap"))?,
                deadline,
                cancelled,
            )
            .map(|member| member.raw)
            .map_err(|_| SourceCommandError::Conflict("record revision selected member read"))
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported(
                "record revision directory deadline or cancellation",
            ));
        }
        let relative = tos_foundation::RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("record revision directory path"))?;
        let prefix = format!("{}/", relative.as_str());
        let mut children = BTreeMap::<String, bool>::new();
        for member in self.cut.current().members() {
            if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(SourceCommandError::Unsupported(
                    "record revision directory deadline or cancellation",
                ));
            }
            let Some(tail) = member.path.as_str().strip_prefix(&prefix) else {
                continue;
            };
            let (name, is_directory) = match tail.split_once('/') {
                Some((name, _)) => (name, true),
                None => (tail, false),
            };
            if name.is_empty() {
                continue;
            }
            if children
                .insert(name.to_owned(), is_directory)
                .is_some_and(|prior| prior != is_directory)
            {
                return Err(SourceCommandError::Conflict(
                    "record revision source path is both file and directory",
                ));
            }
            if children.len() > cmd::SELECTED_SOURCE_MAX_FILES {
                return Err(SourceCommandError::Invalid(
                    "record revision directory child budget",
                ));
            }
        }
        Ok(children.into_iter().collect())
    }

    fn has_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported(
                "record revision member deadline or cancellation",
            ));
        }
        let relative = tos_foundation::RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("record revision selected path"))?;
        Ok(self.cut.current().member(&relative).is_some())
    }
}

fn selected_publication_files(
    transport: &mut impl ReadonlyRecordFiles,
    request_raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
    let control_present = transport.has_file(CONTROL, deadline, cancelled)?;
    let retained_base = transport.retained_state_bytes();
    let control_raw = if control_present {
        transport.set_retained_state_bytes(
            retained_base
                .checked_add(CONTROL.len().saturating_mul(4))
                .and_then(|bytes| bytes.checked_add(256))
                .ok_or(SourceCommandError::Invalid(
                    "record revision retained source state overflow",
                ))?,
            deadline,
            cancelled,
        )?;
        Some(transport.read(CONTROL, 8192, deadline, cancelled)?)
    } else {
        None
    };
    let control = control_raw.as_deref().map(cmd::parse).transpose()?;
    let request = cmd::parse(request_raw)?;
    let mut selected_ids = BTreeSet::new();
    match cmd::text(&request, "operation")? {
        "record.revise" => {
            selected_ids.insert(crate::source_revisions::transaction_id(&request)?);
        }
        "record.recover" => {
            selected_ids.insert(cmd::text(&request, "transaction_id")?.to_owned());
        }
        _ => (),
    }
    if control
        .as_ref()
        .is_some_and(|value| cmd::text(value, "phase").ok() == Some("pending"))
    {
        selected_ids.insert(
            cmd::text(
                control
                    .as_ref()
                    .ok_or(SourceCommandError::Invalid("record revision control"))?,
                "transaction_id",
            )?
            .to_owned(),
        );
    }
    let mut output = BTreeMap::<String, Vec<u8>>::new();
    if let Some(control_raw) = control_raw {
        output.insert(CONTROL.to_owned(), control_raw);
        update_transport_retained_state_from(
            transport,
            retained_base,
            &output,
            deadline,
            cancelled,
        )?;
    }
    let transactions_path = "ToS/source-witnesses/.metadata-transactions";
    for id in selected_ids {
        let digest = Digest256::from_prefixed(&id)
            .map_err(|_| SourceCommandError::Invalid("record revision transaction identity"))?;
        let directory = format!("{transactions_path}/{}", &digest.to_hex());
        let manifest_path = format!("{directory}/manifest.json");
        if !transport.has_file(&manifest_path, deadline, cancelled)? {
            // A request-derived id without retained evidence is not selected
            // by the existing V1 publication reader. Pending ids fail closed
            // below because their control explicitly selects the transaction.
            if control.as_ref().is_some_and(|value| {
                cmd::text(value, "phase").ok() == Some("pending")
                    && cmd::text(value, "transaction_id").ok() == Some(id.as_str())
            }) {
                return Err(SourceCommandError::Conflict(
                    "record revision pending manifest is absent",
                ));
            }
            continue;
        }
        reserve_transport_read_from(
            transport,
            retained_base,
            &output,
            &manifest_path,
            deadline,
            cancelled,
        )?;
        let manifest_raw = transport.read(&manifest_path, 524_288, deadline, cancelled)?;
        let manifest = cmd::parse(&manifest_raw)?;
        output.insert(manifest_path, manifest_raw);
        update_transport_retained_state_from(
            transport,
            retained_base,
            &output,
            deadline,
            cancelled,
        )?;
        let files = cmd::array(cmd::field(&manifest, "plan")?, "files")?;
        if files.len() != 3 {
            return Err(SourceCommandError::Denied(
                "record revision retained transaction file count",
            ));
        }
        let mut selected_side_bytes = [0u64; 2];
        for item in files {
            for (side_index, side) in ["before", "after"].into_iter().enumerate() {
                let binding = cmd::field(item, side)?;
                if binding == &JsonValue::Null {
                    continue;
                }
                cmd::exact_keys(binding, &["sha256", "bytes"])?;
                let bytes = cmd::integer(binding, "bytes")?;
                selected_side_bytes[side_index] = selected_side_bytes[side_index]
                    .checked_add(bytes)
                    .filter(|sum| bytes <= 8_388_608 && *sum <= 8_388_608)
                    .ok_or(SourceCommandError::Invalid(
                        "record revision retained selected side budget",
                    ))?;
                let sha256 = cmd::text(binding, "sha256")?;
                let digest = Digest256::from_prefixed(sha256).map_err(|_| {
                    SourceCommandError::Invalid("record revision retained blob digest")
                })?;
                let blob = format!("{directory}/{}.blob", digest.to_hex());
                if !transport.has_file(&blob, deadline, cancelled)? {
                    return Err(SourceCommandError::Conflict(
                        "record revision retained blob is absent",
                    ));
                }
                if !output.contains_key(&blob) {
                    reserve_transport_read_from(
                        transport,
                        retained_base,
                        &output,
                        &blob,
                        deadline,
                        cancelled,
                    )?;
                    output.insert(
                        blob.clone(),
                        transport.read(&blob, 8_388_608, deadline, cancelled)?,
                    );
                    update_transport_retained_state_from(
                        transport,
                        retained_base,
                        &output,
                        deadline,
                        cancelled,
                    )?;
                }
            }
        }
        let path = format!("{directory}/completion.json");
        if transport.has_file(&path, deadline, cancelled)? {
            reserve_transport_read_from(
                transport,
                retained_base,
                &output,
                &path,
                deadline,
                cancelled,
            )?;
            output.insert(
                path.clone(),
                transport.read(&path, 8192, deadline, cancelled)?,
            );
            update_transport_retained_state_from(
                transport,
                retained_base,
                &output,
                deadline,
                cancelled,
            )?;
        }
    }
    output
        .into_iter()
        .map(|(name, raw)| {
            Ok(cmd::SourceFile {
                path: tos_foundation::RelativePath::parse(&name)
                    .map_err(|_| SourceCommandError::Invalid("record revision retained path"))?,
                raw,
            })
        })
        .collect()
}

fn retained_file_state(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<usize> {
    files.iter().try_fold(0usize, |total, (path, raw)| {
        total
            .checked_add(raw.len())
            .and_then(|bytes| bytes.checked_add(path.len().saturating_mul(4)))
            .and_then(|bytes| bytes.checked_add(256))
            .ok_or(SourceCommandError::Invalid(
                "record revision retained source state overflow",
            ))
    })
}

fn update_transport_retained_state(
    transport: &mut impl ReadonlyRecordFiles,
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    update_transport_retained_state_from(transport, 0, files, deadline, cancelled)
}

fn update_transport_retained_state_from(
    transport: &mut impl ReadonlyRecordFiles,
    base: usize,
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let bytes =
        base.checked_add(retained_file_state(files)?)
            .ok_or(SourceCommandError::Invalid(
                "record revision retained source state overflow",
            ))?;
    transport.set_retained_state_bytes(bytes, deadline, cancelled)
}

fn reserve_transport_read_from(
    transport: &mut impl ReadonlyRecordFiles,
    base: usize,
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let bytes = base
        .checked_add(retained_file_state(files)?)
        .and_then(|bytes| bytes.checked_add(path.len().saturating_mul(4)))
        .and_then(|bytes| bytes.checked_add(256))
        .ok_or(SourceCommandError::Invalid(
            "record revision retained source state overflow",
        ))?;
    transport.set_retained_state_bytes(bytes, deadline, cancelled)
}

struct SelectedContextBudget {
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: usize,
    members: BTreeMap<String, usize>,
    total_bytes: u64,
}

impl SelectedContextBudget {
    fn new(
        max_members: usize,
        max_total_bytes: u64,
        max_member_bytes: u64,
    ) -> SourceCommandResult<Self> {
        if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
            return Err(SourceCommandError::Invalid(
                "record revision selected context budget",
            ));
        }
        Ok(Self {
            max_members,
            max_total_bytes,
            max_member_bytes: usize::try_from(max_member_bytes)
                .map_err(|_| SourceCommandError::Invalid("record revision member byte budget"))?,
            members: BTreeMap::new(),
            total_bytes: 0,
        })
    }

    fn read_cap(&self, path: &str, requested: usize) -> SourceCommandResult<usize> {
        let member_cap = requested.min(self.max_member_bytes);
        let remaining = if self.members.contains_key(path) {
            self.max_member_bytes as u64
        } else {
            self.max_total_bytes.saturating_sub(self.total_bytes)
        };
        let cap = member_cap.min(usize::try_from(remaining).unwrap_or(usize::MAX));
        if cap == 0 {
            return Err(SourceCommandError::Invalid(
                "record revision selected context byte budget",
            ));
        }
        Ok(cap)
    }

    fn record(&mut self, path: &str, bytes: usize) -> SourceCommandResult<()> {
        if bytes > self.max_member_bytes {
            return Err(SourceCommandError::Invalid(
                "record revision selected context member byte budget",
            ));
        }
        if let Some(previous) = self.members.get(path) {
            if *previous != bytes {
                return Err(SourceCommandError::Conflict(
                    "record revision selected member size changed",
                ));
            }
            return Ok(());
        }
        if self.members.len() >= self.max_members {
            return Err(SourceCommandError::Invalid(
                "record revision selected context member budget",
            ));
        }
        let bytes_u64 = u64::try_from(bytes)
            .map_err(|_| SourceCommandError::Invalid("record revision byte budget"))?;
        self.total_bytes = self
            .total_bytes
            .checked_add(bytes_u64)
            .filter(|total| *total <= self.max_total_bytes)
            .ok_or(SourceCommandError::Invalid(
                "record revision selected context byte budget",
            ))?;
        self.members.insert(path.to_owned(), bytes);
        Ok(())
    }
}

struct SelectedContextTransport<'a, T> {
    inner: &'a mut T,
    budget: SelectedContextBudget,
}

impl<'a, T> SelectedContextTransport<'a, T> {
    fn new(
        inner: &'a mut T,
        max_members: usize,
        max_total_bytes: u64,
        max_member_bytes: u64,
    ) -> SourceCommandResult<Self> {
        Ok(Self {
            inner,
            budget: SelectedContextBudget::new(max_members, max_total_bytes, max_member_bytes)?,
        })
    }

    fn record_external_member(&mut self, path: &str, bytes: usize) -> SourceCommandResult<()> {
        self.budget.record(path, bytes)
    }
}

impl<T: ReadonlyRecordFiles> ReadonlyRecordFiles for SelectedContextTransport<'_, T> {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let cap = self.budget.read_cap(path, max_bytes)?;
        let raw = self.inner.read(path, cap, deadline, cancelled)?;
        self.budget.record(path, raw.len())?;
        Ok(raw)
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        self.inner.list_directory(path, deadline, cancelled)
    }

    fn set_retained_state_bytes(
        &mut self,
        bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.inner
            .set_retained_state_bytes(bytes, deadline, cancelled)
    }

    fn retained_state_bytes(&self) -> usize {
        self.inner.retained_state_bytes()
    }

    fn has_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        self.inner.has_file(path, deadline, cancelled)
    }
}

fn bounded_context_files(
    source_path: &str,
    request_raw: &[u8],
    transport: &mut impl ReadonlyRecordFiles,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    for file in crate::source_revisions::collect_revision_record_files(
        transport,
        source_path,
        deadline,
        cancelled,
    )? {
        files.insert(file.path.as_str().to_owned(), file.raw);
    }
    update_transport_retained_state(transport, &files, deadline, cancelled)?;
    let (parent, base) = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("record revision source parent"))?;
    let forms_name = format!(
        "{}.human-forms.json",
        base.strip_suffix(".json").unwrap_or(base)
    );
    let forms_path = format!("{parent}/{forms_name}");
    if transport.has_file(&forms_path, deadline, cancelled)? {
        reserve_transport_read_from(transport, 0, &files, &forms_path, deadline, cancelled)?;
        files.insert(
            forms_path.clone(),
            transport.read(&forms_path, 8_388_608, deadline, cancelled)?,
        );
        update_transport_retained_state(transport, &files, deadline, cancelled)?;
    }
    for file in selected_publication_files(transport, request_raw, deadline, cancelled)? {
        let name = file.path.as_str().to_owned();
        if let Some(previous) = files.get(&name) {
            if previous != &file.raw {
                return Err(SourceCommandError::Conflict(
                    "record revision selected source path has conflicting bytes",
                ));
            }
        } else {
            files.insert(name, file.raw);
        }
        update_transport_retained_state(transport, &files, deadline, cancelled)?;
    }
    for file in crate::source_revisions::collect_readonly_schema_files(
        transport,
        &[
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/human-form.schema.json",
            "ToS/contracts/human-form-set.schema.json",
            "ToS/contracts/human-form-template.schema.json",
        ],
        deadline,
        cancelled,
    )? {
        let name = file.path.as_str().to_owned();
        if let Some(previous) = files.get(&name) {
            if previous != &file.raw {
                return Err(SourceCommandError::Conflict(
                    "record revision schema source changed between reads",
                ));
            }
        } else {
            files.insert(name, file.raw);
        }
        update_transport_retained_state(transport, &files, deadline, cancelled)?;
    }
    if files.len() > cmd::SELECTED_SOURCE_MAX_FILES {
        return Err(SourceCommandError::Invalid(
            "record revision selected context file budget",
        ));
    }
    files
        .into_iter()
        .map(|(name, raw)| {
            Ok(cmd::SourceFile {
                path: tos_foundation::RelativePath::parse(&name)
                    .map_err(|_| SourceCommandError::Invalid("record revision selected path"))?,
                raw,
            })
        })
        .collect()
}
/// Compatibility route for neighboring native owner families. Their callers
/// retain the original whole-context behavior until they select the bounded
/// owner-record route independently.
pub(super) fn context(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut total = 0u64;
    for member in cut.current().members() {
        if !member.path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "Record revision source cut namespace",
            ));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Record revision selected context byte budget",
            ))?;
        if !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Invalid(
                "Record revision duplicate selected path",
            ));
        }
        let raw = cut
            .read_member(
                cut.current().revision(),
                &member.path,
                8_388_608,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("Record revision source member read"))?
            .raw;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    for member in components.members() {
        if member.path.as_str().starts_with("ToS/") || !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Denied(
                "Record revision source/software namespaces overlap",
            ));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Record revision combined selected context byte budget",
            ))?;
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Record revision software component read"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    Ok(cmd::CommandContext {
        base_revision: cut.current().revision(),
        configuration_raw: configuration_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: recorded_at.to_owned(),
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    })
}

fn context_selected_record(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    source_path: &str,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let mut transport = V1RecordFiles { cut };
    context_selected_record_from_transport(
        configuration_raw,
        request_raw,
        recorded_at,
        source_path,
        cut.current().revision(),
        &mut transport,
        software,
        components,
        deadline,
        cancelled,
    )
}

pub(crate) fn context_selected_record_from_transport(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    source_path: &str,
    base_revision: SourceRevision,
    transport: &mut impl ReadonlyRecordFiles,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    context_selected_record_from_transport_with_limits(
        configuration_raw,
        request_raw,
        recorded_at,
        source_path,
        base_revision,
        transport,
        software,
        components,
        cmd::SELECTED_SOURCE_MAX_FILES,
        u64::try_from(cmd::SELECTED_SOURCE_MAX_BYTES)
            .map_err(|_| SourceCommandError::Invalid("record revision context byte budget"))?,
        8_388_608,
        deadline,
        cancelled,
    )
}

/// Bounded PG owners pass their invocation caps here so an oversized selected
/// package is refused before transport reads or software-capture materialize
/// it. The local owner caps remain the outer ceiling.
pub(crate) fn context_selected_record_from_transport_with_limits(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    source_path: &str,
    base_revision: SourceRevision,
    transport: &mut impl ReadonlyRecordFiles,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let max_files = max_members.min(cmd::SELECTED_SOURCE_MAX_FILES);
    let max_bytes = max_total_bytes
        .min(
            u64::try_from(cmd::SELECTED_SOURCE_MAX_BYTES)
                .map_err(|_| SourceCommandError::Invalid("record revision context byte budget"))?,
        )
        .min(33_554_432);
    let max_member_bytes = max_member_bytes.min(8_388_608);
    let mut budgeted =
        SelectedContextTransport::new(transport, max_files, max_bytes, max_member_bytes)?;
    context_selected_record_from_budgeted_transport(
        configuration_raw,
        request_raw,
        recorded_at,
        source_path,
        base_revision,
        &mut budgeted,
        software,
        components,
        deadline,
        cancelled,
    )
}

fn context_selected_record_from_budgeted_transport<T: ReadonlyRecordFiles>(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    source_path: &str,
    base_revision: SourceRevision,
    transport: &mut SelectedContextTransport<'_, T>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let mut files =
        bounded_context_files(source_path, request_raw, transport, deadline, cancelled)?;
    if files.len() > cmd::SELECTED_SOURCE_MAX_FILES {
        return Err(SourceCommandError::Invalid(
            "Record revision selected context file budget",
        ));
    }
    let mut paths = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut total = 0u64;
    for file in &files {
        if !file.path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "Record revision source cut namespace",
            ));
        }
        total = total
            .checked_add(file.raw.len() as u64)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Record revision selected context byte budget",
            ))?;
    }
    for member in components.members() {
        if files.len() >= cmd::SELECTED_SOURCE_MAX_FILES {
            return Err(SourceCommandError::Invalid(
                "Record revision combined selected context file budget",
            ));
        }
        if member.path.as_str().starts_with("ToS/") || !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Denied(
                "Record revision source/software namespaces overlap",
            ));
        }
        let member_total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Record revision combined selected context byte budget",
            ))?;
        let member_bytes = usize::try_from(member.size_bytes)
            .map_err(|_| SourceCommandError::Invalid("Record revision component byte budget"))?;
        transport.record_external_member(member.path.as_str(), member_bytes)?;
        total = member_total;
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                8_388_608.min(
                    u64::try_from(transport.budget.max_member_bytes).map_err(|_| {
                        SourceCommandError::Invalid("Record revision member byte budget overflow")
                    })?,
                ),
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("Record revision software component read"))?;
        if raw.len() != member_bytes {
            return Err(SourceCommandError::Conflict(
                "Record revision software component size changed",
            ));
        }
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    Ok(cmd::CommandContext {
        base_revision,
        configuration_raw: configuration_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: recorded_at.to_owned(),
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    })
}

/// The PostgreSQL caller selects the same owner record and schema closure.
/// Flat owners additionally bind every immediate member of their package;
/// a directory below it remains outside that owner's declared revision scope.
pub(crate) fn context_flat_record_from_transport(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    source_path: &str,
    base_revision: SourceRevision,
    transport: &mut impl ReadonlyRecordFiles,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let max_total_bytes_usize = usize::try_from(max_total_bytes)
        .map_err(|_| SourceCommandError::Invalid("revision context byte budget"))?;
    let configuration = cmd::parse(configuration_raw)?;
    if cmd::text(&configuration, "schema_version")? != "tos_local_corpus_revision_owner_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL flat corpus revision owner",
        ));
    }
    let mut budgeted =
        SelectedContextTransport::new(transport, max_members, max_total_bytes, max_member_bytes)?;
    let mut context = context_selected_record_from_budgeted_transport(
        configuration_raw,
        request_raw,
        recorded_at,
        source_path,
        base_revision,
        &mut budgeted,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let (parent, _) = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("revision selected parent"))?;
    let children = budgeted.list_directory(parent, deadline, cancelled)?;
    if children.len() > 64 || children.iter().any(|(_, directory)| *directory) {
        return Err(SourceCommandError::Unsupported(
            "flat revision package has descendants or exceeds budget",
        ));
    }
    let mut total = context.files.iter().try_fold(0usize, |total, file| {
        total
            .checked_add(file.raw.len())
            .ok_or(SourceCommandError::Invalid("revision context byte budget"))
    })?;
    for (name, _) in children {
        let path = tos_foundation::RelativePath::parse(&format!("{parent}/{name}"))
            .map_err(|_| SourceCommandError::Invalid("revision package member"))?;
        if context.files.iter().any(|file| file.path == path) {
            continue;
        }
        if context.files.len() >= cmd::SELECTED_SOURCE_MAX_FILES {
            return Err(SourceCommandError::Invalid("revision context file budget"));
        }
        budgeted.set_retained_state_bytes(total, deadline, cancelled)?;
        let raw = budgeted.read(path.as_str(), 2_097_152, deadline, cancelled)?;
        total = total
            .checked_add(raw.len())
            .filter(|n| *n <= max_total_bytes_usize)
            .ok_or(SourceCommandError::Invalid("revision context byte budget"))?;
        context.files.push(cmd::SourceFile { path, raw });
    }
    context.check()?;
    Ok(context)
}

/// Constructed only after the maintained revision rules and schema worker FINAL.
/// This is semantic preparation; the durable owner still verifies the readset,
/// current delegation and transaction before admitting its exact changes.
pub(crate) struct PreparedPgRecordRevision {
    context: cmd::CommandContext,
    proposal: cmd::PreparedCommand,
    identity: crate::source_revisions::PgRecordRevisionIdentity,
    family_tag: String,
    legacy_family_encoding: bool,
    work_revision: Option<crate::source_pg_work_revision::Observation>,
    publication_observation_digest: Option<Digest256>,
}

impl PreparedPgRecordRevision {
    pub(crate) fn context(&self) -> &cmd::CommandContext {
        &self.context
    }
    pub(crate) fn proposal(&self) -> &cmd::PreparedCommand {
        &self.proposal
    }

    pub(crate) fn identity(&self) -> crate::source_revisions::PgRecordRevisionIdentity {
        self.identity
    }
    pub(crate) fn family_tag(&self) -> &str {
        &self.family_tag
    }
    pub(crate) fn legacy_family_encoding(&self) -> bool {
        self.legacy_family_encoding
    }
    pub(crate) fn work_revision(&self) -> Option<&crate::source_pg_work_revision::Observation> {
        self.work_revision.as_ref()
    }
    pub(crate) fn publication_observation_digest(&self) -> Option<Digest256> {
        self.publication_observation_digest
    }
}

#[cfg(test)]
pub(crate) fn prepare_pg_record_revision(
    invocation: &Value,
    context: cmd::CommandContext,
    identity: crate::source_revisions::PgRecordRevisionIdentity,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgRecordRevision> {
    prepare_pg_record_revision_with_work(invocation, context, identity, None, deadline, cancelled)
}

pub(crate) fn prepare_pg_record_revision_with_work(
    invocation: &Value,
    context: cmd::CommandContext,
    identity: crate::source_revisions::PgRecordRevisionIdentity,
    work_revision: Option<crate::source_pg_work_revision::Observation>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgRecordRevision> {
    let (config, family) = crate::source_revisions::configuration(&context)?;
    let work_owner = family == crate::source_revisions::RevisionFamily::CorpusSelectedV3
        && crate::source_command::text(&config, "record_type")? == "work";
    if !matches!(
        family,
        crate::source_revisions::RevisionFamily::Historical
            | crate::source_revisions::RevisionFamily::CorpusFlat
            | crate::source_revisions::RevisionFamily::CorpusSelectedV2
            | crate::source_revisions::RevisionFamily::CorpusSelectedV3
            | crate::source_revisions::RevisionFamily::NativeSelected
    ) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL maintained authored revision owner",
        ));
    }
    let family_tag = family.pg_family_tag(&config)?;
    let legacy_family_encoding = family.pg_legacy_compatible(&config)?;
    let revision_request = cmd::parse(&context.request_raw)?;
    let generic_selected_observation = if work_owner || work_revision.is_none() {
        true
    } else {
        family.selected() && cmd::text(&revision_request, "operation")? == "record.revise"
    };
    let observation_matches_owner = if let Some(observation) = &work_revision {
        Digest256::of_bytes(observation.domain.as_bytes()) == identity.domain_digest
            && observation.store_id == identity.store_id
            && observation.source_path == cmd::text(&config, "source_path")?
            && observation.record_id == cmd::text(&config, "record_id")?
    } else {
        true
    };
    if (work_owner && work_revision.is_none())
        || (!family.selected() && work_revision.is_some())
        || !generic_selected_observation
        || !observation_matches_owner
    {
        return Err(SourceCommandError::Conflict(
            "PG selected record read observation differs from owner",
        ));
    }
    let mut worker = selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let read_context = crate::source_revisions::PgRecordRevisionCommandContext::new_with_work(
        context,
        identity,
        work_revision,
    );
    let result = crate::source_revisions::prepare_pg_record_revision_inner(
        &read_context,
        None,
        &mut worker,
        deadline,
        cancelled,
    );
    match result {
        Ok(proposal) => {
            worker
                .finish(deadline, cancelled)
                .map_err(|_| SourceCommandError::Denied("revision worker FINAL"))?;
            let (context, work_revision, publication_observation_digest) =
                read_context.into_parts();
            if family.selected() && !work_owner && proposal.operation == "record.revise" {
                crate::source_revisions::validate_pg_selected_revision_proposal(
                    &context, &proposal,
                )?;
            }
            Ok(PreparedPgRecordRevision {
                context,
                proposal,
                identity,
                family_tag,
                legacy_family_encoding,
                work_revision,
                publication_observation_digest,
            })
        }
        Err(error) => {
            let _ = worker.finish(deadline, cancelled);
            Err(error)
        }
    }
}

pub(crate) fn selected_candidate_schema_from_context(
    invocation: &Value,
    files: &[cmd::SourceFile],
    identity: crate::source_revisions::PgRecordRevisionIdentity,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<
    CandidateCutWorkerSchemaExecutor<crate::source_revisions::PgRecordRevisionIdentity>,
> {
    let worker = &invocation["schema_worker"];
    exact(worker, &["absolute_path", "sha256"])?;
    let budgets = &invocation["budgets"];
    let mut worker_budget = ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    worker_budget.cpu_seconds = capped(budgets, "worker_cpu_seconds", 3)?;
    worker_budget.address_space_bytes =
        capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let worker_identity = ExactWorkerIdentity {
        absolute_path: absolute(text(worker, "absolute_path")?)?,
        sha256: digest(text(worker, "sha256")?)?,
    };
    let worker_image =
        VerifiedWorkerImageHandle::prepare(worker_identity, worker_budget, deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("native schema worker image"))?;
    let limits = CutWorkerLimits {
        max_receipts: usize::try_from(capped(budgets, "max_schema_receipts", 128)?)
            .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
        max_receipt_bytes: usize::try_from(capped(budgets, "max_schema_receipt_bytes", 262_144)?)
            .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
    };
    let mut resources = Vec::new();
    for file in files.iter().filter(|file| {
        file.path.as_str().starts_with("ToS/contracts/")
            && file.path.as_str().ends_with(".schema.json")
    }) {
        if resources.len() >= 128 || file.raw.len() > 4 * 1024 * 1024 {
            return Err(SourceCommandError::Invalid(
                "record revision selected schema budget",
            ));
        }
        let parsed = cmd::parse(&file.raw)?;
        let uri = cmd::text(&parsed, "$id")?.to_owned();
        resources.push((
            file.path.as_str().to_owned(),
            SchemaResource {
                uri,
                raw: file.raw.clone(),
            },
        ));
    }
    let mut executor = CandidateCutWorkerSchemaExecutor::from_selected_source_resources(
        identity,
        resources,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        worker_budget,
        limits,
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Denied("native candidate schema worker"))?;
    // Selected compound Claim validation consumes candidate-bound diagnostics.
    // Select that protocol before any scalar check starts the shared operation;
    // bool checks and Claim diagnostics then retain one exact worker binding.
    executor
        .enable_diagnostics_v2(CutSchemaDiagnosticsLimits {
            max_total_issues: limits.max_receipts,
            max_total_report_bytes: limits.max_receipt_bytes,
            max_total_state_bytes: usize::try_from(super::number(budgets, "max_total_bytes")?)
                .map_err(|_| SourceCommandError::Invalid("candidate schema state budget"))?,
        })
        .map_err(|_| SourceCommandError::Denied("native candidate schema diagnostics budget"))?;
    Ok(executor)
}

pub(crate) fn selected_schema_from_context(
    invocation: &Value,
    files: &[cmd::SourceFile],
    source_revision: SourceRevision,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CutWorkerSchemaExecutor> {
    let worker = &invocation["schema_worker"];
    exact(worker, &["absolute_path", "sha256"])?;
    let budgets = &invocation["budgets"];
    let mut worker_budget = ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    worker_budget.cpu_seconds = capped(budgets, "worker_cpu_seconds", 3)?;
    worker_budget.address_space_bytes =
        capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let mut contracts = BTreeMap::new();
    let mut resources = Vec::new();
    for file in files.iter().filter(|file| {
        file.path.as_str().starts_with("ToS/contracts/")
            && file.path.as_str().ends_with(".schema.json")
    }) {
        if resources.len() >= 128 || file.raw.len() > 4 * 1024 * 1024 {
            return Err(SourceCommandError::Invalid(
                "record revision selected schema budget",
            ));
        }
        let parsed = cmd::parse(&file.raw)?;
        let uri = cmd::text(&parsed, "$id")?.to_owned();
        if uri != format!("https://tree-of-sophia.local/{}", file.path.as_str())
            && uri != format!("https://treeofsophia.local/{}", file.path.as_str())
        {
            return Err(SourceCommandError::Invalid(
                "record revision schema identity",
            ));
        }
        let path = file.path.as_str().to_owned();
        if contracts
            .insert(path, (uri.clone(), Digest256::of_bytes(&file.raw)))
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate selected schema resource",
            ));
        }
        resources.push(SchemaResource {
            uri,
            raw: file.raw.clone(),
        });
    }
    CutWorkerSchemaExecutor::from_selected_resources(
        source_revision,
        contracts,
        resources,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path: absolute(text(worker, "absolute_path")?)?,
            sha256: digest(text(worker, "sha256")?)?,
        },
        worker_budget,
        CutWorkerLimits {
            max_receipts: usize::try_from(capped(budgets, "max_schema_receipts", 128)?)
                .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
            max_receipt_bytes: usize::try_from(capped(
                budgets,
                "max_schema_receipt_bytes",
                262_144,
            )?)
            .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
        },
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Denied("native schema worker"))
}
fn value(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("Record revision response JSON"))
}

// Only the filesystem record owner chooses this resource footprint. The full
// context/cut remains unchanged. Shared PG/text/Claim callers continue to call
// selected_schema_from_context directly with its original all-resource default.
fn filesystem_profile_schema_from_context(
    invocation: &Value,
    ctx: &cmd::CommandContext,
    family: crate::source_revisions::RevisionFamily,
    source_revision: SourceRevision,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CutWorkerSchemaExecutor> {
    let selected = if matches!(
        family,
        crate::source_revisions::RevisionFamily::PublicProfile
            | crate::source_revisions::RevisionFamily::PublicProfileScope
    ) {
        crate::source_revisions::simple_profile_schema_files(ctx, deadline, cancelled)?
    } else {
        None
    };
    selected_schema_from_context(
        invocation,
        selected.as_deref().unwrap_or(&ctx.files),
        source_revision,
        deadline,
        cancelled,
    )
}

pub(super) fn run(
    invocation: &Value,
    request_raw: &[u8],
    store: &CorpusReader,
    current: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if invocation.get("owner_context") != Some(&Value::Null)
        || invocation.get("assessment_schema_worker") != Some(&Value::Null)
    {
        return Err(SourceCommandError::Denied(
            "record revision unused protected selectors",
        ));
    }
    let (filesystem, configuration_raw) = CreationFilesystem::select_protected_native_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let configuration = cmd::parse(&configuration_raw)?;
    if !matches!(
        cmd::text(&configuration, "schema_version")?,
        "tos_local_source_revision_owner_v1"
            | "tos_local_profile_revision_owner_v1"
            | "tos_local_profile_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v1"
            | "tos_local_corpus_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v3"
            | "tos_local_native_metadata_revision_owner_v1"
    ) {
        return Err(SourceCommandError::Denied(
            "record revision exact protected family",
        ));
    }
    let original = match invocation.get("original_source_revision") {
        Some(Value::Null) => None,
        Some(Value::String(revision)) => {
            let budgets = &invocation["budgets"];
            Some(
                store
                    .open_source_cut(
                        SourceRevision(digest(revision)?),
                        CutReadLimits {
                            max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                                .map_err(|_| {
                                    SourceCommandError::Invalid("record original revision budget")
                                })?,
                            max_members: capped(budgets, "max_members", 2048)?,
                            max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                            max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
                        },
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| SourceCommandError::Conflict("record original selected cut"))?,
            )
        }
        _ => {
            return Err(SourceCommandError::Invalid(
                "record original source selector",
            ));
        }
    };
    let pending = owner::revision_publication::pending(&filesystem, deadline, cancelled)?;
    let selected = if pending {
        original.as_ref().ok_or(SourceCommandError::Denied(
            "record pending original semantic cut required",
        ))?
    } else {
        current
    };
    let now = crate::source_serialization::instant()?;
    let source_path = cmd::text(&configuration, "source_path")?;
    let family = crate::source_revisions::RevisionFamily::parse(cmd::text(
        &configuration,
        "schema_version",
    )?)?;
    let ctx = if family.selected() {
        context_selected_record(
            &configuration_raw,
            request_raw,
            &now,
            source_path,
            selected,
            software,
            components,
            deadline,
            cancelled,
        )?
    } else {
        // Flat owners bind every package member, including unrelated retained
        // bytes. Their existing bounded context also retains complete archives.
        context(
            &configuration_raw,
            request_raw,
            &now,
            selected,
            software,
            components,
            deadline,
            cancelled,
        )?
    };
    let mut worker = filesystem_profile_schema_from_context(
        invocation,
        &ctx,
        family,
        selected.current().revision(),
        deadline,
        cancelled,
    )?;
    let result = owner::revision_publication::run(
        &filesystem,
        &ctx,
        selected,
        original.as_ref(),
        software,
        components,
        &mut worker,
        deadline,
        cancelled,
    );
    match result {
        Ok(proposal) => Ok(json!({"schema_version":"tos_local_native_source_result_v1",
            "authentication":"local-unix-account", "result":value(&proposal.response)?, "grants_admission":false})),
        Err(error) => {
            let _ = worker.finish(deadline, cancelled);
            Err(error)
        }
    }
}
