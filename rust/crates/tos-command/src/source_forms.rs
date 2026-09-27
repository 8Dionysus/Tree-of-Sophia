//! Generic source-owned HumanForm preparation and retained lineage mechanics.
//! Schemas and full source admission are supplied by the existing VAL owner;
//! these functions neither consult fixtures nor mutate canonical source files.
use crate::source_command::{self, SourceCommandError as Error, SourceCommandResult as Result, *};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

fn schema_check(
    worker: &mut CutWorkerSchemaExecutor,
    path: &str,
    raw: &[u8],
    contract: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    if !worker
        .check(path, raw, contract, deadline, cancelled)
        .map_err(|_| Error::Unsupported("exact source schema worker unavailable"))?
    {
        return Err(Error::Invalid("exact source schema rejected bytes"));
    }
    Ok(())
}

/// Authenticate all selected reads before executing the existing form proposal
/// engine. Source and software remain separate; configuration is an independent
/// protected observation, and current owner semantics are checked by the engine.
pub fn run_form_command_from_captures(
    ctx: &CommandContext,
    source: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedCommand> {
    ctx.check_from_selected_captures(source, software, components, deadline, cancelled)?;
    run_form_command(ctx, source, worker, deadline, cancelled)
}

/// Execute maintained owner/request/form proposal semantics over caller bytes.
/// Selected-read custody requires `run_form_command_from_captures` or the
/// independent carrier checks at candidate binding.
/// The returned canonical file bytes are proposals; `PreparedCommand::commit`
/// explicitly refuses until complete source admission and owner fencing exist.
pub fn run_form_command(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedCommand> {
    ctx.check()?;
    if worker.source_revision() != ctx.base_revision
        || cut.current().revision() != ctx.base_revision
    {
        return Err(Error::Conflict(
            "schema worker and command source cut differ",
        ));
    }
    let config = parse(&ctx.configuration_raw)?;
    let request = parse(&ctx.request_raw)?;
    let owner = text(&config, "schema_version")?;
    let canonical = owner == "tos_local_canonical_form_owner_v1";
    let claim_owner = matches!(
        owner,
        "tos_local_claim_form_owner_v1" | "tos_local_claim_form_owner_v2"
    );
    if !matches!(
        owner,
        "tos_local_source_command_owner_v1"
            | "tos_local_canonical_form_owner_v1"
            | "tos_local_claim_form_owner_v1"
            | "tos_local_claim_form_owner_v2"
    ) {
        return Err(Error::Unsupported(
            "Claim/profile owner form closure requires its exact source adapter",
        ));
    }
    let mut config_keys = vec![
        "schema_version",
        "uid",
        "principal_id",
        "source_root",
        "source_path",
        "authority_ref",
        "allowed_form_ids",
        "allowed_operations",
        "expires_at",
    ];
    if claim_owner {
        config_keys.push("claim_id");
    }
    if owner == "tos_local_claim_form_owner_v2" {
        config_keys.push("allowed_field_ids");
    }
    exact_keys(&config, &config_keys)?;
    if claim_owner {
        if ctx.configuration_raw.len() > 1_048_576 || ctx.request_raw.len() > 1_048_576 {
            return Err(Error::Invalid("Claim owner/request byte budget"));
        }
        claim_form_field_ids(&config)?;
        if !claim_id(text(&config, "claim_id")?) {
            return Err(Error::Invalid("Claim forms require stable Claim identity"));
        }
    }
    if integer(&config, "uid")? != ctx.effective_uid
        || !nonblank(text(&config, "principal_id")?)
        || !nonblank(text(&config, "authority_ref")?)
    {
        return Err(Error::Denied("owner local identity differs"));
    }
    validate_expiry(text(&config, "expires_at")?, &ctx.recorded_at)?;
    if !text(&config, "source_root")?.starts_with('/') {
        return Err(Error::Denied("owner source root must be absolute"));
    }
    let source_path = tos_foundation::RelativePath::parse(text(&config, "source_path")?)
        .map_err(|_| Error::Denied("unsafe owner source path"))?;
    let path = source_path.as_str();
    let parts = path.split('/').collect::<Vec<_>>();
    if canonical {
        if parts.len() < 4 || !path.starts_with("ToS/canon/") || parts.last() != Some(&"node.json")
        {
            return Err(Error::Denied("canonical owner path"));
        }
    } else if !path.starts_with("ToS/source-witnesses/")
        || path.starts_with("ToS/source-witnesses/owner-local/")
        || (if claim_owner {
            parts.last() != Some(&"source-claims.jsonl")
        } else {
            !path.ends_with(".json")
        })
        || path.ends_with(".human-forms.json")
        || parts
            .iter()
            .any(|part| matches!(*part, "payload" | "local-content" | "catalog"))
    {
        return Err(Error::Denied("source owner path"));
    }
    for (key, operations) in [("allowed_operations", true), ("allowed_form_ids", false)] {
        let values = array(&config, key)?;
        let mut seen = HashSet::new();
        if values.len() > 32 {
            return Err(Error::Invalid("owner scope budget"));
        }
        for value in values {
            let value = value.as_str().ok_or(Error::Invalid("owner scope text"))?;
            if !seen.insert(value)
                || if operations {
                    !matches!(value, "form.create" | "form.revise")
                } else {
                    !form_id(value)
                }
            {
                return Err(Error::Invalid("owner operation or identity scope"));
            }
        }
    }
    let source_raw = ctx
        .file(&source_path)?
        .ok_or(Error::Invalid("explicit source absent"))?;
    let source = if claim_owner {
        select_form_claim(source_raw, text(&config, "claim_id")?)?
    } else {
        parse(source_raw)?
    };
    let version = text(&source, "schema_version")?;
    let schema = if claim_owner {
        ""
    } else {
        match version {
            "tos_canonical_node_v1" if canonical => "ToS/contracts/tos-node-contract.schema.json",
            "tos_artifact_source_witness_v1" if !canonical => {
                "ToS/contracts/artifact-source-witness.schema.json"
            }
            "tos_artifact_source_witness_v2" if !canonical => {
                "ToS/contracts/artifact-source-witness-v2.schema.json"
            }
            "tos_scholarly_composite_witness_v1" if !canonical => {
                "ToS/contracts/scholarly-composite-witness.schema.json"
            }
            "tos_corpus_record_v1" if !canonical => "ToS/contracts/corpus-record.schema.json",
            "tos_historical_record_v1" if !canonical => {
                "ToS/contracts/historical-record.schema.json"
            }
            _ => {
                return Err(Error::Unsupported(
                    "source profile needs complete registry/native binding adapter",
                ));
            }
        }
    };
    if !claim_owner {
        schema_check(worker, path, source_raw, schema, deadline, cancelled)?;
    }
    let subject = metadata_subject(&source)?;
    if canonical {
        let kind = text(&source, "node_type")?;
        let id = text(&source, "node_id")?;
        let slug = id
            .rsplit('.')
            .next()
            .ok_or(Error::Invalid("canonical ID"))?;
        let directory = parts[parts.len() - 2];
        if parts[2] != kind
            || !id.starts_with(&format!("tos.{kind}."))
            || !(directory == slug
                || kind == "source" && directory.starts_with(&format!("{slug}-")))
        {
            return Err(Error::Denied("canonical source type/path/identity differ"));
        }
    }
    if version == "tos_historical_record_v1"
        && !matches!(
            source.object_get("visibility").and_then(JsonValue::as_str),
            Some("public" | "public_metadata_only")
        )
    {
        return Err(Error::Denied("historical metadata visibility"));
    }
    let target = if claim_owner {
        let stem = path
            .strip_suffix(".jsonl")
            .ok_or(Error::Invalid("Claim stream filename"))?;
        let suffix = Digest256::of_bytes(text(&config, "claim_id")?.as_bytes()).to_hex();
        tos_foundation::RelativePath::parse(&format!("{stem}.{suffix}.human-forms.json"))
    } else {
        let stem = path
            .strip_suffix(".json")
            .ok_or(Error::Invalid("source filename"))?;
        tos_foundation::RelativePath::parse(&format!("{stem}.human-forms.json"))
    }
    .map_err(|_| Error::Invalid("adjacent forms path"))?;
    let old_raw = ctx.file(&target)?;
    let old = old_raw.map(parse).transpose()?;
    if let Some(raw) = old_raw {
        schema_check(
            worker,
            target.as_str(),
            raw,
            "ToS/contracts/human-form-set.schema.json",
            deadline,
            cancelled,
        )?;
        validate_history(old.as_ref().ok_or(Error::Invalid("form set"))?, &subject)?;
    }
    if canonical
        && old.as_ref().is_some_and(|set| {
            array(set, "forms")
                .unwrap_or(&[])
                .iter()
                .chain(array(set, "prior_forms").unwrap_or(&[]))
                .any(|form| {
                    form.object_get("content")
                        .and_then(|c| c.object_get("kind"))
                        .and_then(JsonValue::as_str)
                        != Some("source-copy")
                })
        })
    {
        return Err(Error::Denied("canonical history permits only source-copy"));
    }
    let bound_contracts = matches!(
        version,
        "tos_canonical_node_v1"
            | "tos_artifact_source_witness_v1"
            | "tos_artifact_source_witness_v2"
            | "tos_scholarly_composite_witness_v1"
    );
    let contracts = if claim_owner {
        let raw = canonical_bytes(&source)?;
        let report = tos_validation::record_rules::validate_source_claim_from_cut(
            cut,
            &raw,
            worker,
            tos_validation::item_rules::ItemLimits {
                max_member_bytes: 1_048_576,
                // VAL accounts its selected decoded Claim input; the owner
                // snapshot counts the original complete stream exactly once.
                max_total_bytes: 8_388_608 - source_raw.len() as u64 + raw.len() as u64,
                max_state_bytes: 8_388_608,
                max_issues: 128,
                deadline,
            },
            cancelled,
        )
        .map_err(|_| Error::Unsupported("Claim local profile execution incomplete"))?;
        if report.source_revision != ctx.base_revision
            || report.source_input_sha256 != Digest256::of_bytes(&raw)
        {
            return Err(Error::Conflict("Claim local profile input binding differs"));
        }
        if !report.issues.is_empty() {
            return Err(Error::Invalid(
                "Claim violates selected local source profile",
            ));
        }
        if report.dependency_digests.len() > 128 {
            return Err(Error::Invalid("Claim source dependency snapshot count"));
        }
        // The owner serializer retains first-read insertion order in the
        // receipt. Canonical digest equality alone cannot bind these published
        // bytes: an alphabetically rebuilt map has another raw revision.
        if report.dependency_order.len() != report.dependency_digests.len()
            || report.dependency_order.iter().collect::<HashSet<_>>().len()
                != report.dependency_order.len()
            || report
                .dependency_order
                .iter()
                .any(|path| !report.dependency_digests.contains_key(path))
        {
            return Err(Error::Invalid("Claim source dependency order coverage"));
        }
        let mut contracts = object(vec![]);
        for path in &report.dependency_order {
            let digest = &report.dependency_digests[path];
            let dependency = tos_foundation::RelativePath::parse(path)
                .map_err(|_| Error::Invalid("Claim profile dependency path"))?;
            let raw = ctx.file(&dependency)?.ok_or(Error::Invalid(
                "Claim profile dependency absent from command reads",
            ))?;
            if Digest256::of_bytes(raw) != *digest {
                return Err(Error::Conflict(
                    "Claim profile dependency differs from selected cut",
                ));
            }
            source_command::set(&mut contracts, path, string(&digest.to_prefixed()))?;
        }
        // claim_field_catalog is a distinct existing reader predicate:
        // recognized statement language/script uses the corpus field-language
        // fragment. This extra read does not alter SourceClaimProfiles' owner
        // configuration digest or pretend to be source admission.
        if source
            .object_get("qualifiers")
            .and_then(|qualifiers| qualifiers.object_get("statement"))
            .and_then(JsonValue::as_str)
            .is_some_and(nonblank)
        {
            let contract =
                tos_foundation::RelativePath::parse("ToS/contracts/corpus-record.schema.json")
                    .map_err(|_| Error::Invalid("field language schema path"))?;
            let raw = ctx.file(&contract)?.ok_or(Error::Invalid(
                "Claim field language schema absent from command reads",
            ))?;
            let metadata = cut.current().member(&contract).ok_or(Error::Invalid(
                "Claim field language schema absent from cut",
            ))?;
            if Digest256::of_bytes(raw) != metadata.sha256 {
                return Err(Error::Conflict(
                    "Claim field language schema differs from cut",
                ));
            }
            let qualifiers = field(&source, "qualifiers")?;
            let declarations = object(vec![(
                "notes",
                object(vec![
                    (
                        "language",
                        qualifiers
                            .object_get("statement_language")
                            .cloned()
                            .unwrap_or(JsonValue::Null),
                    ),
                    (
                        "script",
                        qualifiers
                            .object_get("statement_script")
                            .cloned()
                            .unwrap_or(JsonValue::Null),
                    ),
                ]),
            )]);
            schema_check(
                worker,
                path,
                &canonical_bytes(&declarations)?,
                "ToS/contracts/corpus-record.schema.json#/properties/field_languages",
                deadline,
                cancelled,
            )?;
        }
        Some(contracts)
    } else if bound_contracts {
        let schema_path = tos_foundation::RelativePath::parse(schema)
            .map_err(|_| Error::Invalid("schema path"))?;
        let raw = ctx
            .file(&schema_path)?
            .ok_or(Error::Invalid("exact selected source schema missing"))?;
        Some(object(vec![(
            schema,
            string(&Digest256::of_bytes(raw).to_prefixed()),
        )]))
    } else {
        None
    };
    let configuration = record_digest(&if let Some(contracts) = &contracts {
        object(vec![
            ("configuration", config.clone()),
            ("source_contracts", contracts.clone()),
        ])
    } else {
        config.clone()
    })?
    .to_prefixed();
    let revision = old_raw
        .map(|raw| string(&Digest256::of_bytes(raw).to_prefixed()))
        .unwrap_or(JsonValue::Null);
    if text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(Error::Invalid("source command request schema"));
    }
    let operation = text(&request, "operation")?;
    if operation == "describe" {
        exact_keys(&request, &["schema_version", "operation"])?;
        return ctx.plan(
            if canonical {
                "canonical-node-forms"
            } else {
                "public-source-forms"
            },
            form_response(
                &source,
                old.as_ref(),
                &config,
                &subject,
                &target,
                &configuration,
                &revision,
                contracts.as_ref(),
                JsonValue::Null,
                false,
            )?,
            vec![],
            false,
        );
    }
    if operation == "prepare" {
        exact_keys(
            &request,
            &["schema_version", "operation", "form_id", "field_id"],
        )?;
        let id = text(&request, "form_id")?;
        if !array(&config, "allowed_form_ids")?
            .iter()
            .any(|value| value.as_str() == Some(id))
        {
            return Err(Error::Denied("prepared form outside scope"));
        }
        if claim_owner && !claim_form_field_ids(&config)?.contains(&text(&request, "field_id")?) {
            return Err(Error::Denied("Claim form field outside delegation"));
        }
        let change = prepare_form_change(
            &source,
            old.as_ref(),
            text(&config, "principal_id")?,
            id,
            text(&request, "field_id")?,
        )?;
        check_changes(&config, std::slice::from_ref(&change), canonical)?;
        if claim_owner {
            check_claim_form_changes(&config, std::slice::from_ref(&change), old.as_ref(), true)?;
        }
        let proposed = apply_form_changes(old.as_ref(), &subject, std::slice::from_ref(&change))?;
        schema_check(
            worker,
            target.as_str(),
            &published(&proposed)?,
            "ToS/contracts/human-form-set.schema.json",
            deadline,
            cancelled,
        )?;
        let views = materialize_source_forms(&source, &proposed)?;
        let preview = views
            .into_iter()
            .find(|view| {
                view.object_get("form")
                    .and_then(|f| f.object_get("id"))
                    .and_then(JsonValue::as_str)
                    == Some(id)
            })
            .ok_or(Error::Invalid("prepared materialization absent"))?;
        if text(&preview, "state")? != "ready" {
            return Err(Error::Invalid("prepared exact source-copy is not ready"));
        }
        let mut response = form_response(
            &source,
            old.as_ref(),
            &config,
            &subject,
            &target,
            &configuration,
            &revision,
            contracts.as_ref(),
            JsonValue::Null,
            false,
        )?;
        source_command::set(&mut response, "prepared_change", change)?;
        source_command::set(&mut response, "prepared_materialization", preview)?;
        return ctx.plan(
            if canonical {
                "canonical-node-forms"
            } else {
                "public-source-forms"
            },
            response,
            vec![],
            false,
        );
    }
    if operation != "apply" {
        return Err(Error::Invalid("unknown form command operation"));
    }
    exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "command_id",
            "expected_source",
            "expected_revision",
            "expected_configuration",
            "changes",
        ],
    )?;
    let command_id = text(&request, "command_id")?;
    if command_id.is_empty() || command_id.chars().count() > 256 {
        return Err(Error::Invalid("command ID length"));
    }
    let changes = array(&request, "changes")?;
    check_changes(&config, changes, canonical)?;
    if claim_owner {
        check_claim_form_changes(&config, changes, old.as_ref(), false)?;
    }
    let request_digest = Digest256::of_bytes(&canonical_bytes(&request)?).to_prefixed();
    if let Some(receipt) = old
        .as_ref()
        .and_then(|set| set.object_get("growth_history"))
        .and_then(JsonValue::as_array)
        .and_then(|history| {
            history.iter().find(|receipt| {
                receipt.object_get("command_id").and_then(JsonValue::as_str) == Some(command_id)
            })
        })
    {
        if text(receipt, "request_digest")? != request_digest {
            return Err(Error::Conflict("command identity reused"));
        }
        if claim_owner {
            let results = JsonValue::Array(
                changes
                    .iter()
                    .map(|change| form_reference(field(change, "form")?))
                    .collect::<Result<Vec<_>>>()?,
            );
            if !same(field(receipt, "results")?, &results)? {
                return Err(Error::Invalid(
                    "Claim form receipt differs from request results",
                ));
            }
        }
        return ctx.plan(
            if canonical {
                "canonical-node-forms"
            } else {
                "public-source-forms"
            },
            form_response(
                &source,
                old.as_ref(),
                &config,
                &subject,
                &target,
                &configuration,
                &revision,
                contracts.as_ref(),
                receipt.clone(),
                true,
            )?,
            vec![],
            true,
        );
    }
    if !same(field(&request, "expected_source")?, &subject)?
        || !same(field(&request, "expected_revision")?, &revision)?
        || text(&request, "expected_configuration")? != configuration
    {
        return Err(Error::Conflict(
            "expected source/configuration/revision stale",
        ));
    }
    if claim_owner {
        check_claim_form_changes(&config, changes, old.as_ref(), true)?;
    }
    let mut successor = apply_form_changes(old.as_ref(), &subject, changes)?;
    let views = materialize_source_forms(&source, &successor)?;
    for change in changes {
        let form = field(change, "form")?;
        if text(field(form, "content")?, "kind")? == "source-copy"
            && views
                .iter()
                .find(|view| {
                    view.object_get("form")
                        .and_then(|v| v.object_get("id"))
                        .and_then(JsonValue::as_str)
                        == form.object_get("form_id").and_then(JsonValue::as_str)
                })
                .is_none_or(|view| {
                    view.object_get("state").and_then(JsonValue::as_str) != Some("ready")
                })
        {
            return Err(Error::Invalid("source-copy does not satisfy source reader"));
        }
    }
    let mut receipt_entries = vec![
        ("command_id", string(command_id)),
        ("request_digest", string(&request_digest)),
        ("principal_id", field(&config, "principal_id")?.clone()),
        ("authority_ref", field(&config, "authority_ref")?.clone()),
        ("owner_configuration", string(&configuration)),
        ("recorded_at", string(&ctx.recorded_at)),
    ];
    if let Some(contracts) = &contracts {
        receipt_entries.push(("source_contracts", contracts.clone()));
    }
    receipt_entries.extend([
        ("source", subject.clone()),
        ("previous_revision", revision),
        (
            "results",
            JsonValue::Array(
                changes
                    .iter()
                    .map(|change| form_reference(field(change, "form")?))
                    .collect::<Result<Vec<_>>>()?,
            ),
        ),
    ]);
    let receipt = object(receipt_entries);
    let mut history = successor
        .object_get("growth_history")
        .and_then(JsonValue::as_array)
        .unwrap_or(&[])
        .to_vec();
    history.push(receipt.clone());
    source_command::set(&mut successor, "growth_history", JsonValue::Array(history))?;
    validate_history(&successor, &subject)?;
    let encoded = published(&successor)?;
    if encoded.len() > 2_097_152 {
        return Err(Error::Invalid("form set publication byte budget"));
    }
    schema_check(
        worker,
        target.as_str(),
        &encoded,
        "ToS/contracts/human-form-set.schema.json",
        deadline,
        cancelled,
    )?;
    let response = form_response(
        &source,
        Some(&successor),
        &config,
        &subject,
        &target,
        &configuration,
        &string(&Digest256::of_bytes(&encoded).to_prefixed()),
        contracts.as_ref(),
        receipt,
        false,
    )?;
    ctx.plan(
        if canonical {
            "canonical-node-forms"
        } else {
            "public-source-forms"
        },
        response,
        vec![SourceChange {
            path: target,
            before: old_raw.map(Digest256::of_bytes),
            after: Some(encoded),
        }],
        false,
    )
}
fn canonical_bytes(value: &JsonValue) -> Result<Vec<u8>> {
    source_command::canonical(value)
}
fn form_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.form.") else {
        return false;
    };
    rest.as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && rest.bytes().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, b'.' | b'_' | b'-')
        })
}
fn claim_id(value: &str) -> bool {
    value.strip_prefix("tos.claim.").is_some_and(|rest| {
        rest.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    })
}

fn select_form_claim(raw: &[u8], id: &str) -> Result<JsonValue> {
    if raw.len() > 1_048_576 {
        return Err(Error::Invalid("Claim form source stream byte budget"));
    }
    let mut selected = None;
    // Maintained _read returns bytes: bytes.splitlines recognizes CR/LF,
    // and bytes.strip has these six ASCII whitespace characters.
    for line in raw.split(|byte| matches!(*byte, b'\r' | b'\n')) {
        if line
            .iter()
            .all(|byte| matches!(*byte, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' '))
        {
            continue;
        }
        let claim = parse(line)?;
        if claim.as_object().is_none() {
            return Err(Error::Invalid("Claim stream row must be an object"));
        }
        if claim.object_get("claim_id").and_then(JsonValue::as_str) == Some(id) {
            if selected.replace(claim).is_some() {
                return Err(Error::Invalid("delegated Claim must resolve exactly once"));
            }
        }
    }
    selected.ok_or(Error::Invalid("delegated Claim must resolve exactly once"))
}

fn claim_form_field_ids(config: &JsonValue) -> Result<Vec<&str>> {
    if text(config, "schema_version")? == "tos_local_claim_form_owner_v1" {
        return Ok(vec!["claim.statement"]);
    }
    let values = array(config, "allowed_field_ids")?;
    if !(1..=4).contains(&values.len()) {
        return Err(Error::Invalid("Claim field scope size"));
    }
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .ok_or(Error::Invalid("Claim field scope text"))?;
            if !matches!(
                value,
                "claim.statement" | "claim.name" | "claim.caption" | "claim.hover"
            ) || !seen.insert(value)
            {
                return Err(Error::Invalid(
                    "Claim field scope must be explicit known unique selectors",
                ));
            }
            Ok(value)
        })
        .collect()
}

fn check_claim_form_changes(
    config: &JsonValue,
    changes: &[JsonValue],
    set: Option<&JsonValue>,
    current: bool,
) -> Result<()> {
    let allowed = claim_form_field_ids(config)?;
    let mut selected_ids = HashSet::new();
    let mut forms = Vec::new();
    let mut predecessors = Vec::new();
    for change in changes {
        let form = field(change, "form")?;
        let predecessor = field(change, "expected_form")?;
        if !same(predecessor, field(form, "revises")?)? {
            return Err(Error::Conflict(
                "Claim form request differs from bound predecessor",
            ));
        }
        let create = text(change, "operation")? == "form.create";
        if create != predecessor.is_null() || create && integer(form, "form_version")? != 1 {
            return Err(Error::Conflict(
                "Claim form operation differs from retained lineage",
            ));
        }
        selected_ids.insert(text(form, "form_id")?);
        forms.push(form);
        if !predecessor.is_null() {
            predecessors.push(predecessor);
        }
    }
    if let Some(set) = set {
        for (candidate, is_current) in array(set, "forms")?
            .iter()
            .map(|form| (form, true))
            .chain(array(set, "prior_forms")?.iter().map(|form| (form, false)))
        {
            let reference = form_reference(candidate)?;
            let mut retained = false;
            for predecessor in &predecessors {
                retained |= same(predecessor, &reference)?;
            }
            if retained
                || current && is_current && selected_ids.contains(text(candidate, "form_id")?)
            {
                forms.push(candidate);
            }
        }
    }
    for form in forms {
        let content = field(form, "content")?;
        if text(content, "kind")? != "source-copy" {
            if text(config, "schema_version")? == "tos_local_claim_form_owner_v2" {
                return Err(Error::Denied(
                    "Claim display delegation permits only source copies",
                ));
            }
            continue; // v1 retains unassessed non-copy proposals.
        }
        let bound = field(field(form, "bindings")?, text(content, "slot")?)?;
        let role = text(form, "role")?;
        let pointer = text(bound, "pointer")?;
        if !allowed.iter().any(|field| match *field {
            "claim.statement" => role == "statement" && pointer == "/qualifiers/statement",
            "claim.name" => role == "name" && pointer == "/qualifiers/display_fields/name/text",
            "claim.caption" => {
                role == "caption" && pointer == "/qualifiers/display_fields/caption/text"
            }
            "claim.hover" => role == "hover" && pointer == "/qualifiers/display_fields/hover/text",
            _ => false,
        }) {
            return Err(Error::Denied(
                "Claim source-copy field outside current delegation",
            ));
        }
    }
    Ok(())
}

fn check_changes(config: &JsonValue, changes: &[JsonValue], canonical: bool) -> Result<()> {
    if changes.is_empty() || changes.len() > 32 {
        return Err(Error::Invalid("form batch size"));
    }
    let mut ids = HashSet::new();
    for change in changes {
        exact_keys(change, &["operation", "expected_form", "form"])?;
        let form = field(change, "form")?;
        let id = text(form, "form_id")?;
        if !ids.insert(id) {
            return Err(Error::Invalid("duplicate batch form ID"));
        }
        if !array(config, "allowed_form_ids")?
            .iter()
            .any(|value| value.as_str() == Some(id))
            || !array(config, "allowed_operations")?.iter().any(|value| {
                value.as_str() == change.object_get("operation").and_then(JsonValue::as_str)
            })
            || text(form, "creator_id")? != text(config, "principal_id")?
        {
            return Err(Error::Denied("form change outside current delegation"));
        }
        if canonical && text(field(form, "content")?, "kind")? != "source-copy" {
            return Err(Error::Denied("canonical form must copy source"));
        }
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn form_response(
    source: &JsonValue,
    payload: Option<&JsonValue>,
    config: &JsonValue,
    subject: &JsonValue,
    target: &tos_foundation::RelativePath,
    configuration: &str,
    revision: &JsonValue,
    contracts: Option<&JsonValue>,
    receipt: JsonValue,
    replayed: bool,
) -> Result<JsonValue> {
    let forms = payload
        .map(|set| array(set, "forms"))
        .transpose()?
        .unwrap_or(&[]);
    let mut response = object(vec![
        (
            "schema_version",
            string("tos_local_source_command_result_v1"),
        ),
        ("authentication", string("local-unix-account")),
        ("owner_configuration", string(configuration)),
        ("source", subject.clone()),
        ("source_path", field(config, "source_path")?.clone()),
        ("target_path", string(target.as_str())),
        ("revision", revision.clone()),
        (
            "supported_operations",
            JsonValue::Array(vec![string("form.create"), string("form.revise")]),
        ),
        (
            "allowed_operations",
            field(config, "allowed_operations")?.clone(),
        ),
        (
            "command_operations",
            JsonValue::Array(vec![string("describe"), string("prepare"), string("apply")]),
        ),
        (
            "source_fields",
            JsonValue::Array(
                metadata_fields(source)?
                    .iter()
                    .map(FormField::public)
                    .collect(),
            ),
        ),
        (
            "allowed_form_ids",
            field(config, "allowed_form_ids")?.clone(),
        ),
        (
            "forms",
            JsonValue::Array(
                forms
                    .iter()
                    .map(form_reference)
                    .collect::<Result<Vec<_>>>()?,
            ),
        ),
        (
            "materializations",
            JsonValue::Array(
                payload
                    .map(|set| materialize_source_forms(source, set))
                    .transpose()?
                    .unwrap_or_default(),
            ),
        ),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    if matches!(
        text(config, "schema_version")?,
        "tos_local_claim_form_owner_v1" | "tos_local_claim_form_owner_v2"
    ) {
        source_command::set(
            &mut response,
            "allowed_field_ids",
            JsonValue::Array(
                claim_form_field_ids(config)?
                    .into_iter()
                    .map(string)
                    .collect(),
            ),
        )?;
    }
    if let Some(contracts) = contracts {
        source_command::set(&mut response, "source_contracts", contracts.clone())?;
    }
    Ok(response)
}

fn record_reference(id: &str, version: u64, body: &JsonValue) -> Result<JsonValue> {
    Ok(object(vec![
        ("id", string(id)),
        ("version", number(version)),
        ("digest", string(&record_digest(body)?.to_prefixed())),
    ]))
}
pub fn form_reference(form: &JsonValue) -> Result<JsonValue> {
    reference(form, "form_id", "form_version")
}
pub fn metadata_subject(source: &JsonValue) -> Result<JsonValue> {
    if source.object_get("claim_id").is_some() {
        return reference(source, "claim_id", "claim_version");
    }
    let id = match text(source, "schema_version")? {
        "tos_canonical_node_v1" => "node_id",
        "tos_scholarly_composite_witness_v1" => "composite_id",
        "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => "artifact_id",
        _ => "record_id",
    };
    reference(source, id, "record_version")
}

fn language(source: &JsonValue, key: &str) -> Result<(JsonValue, JsonValue)> {
    let declaration = source
        .object_get("field_languages")
        .and_then(|v| v.object_get(key));
    let language = declaration
        .and_then(|v| v.object_get("language"))
        .cloned()
        .unwrap_or(JsonValue::Null);
    let script = declaration
        .and_then(|v| v.object_get("script"))
        .cloned()
        .unwrap_or(JsonValue::Null);
    if !language.is_null() && language.as_str().is_none()
        || !script.is_null() && script.as_str().is_none()
    {
        return Err(Error::Invalid("field language declaration"));
    }
    Ok((language, script))
}

/// Literal selectors of the maintained metadata/canonical/Claim adapters.
/// Schemas, record kind/profile and path binding are checked by their callers.
pub fn metadata_fields(source: &JsonValue) -> Result<Vec<FormField>> {
    let mut fields = Vec::new();
    if source.object_get("claim_id").is_some() {
        let qualifiers = field(source, "qualifiers")?;
        let Some(statement) = qualifiers
            .object_get("statement")
            .and_then(JsonValue::as_str)
            .filter(|s| nonblank(s))
        else {
            return Ok(fields);
        };
        let _ = statement;
        fields.push(FormField {
            id: "claim.statement".into(),
            pointer: "/qualifiers/statement".into(),
            role: "statement".into(),
            language: qualifiers
                .object_get("statement_language")
                .cloned()
                .unwrap_or(JsonValue::Null),
            script: qualifiers
                .object_get("statement_script")
                .cloned()
                .unwrap_or(JsonValue::Null),
            context: vec!["".into()],
        });
        if let Some(display) = qualifiers.object_get("display_fields").filter(|v| {
            v.object_get("schema_version").and_then(JsonValue::as_str)
                == Some("tos_claim_display_fields_v1")
        }) {
            for role in ["name", "caption", "hover"] {
                if let Some(wording) = display.object_get(role) {
                    fields.push(FormField {
                        id: format!("claim.{role}"),
                        pointer: format!("/qualifiers/display_fields/{role}/text"),
                        role: role.into(),
                        language: field(wording, "language")?.clone(),
                        script: field(wording, "script")?.clone(),
                        context: vec!["".into()],
                    });
                }
            }
        }
        return Ok(fields);
    }
    let version = text(source, "schema_version")?;
    if matches!(
        version,
        "tos_artifact_source_witness_v1"
            | "tos_artifact_source_witness_v2"
            | "tos_scholarly_composite_witness_v1"
    ) {
        let composite = version == "tos_scholarly_composite_witness_v1";
        let name_context = vec![
            if composite {
                "/identity_status"
            } else {
                "/custody"
            }
            .into(),
            "/layer_separation".into(),
            "/authority".into(),
            "/rights_ref".into(),
        ];
        return Ok(vec![
            FormField {
                id: "metadata.preferred-name".into(),
                pointer: if composite {
                    "/preferred_label"
                } else {
                    "/custody/inventory_numbers/0"
                }
                .into(),
                role: "name".into(),
                language: JsonValue::Null,
                script: JsonValue::Null,
                context: name_context,
            },
            FormField {
                id: "metadata.source-note".into(),
                pointer: if composite {
                    "/editorial_object/description"
                } else {
                    "/path_identity/note"
                }
                .into(),
                role: "hover".into(),
                language: JsonValue::Null,
                script: JsonValue::Null,
                context: vec!["".into()],
            },
        ]);
    }
    let canonical = version == "tos_canonical_node_v1";
    let mut context = if canonical {
        vec!["".into()]
    } else {
        [
            "identity_status",
            "same_as_posture",
            "semantic_scope",
            "semantic_content",
            "form_identity",
            "native_text_binding",
        ]
        .iter()
        .filter(|key| source.object_get(key).is_some())
        .map(|key| format!("/{key}"))
        .collect::<Vec<_>>()
    };
    if version == "tos_source_link_v1" {
        context.extend(
            [
                "uri",
                "provider_label",
                "link_kind",
                "access_status",
                "observed_at",
                "observation_ref",
                "association_claim_refs",
                "provenance_event_ref",
            ]
            .iter()
            .map(|key| format!("/{key}")),
        );
    }
    if source.object_get("record_type").and_then(JsonValue::as_str) == Some("sign") {
        context.push("/promotion_basis".into());
    }
    if let Some(declarations) = source.object_get("field_languages") {
        for (key, _) in declarations
            .as_object()
            .ok_or(Error::Invalid("field language map"))?
        {
            let key = key
                .as_str()
                .ok_or(Error::Invalid("language declaration key"))?;
            if source
                .object_get(key)
                .and_then(JsonValue::as_str)
                .is_none_or(|s| !nonblank(s))
            {
                return Err(Error::Invalid("language declaration has no wording"));
            }
        }
    }
    for (key, id, role) in if canonical {
        vec![
            ("preferred_label", "canonical.preferred-name", "name"),
            ("distilled_thesis", "canonical.thesis", "statement"),
        ]
    } else {
        vec![
            ("preferred_label", "metadata.preferred-name", "name"),
            ("notes", "metadata.source-note", "hover"),
        ]
    } {
        if source
            .object_get(key)
            .and_then(JsonValue::as_str)
            .is_some_and(nonblank)
        {
            let (language, script) = language(source, key)?;
            let mut guards = context.clone();
            if !canonical
                && source
                    .object_get("field_languages")
                    .and_then(|v| v.object_get(key))
                    .is_some()
            {
                guards.push(format!("/field_languages/{key}"));
            }
            fields.push(FormField {
                id: id.into(),
                pointer: format!("/{key}"),
                role: role.into(),
                language,
                script,
                context: guards,
            });
        } else if canonical && key == "distilled_thesis" {
            return Err(Error::Invalid("canonical thesis missing"));
        }
    }
    if let Some(variants) = source.object_get("variant_labels") {
        for (index, variant) in variants
            .as_array()
            .ok_or(Error::Invalid("variant array"))?
            .iter()
            .enumerate()
        {
            if variant
                .object_get("value")
                .and_then(JsonValue::as_str)
                .is_none_or(|s| !nonblank(s))
            {
                if canonical {
                    return Err(Error::Invalid("canonical variant incomplete"));
                } else {
                    continue;
                }
            }
            let base = format!("/variant_labels/{index}/");
            let mut guards = context.clone();
            if !canonical {
                for (key, _) in variant
                    .as_object()
                    .ok_or(Error::Invalid("variant object"))?
                {
                    let key = key.as_str().ok_or(Error::Invalid("variant field"))?;
                    if key != "value" {
                        guards.push(format!("{base}{}", pointer_token(key)));
                    }
                }
            }
            fields.push(FormField {
                id: format!(
                    "{}.variant-name:{index}",
                    if canonical { "canonical" } else { "metadata" }
                ),
                pointer: format!("{base}value"),
                role: "name".into(),
                language: variant
                    .object_get("language")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
                script: variant
                    .object_get("script")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
                context: guards,
            });
        }
    }
    // Canonical catalogue orders thesis after variant labels, as the owner does.
    if canonical {
        if let Some(at) = fields.iter().position(|f| f.id == "canonical.thesis") {
            let thesis = fields.remove(at);
            fields.push(thesis);
        }
    }
    Ok(fields)
}

pub fn prepare_form_change(
    source: &JsonValue,
    set: Option<&JsonValue>,
    principal: &str,
    id: &str,
    field_id: &str,
) -> Result<JsonValue> {
    let fields = metadata_fields(source)?;
    let selected = fields
        .iter()
        .find(|field| field.id == field_id)
        .ok_or(Error::Invalid("unknown source field selector"))?;
    let subject = metadata_subject(source)?;
    let empty = empty_set(&subject);
    prepared_change(set.unwrap_or(&empty), &subject, principal, id, selected)
}
fn empty_set(subject: &JsonValue) -> JsonValue {
    object(vec![
        ("schema_version", string("tos_human_form_set_v1")),
        ("subject", subject.clone()),
        ("forms", JsonValue::Array(vec![])),
        ("prior_forms", JsonValue::Array(vec![])),
    ])
}

pub fn apply_form_changes(
    set: Option<&JsonValue>,
    subject: &JsonValue,
    changes: &[JsonValue],
) -> Result<JsonValue> {
    let mut successor = match set {
        Some(v) => parse(&canonical(v)?)?,
        None => empty_set(subject),
    };
    for change in changes {
        exact_keys(change, &["operation", "expected_form", "form"])?;
        let form = field(change, "form")?.clone();
        if !same(field(&form, "subject")?, subject)? {
            return Err(Error::Conflict("form subject changed"));
        }
        let id = text(&form, "form_id")?;
        let at = array(&successor, "forms")?
            .iter()
            .position(|old| old.object_get("form_id").and_then(JsonValue::as_str) == Some(id));
        match text(change, "operation")? {
            "form.create" => {
                if at.is_some()
                    || !field(change, "expected_form")?.is_null()
                    || integer(&form, "form_version")? != 1
                    || !field(&form, "revises")?.is_null()
                {
                    return Err(Error::Conflict("create requires absent initial form"));
                }
                array_mut(&mut successor, "forms")?.push(form);
            }
            "form.revise" => {
                let at = at.ok_or(Error::Conflict("form predecessor absent"))?;
                let old = &array(&successor, "forms")?[at];
                let previous = form_reference(old)?;
                if !same(field(change, "expected_form")?, &previous)?
                    || !same(field(&form, "revises")?, &previous)?
                    || integer(&form, "form_version")?
                        != integer(old, "form_version")?
                            .checked_add(1)
                            .ok_or(Error::Invalid("version overflow"))?
                {
                    return Err(Error::Conflict("revision must extend exact form"));
                }
                let old = std::mem::replace(&mut array_mut(&mut successor, "forms")?[at], form);
                array_mut(&mut successor, "prior_forms")?.push(old);
            }
            _ => return Err(Error::Invalid("unknown form operation")),
        }
    }
    source_command::set(&mut successor, "subject", subject.clone())?;
    validate_history(&successor, subject)?;
    Ok(successor)
}

fn stopped(form: &JsonValue, subject: &JsonValue, state: &str, issue: &str) -> Result<JsonValue> {
    Ok(object(vec![
        (
            "schema_version",
            string("tos_human_form_materialization_v1"),
        ),
        ("form", form_reference(form)?),
        ("subject", subject.clone()),
        ("state", string(state)),
        ("display_text", JsonValue::Null),
        ("context", JsonValue::Array(vec![])),
        ("issues", JsonValue::Array(vec![string(issue)])),
        ("admission", JsonValue::Null),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]))
}
pub fn materialize_source_forms(source: &JsonValue, set: &JsonValue) -> Result<Vec<JsonValue>> {
    let subject = metadata_subject(source)?;
    validate_history(set, &subject)?;
    if source
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        == Some("tos_canonical_node_v1")
        && array(set, "forms")?
            .iter()
            .chain(array(set, "prior_forms")?)
            .any(|form| {
                form.object_get("content")
                    .and_then(|content| content.object_get("kind"))
                    .and_then(JsonValue::as_str)
                    != Some("source-copy")
            })
    {
        return Err(Error::Denied(
            "canonical current and retained forms permit only source-copy",
        ));
    }
    let fields = metadata_fields(source)?;
    let mut output = Vec::new();
    let mut bytes = 0usize;
    for form in array(set, "forms")? {
        let mut view = materialize_one(source, &subject, set, form, &fields)?;
        if source.object_get("claim_id").is_some()
            && !matches!(
                source.object_get("visibility").and_then(JsonValue::as_str),
                Some("public" | "public_metadata_only")
            )
        {
            view = stopped(form, &subject, "restricted", "scope.access-denied")?;
        }
        bytes = bytes
            .checked_add(canonical(&view)?.len())
            .filter(|size| *size <= 262_144)
            .ok_or(Error::Invalid("form materialization output budget"))?;
        output.push(view);
    }
    Ok(output)
}
fn materialize_one(
    source: &JsonValue,
    subject: &JsonValue,
    set: &JsonValue,
    form: &JsonValue,
    fields: &[FormField],
) -> Result<JsonValue> {
    if !same(field(set, "subject")?, subject)? || !same(field(form, "subject")?, subject)? {
        return stopped(form, subject, "stale", "metadata-adapter.subject-changed");
    }
    let content = field(form, "content")?;
    if text(content, "kind")? != "source-copy" {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        );
    }
    let bindings = field(form, "bindings")?;
    let slot = text(content, "slot")?;
    let Some(selected) = bindings.object_get(slot) else {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        );
    };
    if !same(field(selected, "record")?, subject)? {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        );
    }
    let matches = fields
        .iter()
        .filter(|candidate| {
            candidate.role == text(form, "role").unwrap_or("")
                && candidate.pointer == text(selected, "pointer").unwrap_or("")
        })
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(Error::Invalid("ambiguous source field catalogue"));
    }
    let Some(chosen) = matches.first() else {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        );
    };
    if !same(field(form, "language")?, &chosen.language)?
        || !same(field(form, "script")?, &chosen.script)?
    {
        return stopped(
            form,
            subject,
            "invalid",
            "source-copy.language-not-bound-to-source",
        );
    }
    if form
        .object_get("language_context")
        .is_some_and(|v| !v.is_null())
    {
        return stopped(
            form,
            subject,
            "invalid",
            "language-context.outside-owner-scope",
        );
    }
    // Resolve every authored binding, including extra slots. A valid wording
    // slot cannot hide a stale or unavailable dependency in another binding.
    for (binding_slot, bound) in bindings
        .as_object()
        .ok_or(Error::Invalid("form binding map"))?
    {
        let binding_slot = binding_slot
            .as_str()
            .ok_or(Error::Invalid("form binding slot"))?;
        if text(field(bound, "record")?, "id")? != text(subject, "id")? {
            return stopped(
                form,
                subject,
                "unavailable",
                &format!("binding.unavailable:{binding_slot}"),
            );
        }
        if !same(field(bound, "record")?, subject)? {
            return stopped(
                form,
                subject,
                "stale",
                &format!("binding.changed:{binding_slot}"),
            );
        }
        if pointer(source, text(bound, "pointer")?).is_err() {
            return stopped(
                form,
                subject,
                "invalid",
                &format!("binding.pointer:{binding_slot}"),
            );
        }
    }
    let mut context = Vec::new();
    for path in &chosen.context {
        let expected = binding(subject, path);
        let matching = bindings
            .as_object()
            .ok_or(Error::Invalid("form binding map"))?
            .iter()
            .find(|(_, value)| same(value, &expected).unwrap_or(false));
        let Some((key, _)) = matching else {
            return stopped(form, subject, "invalid", "context.omitted");
        };
        context.push(object(vec![
            (
                "slot",
                string(key.as_str().ok_or(Error::Invalid("form binding slot"))?),
            ),
            ("binding", expected),
            ("value", pointer(source, path)?),
        ]));
    }
    let wording = pointer(source, &chosen.pointer)?;
    if wording.as_str().is_none_or(|s| !nonblank(s)) {
        return stopped(
            form,
            subject,
            "invalid",
            "source-copy.requires-complete-nonempty-string",
        );
    }
    let mut view = object(vec![
        (
            "schema_version",
            string("tos_human_form_materialization_v1"),
        ),
        ("form", form_reference(form)?),
        ("subject", subject.clone()),
        ("state", string("ready")),
        ("display_text", wording),
        ("context", JsonValue::Array(context)),
        ("issues", JsonValue::Array(vec![])),
        ("admission", JsonValue::Null),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
        ("role", string(&chosen.role)),
        ("language", chosen.language.clone()),
        ("script", chosen.script.clone()),
        ("derivation", string("source-copy")),
        ("dependencies", JsonValue::Array(vec![subject.clone()])),
        ("standalone_reading", JsonValue::Bool(false)),
    ]);
    if canonical(&view)?.len() > 65_536 {
        view = stopped(
            form,
            subject,
            "over-budget",
            "form.output-budget-exceeded-do-not-truncate",
        )?;
    }
    Ok(view)
}
#[derive(Clone, Debug)]
pub struct FormField {
    pub id: String,
    pub pointer: String,
    pub role: String,
    pub language: JsonValue,
    pub script: JsonValue,
    pub context: Vec<String>,
}

impl FormField {
    fn public(&self) -> JsonValue {
        object(vec![
            ("field_id", string(&self.id)),
            ("role", string(&self.role)),
            ("language", self.language.clone()),
            ("script", self.script.clone()),
        ])
    }
}

fn pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn pointer(source: &JsonValue, path: &str) -> Result<JsonValue> {
    if path.is_empty() {
        return Ok(source.clone());
    }
    let mut value = source;
    for token in path
        .strip_prefix('/')
        .ok_or(Error::Invalid("invalid source pointer"))?
        .split('/')
    {
        let token = token.replace("~1", "/").replace("~0", "~");
        value = match value {
            JsonValue::Object(_) => value.object_get(&token),
            JsonValue::Array(items) => token.parse::<usize>().ok().and_then(|at| items.get(at)),
            _ => None,
        }
        .ok_or(Error::Invalid("source pointer does not resolve"))?;
    }
    Ok(value.clone())
}

fn binding(subject: &JsonValue, path: &str) -> JsonValue {
    object(vec![("record", subject.clone()), ("pointer", string(path))])
}

fn prepared_change(
    set: &JsonValue,
    subject: &JsonValue,
    principal: &str,
    id: &str,
    field: &FormField,
) -> Result<JsonValue> {
    let old = array(set, "forms")?
        .iter()
        .find(|form| form.object_get("form_id").and_then(JsonValue::as_str) == Some(id));
    let operation = if old.is_some() {
        "form.revise"
    } else {
        "form.create"
    };
    let predecessor = old
        .map(form_reference)
        .transpose()?
        .unwrap_or(JsonValue::Null);
    let mut bindings = vec![(
        JsonString::from_utf8("wording"),
        binding(subject, &field.pointer),
    )];
    for (index, path) in field.context.iter().enumerate() {
        bindings.push((
            JsonString::from_utf8(&format!("context-{index}")),
            binding(subject, path),
        ));
    }
    let form = object(vec![
        ("schema_version", string("tos_human_form_v1")),
        ("form_id", string(id)),
        (
            "form_version",
            number(
                old.map(|form| integer(form, "form_version"))
                    .transpose()?
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(Error::Invalid("form version overflow"))?,
            ),
        ),
        ("subject", subject.clone()),
        ("role", string(&field.role)),
        ("language", field.language.clone()),
        ("script", field.script.clone()),
        ("creator_id", string(principal)),
        ("revises", predecessor.clone()),
        ("bindings", JsonValue::Object(bindings)),
        (
            "content",
            object(vec![
                ("kind", string("source-copy")),
                ("slot", string("wording")),
            ]),
        ),
    ]);
    Ok(object(vec![
        ("operation", string(operation)),
        ("expected_form", predecessor),
        ("form", form),
    ]))
}

fn validate_history(set: &JsonValue, subject: &JsonValue) -> Result<()> {
    if text(set, "schema_version")? != "tos_human_form_set_v1" {
        return Err(Error::Unsupported("other HumanForm set schema"));
    }
    if text(field(set, "subject")?, "id")? != text(subject, "id")? {
        return Err(Error::Conflict("source and HumanForm subject differ"));
    }
    let current = array(set, "forms")?;
    let prior = array(set, "prior_forms")?;
    if current.len() > 32 || prior.len() > 256 {
        return Err(Error::Unsupported(
            "source form history exceeds command budget",
        ));
    }
    let mut indexed: HashMap<(String, u64), &JsonValue> = HashMap::new();
    let mut current_ids = HashSet::new();
    for form in prior.iter().chain(current.iter()) {
        let id = text(form, "form_id")?.to_owned();
        let version = integer(form, "form_version")?;
        if version == 0
            || text(field(form, "subject")?, "id")? != text(subject, "id")?
            || indexed.insert((id, version), form).is_some()
        {
            return Err(Error::Unsupported("broken source form identity history"));
        }
    }
    for form in current {
        if !current_ids.insert(text(form, "form_id")?.to_owned()) {
            return Err(Error::Unsupported("duplicate current source form identity"));
        }
    }
    for ((id, version), form) in &indexed {
        if !current_ids.contains(id) {
            return Err(Error::Unsupported(
                "prior source form has no current identity",
            ));
        }
        let predecessor = field(form, "revises")?;
        if *version == 1 {
            if !predecessor.is_null() {
                return Err(Error::Unsupported("initial source form has predecessor"));
            }
        } else {
            let Some(previous) = indexed.get(&(id.clone(), version - 1)) else {
                return Err(Error::Unsupported("missing source form predecessor"));
            };
            if !same(predecessor, &form_reference(previous)?)? {
                return Err(Error::Unsupported("wrong source form predecessor"));
            }
        }
    }
    for form in current {
        let id = text(form, "form_id")?;
        let version = integer(form, "form_version")?;
        if indexed
            .keys()
            .any(|(other, next)| other == id && *next > version)
        {
            return Err(Error::Unsupported("current source form is not latest"));
        }
    }
    let mut command_ids = HashSet::new();
    if let Some(history) = set.object_get("growth_history") {
        for receipt in history
            .as_array()
            .ok_or(Error::Invalid("growth history must be array"))?
        {
            validate_instant(text(receipt, "recorded_at")?)?;
            if !command_ids.insert(text(receipt, "command_id")?.to_owned())
                || text(field(receipt, "source")?, "id")? != text(subject, "id")?
            {
                return Err(Error::Unsupported("duplicate or foreign source receipt"));
            }
            for reference in array(receipt, "results")? {
                let key = (
                    text(reference, "id")?.to_owned(),
                    integer(reference, "version")?,
                );
                let Some(form) = indexed.get(&key) else {
                    return Err(Error::Unsupported("receipt result not retained"));
                };
                if !same(reference, &form_reference(form)?)? {
                    return Err(Error::Unsupported("receipt result digest differs"));
                }
            }
        }
    }
    Ok(())
}

fn array_mut<'a>(root: &'a mut JsonValue, name: &str) -> Result<&'a mut Vec<JsonValue>> {
    let entries = match root {
        JsonValue::Object(entries) => entries,
        _ => return Err(Error::Invalid("form set is not an object")),
    };
    entries
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .and_then(|(_, value)| match value {
            JsonValue::Array(items) => Some(items),
            _ => None,
        })
        .ok_or(Error::Invalid("missing form-set array"))
}
