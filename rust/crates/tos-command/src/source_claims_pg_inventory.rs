//! Digest-only cold source owner projection for maintained Global Claims.
//!
//! This is a full-cut bootstrap reader. Ordinary Claim create/revise continues
//! to use the bounded selected-package adapter and must not call this module.

use super::*;

const CLAIM_OWNER_SCHEMA: &str = "tos_managed_global_claim_owner_inventory_v1";
const IMPORTED_LEGACY_CLAIM_OWNER_SCHEMA: &str =
    "tos_managed_imported_legacy_claim_owner_inventory_v1";

/// Read-only owner marker for one public legacy Claim stream that has no
/// maintained typed Claim projector. It preserves the source catalog rows and
/// refuses sidecar packages that require a stronger owner route.
pub(crate) fn imported_legacy_claim_owner_contribution(
    ctx: &CommandContext,
    file: &SourceFile,
) -> SourceCommandResult<JsonValue> {
    ctx.check()?;
    let location = file.path.as_str();
    if !super::is_imported_legacy_claim_path(location)
        || ctx
            .files
            .iter()
            .filter(|member| member.path == file.path && member.raw == file.raw)
            .count()
            != 1
    {
        return Err(SourceCommandError::Conflict(
            "imported legacy Claim source binding differs",
        ));
    }
    let home = location
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(SourceCommandError::Invalid("imported legacy Claim home"))?;
    let stream = location.rsplit('/').next().unwrap_or(location);
    let stem = stream
        .strip_suffix(".jsonl")
        .ok_or(SourceCommandError::Invalid(
            "imported legacy Claim stream suffix",
        ))?;
    let rows = super::imported_legacy_claim_rows(file)?;
    let claim_form_paths = rows
        .iter()
        .map(|row| {
            format!(
                "{home}/{stem}.{}.human-forms.json",
                Digest256::of_bytes(row.claim_id.as_bytes()).to_hex()
            )
        })
        .collect::<BTreeSet<_>>();
    let current_claim_path = format!("{home}/{}", super::CLAIM_STREAM);
    let claim_history_path = format!("{home}/{}", super::CLAIM_HISTORY);
    if ctx
        .files
        .iter()
        .any(|member| claim_form_paths.contains(member.path.as_str()))
        || (ctx
            .files
            .iter()
            .any(|member| member.path.as_str() == claim_history_path)
            && !ctx
                .files
                .iter()
                .any(|member| member.path.as_str() == current_claim_path))
    {
        return Err(SourceCommandError::Unsupported(
            "imported legacy Claim sidecar requires its source owner",
        ));
    }

    let mut claims_by_id = BTreeMap::new();
    for row in rows {
        let catalog = super::catalogue_claim(
            ctx,
            &row.record,
            location,
            usize::try_from(row.physical_line)
                .map_err(|_| SourceCommandError::Invalid("imported Claim line overflow"))?,
            false,
            None,
        )?
        .construct()?;
        let claim_sha256 = super::record_digest(&row.record)?.to_prefixed();
        let contribution = object(vec![
            ("claim_id", string(&row.claim_id)),
            ("source_claim_line", number(row.physical_line)),
            ("claim_sha256", string(&claim_sha256)),
            ("catalogue_entry", catalog),
        ]);
        if claims_by_id.insert(row.claim_id, contribution).is_some() {
            return Err(SourceCommandError::Conflict("duplicate Claim identity"));
        }
    }
    let owner_core = object(vec![
        ("schema_version", string(IMPORTED_LEGACY_CLAIM_OWNER_SCHEMA)),
        ("source_path", string(location)),
        ("owner_mode", string("imported_read_only")),
        (
            "stream_sha256",
            string(&Digest256::of_bytes(&file.raw).to_prefixed()),
        ),
        ("form_refs", JsonValue::Array(Vec::new())),
        ("claim_history_ref", JsonValue::Null),
        (
            "claims",
            JsonValue::Array(claims_by_id.into_values().collect()),
        ),
    ]);
    let closure_sha256 = super::record_digest(&owner_core)?.to_prefixed();
    let mut owner = owner_core;
    set(&mut owner, "closure_sha256", string(&closure_sha256))?;
    Ok(owner)
}

/// Retain only the path-based private/local custody classification. No source
/// body, bytes, or digest is copied into this owner marker.
pub(crate) fn nonprojecting_custody_owner_contribution(
    location: &str,
) -> SourceCommandResult<JsonValue> {
    let custody_kind = super::nonprojecting_custody_kind(location).ok_or(
        SourceCommandError::Unsupported("nonprojecting source custody path"),
    )?;
    Ok(object(vec![
        (
            "schema_version",
            string("tos_managed_nonprojecting_source_custody_v1"),
        ),
        ("source_path", string(location)),
        ("custody_kind", string(custody_kind)),
        ("content_projected", JsonValue::Bool(false)),
    ]))
}

/// Reuse the exact profile files already validated for the mixed inventory
/// epoch when a streamed owner assembles a bounded per-package context.
pub(crate) fn claim_profile_source_files(
    profile: &AgentInventoryAssessmentProfile,
) -> Vec<SourceFile> {
    profile.profile_files.clone()
}

pub(crate) fn claim_profile_sources_from_validated_profile(
    profile: &AgentInventoryAssessmentProfile,
) -> SourceCommandResult<JsonValue> {
    profile_sources(profile)
}

pub(crate) fn historical_claim_owner_contribution(
    ctx: &CommandContext,
    file: &SourceFile,
    grouped: &AgentInventoryMembers<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    historical_claim_owner_contribution_from_profile(
        ctx,
        file,
        &grouped.profile,
        worker,
        deadline,
        cancelled,
    )
}

pub(crate) fn historical_claim_owner_contribution_from_profile(
    ctx: &CommandContext,
    file: &SourceFile,
    profile: &AgentInventoryAssessmentProfile,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let mut source_profiles = profile_sources(profile)?;
    for path in crate::source_native_cli::pg_historical_claim_required_contract_paths() {
        let contract = ctx
            .files
            .iter()
            .find(|member| member.path.as_str() == path)
            .ok_or(SourceCommandError::Conflict(
                "historical Claim profile contract absent",
            ))?;
        let digest = Digest256::of_bytes(&contract.raw).to_hex();
        if source_profiles
            .object_get(&path)
            .and_then(JsonValue::as_str)
            .is_some_and(|profiled| profiled != digest.as_str())
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim validated profile bytes changed",
            ));
        }
        set(&mut source_profiles, &path, string(&digest))?;
    }
    crate::source_native_cli::pg_current_historical_claim_owner_contribution(
        ctx,
        file,
        source_profiles,
        worker,
        deadline,
        cancelled,
    )
}

fn role(name: &str) -> SourceCommandResult<&'static str> {
    Ok(match name {
        CLAIM_STREAM => "claim-stream",
        "source-create-request.json" => "creation-request",
        "source-create-environment.json" => "creation-environment",
        "source-create-provenance.jsonl" => "creation-event",
        "source-create-receipt.json" => "creation-receipt",
        CLAIM_HISTORY => "revision-history",
        name if name.ends_with(".human-forms.json") => "human-form-set",
        _ => {
            return Err(SourceCommandError::Conflict(
                "unrecognized maintained Claim package member",
            ));
        }
    })
}

fn member_ref(path: &str, role: &str, raw: &[u8]) -> JsonValue {
    object(vec![
        ("ref", string(path)),
        ("role", string(role)),
        ("sha256", string(&Digest256::of_bytes(raw).to_prefixed())),
        ("bytes", number(raw.len() as u64)),
    ])
}

fn profile_sources(profile: &AgentInventoryAssessmentProfile) -> SourceCommandResult<JsonValue> {
    let mut profiles = profile.base_profiles.clone();
    for input in &profile.profile_files {
        let name = input.path.as_str();
        if name.starts_with("ToS/contracts/") && name.ends_with(".schema.json") {
            set(
                &mut profiles,
                name,
                string(&Digest256::of_bytes(&input.raw).to_hex()),
            )?;
        }
    }
    Ok(profiles)
}

fn evidence(
    grouped: &AgentInventoryMembers<'_>,
    location: &str,
    anchors: bool,
) -> SourceCommandResult<JsonValue> {
    let member = grouped.members.get(location);
    let mut entries = member
        .into_iter()
        .flat_map(|member| {
            if anchors {
                member.anchors.iter()
            } else {
                member.events.iter()
            }
        })
        .map(|(key, entry)| {
            Ok(AgentInventoryEvidenceRow {
                id: key
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("mixed evidence identity"))?
                    .to_owned(),
                physical_line: integer(entry, "source_line")?,
                source_sha256: Digest256::from_hex(text(entry, "source_sha256")?)
                    .map_err(|_| SourceCommandError::Invalid("mixed evidence source digest"))?,
                canonical_payload: canonical(field(entry, "payload")?)?,
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    entries.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    agent_evidence_map(
        location,
        if anchors {
            AgentInventoryEvidenceKind::Anchor
        } else {
            AgentInventoryEvidenceKind::Event
        },
        if anchors { "anchor_id" } else { "event_id" },
        entries.into_iter().map(Ok),
    )
}

fn empty_records() -> SourceCommandResult<JsonValue> {
    let mut records = object(vec![]);
    for kind in MIXED_INVENTORY_RECORD_KINDS {
        set(&mut records, kind, JsonValue::Array(Vec::new()))?;
    }
    Ok(records)
}

fn base_projection(
    file: &SourceFile,
    profile: &AgentInventoryAssessmentProfile,
    grouped: &AgentInventoryMembers<'_>,
    form: JsonValue,
    claim_owner: JsonValue,
) -> SourceCommandResult<JsonValue> {
    let location = file.path.as_str();
    Ok(object(vec![
        (
            "schema_version",
            string("tos_managed_mixed_source_inventory_member_v2"),
        ),
        ("path", string(location)),
        (
            "raw_sha256",
            string(&Digest256::of_bytes(&file.raw).to_hex()),
        ),
        ("records", empty_records()?),
        ("source_profiles", profile_sources(profile)?),
        ("events", evidence(grouped, location, false)?),
        ("anchors", evidence(grouped, location, true)?),
        ("form", form),
        ("claim_owner", claim_owner),
        ("public_text_owner", JsonValue::Null),
    ]))
}

fn creation_receipt_check(
    path: &RelativePath,
    request: &JsonValue,
    direct: &BTreeMap<String, Vec<u8>>,
    original: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<JsonValue> {
    let raw = direct
        .get("source-create-receipt.json")
        .ok_or(SourceCommandError::Conflict(
            "Claim creation receipt absent",
        ))?;
    let receipt = parse(raw)?;
    exact_keys(
        &receipt,
        &[
            "schema_version",
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "source_path",
            "dependencies",
            "source_bindings",
            "claims",
            "files",
            "grants_admission",
        ],
    )?;
    if text(&receipt, "schema_version")? != "tos_local_claim_create_receipt_v1"
        || text(&receipt, "source_path")? != path.as_str()
        || !same(
            field(&receipt, "command_id")?,
            field(request, "command_id")?,
        )?
        || text(&receipt, "request_digest")? != record_digest(request)?.to_prefixed()
        || !same(
            field(&receipt, "owner_configuration")?,
            field(request, "expected_configuration")?,
        )?
        || !same(
            field(&receipt, "dependencies")?,
            field(request, "expected_dependencies")?,
        )?
        || !same(
            field(&receipt, "source_bindings")?,
            field(request, "expected_inputs")?,
        )?
        || field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || text(&receipt, "principal_id")?.is_empty()
        || text(&receipt, "authority_ref")?.is_empty()
    {
        return Err(SourceCommandError::Conflict(
            "Claim creation receipt binding differs",
        ));
    }
    validate_instant(text(&receipt, "recorded_at")?)?;
    let expected_claims = array(request, "claims")?
        .iter()
        .map(metadata_subject)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if !same(
        field(&receipt, "claims")?,
        &JsonValue::Array(expected_claims),
    )? {
        return Err(SourceCommandError::Conflict(
            "Claim creation receipt identity set differs",
        ));
    }
    let receipt_files = field(&receipt, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Claim creation receipt files"))?;
    let original_names = [
        CLAIM_STREAM,
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
    ];
    if receipt_files.len() != original_names.len() {
        return Err(SourceCommandError::Conflict(
            "Claim creation receipt member set differs",
        ));
    }
    let receipt_file_map = JsonValue::Object(receipt_files.to_vec());
    for name in original_names {
        let bytes = original.get(name).ok_or(SourceCommandError::Conflict(
            "Claim original creation member absent",
        ))?;
        let binding = field(&receipt_file_map, name)?;
        exact_keys(binding, &["sha256", "bytes"])?;
        if text(binding, "sha256")? != Digest256::of_bytes(bytes).to_prefixed()
            || integer(binding, "bytes")? != bytes.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "Claim creation receipt member digest differs",
            ));
        }
    }
    Ok(receipt)
}

fn claim_owner_projection(
    ctx: &CommandContext,
    file: &SourceFile,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let location = file.path.as_str();
    let parts = location.split('/').collect::<Vec<_>>();
    if parts.len() != 5
        || parts[..3] != ["ToS", "source-witnesses", "relations"]
        || parts[4] != CLAIM_STREAM
    {
        return Err(SourceCommandError::Unsupported(
            "maintained Global Claim owner path",
        ));
    }
    let home = parts[..4].join("/");
    let direct = package(ctx, &file.path)?;
    let request = parse(direct.get("source-create-request.json").ok_or(
        SourceCommandError::Conflict("Claim creation request absent"),
    )?)?;
    grammar(&request, true, false)?;
    if text(&request, "operation")? != "claims.create" {
        return Err(SourceCommandError::Conflict(
            "maintained Claim creation request differs",
        ));
    }
    let config = object(vec![("source_path", string(location))]);
    let mut original =
        retained_claim_creation_files(ctx, &config, &request, &file.path, &direct, None)?;
    let receipt = creation_receipt_check(&file.path, &request, &direct, &original)?;
    let _history = verify_claim_history(ctx, &config, &file.path, &direct)?;
    // Creation provenance describes the four original capture files, before
    // the receipt, forms and subsequent revisions existed. Historical source
    // captures retain their recorded procedure; reading never rewrites them
    // as evidence of execution by the current native writer.
    original.remove("source-create-receipt.json");
    check_claim_capture(
        ctx,
        &path(&home)?,
        &original,
        &["native-claim-serialization", "source-claim-serialization"],
        worker,
        deadline,
        cancelled,
    )?;

    let stream_claims = rows(&file.raw)?;
    if stream_claims.is_empty() || stream_claims.len() > 32 {
        return Err(SourceCommandError::Conflict(
            "maintained Claim batch size differs",
        ));
    }
    let history_raw = direct.get(CLAIM_HISTORY);
    let history_value = _history;
    let history_receipts = array(&history_value, "receipts")?;
    let mut referenced_archives = BTreeMap::<String, JsonValue>::new();
    let mut history_rows = Vec::with_capacity(history_receipts.len());
    for retained in history_receipts {
        let archive_path = text(retained, "archive_path")?.to_owned();
        let archive_home = path(&format!("{archive_path}/manifest.json"))?;
        let archive = package(ctx, &archive_home)?;
        let manifest_raw = archive
            .get("manifest.json")
            .ok_or(SourceCommandError::Conflict(
                "Claim archive manifest absent",
            ))?;
        let manifest = parse(manifest_raw)?;
        let manifest_files =
            field(&manifest, "files")?
                .as_object()
                .ok_or(SourceCommandError::Invalid(
                    "Claim archive manifest members",
                ))?;
        let mut archive_members = vec![member_ref(
            &format!("{archive_path}/manifest.json"),
            "archive-manifest",
            manifest_raw,
        )];
        let mut expected_names = BTreeSet::from(["manifest.json".to_owned()]);
        for (name, binding) in manifest_files {
            let name = name
                .as_str()
                .ok_or(SourceCommandError::Invalid("Claim archive member name"))?;
            let blob = text(binding, "blob")?;
            let raw = archive
                .get(blob)
                .ok_or(SourceCommandError::Conflict("Claim archive blob absent"))?;
            if expected_names.insert(blob.to_owned()) {
                archive_members.push(member_ref(
                    &format!("{archive_path}/{blob}"),
                    "archive-blob",
                    raw,
                ));
            }
            if integer(binding, "bytes")? != raw.len() as u64
                || text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
                || blob_name(raw) != blob
            {
                return Err(SourceCommandError::Conflict(
                    "Claim archive blob binding differs",
                ));
            }
            let _ = name;
        }
        if archive.keys().cloned().collect::<BTreeSet<_>>() != expected_names {
            return Err(SourceCommandError::Conflict(
                "Claim archive package membership differs",
            ));
        }
        archive_members.sort_by(|left, right| {
            text(left, "ref")
                .unwrap_or("")
                .as_bytes()
                .cmp(text(right, "ref").unwrap_or("").as_bytes())
        });
        let package = object(vec![
            ("archive_path", string(&archive_path)),
            (
                "manifest_sha256",
                string(&Digest256::of_bytes(manifest_raw).to_prefixed()),
            ),
            ("members", JsonValue::Array(archive_members)),
        ]);
        if referenced_archives
            .insert(archive_path.clone(), package.clone())
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "duplicate Claim history archive",
            ));
        }
        history_rows.push(object(vec![
            ("command_id", field(retained, "command_id")?.clone()),
            (
                "claim_id",
                field(field(retained, "previous_source")?, "id")?.clone(),
            ),
            (
                "previous_revision",
                field(retained, "previous_revision")?.clone(),
            ),
            ("archive_path", string(&archive_path)),
            (
                "archive_manifest_sha256",
                string(&Digest256::of_bytes(manifest_raw).to_prefixed()),
            ),
            ("forms", field(retained, "forms")?.clone()),
        ]));
    }
    let known_archive_paths = referenced_archives.keys().cloned().collect::<BTreeSet<_>>();
    for claim_id in stream_claims.keys() {
        let prefix = format!(
            "ToS/source-witnesses/.record-revisions/{}-",
            Digest256::of_bytes(claim_id.as_bytes())
                .to_prefixed()
                .trim_start_matches("sha256:")
        );
        for selected in &ctx.files {
            if selected.path.as_str().starts_with(&prefix) {
                let archive_path = selected
                    .path
                    .as_str()
                    .rsplit_once('/')
                    .ok_or(SourceCommandError::Invalid("Claim archive member path"))?
                    .0;
                if !known_archive_paths.contains(archive_path) {
                    return Err(SourceCommandError::Conflict(
                        "unreferenced Claim archive selected",
                    ));
                }
            }
        }
    }
    let mut direct_members = Vec::with_capacity(direct.len());
    for (name, raw) in &direct {
        // retained_claim_creation_files has checked their exact allowed
        // names and empty bytes. Locks are operational sidecars, not authored
        // members of the source package.
        if name.ends_with(".writer.lock") {
            continue;
        }
        direct_members.push(member_ref(&format!("{home}/{name}"), role(name)?, raw));
    }
    let mut claims = Vec::with_capacity(stream_claims.len());
    for (claim_id, record) in &stream_claims {
        let name = form_name(claim_id);
        let form_path = format!("{home}/{name}");
        let current_form = direct.get(&name).map(|raw| parse(raw)).transpose()?;
        let form_ids = if let Some(form) = &current_form {
            let subject = metadata_subject(record)?;
            if text(field(form, "subject")?, "id")? != text(&subject, "id")? {
                return Err(SourceCommandError::Conflict("Claim form subject differs"));
            }
            crate::source_forms::projection_form_ids(form)?
        } else {
            Vec::new()
        };
        let current_form_raw = direct.get(&name);
        claims.push(object(vec![
            ("claim_id", string(claim_id)),
            ("claim_version", number(integer(record, "claim_version")?)),
            (
                "claim_sha256",
                string(&record_digest(record)?.to_prefixed()),
            ),
            ("form_ref", string(&form_path)),
            ("form_present", JsonValue::Bool(current_form_raw.is_some())),
            (
                "form_sha256",
                current_form_raw
                    .map(|raw| string(&Digest256::of_bytes(raw).to_prefixed()))
                    .unwrap_or(JsonValue::Null),
            ),
            ("form_ids", JsonValue::Array(form_ids)),
        ]));
    }
    let owner_core = object(vec![
        ("schema_version", string(CLAIM_OWNER_SCHEMA)),
        ("package_home", string(&home)),
        ("source_path", string(location)),
        (
            "stream_sha256",
            string(&Digest256::of_bytes(&file.raw).to_prefixed()),
        ),
        ("direct_members", JsonValue::Array(direct_members)),
        (
            "history",
            object(vec![
                ("ref", string(&format!("{home}/{CLAIM_HISTORY}"))),
                ("present", JsonValue::Bool(history_raw.is_some())),
                (
                    "sha256",
                    history_raw
                        .map(|raw| string(&Digest256::of_bytes(raw).to_prefixed()))
                        .unwrap_or(JsonValue::Null),
                ),
                ("receipts", JsonValue::Array(history_rows)),
            ]),
        ),
        (
            "archives",
            JsonValue::Array(referenced_archives.into_values().collect()),
        ),
        ("claims", JsonValue::Array(claims)),
        (
            "creation_receipt_sha256",
            string(
                &Digest256::of_bytes(direct["source-create-receipt.json"].as_slice()).to_prefixed(),
            ),
        ),
        (
            "creation_receipt_command_id",
            field(&receipt, "command_id")?.clone(),
        ),
    ]);
    let closure_sha256 = record_digest(&owner_core)?.to_prefixed();
    let mut owner = owner_core.clone();
    set(&mut owner, "closure_sha256", string(&closure_sha256))?;
    Ok(owner)
}

/// Validate and serialize the exact current Global Claim owner package for a
/// selected member. Callers supply only the current package and its retained
/// archive closure; this does not assert that those files are a complete
/// corpus inventory.
pub(crate) fn claim_owner_contribution(
    ctx: &CommandContext,
    file: &SourceFile,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    claim_owner_projection(ctx, file, worker, deadline, cancelled)
}

pub(crate) fn claim_inventory_contribution(
    ctx: &CommandContext,
    file: &SourceFile,
    grouped: &AgentInventoryMembers<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let owner = claim_owner_projection(ctx, file, worker, deadline, cancelled)?;
    base_projection(file, &grouped.profile, grouped, JsonValue::Null, owner)
}

/// Resolve only maintained per-Claim HumanForm files to their exact current
/// stream. Item streams are admitted here because their separate Item owner
/// contribution validates the adoption receipt and retained package history.
pub(crate) fn claim_form_stream_path(location: &str) -> Option<String> {
    if let Some(stream) = crate::source_claims::historical_claim_form_stream_path(location) {
        return Some(stream);
    }
    let basename = location.rsplit('/').next()?;
    if !basename.starts_with("source-claims.") || !basename.ends_with(".human-forms.json") {
        return None;
    }
    let parent = location.rsplit_once('/')?.0;
    let stream = format!("{parent}/{CLAIM_STREAM}");
    (location.starts_with("ToS/source-witnesses/relations/")
        || crate::source_claims::is_managed_item_claim_stream_path(&stream))
    .then_some(stream)
}

pub(crate) fn claim_form_inventory_contribution(
    file: &SourceFile,
    grouped: &AgentInventoryMembers<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let location = file.path.as_str();
    let suffix = ".human-forms.json";
    let stream_path = claim_form_stream_path(location).ok_or(SourceCommandError::Unsupported(
        "maintained Claim form path",
    ))?;
    let stream = grouped
        .files
        .get(stream_path.as_str())
        .ok_or(SourceCommandError::Conflict("Claim form stream absent"))?;
    let basename = location.rsplit('/').next().unwrap_or(location);
    let stem = basename
        .strip_suffix(suffix)
        .ok_or(SourceCommandError::Invalid("Claim form basename"))?;
    let stream_stem = stream_path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".jsonl"))
        .ok_or(SourceCommandError::Invalid("Claim form stream basename"))?;
    let hash = stem
        .strip_prefix(&format!("{stream_stem}."))
        .ok_or(SourceCommandError::Invalid("Claim form filename"))?;
    let candidates = rows(&stream.raw)?
        .into_iter()
        .filter(|(id, _)| {
            Digest256::of_bytes(id.as_bytes())
                .to_prefixed()
                .trim_start_matches("sha256:")
                == hash
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "Claim form owner identity differs",
        ));
    }
    let (_, record) = &candidates[0];
    let form = parse(&file.raw)?;
    let subject = metadata_subject(record)?;
    if !same(field(&form, "subject")?, &subject)? {
        return Err(SourceCommandError::Conflict("Claim form subject differs"));
    }
    let form_ids = crate::source_forms::projection_form_ids(&form)?;
    let mut schema_files = grouped.profile.profile_files.clone();
    schema_files.push(file.clone());
    let input = crate::source_revisions::RecordVersionReadInput {
        files: &schema_files,
        source_revision: grouped.profile.source_revision,
        effective_uid: grouped.profile.effective_uid,
        schema_source_path: location,
    };
    crate::source_revisions::validate_readonly_schema(
        &input,
        location,
        &[AGENT_FORM_SET_SCHEMA.to_owned()],
        AGENT_FORM_SET_SCHEMA,
        &form,
        worker,
        deadline,
        cancelled,
    )?;
    let form_contribution = object(vec![
        (
            "raw_sha256",
            string(&Digest256::of_bytes(&file.raw).to_prefixed()),
        ),
        ("form_ids", JsonValue::Array(form_ids)),
    ]);
    base_projection(
        file,
        &grouped.profile,
        grouped,
        form_contribution,
        JsonValue::Null,
    )
}

/// Build the mixed-inventory member projection for a selected historical
/// Claim stream or one of its exact per-Claim HumanForm sets. The owner marker
/// belongs only to the stream; forms carry the shared form block.
pub(crate) fn historical_claim_inventory_contribution(
    file: &SourceFile,
    grouped: &AgentInventoryMembers<'_>,
    owner: Option<&JsonValue>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let location = file.path.as_str();
    if crate::source_claims::is_historical_claim_stream_path(location) {
        let owner = owner.ok_or(SourceCommandError::Conflict(
            "historical Claim owner contribution absent",
        ))?;
        return base_projection(
            file,
            &grouped.profile,
            grouped,
            JsonValue::Null,
            owner.clone(),
        );
    }
    if crate::source_claims::historical_claim_form_stream_path(location).is_none() {
        return Err(SourceCommandError::Unsupported(
            "historical Claim projection path",
        ));
    }
    let member = claim_form_inventory_contribution(file, grouped, worker, deadline, cancelled)?;
    base_projection(
        file,
        &grouped.profile,
        grouped,
        field(&member, "form")?.clone(),
        JsonValue::Null,
    )
}

pub(crate) fn claim_form_contribution_with_validated_profile(
    ctx: &CommandContext,
    file: &SourceFile,
    profile: &AgentInventoryAssessmentProfile,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let location = file.path.as_str();
    let stream_path = claim_form_stream_path(location)
        .ok_or(SourceCommandError::Unsupported("Claim form path"))?;
    let stream = ctx
        .files
        .iter()
        .find(|member| member.path.as_str() == stream_path)
        .ok_or(SourceCommandError::Conflict("Claim form stream absent"))?;
    let stream_stem = stream_path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".jsonl"))
        .ok_or(SourceCommandError::Invalid("Claim form stream basename"))?;
    let basename = location.rsplit('/').next().unwrap_or(location);
    let hash = basename
        .strip_suffix(".human-forms.json")
        .and_then(|stem| stem.strip_prefix(&format!("{stream_stem}.")))
        .ok_or(SourceCommandError::Invalid("Claim form filename"))?;
    let candidates = rows(&stream.raw)?
        .into_iter()
        .filter(|(id, _)| {
            Digest256::of_bytes(id.as_bytes())
                .to_prefixed()
                .trim_start_matches("sha256:")
                == hash
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "Claim form owner identity differs",
        ));
    }
    let (_, record) = &candidates[0];
    let form = parse(&file.raw)?;
    let subject = metadata_subject(record)?;
    if !same(field(&form, "subject")?, &subject)? {
        return Err(SourceCommandError::Conflict("Claim form subject differs"));
    }
    apply_form_changes(Some(&form), &subject, &[])?;
    let mut schema_files = profile.profile_files.clone();
    schema_files.push(file.clone());
    let input = crate::source_revisions::RecordVersionReadInput {
        files: &schema_files,
        source_revision: profile.source_revision,
        effective_uid: profile.effective_uid,
        schema_source_path: location,
    };
    crate::source_revisions::validate_readonly_schema(
        &input,
        location,
        &[AGENT_FORM_SET_SCHEMA.to_owned()],
        AGENT_FORM_SET_SCHEMA,
        &form,
        worker,
        deadline,
        cancelled,
    )?;
    let form_ids = crate::source_forms::projection_form_ids(&form)?;
    Ok(object(vec![
        (
            "raw_sha256",
            string(&Digest256::of_bytes(&file.raw).to_prefixed()),
        ),
        ("form_ids", JsonValue::Array(form_ids)),
    ]))
}

/// Prepare only the Claim-owned fields for each changed Claim stream or form.
/// The shared PG writer supplies the common path and exact successor digest.
pub(crate) fn revision_owner_contributions(
    context: &CommandContext,
    proposal: &PreparedCommand,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    if proposal.replayed || proposal.changes.is_empty() {
        return Ok(BTreeMap::new());
    }
    let successor = claim_successor_context(context, proposal)?;
    let grouped = crate::source_claims::agent_inventory_members_without_evidence(
        &successor, worker, deadline, cancelled,
    )?;
    let mut contributions = BTreeMap::new();
    for change in &proposal.changes {
        let Some(raw) = &change.after else {
            continue;
        };
        let path = change.path.as_str();
        let Some(file) = successor
            .files
            .iter()
            .find(|file| file.path == change.path && file.raw == *raw)
        else {
            return Err(SourceCommandError::Conflict(
                "Claim successor member differs from owner proposal",
            ));
        };
        let basename = path.rsplit('/').next().unwrap_or(path);
        let contribution =
            if path.starts_with("ToS/source-witnesses/relations/") && basename == CLAIM_STREAM {
                let member = claim_inventory_contribution(
                    &successor, file, &grouped, worker, deadline, cancelled,
                )?;
                object(vec![
                    (
                        "schema_version",
                        string("tos_pg_global_claim_projection_contribution_v1"),
                    ),
                    ("claim_owner", field(&member, "claim_owner")?.clone()),
                    (
                        "source_profiles",
                        field(&member, "source_profiles")?.clone(),
                    ),
                    ("events", field(&member, "events")?.clone()),
                    ("anchors", field(&member, "anchors")?.clone()),
                ])
            } else if path.starts_with("ToS/source-witnesses/relations/")
                && basename.ends_with(".human-forms.json")
            {
                let member =
                    claim_form_inventory_contribution(file, &grouped, worker, deadline, cancelled)?;
                object(vec![
                    (
                        "schema_version",
                        string("tos_pg_global_claim_form_projection_contribution_v1"),
                    ),
                    ("form", field(&member, "form")?.clone()),
                    (
                        "source_profiles",
                        field(&member, "source_profiles")?.clone(),
                    ),
                    ("events", field(&member, "events")?.clone()),
                    ("anchors", field(&member, "anchors")?.clone()),
                ])
            } else {
                continue;
            };
        if contributions
            .insert(path.to_owned(), contribution)
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "duplicate Claim owner contribution path",
            ));
        }
    }
    Ok(contributions)
}

fn claim_successor_context(
    context: &CommandContext,
    proposal: &PreparedCommand,
) -> SourceCommandResult<CommandContext> {
    let mut files = context
        .files
        .iter()
        .cloned()
        .map(|file| (file.path.as_str().to_owned(), file))
        .collect::<BTreeMap<_, _>>();
    let mut changed_paths = BTreeSet::new();
    for change in &proposal.changes {
        if !changed_paths.insert(change.path.as_str()) {
            return Err(SourceCommandError::Conflict(
                "Claim proposal has duplicate successor paths",
            ));
        }
    }
    for change in &proposal.changes {
        let path = change.path.as_str();
        let current = files.get(path);
        let before_matches = match (change.before, current) {
            (None, None) => true,
            (Some(expected), Some(current)) => Digest256::of_bytes(&current.raw) == expected,
            _ => false,
        };
        if !before_matches {
            return Err(SourceCommandError::Conflict(
                "Claim proposal predecessor differs from selected bytes",
            ));
        }
        if let Some(raw) = &change.after {
            files.insert(
                path.to_owned(),
                SourceFile {
                    path: change.path.clone(),
                    raw: raw.clone(),
                },
            );
        } else {
            files.remove(path);
        }
    }
    let successor = CommandContext {
        base_revision: context.base_revision,
        configuration_raw: context.configuration_raw.clone(),
        request_raw: context.request_raw.clone(),
        recorded_at: context.recorded_at.clone(),
        effective_uid: context.effective_uid,
        files: files.into_values().collect(),
    };
    successor.check()?;
    Ok(successor)
}

#[cfg(test)]
mod imported_legacy_owner_tests {
    use super::*;

    #[test]
    fn imported_legacy_owner_preserves_public_assertion_without_admission() {
        let location = "ToS/source-witnesses/works/example/publication-claims.jsonl";
        let record = object(vec![
            ("schema_version", string("tos_claim_packet_v1")),
            ("claim_id", string("tos.claim.example.rights")),
            ("claim_type", string("bibliographic")),
            ("assertion_layer", string("bibliographic_assertion")),
            (
                "subject_ref",
                object(vec![("id", string("tos.work.example"))]),
            ),
            ("predicate", string("rights_statement_at")),
            (
                "object",
                object(vec![
                    (
                        "statement_ref",
                        object(vec![("id", string("tos.rights.example"))]),
                    ),
                    ("statement_at", string("2026-10-12T00:00:00Z")),
                ]),
            ),
            ("evidence_refs", JsonValue::Array(Vec::new())),
            ("maker", JsonValue::Null),
            ("provenance_event_ref", JsonValue::Null),
            ("epistemic_status", string("asserted")),
            ("review_status", string("unreviewed")),
            ("visibility", string("public")),
            ("claim_version", number(1)),
        ]);
        let mut raw = canonical(&record).unwrap();
        raw.push(b'\n');
        let file = SourceFile {
            path: path(location).unwrap(),
            raw,
        };
        let ctx = CommandContext {
            base_revision: tos_foundation::SourceRevision(Digest256::of_bytes(b"test cut")),
            configuration_raw: b"{}".to_vec(),
            request_raw: b"{}".to_vec(),
            recorded_at: "2026-10-12T00:00:00Z".into(),
            effective_uid: 0,
            files: vec![
                file.clone(),
                SourceFile {
                    path: path("ToS/source-witnesses/works/example/work.human-forms.json").unwrap(),
                    raw: b"{}".to_vec(),
                },
            ],
        };

        let owner = imported_legacy_claim_owner_contribution(&ctx, &file).unwrap();
        assert_eq!(
            text(&owner, "schema_version").unwrap(),
            IMPORTED_LEGACY_CLAIM_OWNER_SCHEMA
        );
        assert_eq!(text(&owner, "owner_mode").unwrap(), "imported_read_only");
        assert!(array(&owner, "form_refs").unwrap().is_empty());
        assert_eq!(
            field(&owner, "claim_history_ref").unwrap(),
            &JsonValue::Null
        );
        let row = &array(&owner, "claims").unwrap()[0];
        let catalogue = field(row, "catalogue_entry").unwrap();
        assert_eq!(text(catalogue, "predicate").unwrap(), "rights_statement_at");
        assert_eq!(
            canonical(field(catalogue, "object").unwrap()).unwrap(),
            canonical(field(&record, "object").unwrap()).unwrap()
        );

        let claim_form_path = format!(
            "ToS/source-witnesses/works/example/publication-claims.{}.human-forms.json",
            Digest256::of_bytes(b"tos.claim.example.rights").to_hex()
        );
        let mut sidecar_ctx = ctx.clone();
        sidecar_ctx.files.push(SourceFile {
            path: path(&claim_form_path).unwrap(),
            raw: b"{}".to_vec(),
        });
        assert!(matches!(
            imported_legacy_claim_owner_contribution(&sidecar_ctx, &file),
            Err(SourceCommandError::Unsupported(_))
        ));
    }
}
