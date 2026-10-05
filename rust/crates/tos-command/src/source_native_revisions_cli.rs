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
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor};
use tos_validation::{FormatProfile, SchemaResource};
use super::source_session::{NativeSourceV2RecordFiles, NativeSourceV2Session};

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

fn bounded_context_files(
    source_path: &str,
    request_raw: &[u8],
    transport: &mut impl ReadonlyRecordFiles,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    for file in crate::source_revisions::collect_readonly_record_files(
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
        reserve_transport_read(transport, &files, &forms_path, deadline, cancelled)?;
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

pub(super) fn context_selected_record_from_transport(
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
        base_revision,
        configuration_raw: configuration_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: recorded_at.to_owned(),
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    })
}

fn selected_schema_from_context(
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
    let ctx = context_selected_record(
        &configuration_raw,
        request_raw,
        &now,
        source_path,
        selected,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let mut worker = selected_schema_from_context(
        invocation,
        &ctx.files,
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

/// Read-only selected-record route for an explicitly protected V2 current
/// source. Durable V2 transaction/recovery publication is delegated to the
/// storage owner path and is refused here until that path accepts this exact
/// persisted readset and rebuilds against the latest rootset.
pub(super) fn run_v2(
    invocation: &Value,
    request_raw: &[u8],
    session: &mut NativeSourceV2Session,
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
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    if !matches!(operation, "describe" | "prepare-revise") {
        return Err(SourceCommandError::Unsupported(
            "V2 source revision write or recovery requires durable readset publication",
        ));
    }
    let (filesystem, configuration_raw) = CreationFilesystem::select_protected_native_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let configuration = cmd::parse(&configuration_raw)?;
    if cmd::text(&configuration, "schema_version")?
        != "tos_local_native_metadata_revision_owner_v1"
    {
        return Err(SourceCommandError::Denied(
            "V2 source revision exact protected family",
        ));
    }
    if owner::revision_publication::pending(&filesystem, deadline, cancelled)? {
        return Err(SourceCommandError::Unsupported(
            "V2 source revision recovery requires persisted readset revalidation",
        ));
    }
    if components.capture() != software.selection() {
        return Err(SourceCommandError::Conflict(
            "selected software capture components differ",
        ));
    }
    let now = crate::source_serialization::instant()?;
    let source_path = cmd::text(&configuration, "source_path")?;
    let selected_revision = session.selected_revision();
    let mut transport = NativeSourceV2RecordFiles::new(session, selected_revision)?;
    let ctx = context_selected_record_from_transport(
        &configuration_raw,
        request_raw,
        &now,
        source_path,
        selected_revision,
        &mut transport,
        software,
        components,
        deadline,
        cancelled,
    )?;
    filesystem.current_context(&ctx, deadline, cancelled)?;
    ctx.check()?;
    let publication = crate::source_revisions::read_record_revision_publication(&ctx, &[])?;
    let mut worker = selected_schema_from_context(
        invocation,
        &ctx.files,
        selected_revision,
        deadline,
        cancelled,
    )?;
    let prepared = crate::source_revisions::prepare_record_revision_inner(
        &ctx,
        Some(&publication),
        None,
        &mut worker,
        deadline,
        cancelled,
    );
    let proposal = match prepared {
        Ok(proposal) => {
            worker
                .finish(deadline, cancelled)
                .map_err(|_| SourceCommandError::Denied("revision worker FINAL"))?;
            proposal
        }
        Err(error) => {
            let _ = worker.finish(deadline, cancelled);
            return Err(error);
        }
    };
    let verified = transport.verify_readset_current(deadline, cancelled)?;
    if verified.format != tos_source_store::SourceCutFormat::NativeAdmissionV2 {
        return Err(SourceCommandError::Conflict(
            "V2 source revision readset selected another source format",
        ));
    }
    filesystem.current_context(&ctx, deadline, cancelled)?;
    let opened = transport.readset().base_revision;
    Ok(json!({
        "schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account",
        "result":value(&proposal.response)?,
        "source_readset_base_revision":opened.0.to_prefixed(),
        "source_readset_current_revision":verified.current_revision.0.to_prefixed(),
        "grants_admission":false
    }))
}
