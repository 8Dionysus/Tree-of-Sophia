//! Read-only reconstruction of maintained native bibliographic publications.
//! Historical transport evidence never grants a current writer or admission.
use crate::PredicateRead;
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_biblio_cut::{
    SourceCutInput, SourceCutInputCoverage, SourceCutInputWithIdentity, SourceCutMemberMeta,
    account, check, current, reserve,
};
use crate::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutExecutionBinding, CutPreparedSchemaExecutionBinding,
    CutSchemaExecutor, CutSchemaReceiptRange, CutWorkerSchemaExecutor,
};
use crate::source_foundation_records::SourceFoundationRecordsStreamedReport;
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonValue,
    RelativePath, SourceRevision, canonical_bytes_v1, canonical_count_v1, emit_json_profile,
    parse_json,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};

const HOME: &str = "ToS/source-witnesses";
const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
const TRANSACTIONS: &str = "ToS/source-witnesses/.metadata-transactions";
const HISTORY: &str = "source-revision-history.json";
const PROTOCOL: &str = "tos_selected_source_metadata_v1";
const MAX_GENERATION: u64 = 9_007_199_254_740_991;
const MAX_SIDE: usize = 8 * 1024 * 1024;
const MAX_FILE: usize = 2 * 1024 * 1024;
const MAX_MANIFEST: usize = 512 * 1024;
const MAX_HISTORY: usize = 128;
const WORK_GRAMMAR_EXTRA: [&str; 5] = [
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];
const ITEM_GRAMMAR_EXTRA: [&str; 9] = [
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/source-resource-inventory.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/provenance-event.schema.json",
];
fn preparation_grammar_extra(kind: CompoundKind) -> Result<&'static [&'static str], ItemRefusal> {
    match kind {
        // These metadata families require the same five exact contract files;
        // their scope, Claim predicate and output recipes remain kind-specific.
        CompoundKind::WorkExpression
        | CompoundKind::ExpressionEdition
        | CompoundKind::ExpressionResponsibility
        | CompoundKind::CollectionWork => Ok(&WORK_GRAMMAR_EXTRA),
        CompoundKind::EditionItem => Ok(&ITEM_GRAMMAR_EXTRA),
        _ => Err(bad("no native preparation grammar for compound family")),
    }
}
const OBJECT_LINK_RECEIPT: &str = "object-link-creation-receipt.json";
const OBJECT_LINK_CLAIM: &str = "tos_object_link_claim_v2";
const OBJECT_LINK_OPERATION: &str = "object.link.create";
const OBJECT_LINK_MODULE: &str =
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_link_commands.py";
const OBJECT_LINK_SCOPE: [&str; 14] = [
    "subject_id",
    "subject_source_path",
    "subject_record_type",
    "link_id",
    "link_source_path",
    "claim_id",
    "claim_source_path",
    "predicate",
    "provenance_event_id",
    "allowed_link_form_ids",
    "allowed_claim_form_ids",
    "allowed_evidence_refs",
    "uri",
    "observation_ref",
];
fn object_link_predicate(predicate: &str) -> bool {
    matches!(
        predicate,
        "described_by" | "metadata_at" | "downloadable_at" | "rights_statement_at"
    )
}
pub fn validate_object_link_scope(scope: &Value) -> Result<(), ItemRefusal> {
    keys(scope, &OBJECT_LINK_SCOPE)?;
    let kind = text(scope, "subject_record_type")?;
    if !matches!(
        kind,
        "work" | "expression" | "edition" | "collection" | "item" | "artifact"
    ) || !object_link_predicate(text(scope, "predicate")?)
    {
        return Err(bad("object-Link delegated type/predicate"));
    }
    for (field, kind) in [
        ("subject_id", kind),
        ("link_id", "link"),
        ("claim_id", "claim"),
        ("provenance_event_id", "event"),
    ] {
        if !typed_id(text(scope, field)?, kind) {
            return Err(bad("object-Link typed identity"));
        }
    }
    let subject = text(scope, "subject_source_path")?;
    let link = text(scope, "link_source_path")?;
    let claim = text(scope, "claim_source_path")?;
    for path in [subject, link, claim] {
        metadata_path(path, false)?;
    }
    if kind == "artifact" && !subject.starts_with("ToS/source-witnesses/artifacts/") {
        return Err(bad("object-Link artifact owner path"));
    }
    if !subject.ends_with(if kind == "artifact" {
        "/artifact-witness.json"
    } else if kind == "work" {
        "/work.json"
    } else if kind == "item" {
        "/item.json"
    } else if kind == "edition" {
        "/edition.json"
    } else if kind == "expression" {
        "/expression.json"
    } else {
        "/collection.json"
    }) || !link.starts_with("ToS/source-witnesses/links/")
        || link.split('/').count() != 5
        || !link.ends_with("/link.json")
        || !claim.starts_with("ToS/source-witnesses/relations/")
        || claim.split('/').count() != 5
        || !claim.ends_with("/source-claims.jsonl")
        || parent(link)? == parent(claim)?
    {
        return Err(bad("object-Link separate exact homes"));
    }
    let mut forms = BTreeSet::new();
    for field in ["allowed_link_form_ids", "allowed_claim_form_ids"] {
        let ids = array(scope, field)?;
        if !(1..=32).contains(&ids.len()) {
            return Err(bad("object-Link form grants"));
        }
        for value in ids {
            let id = value
                .as_str()
                .ok_or_else(|| bad("object-Link form grant"))?;
            let tail = id.strip_prefix("tos.form.").unwrap_or("");
            if tail.is_empty()
                || !tail
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !tail.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
                })
                || !forms.insert(id)
            {
                return Err(bad("object-Link disjoint form grants"));
            }
        }
    }
    let evidence = array(scope, "allowed_evidence_refs")?;
    if !(1..=128).contains(&evidence.len())
        || evidence.iter().any(|v| {
            v.as_str()
                .is_none_or(|s| s.trim().is_empty() || s.chars().count() > 4096)
        })
        || evidence
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            != evidence.len()
        || !evidence.contains(&scope["observation_ref"])
    {
        return Err(bad("object-Link evidence grant"));
    }
    for value in evidence {
        let _ = value
            .as_str()
            .ok_or_else(|| bad("object-Link evidence reference"))?;
    }
    object_link_address(text(scope, "uri")?)?;
    Ok(())
}
/// Recheck a renewed exact scope against the retained request and original maker.
pub fn validate_object_link_recovery_scope(
    scope: &Value,
    request: &Value,
    original_authorization: &Value,
) -> Result<(), ItemRefusal> {
    object_link_scope(scope, request, original_authorization)
}
fn object_link_scope(scope: &Value, request: &Value, authority: &Value) -> Result<(), ItemRefusal> {
    validate_object_link_scope(scope)?;
    let kind = text(scope, "subject_record_type")?;
    let evidence = array(scope, "allowed_evidence_refs")?;
    let subject_record = &request["subject"];
    let link_record = &request["link"];
    let claim_record = &request["claim"];
    let subject_id = if kind == "artifact" {
        text(subject_record, "artifact_id")?
    } else {
        text(subject_record, "record_id")?
    };
    if subject_id != text(scope, "subject_id")?
        || text(link_record, "record_id")? != text(scope, "link_id")?
        || kind != "artifact" && subject_record["record_type"] != kind
        || link_record["record_type"] != "link"
        || link_record["uri"] != scope["uri"]
        || link_record["observation_ref"] != scope["observation_ref"]
        || link_record["association_claim_refs"] != json!([scope["claim_id"]])
        || link_record["provenance_event_ref"] != scope["provenance_event_id"]
        || integer(link_record, "record_version")? != 1
        || !link_record["supersedes_ref"].is_null()
        || link_record["identity_status"] != "provisional"
        || !matches!(
            text(link_record, "same_as_posture")?,
            "not_assessed" | "no_equivalence_claim"
        )
        || !array(link_record, "external_identifiers")?.is_empty()
        || !array(link_record, "variant_labels")?.is_empty()
        || text(claim_record, "claim_id")? != text(scope, "claim_id")?
        || integer(claim_record, "claim_version")? != 1
        || claim_record["subject_ref"] != scope["subject_id"]
        || claim_record["object"] != scope["link_id"]
        || claim_record["predicate"] != scope["predicate"]
        || claim_record["provenance_event_ref"] != scope["provenance_event_id"]
        || claim_record["maker"]
            != json!({"maker_type":authority["maker_type"],"agent_ref":authority["principal_id"]})
        || claim_record
            .get("assessment_refs")
            .is_some_and(|value| value.as_array().is_none_or(|refs| !refs.is_empty()))
        || !claim_record["supersedes_claim_ref"].is_null()
        || claim_record["schema_version"] != OBJECT_LINK_CLAIM
    {
        return Err(bad("object-Link exact initial scope"));
    }
    let q = &claim_record["qualifiers"];
    if q["availability_is_rights_conclusion"] != false
        || [
            "statement",
            "statement_language",
            "statement_script",
            "link_role",
        ]
        .iter()
        .any(|k| text(q, k).map_or(true, |s| s.trim().is_empty()))
    {
        return Err(bad("object-Link qualified no-rights statement"));
    }
    for values in [
        array(link_record, "source_refs")?,
        array(claim_record, "evidence_refs")?,
    ] {
        if values.iter().any(|v| !evidence.contains(v)) {
            return Err(bad("object-Link evidence outside grant"));
        }
        for value in values {
            let reference = value.as_str().ok_or_else(|| bad("object-Link evidence"))?;
            if !reference.starts_with("ToS/") {
                object_link_address(reference)?;
            } else {
                RelativePath::parse(reference).map_err(|_| bad("object-Link evidence locator"))?;
            }
        }
    }
    if claim_record.get("counterevidence_refs").is_some() {
        for value in array(claim_record, "counterevidence_refs")? {
            if !evidence.contains(value) {
                return Err(bad("object-Link counterevidence outside grant"));
            }
            let reference = value
                .as_str()
                .ok_or_else(|| bad("object-Link counterevidence"))?;
            if !reference.starts_with("ToS/") {
                object_link_address(reference)?;
            } else {
                RelativePath::parse(reference)
                    .map_err(|_| bad("object-Link counterevidence locator"))?;
            }
        }
    }
    let observation = text(scope, "observation_ref")?;
    if !observation.starts_with("ToS/") {
        object_link_address(observation)?;
    } else {
        RelativePath::parse(observation).map_err(|_| bad("object-Link observation locator"))?;
    }
    for (selection, grant) in [
        ("forms", "allowed_link_form_ids"),
        ("claim_forms", "allowed_claim_form_ids"),
    ] {
        let rows = array(request, selection)?;
        if !(1..=32).contains(&rows.len()) {
            return Err(bad("object-Link bounded form selections"));
        }
        let mut ids = BTreeSet::new();
        for row in rows {
            keys(row, &["form_id", "field_id"])?;
            let id = text(row, "form_id")?;
            if !array(scope, grant)?.contains(&json!(id)) || !ids.insert(id) {
                return Err(bad("object-Link form selection grant"));
            }
        }
    }
    Ok(())
}
/// Validate the maintained ObjectLink external address grammar without fetching.
pub fn validate_object_link_address(uri: &str) -> Result<(), ItemRefusal> {
    object_link_address(uri)
}
fn object_link_address(uri: &str) -> Result<(), ItemRefusal> {
    use unicode_normalization::UnicodeNormalization;
    if !(1..=4096).contains(&uri.chars().count())
        || uri.chars().any(|c| {
            c.is_whitespace()
                || unicode_general_category::get_general_category(c)
                    .abbreviation()
                    .starts_with('C')
                || c == '\\'
        })
    {
        return Err(bad("object-Link unsafe external address"));
    }
    let (scheme, rest) = uri
        .split_once("://")
        .ok_or_else(|| bad("object-Link HTTP(S) address"))?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(bad("object-Link HTTP(S) address"));
    }
    let authority = rest.split(&['/', '?', '#'][..]).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err(bad("object-Link credential-free host"));
    }
    // urllib.parse rejects a netloc whose NFKC form creates a delimiter.
    // The source string itself is never normalized or used for a network read.
    if !authority.is_ascii() {
        let without_delimiters: String = authority
            .chars()
            .filter(|c| !matches!(c, '@' | ':' | '#' | '?'))
            .collect();
        let normalized: String = without_delimiters.nfkc().collect();
        if normalized != without_delimiters
            && normalized
                .chars()
                .any(|c| matches!(c, '/' | '?' | '#' | '@' | ':'))
        {
            return Err(bad("object-Link NFKC netloc delimiter"));
        }
    }
    // Match urllib.parse.urlsplit's netloc/hostname/port boundary. In
    // particular an empty port is allowed, while any non-decimal port or a
    // nonempty suffix after ']' without ':' is not. No address is fetched.
    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| bad("object-Link bracketed host"))?;
        let inside = &authority[1..end];
        if inside.starts_with('v') || inside.starts_with('V') {
            let (version, address) = inside[1..]
                .split_once('.')
                .ok_or_else(|| bad("object-Link IPvFuture host"))?;
            if version.is_empty()
                || !version.bytes().all(|b| b.is_ascii_hexdigit())
                || address.is_empty()
            {
                return Err(bad("object-Link IPvFuture host"));
            }
        } else {
            let address = if let Some((base, zone)) = inside.split_once('%') {
                if zone.is_empty() || zone.contains('%') {
                    return Err(bad("object-Link IPv6 zone"));
                }
                base
            } else {
                inside
            };
            address
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| bad("object-Link IPv6 host"))?;
        }
        let suffix = &authority[end + 1..];
        let port = if suffix.is_empty() {
            ""
        } else {
            suffix
                .strip_prefix(':')
                .ok_or_else(|| bad("object-Link bracketed host suffix"))?
        };
        (inside, port)
    } else {
        if authority.contains('[') || authority.contains(']') {
            return Err(bad("object-Link malformed bracketed host"));
        }
        authority
            .split_once(':')
            .map_or((authority, ""), |(h, p)| (h, p))
    };
    let significant_port = port.trim_start_matches('0');
    if host.is_empty()
        || (!port.is_empty()
            && (!port.bytes().all(|b| b.is_ascii_digit())
                || significant_port.len() > 5
                || (!significant_port.is_empty() && significant_port.parse::<u16>().is_err())))
    {
        return Err(bad("object-Link address port/host"));
    }
    Ok(())
}
pub(crate) type Package = BTreeMap<String, Vec<u8>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTransportState {
    Committed,
    RolledBack,
    Pending,
    Orphan,
}

#[derive(Debug)]
pub struct NativeCompoundObservation {
    pub claim_path: String,
    pub claim_id: String,
    pub transaction_id: String,
    pub manifest_sha256: String,
    pub transport: NativeTransportState,
    /// Exact reconstructed parent receipt digest after full Work verification.
    /// None for other families and incomplete transport states.
    pub work_parent_transition_sha256: Option<String>,
}

/// Read the exact selected Work compound and its retained/current lineage.
/// The transport observation is descriptive; CMD still owns the physical
/// publication, source/software and journal fences for any replay decision.
pub fn verify_work_expression_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundObservation, ItemRefusal> {
    if claim["predicate"] != "has_expression"
        || schemas.source_revision() != cut.current().revision()
    {
        return Err(bad("selected Work compound type/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    reader.verify(claim_path, claim, schemas)
}

fn strip_jsonl_line_ending(raw: &[u8]) -> &[u8] {
    raw.strip_suffix(b"\n").unwrap_or(raw)
}

/// Verify one exact candidate Claim against its current source carrier and the
/// same retained native transaction/history kernel used by cut-backed calls.
/// The caller supplies the complete source coverage produced by the Records
/// pass; this function verifies that fence on both sides of the replay.
#[allow(clippy::too_many_arguments)]
pub fn verify_native_compound_from_input<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    coverage: &SourceCutInputCoverage,
    records: &SourceFoundationRecordsStreamedReport<'_, I>,
    schemas: &mut CandidateCutWorkerSchemaExecutor<I>,
    claim_path: &str,
    selected_claim_raw: &[u8],
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<CandidateNativeCompoundReadObservation<I>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if input.input_identity() != records.input_identity()
        || input.input_identity() != schemas.input_identity()
        || coverage.membership() != *records.source_membership()
        || coverage.source_bytes_read() != records.cost().selected_current_member_bytes
    {
        return Err(bad("candidate native Claim input/report identity"));
    }
    metadata_path(claim_path, false)?;
    if !claim_path.ends_with("/source-claims.jsonl") {
        return Err(bad("candidate native Claim carrier"));
    }
    let candidate_schema = records.candidate_schema_identity().ok_or_else(|| {
        ItemRefusal::Unsupported("candidate native replay requires streamed schema identity".into())
    })?;
    if candidate_schema.profile() != schemas.profile()
        || candidate_schema.schema_set_digest() != schemas.schema_set_digest()
        || candidate_schema.contract_selection_digest() != schemas.contract_selection_digest()
        || candidate_schema.prepared_execution_binding() != schemas.prepared_execution_binding()
        || candidate_schema.selected_resource_count()
            != u64::try_from(schemas.source_resource_count()).map_err(|_| ItemRefusal::Budget)?
        || candidate_schema.selected_resource_bytes()
            != u64::try_from(schemas.schema_bytes()).map_err(|_| ItemRefusal::Budget)?
    {
        return Err(bad("candidate native Claim schema/report binding"));
    }
    input.verify_current_fence(coverage, limits.deadline, cancelled)?;
    let selected_body = strip_jsonl_line_ending(selected_claim_raw);
    if selected_body.is_empty() || selected_body.len() > limits.max_member_bytes.min(MAX_FILE) {
        return Err(ItemRefusal::BudgetCheck {
            check: "candidate native selected Claim row bytes",
            used: Some(selected_body.len() as u64),
            limit: Some(limits.max_member_bytes.min(MAX_FILE) as u64),
        });
    }
    let mut reader = NativeCompoundReader::new_from_input(input.source_input(), limits, cancelled)?;
    let carrier_limit = limits.max_member_bytes.min(MAX_SIDE);
    let claims_raw = reader.required(claim_path, carrier_limit)?;
    let mut selected_line = None;
    let mut exact_matches = 0usize;
    for line in claims_raw.split(|byte| *byte == b'\n') {
        check(limits.deadline, cancelled)?;
        if line == selected_body {
            exact_matches = exact_matches.checked_add(1).ok_or(ItemRefusal::Budget)?;
            if exact_matches == 1 {
                selected_line = Some(line);
            }
        }
    }
    if exact_matches != 1 {
        return Err(bad(
            "candidate native selected Claim row is not unique/current",
        ));
    }
    let selected_line = selected_line.ok_or_else(|| bad("candidate native selected Claim row"))?;
    let claim = reader.decoded(selected_line)?;
    let claim_state = crate::record_biblio_cut::decoded_state(&claim)?;
    if text(&claim, "claim_id")?.is_empty() || !claim.is_object() {
        return Err(bad("candidate native selected Claim object"));
    }
    let mut diagnostics_cost = crate::record_rules::CandidateLocalClaimDiagnosticsCost::default();
    let input_identity = *input.input_identity();
    let current_membership = *records.source_membership();
    let prepared_execution_binding = schemas.prepared_execution_binding();
    let schema_set_sha256 = schemas.schema_set_digest();
    let contract_selection_sha256 = schemas.contract_selection_digest();
    let mut validate_local_claim = |raw: &[u8],
                                    schemas: &mut CandidateCutWorkerSchemaExecutor<I>,
                                    local_limits: ItemLimits| {
        let (decoded, _) = crate::record_biblio_cut::bounded_decoded_state(
            raw,
            JsonLimits::default(),
            local_limits.max_state_bytes,
            local_limits.deadline,
            cancelled,
        )?;
        let is_current_claim = &decoded == &claim;
        drop(decoded);
        let current_raw = if is_current_claim { selected_line } else { raw };
        let mut local = crate::record_rules::validate_source_claim_from_input(
            input,
            records,
            current_raw,
            schemas,
            local_limits,
            cancelled,
        )?;
        if !local.issues.is_empty()
            || local.input_identity != input_identity
            || local.current_membership != current_membership
            || local.prepared_execution_binding != schemas.prepared_execution_binding()
            || local.schema_set_sha256 != schemas.schema_set_digest()
            || local.contract_selection_sha256 != schemas.contract_selection_digest()
        {
            return Err(bad("candidate native Claim local-profile binding"));
        }
        merge_candidate_diagnostics_cost(&mut diagnostics_cost, local.diagnostics_cost)?;
        Ok(LocalClaimValidation {
            dependency_digests: std::mem::take(&mut local.dependency_digests),
            dependency_bytes_read: local.dependency_bytes_read,
        })
    };
    let observation = reader.verify_with_local_claim_validator(
        claim_path,
        &claim,
        schemas,
        &mut validate_local_claim,
    )?;
    drop(validate_local_claim);
    drop(claim);
    reader.release_temporary(claim_state);
    let claims_raw_state = claims_raw
        .len()
        .checked_add(std::mem::size_of::<Vec<u8>>())
        .ok_or(ItemRefusal::Budget)?;
    drop(selected_line);
    drop(claims_raw);
    reader.release_temporary(claims_raw_state);
    reader.release_raw_cache();
    input.verify_current_fence(coverage, limits.deadline, cancelled)?;
    let observation = measured_compound_observation(reader, observation)?;
    let typed_wrapper_state = std::mem::size_of::<CandidateNativeCompoundReadObservation<I>>()
        .checked_sub(std::mem::size_of::<NativeCompoundReadObservation>())
        .ok_or(ItemRefusal::Budget)?;
    let returned_state_bytes = observation
        .returned_state_bytes
        .checked_add(typed_wrapper_state)
        .ok_or(ItemRefusal::Budget)?;
    if returned_state_bytes > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "candidate native compound returned state",
            used: Some(returned_state_bytes as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    Ok(CandidateNativeCompoundReadObservation {
        input_identity,
        current_membership,
        prepared_execution_binding,
        schema_set_sha256,
        contract_selection_sha256,
        observation,
        diagnostics_cost,
        returned_state_bytes,
    })
}

/// Actual read cost returned to the Item owner for cumulative topology admission.
#[derive(Debug)]
pub struct NativeCompoundReadObservation {
    pub claim_path: String,
    pub claim_id: String,
    pub transaction_id: String,
    pub manifest_sha256: String,
    pub transport: NativeTransportState,
    pub work_parent_transition_sha256: Option<String>,
    pub reads: Vec<PredicateRead>,
    pub bytes_read: u64,
    pub returned_state_bytes: usize,
}

/// Candidate-fenced native replay evidence. The input identity and complete
/// membership stay in the source adapter's opaque domain; no SourceRevision
/// is synthesized for a spooled candidate.
#[derive(Debug)]
pub struct CandidateNativeCompoundReadObservation<I> {
    input_identity: I,
    current_membership: SourceMembershipV1,
    prepared_execution_binding: CutPreparedSchemaExecutionBinding,
    schema_set_sha256: Digest256,
    contract_selection_sha256: Digest256,
    observation: NativeCompoundReadObservation,
    diagnostics_cost: crate::record_rules::CandidateLocalClaimDiagnosticsCost,
    returned_state_bytes: usize,
}

impl<I> CandidateNativeCompoundReadObservation<I> {
    pub fn input_identity(&self) -> &I {
        &self.input_identity
    }

    pub fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    pub fn prepared_execution_binding(&self) -> CutPreparedSchemaExecutionBinding {
        self.prepared_execution_binding
    }

    pub fn schema_set_sha256(&self) -> Digest256 {
        self.schema_set_sha256
    }

    pub fn contract_selection_sha256(&self) -> Digest256 {
        self.contract_selection_sha256
    }

    pub fn observation(&self) -> &NativeCompoundReadObservation {
        &self.observation
    }

    pub fn diagnostics_cost(&self) -> crate::record_rules::CandidateLocalClaimDiagnosticsCost {
        self.diagnostics_cost
    }

    pub fn returned_state_bytes(&self) -> usize {
        self.returned_state_bytes
    }
}

/// Successful exact-cut observation of one maintained native source record's
/// current revision chain and the publication transports named by that chain.
/// The constructor is private to this module so callers cannot manufacture
/// lineage evidence from a LayerFamilySource read or a copied JSON document.
#[derive(Debug)]
pub struct NativeRecordHistoryReadObservation<I = SourceRevision> {
    input_identity: I,
    current_membership: SourceMembershipV1,
    record_path: String,
    identity_field: &'static str,
    identity: String,
    selected_package: Package,
    current_record: Value,
    origin_record_sha256: String,
    origin_record_byte_size: usize,
    history_ref: Option<String>,
    history_sha256: Option<String>,
    history: Value,
    transactions: Vec<NativeRecordHistoryTransactionObservation>,
    reads: Vec<PredicateRead>,
    bytes_read: u64,
    returned_state_bytes: usize,
}

#[derive(Debug)]
pub struct NativeRecordHistoryTransactionObservation {
    transaction_id: String,
    manifest_sha256: String,
    transport: NativeTransportState,
}

/// Candidate-fenced history observations preserve the caller's opaque input
/// identity and never synthesize a `SourceRevision`.
pub type CandidateNativeRecordHistoryReadObservation<I> = NativeRecordHistoryReadObservation<I>;

impl NativeRecordHistoryReadObservation<SourceRevision> {
    pub fn source_revision(&self) -> SourceRevision {
        self.input_identity
    }
}

impl<I> NativeRecordHistoryReadObservation<I> {
    pub fn input_identity(&self) -> &I {
        &self.input_identity
    }

    pub fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    pub fn identity_field(&self) -> &'static str {
        self.identity_field
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn selected_package(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.selected_package
    }

    pub fn current_record(&self) -> &Value {
        &self.current_record
    }

    pub fn origin_record_sha256(&self) -> &str {
        &self.origin_record_sha256
    }

    pub fn origin_record_byte_size(&self) -> usize {
        self.origin_record_byte_size
    }

    pub fn history_ref(&self) -> Option<&str> {
        self.history_ref.as_deref()
    }

    pub fn history_sha256(&self) -> Option<&str> {
        self.history_sha256.as_deref()
    }

    pub fn history(&self) -> &Value {
        &self.history
    }

    pub fn history_receipt_count(&self) -> usize {
        self.history
            .get("receipts")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }

    pub fn transactions(&self) -> &[NativeRecordHistoryTransactionObservation] {
        &self.transactions
    }

    pub fn reads(&self) -> &[PredicateRead] {
        &self.reads
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    pub fn returned_state_bytes(&self) -> usize {
        self.returned_state_bytes
    }
}

impl NativeRecordHistoryTransactionObservation {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    pub fn transport(&self) -> NativeTransportState {
        self.transport
    }
}

/// Reconstruct one maintained selected record package and its complete native
/// history from a real source cut. Historical archive custody is delegated to
/// `history_typed`; transaction, publication-control, and completion custody
/// are delegated to `transaction`. Unsupported retained operations remain an
/// exact kernel refusal for the source caller to report as such.
pub fn selected_record_history_from_cut(
    cut: &CorpusCutReader,
    record_path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeRecordHistoryReadObservation, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let source_revision = cut.current().revision();
    let current_membership = cut
        .stream(source_revision)
        .map_err(|_| bad("selected native record source-cut membership"))?
        .expectation();
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    selected_record_history_with_reader(
        source_revision,
        current_membership,
        record_path,
        limits,
        cancelled,
        reader,
    )
}

/// Reconstruct the same maintained native record history through a borrowed
/// current-input adapter. The adapter identity and report membership stay
/// opaque; all exact members and prefix closures are read from that input.
pub fn selected_record_history_from_input<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    current_membership: SourceMembershipV1,
    record_path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<CandidateNativeRecordHistoryReadObservation<I>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let input_identity = *input.input_identity();
    let reader = NativeCompoundReader::new_from_input(input.source_input(), limits, cancelled)?;
    selected_record_history_with_reader(
        input_identity,
        current_membership,
        record_path,
        limits,
        cancelled,
        reader,
    )
}

fn selected_record_history_with_reader<I: Copy>(
    input_identity: I,
    current_membership: SourceMembershipV1,
    record_path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    mut reader: NativeCompoundReader<'_>,
) -> Result<NativeRecordHistoryReadObservation<I>, ItemRefusal> {
    let scope = reader.temporary_state;
    let selected_package = reader.selected(record_path)?;
    let basename = record_path
        .rsplit('/')
        .next()
        .ok_or_else(|| bad("selected native record basename"))?;
    let record_raw = selected_package
        .get(basename)
        .ok_or_else(|| bad("selected native record bytes"))?;
    let current_record = reader.decoded(record_raw)?;
    let (identity_field, expected_kind, expected_schema) =
        native_record_history_discriminator(record_path, &current_record)?;
    let schema_matches = match expected_schema {
        Some(schema) => text(&current_record, "schema_version")? == schema,
        None => true,
    };
    let kind_matches =
        identity_field != "record_id" || text(&current_record, "record_type")? == expected_kind;
    if !schema_matches || !kind_matches {
        return Err(bad("selected native record basename/schema discriminator"));
    }
    let identity_ref = text(&current_record, identity_field)?;
    if identity_ref.is_empty() {
        return Err(bad("selected native record identity"));
    }
    if !typed_id(identity_ref, expected_kind) {
        return Err(bad("selected native record typed identity"));
    }
    if integer(&current_record, "record_version")? == 0 {
        return Err(bad("selected native record positive version"));
    }
    reader.temporary(identity_ref.len())?;
    let identity = identity_ref.to_owned();
    let home = parent(record_path)?;
    let has_history = selected_package.contains_key(HISTORY);
    if has_history {
        reader.temporary(
            home.len()
                .checked_add(1 + HISTORY.len())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        reader.temporary("sha256:".len() + 64)?;
    }
    let history_ref = has_history.then(|| format!("{home}/{HISTORY}"));
    let history_sha256 = selected_package
        .get(HISTORY)
        .map(|raw| Digest256::of_bytes(raw).to_prefixed());
    let current_record_byte_size = record_raw.len();
    let (history, origin_record) =
        reader.history_typed_with_origin(record_path, &selected_package, identity_field, true)?;
    let (origin_record_sha256, origin_record_byte_size) = match origin_record {
        Some(origin) => origin,
        None => (
            Digest256::of_bytes(record_raw).to_hex(),
            current_record_byte_size,
        ),
    };
    let origin_record_state = origin_record_sha256
        .len()
        .checked_add(std::mem::size_of::<String>())
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(origin_record_state)?;
    let receipts = array(&history, "receipts")?;
    let transaction_slots = receipts
        .len()
        .checked_mul(std::mem::size_of::<NativeRecordHistoryTransactionObservation>())
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(
        std::mem::size_of::<BTreeSet<String>>()
            .checked_add(transaction_slots)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let mut seen_transactions = BTreeSet::new();
    let mut transactions = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        check(limits.deadline, cancelled)?;
        let Some(publication) = receipt.get("publication") else {
            continue;
        };
        let transaction_id = text(publication, "transaction_id")?;
        if !seen_transactions.contains(transaction_id) {
            reader.temporary(
                transaction_id
                    .len()
                    .checked_add(std::mem::size_of::<String>() * 2)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            seen_transactions.insert(transaction_id.to_owned());
            let transaction = reader.transaction(transaction_id)?;
            reader.temporary(
                transaction_id
                    .len()
                    .checked_add(transaction.manifest_sha256.len())
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            let transport = match transaction.status.as_str() {
                "committed" => NativeTransportState::Committed,
                "rolled-back" => NativeTransportState::RolledBack,
                "pending" => NativeTransportState::Pending,
                "orphan" => NativeTransportState::Orphan,
                _ => return Err(bad("selected native history transaction status")),
            };
            transactions.push(NativeRecordHistoryTransactionObservation {
                transaction_id: transaction_id.to_owned(),
                manifest_sha256: transaction.manifest_sha256.clone(),
                transport,
            });
        }
    }

    let current_record_state = crate::record_biblio_cut::decoded_state(&current_record)?
        .checked_sub(std::mem::size_of::<Value>())
        .ok_or(ItemRefusal::Budget)?;
    let history_state = crate::record_biblio_cut::decoded_state(&history)?;
    reader.temporary(
        history_state
            .checked_sub(std::mem::size_of::<Value>())
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let selected_state = package_state(&selected_package)?;
    reader.temporary(
        selected_state
            .checked_sub(std::mem::size_of::<Package>())
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let identity_state = identity
        .len()
        .checked_add(record_path.len())
        .and_then(|n| n.checked_add(std::mem::size_of::<NativeRecordHistoryReadObservation<I>>()))
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(identity_state)?;
    let history_path_state = history_ref.as_ref().map_or(0, String::len)
        + history_sha256.as_ref().map_or(0, String::len);
    reader.temporary(history_path_state)?;
    let transaction_list_state = transactions
        .capacity()
        .checked_mul(std::mem::size_of::<NativeRecordHistoryTransactionObservation>())
        .and_then(|n| {
            transactions.iter().try_fold(n, |sum, transaction| {
                sum.checked_add(transaction.transaction_id.len())?
                    .checked_add(transaction.manifest_sha256.len())
            })
        })
        .ok_or(ItemRefusal::Budget)?;

    let history = (*history).clone();
    let bytes_read = reader.bytes;
    let reads = std::mem::take(&mut reader.reads);
    let read_state = reads
        .iter()
        .try_fold(0usize, |sum, read| {
            sum.checked_add(crate::record_biblio_cut::predicate_state(read).ok()?)
        })
        .ok_or(ItemRefusal::Budget)?;
    reader.state = reader
        .state
        .checked_sub(read_state)
        .ok_or(ItemRefusal::Budget)?;
    reader.release_temporary_since(scope);
    reader.release_raw_cache();
    let returned_state_bytes = selected_state
        .checked_sub(std::mem::size_of::<Package>())
        .and_then(|n| {
            crate::record_biblio_cut::decoded_state(&history)
                .ok()?
                .checked_sub(std::mem::size_of::<Value>())
                .and_then(|tree| n.checked_add(tree))
        })
        .and_then(|n| n.checked_add(current_record_state))
        .and_then(|n| n.checked_add(transaction_list_state))
        .and_then(|n| n.checked_add(identity_state))
        .and_then(|n| n.checked_add(origin_record_state))
        .and_then(|n| n.checked_add(history_path_state))
        .and_then(|n| n.checked_add(read_state))
        .ok_or(ItemRefusal::Budget)?;
    if reader
        .retained_state_bytes()
        .checked_add(returned_state_bytes)
        .is_none_or(|used| used > limits.max_state_bytes)
        || returned_state_bytes > limits.max_state_bytes
    {
        return Err(ItemRefusal::BudgetCheck {
            check: "native selected record history observation state",
            used: Some(returned_state_bytes as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }

    Ok(NativeRecordHistoryReadObservation {
        input_identity,
        current_membership,
        record_path: record_path.to_owned(),
        identity_field,
        identity,
        selected_package,
        current_record,
        origin_record_sha256,
        origin_record_byte_size,
        history_ref,
        history_sha256,
        history,
        transactions,
        reads,
        bytes_read,
        returned_state_bytes,
    })
}

/// Exact historical input evidence reconstructed by the maintained archive
/// and lineage owner. This does not mint a revision or accept source meaning.
pub(crate) struct NativeRetainedInputObservation {
    pub resolved: bool,
    pub reads: Vec<PredicateRead>,
    pub bytes_read: u64,
    pub returned_state_bytes: usize,
}

pub(crate) fn resolve_retained_record_input(
    input: &dyn SourceCutInput,
    path: &str,
    expected: Digest256,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeRetainedInputObservation, ItemRefusal> {
    let reader = NativeCompoundReader::new_from_input(input, limits, cancelled)?;
    resolve_retained_record_input_with_reader(reader, path, expected, limits, cancelled)
}

pub(crate) fn resolve_retained_record_input_from_cut(
    cut: &CorpusCutReader,
    path: &str,
    expected: Digest256,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeRetainedInputObservation, ItemRefusal> {
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    resolve_retained_record_input_with_reader(reader, path, expected, limits, cancelled)
}

fn resolve_retained_record_input_with_reader(
    mut reader: NativeCompoundReader<'_>,
    path: &str,
    expected: Digest256,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeRetainedInputObservation, ItemRefusal> {
    let scope = reader.temporary_state;
    let package = reader.selected(path)?;
    let name = path
        .rsplit('/')
        .next()
        .ok_or_else(|| bad("recorded input basename"))?;
    let raw = package
        .get(name)
        .ok_or_else(|| bad("recorded input current record"))?;
    let record = reader.decoded(raw)?;
    let (identity_field, _, _) = native_record_history_discriminator(path, &record)?;
    let id = text(&record, identity_field)?;
    // The same owner checks current revision, archive locator/package digest,
    // exact blob bytes, predecessor identities and continuous receipt prefixes.
    let history = reader.history_typed(path, &package, identity_field)?;
    let mut resolved = false;
    for receipt in array(&history, "receipts")? {
        check(limits.deadline, cancelled)?;
        let before = reader.temporary_state;
        let archived = reader.archive_typed(path, id, receipt, identity_field)?;
        let archived_raw = archived
            .get(name)
            .ok_or_else(|| bad("recorded input archive record"))?;
        if Digest256::of_bytes(archived_raw) == expected {
            resolved = true;
        }
        drop(archived);
        reader.release_temporary_since(before);
        if resolved {
            break;
        }
    }
    drop(history);
    drop(record);
    drop(package);
    reader.release_temporary_since(scope);
    reader.release_raw_cache();
    let returned_state_bytes = reader
        .reads
        .iter()
        .try_fold(
            std::mem::size_of::<NativeRetainedInputObservation>(),
            |sum, read| sum.checked_add(crate::record_biblio_cut::predicate_state(read).ok()?),
        )
        .ok_or(ItemRefusal::Budget)?;
    if reader
        .retained_state_bytes()
        .checked_add(std::mem::size_of::<NativeRetainedInputObservation>())
        .is_none_or(|bytes| bytes > limits.max_state_bytes)
        || returned_state_bytes > limits.max_state_bytes
    {
        return Err(ItemRefusal::Budget);
    }
    Ok(NativeRetainedInputObservation {
        resolved,
        reads: reader.reads,
        bytes_read: reader.bytes,
        returned_state_bytes,
    })
}

fn native_record_history_discriminator(
    record_path: &str,
    record: &Value,
) -> Result<(&'static str, &'static str, Option<&'static str>), ItemRefusal> {
    if !record_path.starts_with("ToS/source-witnesses/") || record_path.split('/').count() < 4 {
        return Err(bad("selected native record owner path"));
    }
    let owner_group = record_path.split('/').nth(2).unwrap_or("");
    let basename = record_path.rsplit('/').next().unwrap_or("");
    let (identity_field, kind, schema) = match basename {
        "agent.json" => ("record_id", "agent", Some("tos_corpus_record_v1")),
        "place.json" => ("record_id", "place", Some("tos_corpus_record_v1")),
        "organization.json" => ("record_id", "organization", Some("tos_corpus_record_v1")),
        "work.json" => ("record_id", "work", Some("tos_corpus_record_v1")),
        "expression.json" => ("record_id", "expression", Some("tos_corpus_record_v1")),
        "edition.json" => ("record_id", "edition", Some("tos_corpus_record_v1")),
        "collection.json" => ("record_id", "collection", Some("tos_corpus_record_v1")),
        "item.json" => ("record_id", "item", Some("tos_corpus_record_v1")),
        "link.json" if owner_group == "links" => ("record_id", "link", Some("tos_source_link_v1")),
        "artifact-witness.json" if owner_group == "artifacts" => {
            let schema = match text(record, "schema_version")? {
                "tos_artifact_source_witness_v1" => "tos_artifact_source_witness_v1",
                "tos_artifact_source_witness_v2" => "tos_artifact_source_witness_v2",
                _ => {
                    return Err(ItemRefusal::Unsupported(
                        "native artifact history schema".into(),
                    ));
                }
            };
            ("artifact_id", "artifact", Some(schema))
        }
        "composite-witness.json" if owner_group == "scholarly-composites" => (
            "composite_id",
            "composite",
            Some("tos_scholarly_composite_witness_v1"),
        ),
        _ => {
            return Err(ItemRefusal::Unsupported(
                "native record history carrier".into(),
            ));
        }
    };
    Ok((identity_field, kind, schema))
}
fn measured_compound_observation(
    reader: NativeCompoundReader<'_>,
    observation: NativeCompoundObservation,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    let NativeCompoundObservation {
        claim_path,
        claim_id,
        transaction_id,
        manifest_sha256,
        transport,
        work_parent_transition_sha256,
    } = observation;
    let identity = std::mem::size_of::<NativeCompoundReadObservation>()
        .checked_add(claim_path.len())
        .and_then(|n| n.checked_add(claim_id.len()))
        .and_then(|n| n.checked_add(transaction_id.len()))
        .and_then(|n| n.checked_add(manifest_sha256.len()))
        .and_then(|n| {
            n.checked_add(
                work_parent_transition_sha256
                    .as_ref()
                    .map_or(0, String::len),
            )
        })
        .ok_or(ItemRefusal::Budget)?;
    let returned_state_bytes = reader
        .reads
        .iter()
        .try_fold(identity, |n, read| {
            n.checked_add(crate::record_biblio_cut::predicate_state(read).ok()?)
        })
        .ok_or(ItemRefusal::Budget)?;
    if reader
        .state
        .checked_add(identity)
        .is_none_or(|n| n > reader.limits.max_state_bytes)
        || returned_state_bytes > reader.limits.max_state_bytes
    {
        return Err(ItemRefusal::Budget);
    }
    Ok(NativeCompoundReadObservation {
        claim_path,
        claim_id,
        transaction_id,
        manifest_sha256,
        transport,
        work_parent_transition_sha256,
        reads: reader.reads,
        bytes_read: reader.bytes,
        returned_state_bytes,
    })
}

/// Same Work-origin verification, returning its actual reads and cumulative cost.
/// Existing descriptive Work observation API remains unchanged.
pub fn verify_work_expression_reads_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    if claim["predicate"] != "has_expression"
        || schemas.source_revision() != cut.current().revision()
    {
        return Err(bad("selected Work compound type/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let observation = reader.verify(claim_path, claim, schemas)?;
    measured_compound_observation(reader, observation)
}
/// Exact qualified Collection membership verification and existing read custody.
pub fn verify_collection_membership_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    if claim["predicate"] != "contains_work"
        || schemas.source_revision() != cut.current().revision()
    {
        return Err(bad("selected Collection membership compound type/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let observation = reader.verify(claim_path, claim, schemas)?;
    measured_compound_observation(reader, observation)
}

/// Read the exact selected Expression responsibility compound and its retained/current lineage.
/// The transport observation is descriptive; CMD still owns the physical
/// publication, source/software and journal fences for any replay decision.
pub fn verify_expression_responsibility_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    if claim["predicate"] != "translated_by"
        || schemas.source_revision() != cut.current().revision()
    {
        return Err(bad("selected Expression responsibility compound type/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let observation = reader.verify(claim_path, claim, schemas)?;
    measured_compound_observation(reader, observation)
}

/// Read the exact selected Expression/Edition compound and its retained/current lineage.
/// The transport observation is descriptive; CMD still owns the physical
/// publication, source/software and journal fences for any replay decision.
pub fn verify_expression_edition_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    if claim["predicate"] != "embodied_by" || schemas.source_revision() != cut.current().revision()
    {
        return Err(bad("selected Expression/Edition compound type/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let observation = reader.verify(claim_path, claim, schemas)?;
    measured_compound_observation(reader, observation)
}

/// Read the exact selected Edition/Item compound and its retained/current lineage.
/// The transport observation is descriptive; CMD still owns the physical
/// publication, source/software and journal fences for any replay decision.
pub fn verify_edition_item_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    if claim["predicate"] != "exemplified_by"
        || schemas.source_revision() != cut.current().revision()
    {
        return Err(bad("selected Edition/Item compound type/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let observation = reader.verify(claim_path, claim, schemas)?;
    measured_compound_observation(reader, observation)
}

/// An exact Collection record version and the selected bytes that established
/// it. A historical version remains historical; this is no writer grant.
#[derive(Debug)]
pub struct CollectionVersionObservation {
    pub record_raw: Vec<u8>,
    pub source_path: String,
    pub current_ref: Value,
    pub version_status: &'static str,
    pub archive_blob_ref: Option<String>,
    pub archive_manifest_ref: Option<String>,
    pub archive_manifest_sha256: Option<String>,
    pub package_revision: Option<String>,
    pub history_ref: Option<String>,
    pub history_sha256: Option<String>,
    pub history_receipt_count: usize,
    pub retained_baseline_ref: Value,
    pub transition: Option<Value>,
    pub transaction_id: Option<String>,
    pub transaction_manifest_sha256: Option<String>,
    pub reads: Vec<PredicateRead>,
    pub bytes_read: u64,
    /// Logical objects/slots/payloads retained by this returned observation.
    pub returned_state_bytes: usize,
}

/// Resolve one Collection version through the selected current package and
/// its complete retained history. Each native membership transition used by
/// that lineage still goes through the existing compound verifier.
pub fn verify_collection_version_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    collection_source_path: &str,
    exact: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<CollectionVersionObservation, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    keys(exact, &["id", "version", "digest"])?;
    if schemas.source_revision() != cut.current().revision()
        || !collection_source_path.starts_with("ToS/source-witnesses/collections/")
        || !collection_source_path.ends_with("/collection.json")
        || !typed_id(text(exact, "id")?, "collection")
        || integer(exact, "version")? == 0
    {
        return Err(bad("selected Collection exact source/worker/type"));
    }
    hash(text(exact, "digest")?)?;
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let selected_phase = reader.temporary_state;
    let mut selected = reader.selected(collection_source_path)?;
    // selected() already charges each member buffer, map entry and key.
    reader.temporary(std::mem::size_of::<Package>())?;
    let current_raw = selected
        .get("collection.json")
        .ok_or_else(|| bad("selected Collection record absent"))?;
    let current = reader.decoded(current_raw)?;
    if current["record_type"] != "collection" || text(&current, "record_id")? != text(exact, "id")?
    {
        return Err(bad("selected Collection typed identity"));
    }
    let current_ref = json!({
        "id":text(&current,"record_id")?,
        "version":integer(&current,"record_version")?,
        "digest":reader.canonical_observation(&current)?.0,
    });
    reader.temporary(crate::record_biblio_cut::decoded_state(&current_ref)?)?;
    let history = reader.history(collection_source_path, &selected)?;
    let receipts = array(&history, "receipts")?;
    let history_receipt_count = receipts.len();
    let baseline = receipts
        .first()
        .map(|receipt| &receipt["previous_source"])
        .unwrap_or(&current_ref);
    reader.temporary(crate::record_biblio_cut::decoded_state(baseline)?)?;
    let retained_baseline_ref = baseline.clone();
    let mut historical_index = None;
    for (index, receipt) in receipts.iter().enumerate() {
        check(limits.deadline, cancelled)?;
        let operation = text(&receipt["request"], "operation")?;
        if operation == "collection.work.attach" {
            let publication = &receipt["publication"];
            let transaction_id = text(publication, "transaction_id")?;
            let transaction = reader.transaction(transaction_id)?;
            if transaction.status != "committed" {
                return Err(bad("Collection membership transition not committed"));
            }
            let scope = &transaction.manifest["plan"]["authorization"]["scope"];
            if text(scope, "collection_source_path")? != collection_source_path
                || text(scope, "collection_id")? != text(exact, "id")?
            {
                return Err(bad("Collection transition selected parent"));
            }
            let claim_path = text(scope, "claim_source_path")?;
            let claim_id = text(scope, "claim_id")?;
            let claim_phase = reader.temporary_state;
            let request_path = format!("{}/source-create-request.json", parent(claim_path)?);
            let request_raw = transaction
                .files
                .get(&request_path)
                .and_then(|(_, after)| after.as_deref())
                .ok_or_else(|| bad("Collection transition original request absent"))?;
            if reader.decoded(request_raw)? != receipt["request"] {
                return Err(bad("Collection parent receipt/request transaction changed"));
            }
            let claim_raw = reader.required(claim_path, MAX_FILE)?;
            let claim = reader.single_membership_claim(&claim_raw, claim_id)?;
            if claim["predicate"] != "contains_work"
                || claim["subject_ref"] != scope["collection_id"]
                || claim["object"] != scope["work_id"]
            {
                return Err(bad("Collection transition current qualified association"));
            }
            let observed = reader.verify(claim_path, &claim, schemas)?;
            if observed.transport != NativeTransportState::Committed
                || observed.transaction_id != transaction_id
                || observed.manifest_sha256 != transaction.manifest_sha256
                || observed.claim_path != claim_path
                || observed.claim_id != claim_id
                || text(&receipt["request"]["claim"], "claim_id")? != claim_id
            {
                return Err(bad("Collection transition exact Claim/manifest"));
            }
            reader.release_temporary_since(claim_phase);
        } else if operation != "record.revise" {
            return Err(ItemRefusal::Unsupported(format!(
                "Collection parent transition {operation}"
            )));
        }
        if receipt["previous_source"] == *exact {
            if historical_index.replace(index).is_some() {
                return Err(bad("Collection historical version ambiguous"));
            }
        }
    }
    let current_match = current_ref == *exact;
    if current_match && historical_index.is_some() {
        return Err(bad("Collection exact version current/archive ambiguity"));
    }
    if !current_match && historical_index.is_none() {
        return Err(bad("Collection exact historical version absent"));
    }
    let collection_home = parent(collection_source_path)?;
    let history_ref = selected
        .get(HISTORY)
        .map(|_| format!("{collection_home}/{HISTORY}"));
    let history_sha256 = selected
        .get(HISTORY)
        .map(|raw| Digest256::of_bytes(raw).to_prefixed());
    let mut archive_blob_ref = None;
    let mut archive_manifest_ref = None;
    let mut archive_manifest_sha256 = None;
    let mut package_revision = None;
    let mut transition = None;
    let mut transaction_id = None;
    let mut transaction_manifest_sha256 = None;
    let record_raw = if let Some(index) = historical_index {
        let receipt = &receipts[index];
        let archive_phase = reader.temporary_state;
        let mut archived = reader.archive(collection_source_path, text(exact, "id")?, receipt)?;
        let raw = archived
            .remove("collection.json")
            .ok_or_else(|| bad("Collection archived record absent"))?;
        let archived_record = reader.decoded(&raw)?;
        if archived_record["record_type"] != "collection"
            || !reader.reference_matches(&archived_record, "record_id", "record_version", exact)?
        {
            return Err(bad("Collection archive exact version changed"));
        }
        let archive_path = text(receipt, "archive_path")?;
        let manifest_ref = format!("{archive_path}/manifest.json");
        let manifest_raw = reader.required(&manifest_ref, MAX_FILE)?;
        let manifest = reader.decoded(&manifest_raw)?;
        let blob = text(&manifest["files"]["collection.json"], "blob")?;
        archive_blob_ref = Some(format!("{archive_path}/{blob}"));
        archive_manifest_sha256 = Some(Digest256::of_bytes(&manifest_raw).to_prefixed());
        archive_manifest_ref = Some(manifest_ref);
        package_revision = Some(text(receipt, "previous_revision")?.to_owned());
        transition = Some(reader.value_copy(receipt)?);
        if let Some(publication) = receipt.get("publication") {
            let id = text(publication, "transaction_id")?;
            let tx = reader.transaction(id)?;
            if tx.status != "committed" {
                return Err(bad("Collection historical transition not committed"));
            }
            transaction_id = Some(id.to_owned());
            transaction_manifest_sha256 = Some(tx.manifest_sha256.clone());
        }
        drop(archived);
        reader.release_temporary_since(archive_phase);
        raw
    } else {
        selected
            .remove("collection.json")
            .ok_or_else(|| bad("selected Collection record absent"))?
    };
    drop(current);
    drop(selected);
    drop(history);
    reader.release_temporary_since(selected_phase);
    reader.release_raw_cache();
    let output_early = (std::mem::size_of::<Vec<u8>>() + record_raw.len())
        .checked_add(crate::record_biblio_cut::decoded_state(&current_ref)?)
        .and_then(|n| {
            n.checked_add(crate::record_biblio_cut::decoded_state(&retained_baseline_ref).ok()?)
        })
        .and_then(|n| {
            n.checked_add(match &transition {
                Some(value) => crate::record_biblio_cut::decoded_state(value).ok()?,
                None => 0,
            })
        })
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(output_early)?;
    let mut observation = CollectionVersionObservation {
        record_raw,
        source_path: collection_source_path.to_owned(),
        current_ref,
        version_status: if current_match {
            "current"
        } else {
            "historical"
        },
        archive_blob_ref,
        archive_manifest_ref,
        archive_manifest_sha256,
        package_revision,
        history_ref,
        history_sha256,
        history_receipt_count,
        retained_baseline_ref,
        transition,
        transaction_id,
        transaction_manifest_sha256,
        reads: std::mem::take(&mut reader.reads),
        bytes_read: reader.bytes,
        returned_state_bytes: 0,
    };
    let reads_state = observation
        .reads
        .iter()
        .try_fold(0usize, |sum, read| {
            sum.checked_add(crate::record_biblio_cut::predicate_state(read).ok()?)
        })
        .ok_or(ItemRefusal::Budget)?;
    let retained = std::mem::size_of::<CollectionVersionObservation>()
        .checked_add(observation.record_raw.len())
        .and_then(|n| n.checked_add(observation.source_path.len()))
        .and_then(|n| {
            n.checked_add(
                crate::record_biblio_cut::decoded_state(&observation.current_ref)
                    .ok()?
                    .checked_sub(std::mem::size_of::<Value>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                crate::record_biblio_cut::decoded_state(&observation.retained_baseline_ref)
                    .ok()?
                    .checked_sub(std::mem::size_of::<Value>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(match &observation.transition {
                Some(value) => crate::record_biblio_cut::decoded_state(value)
                    .ok()?
                    .checked_sub(std::mem::size_of::<Value>())?,
                None => 0,
            })
        })
        .and_then(|n| {
            [
                &observation.archive_blob_ref,
                &observation.archive_manifest_ref,
                &observation.archive_manifest_sha256,
                &observation.package_revision,
                &observation.history_ref,
                &observation.history_sha256,
                &observation.transaction_id,
                &observation.transaction_manifest_sha256,
            ]
            .into_iter()
            .try_fold(n, |sum, value| {
                sum.checked_add(value.as_ref().map_or(0, String::len))
            })
        })
        .and_then(|n| n.checked_add(reads_state))
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(
        retained
            .checked_sub(reads_state)
            .and_then(|n| n.checked_sub(output_early))
            .ok_or(ItemRefusal::Budget)?,
    )?;
    observation.returned_state_bytes = retained;
    check(limits.deadline, cancelled)?;
    Ok(observation)
}

// Maintained bibliographic recipes share only their transport and exact
// buffer-construction law. These constants are owner profiles, not grants.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompoundKind {
    CollectionWork,
    ExpressionResponsibility,
    WorkExpression,
    ExpressionEdition,
    EditionItem,
}
impl CompoundKind {
    fn from_operation(operation: &str) -> Result<Self, ItemRefusal> {
        match operation {
            "collection.work.attach" => Ok(Self::CollectionWork),
            "expression.responsibility.attach" => Ok(Self::ExpressionResponsibility),
            "work.expression.create" => Ok(Self::WorkExpression),
            "expression.edition.create" => Ok(Self::ExpressionEdition),
            "item.adopt" => Ok(Self::EditionItem),
            other => Err(ItemRefusal::Unsupported(format!(
                "retained compound parent handler {other}"
            ))),
        }
    }
    fn from_predicate(predicate: &str) -> Result<Self, ItemRefusal> {
        match predicate {
            "contains_work" => Ok(Self::CollectionWork),
            "translated_by" => Ok(Self::ExpressionResponsibility),
            "has_expression" => Ok(Self::WorkExpression),
            "embodied_by" => Ok(Self::ExpressionEdition),
            "exemplified_by" => Ok(Self::EditionItem),
            other => Err(ItemRefusal::Unsupported(format!(
                "native compound predicate {other}"
            ))),
        }
    }
    fn parent_kind(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection",
            Self::ExpressionResponsibility => "expression",
            Self::WorkExpression => "work",
            Self::ExpressionEdition => "expression",
            Self::EditionItem => "edition",
        }
    }
    fn child_kind(self) -> &'static str {
        match self {
            Self::CollectionWork => "work",
            Self::ExpressionResponsibility => "agent",
            Self::WorkExpression => "expression",
            Self::ExpressionEdition => "edition",
            Self::EditionItem => "item",
        }
    }
    fn parent_key(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection_id",
            Self::ExpressionResponsibility => "expression_id",
            Self::WorkExpression => "work_id",
            Self::ExpressionEdition => "expression_id",
            Self::EditionItem => "edition_id",
        }
    }
    fn child_key(self) -> &'static str {
        match self {
            Self::CollectionWork => "work_id",
            Self::ExpressionResponsibility => "agent_id",
            Self::WorkExpression => "expression_id",
            Self::ExpressionEdition => "edition_id",
            Self::EditionItem => "item_id",
        }
    }
    fn parent_path(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection_source_path",
            Self::ExpressionResponsibility => "expression_source_path",
            Self::WorkExpression => "work_source_path",
            Self::ExpressionEdition => "expression_source_path",
            Self::EditionItem => "edition_source_path",
        }
    }
    fn child_path(self) -> &'static str {
        match self {
            Self::CollectionWork => "work_source_path",
            Self::ExpressionResponsibility => "agent_source_path",
            Self::WorkExpression => "expression_source_path",
            Self::ExpressionEdition => "edition_source_path",
            Self::EditionItem => "item_source_path",
        }
    }
    fn parent_file(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection.json",
            Self::ExpressionResponsibility => "expression.json",
            Self::WorkExpression => "work.json",
            Self::ExpressionEdition => "expression.json",
            Self::EditionItem => "edition.json",
        }
    }
    fn child_file(self) -> &'static str {
        match self {
            Self::CollectionWork => "work.json",
            Self::ExpressionResponsibility => "agent.json",
            Self::WorkExpression => "expression.json",
            Self::ExpressionEdition => "edition.json",
            Self::EditionItem => "item.json",
        }
    }
    fn parent_forms(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection.human-forms.json",
            Self::ExpressionResponsibility => "expression.human-forms.json",
            Self::WorkExpression => "work.human-forms.json",
            Self::ExpressionEdition => "expression.human-forms.json",
            Self::EditionItem => "edition.human-forms.json",
        }
    }
    fn child_forms(self) -> &'static str {
        match self {
            Self::CollectionWork => "work.human-forms.json",
            Self::ExpressionResponsibility => "agent.human-forms.json",
            Self::WorkExpression => "expression.human-forms.json",
            Self::ExpressionEdition => "edition.human-forms.json",
            Self::EditionItem => "item.human-forms.json",
        }
    }
    fn child_form_request(self) -> &'static str {
        match self {
            Self::CollectionWork => "forms",
            Self::ExpressionResponsibility => "forms",
            Self::WorkExpression => "expression_forms",
            Self::ExpressionEdition => "edition_forms",
            Self::EditionItem => "item_forms",
        }
    }
    fn field(self) -> &'static str {
        match self {
            Self::CollectionWork => "membership_claim_refs",
            Self::ExpressionResponsibility => "responsibility_claim_refs",
            Self::WorkExpression => "expression_claim_refs",
            Self::ExpressionEdition => "embodiment_claim_refs",
            Self::EditionItem => "exemplar_claim_refs",
        }
    }
    fn predicate(self) -> &'static str {
        match self {
            Self::CollectionWork => "contains_work",
            Self::ExpressionResponsibility => "translated_by",
            Self::WorkExpression => "has_expression",
            Self::ExpressionEdition => "embodied_by",
            Self::EditionItem => "exemplified_by",
        }
    }
    fn operation(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection.work.attach",
            Self::ExpressionResponsibility => "expression.responsibility.attach",
            Self::WorkExpression => "work.expression.create",
            Self::ExpressionEdition => "expression.edition.create",
            Self::EditionItem => "item.adopt",
        }
    }
    fn request_schema(self) -> &'static str {
        match self {
            Self::CollectionWork => "tos_local_collection_membership_command_v1",
            Self::ExpressionResponsibility => "tos_local_expression_responsibility_command_v1",
            Self::WorkExpression => "tos_local_work_expression_command_v1",
            Self::ExpressionEdition => "tos_local_expression_edition_command_v1",
            Self::EditionItem => "tos_local_item_adoption_command_v1",
        }
    }
    fn authorization_schema(self) -> &'static str {
        match self {
            Self::CollectionWork => "tos_collection_membership_authorization_v1",
            Self::ExpressionResponsibility => "tos_expression_responsibility_authorization_v1",
            Self::WorkExpression => "tos_work_expression_authorization_v1",
            Self::ExpressionEdition => "tos_expression_edition_authorization_v1",
            Self::EditionItem => "tos_item_adoption_authorization_v1",
        }
    }
    fn receipt_schema(self) -> &'static str {
        match self {
            Self::CollectionWork => "tos_collection_membership_receipt_v1",
            Self::ExpressionResponsibility => "tos_expression_responsibility_receipt_v1",
            Self::WorkExpression => "tos_work_expression_receipt_v1",
            Self::ExpressionEdition => "tos_expression_edition_receipt_v1",
            Self::EditionItem => "tos_edition_item_receipt_v1",
        }
    }
    fn receipt_file(self) -> &'static str {
        match self {
            Self::CollectionWork => "membership-attachment-receipt.json",
            Self::ExpressionResponsibility => "responsibility-attachment-receipt.json",
            Self::WorkExpression => "work-expression-receipt.json",
            Self::ExpressionEdition => "expression-edition-receipt.json",
            Self::EditionItem => "edition-item-receipt.json",
        }
    }
    fn module(self) -> &'static str {
        match self {
            Self::CollectionWork => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_collection_commands.py"
            }
            Self::ExpressionResponsibility => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py"
            }
            Self::WorkExpression => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py"
            }
            Self::ExpressionEdition => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_edition_commands.py"
            }
            Self::EditionItem => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_commands.py"
            }
        }
    }
    fn executor(self) -> &'static str {
        match self {
            Self::CollectionWork => "software:tos-source-membership-commands",
            Self::ExpressionResponsibility => "software:tos-source-responsibility-commands",
            Self::WorkExpression => "software:tos-source-expression-commands",
            Self::ExpressionEdition => "software:tos-source-edition-commands",
            Self::EditionItem => "software:tos-source-item-commands",
        }
    }
    fn procedure(self) -> &'static str {
        match self {
            Self::CollectionWork => "native-collection-membership-metadata-serialization",
            Self::ExpressionResponsibility => {
                "native-expression-responsibility-metadata-serialization"
            }
            Self::WorkExpression => "native-work-expression-metadata-serialization",
            Self::ExpressionEdition => "native-expression-edition-metadata-serialization",
            Self::EditionItem => "native-item-adoption-metadata-serialization",
        }
    }
    fn component(self) -> &'static str {
        match self {
            Self::CollectionWork => "ToS native Collection membership adapter",
            Self::ExpressionResponsibility => "ToS native Expression responsibility adapter",
            Self::WorkExpression => "ToS native Work Expression adapter",
            Self::ExpressionEdition => "ToS native Expression Edition adapter",
            Self::EditionItem => "ToS native Edition Item adapter",
        }
    }
    fn relation_attachment(self) -> bool {
        matches!(self, Self::CollectionWork | Self::ExpressionResponsibility)
    }
    fn parent_form_grant(self) -> &'static str {
        if self == Self::CollectionWork {
            "allowed_collection_form_ids"
        } else {
            "allowed_expression_form_ids"
        }
    }
    fn record_key(self) -> &'static str {
        match self {
            Self::CollectionWork => "work",
            Self::ExpressionResponsibility => "agent",
            _ => "record",
        }
    }
    fn publication_home<'a>(self, scope: &'a Value) -> Result<&'a str, ItemRefusal> {
        parent(text(
            scope,
            if self.relation_attachment() {
                "claim_source_path"
            } else {
                self.child_path()
            },
        )?)
    }
    fn initial_backlink(self, record: &Value, parent: &Value) -> bool {
        match self {
            Self::ExpressionResponsibility => true, // Existing Agent receives no invented backlink.
            Self::CollectionWork => true, // Existing Work has no invented Collection backlink.
            Self::EditionItem => true, // Item backlink is its exact manifest path, checked at its scope/current read.
            Self::WorkExpression => record["work_ref"] == *parent,
            Self::ExpressionEdition => record["embodies_expression_refs"]
                .as_array()
                .is_some_and(|refs| refs.contains(parent)),
        }
    }
}

fn bad(message: &str) -> ItemRefusal {
    ItemRefusal::Source(format!("native compound: {message}"))
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, ItemRefusal> {
    v.get(key).and_then(Value::as_str).ok_or_else(|| bad(key))
}
fn integer(v: &Value, key: &str) -> Result<u64, ItemRefusal> {
    v.get(key).and_then(Value::as_u64).ok_or_else(|| bad(key))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, ItemRefusal> {
    v.get(key).and_then(Value::as_array).ok_or_else(|| bad(key))
}
fn keys(v: &Value, wanted: &[&str]) -> Result<(), ItemRefusal> {
    let object = v.as_object().ok_or_else(|| bad("object required"))?;
    if object.len() != wanted.len() || wanted.iter().any(|k| !object.contains_key(*k)) {
        return Err(bad("exact fields"));
    }
    Ok(())
}
fn hash(s: &str) -> Result<&str, ItemRefusal> {
    let raw = s
        .strip_prefix("sha256:")
        .ok_or_else(|| bad("prefixed sha256"))?;
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(bad("sha256 grammar"));
    }
    Ok(raw)
}
fn limits() -> JsonLimits {
    JsonLimits::new(MAX_SIDE, 64, 300_000, 4_300).expect("finite JSON limits")
}
fn ordered(raw: &[u8]) -> Result<JsonValue, ItemRefusal> {
    parse_json(raw, JsonMode::PublishedStrict, limits())
        .map(|v| v.into_root())
        .map_err(|e| ItemRefusal::Unsupported(format!("compound published JSON: {e:?}")))
}
fn decode(raw: &[u8]) -> Result<Value, ItemRefusal> {
    ordered(raw)?;
    serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("compound decoded representation".into()))
}
fn canonical(v: &Value) -> Result<Vec<u8>, ItemRefusal> {
    canonical_ordered(&ordered(
        &serde_json::to_vec(v).map_err(|_| bad("serialization"))?,
    )?)
}
fn canonical_ordered(v: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
    canonical_bytes_v1(v, CanonicalProfile::SourceCommandInputV1, limits())
        .map_err(|e| ItemRefusal::Unsupported(format!("compound canonical: {e:?}")))
}
fn digest(v: &Value) -> Result<String, ItemRefusal> {
    Ok(Digest256::of_bytes(&canonical(v)?).to_prefixed())
}
fn pretty(v: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
    emit_json_profile(v, JsonEmissionProfile::SourceFormSetPublishedV1, limits())
        .map(|v| v.bytes)
        .map_err(|e| ItemRefusal::Unsupported(format!("compound record bytes: {e:?}")))
}
fn reference(v: &Value, id: &str, version: &str) -> Result<Value, ItemRefusal> {
    let n = integer(v, version)?;
    if n == 0 {
        return Err(bad("positive record version"));
    }
    Ok(json!({"id":text(v,id)?,"version":n,"digest":digest(v)?}))
}
// The real reference has these three owned fields. Price its shape without
// constructing a stand-in reference or executing its source digest.
fn reference_state(value: &Value, id: &str, version: &str) -> Result<usize, ItemRefusal> {
    let id = text(value, id)?;
    let number = value
        .get(version)
        .and_then(Value::as_number)
        .ok_or_else(|| bad("record version number"))?;
    std::mem::size_of::<Value>()
        .checked_add(3 * std::mem::size_of::<(String, Value)>())
        .and_then(|n| n.checked_add("id".len() + "version".len() + "digest".len()))
        .and_then(|n| n.checked_add(id.len()))
        .and_then(|n| n.checked_add(number.as_str().len()))
        .and_then(|n| n.checked_add("sha256:".len() + 2 * std::mem::size_of::<Digest256>()))
        .ok_or(ItemRefusal::Budget)
}
fn file_refs(files: &Package) -> Value {
    Value::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    name.clone(),
                    json!({"sha256":Digest256::of_bytes(raw).to_prefixed(),"bytes":raw.len()}),
                )
            })
            .collect(),
    )
}
fn selected_names(path: &str) -> Result<[String; 3], ItemRefusal> {
    let base = path
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("record path"))?;
    let stem = base
        .strip_suffix(".json")
        .ok_or_else(|| bad("record basename"))?;
    Ok([
        base.into(),
        format!("{stem}.human-forms.json"),
        HISTORY.into(),
    ])
}
fn parent(path: &str) -> Result<&str, ItemRefusal> {
    path.rsplit_once('/')
        .map(|v| v.0)
        .ok_or_else(|| bad("source parent"))
}
fn metadata_path(path: &str, directory: bool) -> Result<(), ItemRefusal> {
    RelativePath::parse(path).map_err(|_| bad("canonical metadata path"))?;
    let parts: Vec<_> = path.split('/').collect();
    if path.len() > 1024
        || !(3..=24).contains(&parts.len())
        || !path.starts_with(&format!("{HOME}/"))
        || parts.iter().any(|p| {
            p.starts_with('.')
                || matches!(
                    *p,
                    "payload" | "private" | "local-content" | "owner-local" | "catalog"
                )
        })
        || !directory && (parts.len() < 4 || !(path.ends_with(".json") || path.ends_with(".jsonl")))
    {
        return Err(bad("public metadata path scope"));
    }
    Ok(())
}
fn is_ancestor(a: &str, b: &str) -> bool {
    b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
}

struct Transaction {
    manifest: Value,
    manifest_sha256: String,
    status: String,
    files: BTreeMap<String, (Option<Vec<u8>>, Option<Vec<u8>>)>,
}

/// One family invocation owns the directory index and deduplicated exact reads.
/// Neither cache nor historical state survives the selected operation.
pub(crate) struct NativeCompoundReader<'a> {
    cut: Option<&'a CorpusCutReader>,
    input: Option<&'a dyn SourceCutInput>,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    paths: BTreeSet<&'a str>,
    raw: BTreeMap<String, Vec<u8>>,
    raw_cache_state: usize,
    transactions: BTreeMap<String, std::sync::Arc<Transaction>>,
    histories: BTreeMap<(String, String), std::sync::Arc<Value>>,
    state: usize,
    temporary_state: usize,
    bytes: u64,
    reads: Vec<PredicateRead>,
    publication: Option<Value>,
}
impl<'a> NativeCompoundReader<'a> {
    pub(crate) fn new(
        cut: &'a CorpusCutReader,
        limits: ItemLimits,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let mut this = Self {
            cut: Some(cut),
            input: None,
            limits,
            cancelled,
            paths: BTreeSet::new(),
            raw: BTreeMap::new(),
            raw_cache_state: 0,
            transactions: BTreeMap::new(),
            histories: BTreeMap::new(),
            state: std::mem::size_of::<Self>(),
            temporary_state: 0,
            bytes: 0,
            reads: Vec::new(),
            publication: None,
        };
        for member in cut.current().members() {
            check(limits.deadline, cancelled)?;
            reserve(
                &mut this.state,
                std::mem::size_of::<&str>(),
                limits.max_state_bytes,
            )?;
            this.paths.insert(member.path.as_str());
        }
        Self::initialize(this)
    }

    /// Candidate history reads retain no whole-source path set. Exact member
    /// probes and bounded directory-prefix cursors stay on the borrowed input.
    fn new_from_input(
        input: &'a dyn SourceCutInput,
        limits: ItemLimits,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let this = Self {
            cut: None,
            input: Some(input),
            limits,
            cancelled,
            paths: BTreeSet::new(),
            raw: BTreeMap::new(),
            raw_cache_state: 0,
            transactions: BTreeMap::new(),
            histories: BTreeMap::new(),
            state: std::mem::size_of::<Self>(),
            temporary_state: 0,
            bytes: 0,
            reads: Vec::new(),
            publication: None,
        };
        Self::initialize(this)
    }

    fn initialize(mut this: Self) -> Result<Self, ItemRefusal> {
        let startup_temporary = this.temporary_state;
        if let Some(raw) = this.optional(CONTROL, 8192)? {
            let state = this.decoded(&raw)?;
            let retained = crate::record_biblio_cut::decoded_state(&state)?;
            this.temporary(retained)?;
            state_valid_with(&state, &mut |value| this.canonical_observation(value))?;
            this.release_temporary(retained);
            // Move the same tree from the temporary scope into publication.
            this.temporary_state -= retained;
            this.publication = Some(state);
        }
        this.release_temporary_since(startup_temporary);
        this.release_raw_cache();
        Ok(this)
    }

    fn cut(&self) -> &CorpusCutReader {
        self.cut.expect("cut-backed compound reader")
    }
    // `verify` has released its reconstruction temporaries and optional raw
    // cache before bibliography grows Rules. The remaining state still owns
    // the selected index, publication, transaction/history caches and reads.
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.state
    }
    pub(crate) fn set_remaining_state(&mut self, available: usize) -> Result<(), ItemRefusal> {
        if self.state > available {
            return Err(ItemRefusal::BudgetCheck {
                check: "compound retained state after Rules growth",
                used: Some(self.state as u64),
                limit: Some(available as u64),
            });
        }
        self.limits.max_state_bytes = available;
        Ok(())
    }
    fn temporary(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        let next = self.state.checked_add(amount);
        self.state =
            next.filter(|n| *n <= self.limits.max_state_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "compound live reconstruction/history state",
                    used: next.map(|n| n as u64),
                    limit: Some(self.limits.max_state_bytes as u64),
                })?;
        self.temporary_state = self
            .temporary_state
            .checked_add(amount)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
    fn release_temporary(&mut self, amount: usize) {
        self.state -= amount;
        self.temporary_state -= amount;
    }
    fn release_temporary_since(&mut self, before: usize) {
        let released = self.temporary_state - before;
        self.state -= released;
        self.temporary_state = before;
    }
    fn decoded(&mut self, raw: &[u8]) -> Result<Value, ItemRefusal> {
        let available = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        let (value, state) = crate::record_biblio_cut::bounded_decoded_state(
            raw,
            limits(),
            available,
            self.limits.deadline,
            self.cancelled,
        )?;
        self.temporary(state)?;
        Ok(value)
    }
    fn ordered_value(&mut self, raw: &[u8]) -> Result<JsonValue, ItemRefusal> {
        let available = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        let value = crate::record_biblio_cut::bounded_ordered(
            raw,
            limits(),
            available,
            self.limits.deadline,
            self.cancelled,
        )?;
        self.temporary(crate::record_biblio_cut::ordered_state(&value)?)?;
        Ok(value)
    }
    // One actual canonicalization supplies the consumed digest and byte length.
    // Counting serialization admits its buffer; no canonical result is built
    // solely to estimate another execution of the same codec pipeline.
    fn canonical_buffer(&self, value: &Value) -> Result<Vec<u8>, ItemRefusal> {
        let available = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        crate::record_biblio_cut::decoded_wire_size(
            value,
            available.saturating_sub(std::mem::size_of::<Vec<u8>>()),
        )?;
        let raw = serde_json::to_vec(value).map_err(|_| bad("serialization"))?;
        let raw_state = raw
            .len()
            .checked_add(std::mem::size_of::<Vec<u8>>())
            .ok_or(ItemRefusal::Budget)?;
        let remaining = available
            .checked_sub(raw_state)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "compound canonical input workspace",
                used: Some(raw_state as u64),
                limit: Some(available as u64),
            })?;
        let tree = crate::record_biblio_cut::bounded_ordered(
            &raw,
            limits(),
            remaining,
            self.limits.deadline,
            self.cancelled,
        )?;
        let parse_peak = raw_state
            .checked_add(crate::record_biblio_cut::ordered_codec_state(&tree)?)
            .ok_or(ItemRefusal::Budget)?;
        let emit_base = raw_state
            .checked_add(crate::record_biblio_cut::ordered_state(&tree)?)
            .and_then(|n| n.checked_add(crate::record_biblio_cut::ordered_emit_state(&tree).ok()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<u8>>()))
            .ok_or(ItemRefusal::Budget)?;
        let room = available
            .checked_sub(emit_base)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "compound canonical emit indexes",
                used: Some(emit_base as u64),
                limit: Some(available as u64),
            })?;
        let mut emission = limits();
        emission.max_bytes = emission.max_bytes.min(room);
        let output = canonical_bytes_v1(&tree, CanonicalProfile::SourceCommandInputV1, emission)
            .map_err(|error| {
                if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                    ItemRefusal::BudgetCheck {
                        check: "compound canonical output workspace",
                        used: None,
                        limit: Some(room as u64),
                    }
                } else {
                    ItemRefusal::Unsupported(format!("compound canonical: {error:?}"))
                }
            })?;
        let peak = parse_peak.max(
            emit_base
                .checked_add(output.len())
                .ok_or(ItemRefusal::Budget)?,
        );
        if peak > available {
            return Err(ItemRefusal::BudgetCheck {
                check: "compound canonical codec workspace",
                used: Some(peak as u64),
                limit: Some(available as u64),
            });
        }
        check(self.limits.deadline, self.cancelled)?;
        Ok(output)
    }
    fn canonical_observation(&self, value: &Value) -> Result<(String, usize), ItemRefusal> {
        let output = self.canonical_buffer(value)?;
        Ok((Digest256::of_bytes(&output).to_prefixed(), output.len()))
    }
    fn ordered_buffer(&mut self, value: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
        let available = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        let base =
            std::mem::size_of::<Vec<u8>>() + crate::record_biblio_cut::ordered_emit_state(value)?;
        let room = available.checked_sub(base).ok_or(ItemRefusal::Budget)?;
        let mut emit = limits();
        emit.max_bytes = emit.max_bytes.min(room);
        let bytes = canonical_bytes_v1(value, CanonicalProfile::SourceCommandInputV1, emit)
            .map_err(|error| {
                if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                    ItemRefusal::BudgetCheck {
                        check: "membership ordered canonical buffer",
                        used: None,
                        limit: Some(room as u64),
                    }
                } else {
                    bad("membership ordered canonical buffer")
                }
            })?;
        self.buffer(bytes)
    }
    fn advance_claim(&mut self, previous: &Value, request: &Value) -> Result<Value, ItemRefusal> {
        // One previous clone and the exact incoming patch can coexist during
        // replacement. Removed subtrees are released after the real mutation.
        let admission = crate::record_biblio_cut::decoded_state(previous)?
            .checked_add(crate::record_biblio_cut::decoded_state(&request["fields"])?)
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(admission)?;
        let result = advance_membership_claim(previous, request)?;
        let retained = crate::record_biblio_cut::decoded_state(&result)?;
        if retained > admission {
            return Err(bad("membership correction shape accounting"));
        }
        self.release_temporary(admission - retained);
        Ok(result)
    }
    fn package_revision(&mut self, files: &Package) -> Result<String, ItemRefusal> {
        let refs = file_refs(files);
        let tree = crate::record_biblio_cut::decoded_state(&refs)?;
        self.temporary(tree)?;
        let result = self.canonical_observation(&refs).map(|(digest, _)| digest);
        drop(refs);
        self.release_temporary(tree);
        result
    }
    fn reference_matches(
        &mut self,
        value: &Value,
        id: &str,
        version: &str,
        expected: &Value,
    ) -> Result<bool, ItemRefusal> {
        let n = integer(value, version)?;
        if n == 0 {
            return Err(bad("positive record version"));
        }
        let (digest, _) = self.canonical_observation(value)?;
        let reference = json!({"id":text(value,id)?,"version":n,"digest":digest});
        let tree = crate::record_biblio_cut::decoded_state(&reference)?;
        self.temporary(tree)?;
        let result = &reference == expected;
        drop(reference);
        self.release_temporary(tree);
        Ok(result)
    }
    fn value_copy(&mut self, value: &Value) -> Result<Value, ItemRefusal> {
        self.temporary(crate::record_biblio_cut::decoded_state(value)?)?;
        Ok(value.clone())
    }
    fn ordered_copy(&mut self, value: &JsonValue) -> Result<JsonValue, ItemRefusal> {
        self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        Ok(value.clone())
    }
    fn buffer(&mut self, raw: Vec<u8>) -> Result<Vec<u8>, ItemRefusal> {
        self.temporary(
            std::mem::size_of::<Vec<u8>>()
                .checked_add(raw.len())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        Ok(raw)
    }
    fn optional(&mut self, path: &str, cap: usize) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        if let Some(raw) = self.raw.get(path) {
            if raw.len() > cap {
                return Err(ItemRefusal::BudgetCheck {
                    check: "compound cached member bytes",
                    used: Some(raw.len() as u64),
                    limit: Some(cap as u64),
                });
            }
            let size = raw.len() + std::mem::size_of::<Vec<u8>>();
            self.temporary(size)?;
            let copy = self
                .raw
                .get(path)
                .expect("selected cache entry remains")
                .clone();
            return Ok(Some(copy));
        }
        let raw = if let Some(input) = self.input {
            if input.path_presence(path, self.limits.deadline, self.cancelled)?
                != Some(SourcePresenceV1::File)
            {
                return Ok(None);
            }
            // Bound the provider read before IO by this same phase's remaining
            // bytes. Cached buffers above were already charged when first read.
            let remaining = self
                .limits
                .max_total_bytes
                .checked_sub(self.bytes)
                .ok_or(ItemRefusal::Budget)?;
            let read_cap = usize::try_from(
                remaining.min(u64::try_from(cap).map_err(|_| ItemRefusal::Budget)?),
            )
            .map_err(|_| ItemRefusal::Budget)?;
            let mut selected = None;
            let state_before = self.state;
            let state_limit = self.limits.max_state_bytes;
            input.with_current_member(
                path,
                read_cap,
                self.limits.deadline,
                self.cancelled,
                &mut |meta: SourceCutMemberMeta<'_>, bytes| {
                    let copied_state = bytes
                        .len()
                        .checked_add(std::mem::size_of::<Vec<u8>>())
                        .ok_or(ItemRefusal::Budget)?;
                    if bytes.len() > read_cap || meta.size_bytes != bytes.len() as u64 {
                        return Err(bad("compound candidate member size"));
                    }
                    if state_before
                        .checked_add(copied_state)
                        .is_none_or(|used| used > state_limit)
                    {
                        return Err(ItemRefusal::BudgetCheck {
                            check: "compound candidate member copy",
                            used: state_before
                                .checked_add(copied_state)
                                .map(|used| used as u64),
                            limit: Some(state_limit as u64),
                        });
                    }
                    selected = Some(bytes.to_vec());
                    Ok(())
                },
            )?;
            let raw = selected.ok_or_else(|| bad("compound candidate member disappeared"))?;
            account(&mut self.bytes, raw.len(), self.limits.max_total_bytes)?;
            raw
        } else {
            if !self.paths.contains(path) {
                return Ok(None);
            }
            current(
                self.cut.ok_or_else(|| bad("compound corpus cut absent"))?,
                path,
                self.limits,
                self.cancelled,
                &mut self.bytes,
            )?
        };
        if raw.len() > cap {
            return Err(ItemRefusal::BudgetCheck {
                check: "compound selected member bytes",
                used: Some(raw.len() as u64),
                limit: Some(cap as u64),
            });
        }
        // One retained cache buffer, one owned map key and one entry slot.
        // The returned buffer is a distinct scoped temporary, never a third copy.
        let raw_state = raw
            .len()
            .checked_add(path.len())
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, Vec<u8>)>()))
            .ok_or(ItemRefusal::Budget)?;
        let cache_state = self
            .raw_cache_state
            .checked_add(raw_state)
            .ok_or(ItemRefusal::Budget)?;
        reserve(&mut self.state, raw_state, self.limits.max_state_bytes)?;
        self.raw_cache_state = cache_state;
        self.temporary(raw.len() + std::mem::size_of::<Vec<u8>>())?;
        self.record_read(PredicateRead::ExactPath {
            path: path.into(),
            digest: Digest256::of_bytes(&raw).to_prefixed(),
        })?;
        self.raw.insert(path.into(), raw.clone());
        Ok(Some(raw))
    }
    fn release_raw_cache(&mut self) {
        // Called after a verification phase or a prepared Work core no longer
        // needs selected reads. Owned package buffers, decoded histories and
        // read observations retain their separate charges.
        self.raw = BTreeMap::new();
        self.state -= self.raw_cache_state;
        self.raw_cache_state = 0;
    }
    fn required(&mut self, path: &str, cap: usize) -> Result<Vec<u8>, ItemRefusal> {
        self.optional(path, cap)?
            .ok_or_else(|| bad(&format!("missing selected member {path}")))
    }
    fn current_digest_size(&mut self, path: &str) -> Result<(Digest256, u64), ItemRefusal> {
        if let Some(cut) = self.cut {
            let relative =
                RelativePath::parse(path).map_err(|_| bad("compound current member path"))?;
            let member = cut
                .current()
                .member(&relative)
                .ok_or_else(|| bad("compound current member absent"))?;
            return Ok((member.sha256, member.size_bytes));
        }
        let raw = self.required(path, MAX_SIDE)?;
        let size = u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?;
        let digest = Digest256::of_bytes(&raw);
        let raw_state = raw
            .len()
            .checked_add(std::mem::size_of::<Vec<u8>>())
            .ok_or(ItemRefusal::Budget)?;
        drop(raw);
        self.release_temporary(raw_state);
        Ok((digest, size))
    }
    /// Select only the names under one exact source directory. Cut-backed
    /// history keeps its existing borrowed full-cut index; candidate-backed
    /// history obtains the same closure from the indexed prefix cursor and
    /// retains no corpus-wide path map.
    fn current_member_names_under(
        &mut self,
        directory: &str,
        max_entries: usize,
    ) -> Result<BTreeSet<String>, ItemRefusal> {
        let parsed =
            RelativePath::parse(directory).map_err(|_| bad("compound current directory path"))?;
        if parsed.as_str() != directory || max_entries == 0 {
            return Err(bad("compound canonical current directory"));
        }
        let prefix_bytes = directory
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(prefix_bytes)?;
        let prefix = format!("{directory}/");
        let state_before_rows = self.state;
        let state_limit = self.limits.max_state_bytes;
        let mut retained = 0usize;
        let mut names = BTreeSet::new();
        let mut insert_name = |path: &str| -> Result<(), ItemRefusal> {
            let name = path
                .strip_prefix(&prefix)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| bad("compound current directory range path"))?;
            if names.len() >= max_entries {
                return Err(ItemRefusal::BudgetCheck {
                    check: "compound current directory member count",
                    used: Some((names.len() as u64).saturating_add(1)),
                    limit: Some(max_entries as u64),
                });
            }
            let next_retained = retained
                .checked_add(name.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<String>() + 128))
                .ok_or(ItemRefusal::Budget)?;
            if state_before_rows
                .checked_add(next_retained)
                .is_none_or(|used| used > state_limit)
            {
                return Err(ItemRefusal::BudgetCheck {
                    check: "compound current directory name state",
                    used: state_before_rows
                        .checked_add(next_retained)
                        .map(|used| used as u64),
                    limit: Some(state_limit as u64),
                });
            }
            if !names.insert(name.to_owned()) {
                return Err(bad("compound current directory duplicate path"));
            }
            retained = next_retained;
            Ok(())
        };
        if let Some(input) = self.input {
            let mut callback_count = 0u64;
            let coverage = input.for_each_current_member_meta_under(
                directory,
                self.limits.deadline,
                self.cancelled,
                &mut |meta| {
                    callback_count = callback_count.checked_add(1).ok_or(ItemRefusal::Budget)?;
                    insert_name(meta.path)
                },
            )?;
            if coverage.directory() != directory
                || coverage.member_count() != callback_count
                || names.len() as u64 != callback_count
            {
                return Err(bad("compound current directory coverage differs"));
            }
        } else {
            for path in self
                .paths
                .range::<str, _>((
                    std::ops::Bound::Included(prefix.as_str()),
                    std::ops::Bound::Unbounded,
                ))
                .take_while(|path| path.starts_with(&prefix))
            {
                insert_name(path)?;
            }
        }
        self.temporary(retained)?;
        self.release_temporary(prefix_bytes);
        Ok(names)
    }
    fn record_read(&mut self, read: PredicateRead) -> Result<(), ItemRefusal> {
        reserve(
            &mut self.state,
            crate::record_biblio_cut::predicate_state(&read)?,
            self.limits.max_state_bytes,
        )?;
        self.reads.push(read);
        Ok(())
    }
    fn selected(&mut self, path: &str) -> Result<Package, ItemRefusal> {
        metadata_path(path, false)?;
        let home = parent(path)?;
        let mut files = Package::new();
        let mut total = 0;
        for name in selected_names(path)? {
            if let Some(raw) = self.optional(&format!("{home}/{name}"), MAX_FILE)? {
                total += raw.len();
                if total > MAX_SIDE {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "compound selected package bytes",
                        used: Some(total as u64),
                        limit: Some(MAX_SIDE as u64),
                    });
                }
                files.insert(name, raw);
            }
        }
        if !files.contains_key(path.rsplit('/').next().unwrap_or("")) {
            return Err(bad("selected record absent"));
        }
        Ok(files)
    }
    fn transaction(&mut self, id: &str) -> Result<std::sync::Arc<Transaction>, ItemRefusal> {
        let before = self.temporary_state;
        let result = self.transaction_inner(id);
        self.release_temporary_since(before);
        if let Ok(tx) = &result {
            if !self.transactions.contains_key(id) {
                let mut retained = std::mem::size_of::<Transaction>()
                    + std::mem::size_of::<(String, std::sync::Arc<Transaction>)>()
                    + 2 * std::mem::size_of::<usize>()
                    + id.len()
                    + tx.manifest_sha256.len()
                    + tx.status.len();
                retained = retained
                    .checked_add(
                        crate::record_biblio_cut::decoded_state(&tx.manifest)?
                            .checked_sub(std::mem::size_of::<Value>())
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
                for (path, (before, after)) in &tx.files {
                    retained = retained
                        .checked_add(
                            std::mem::size_of::<(String, (Option<Vec<u8>>, Option<Vec<u8>>))>()
                                + path.len(),
                        )
                        .and_then(|n| n.checked_add(before.as_ref().map_or(0, Vec::len)))
                        .and_then(|n| n.checked_add(after.as_ref().map_or(0, Vec::len)))
                        .ok_or(ItemRefusal::Budget)?;
                }
                reserve(&mut self.state, retained, self.limits.max_state_bytes)?;
                self.transactions.insert(id.into(), tx.clone());
            }
        }
        result
    }
    fn transaction_inner(&mut self, id: &str) -> Result<std::sync::Arc<Transaction>, ItemRefusal> {
        hash(id)?;
        if let Some(tx) = self.transactions.get(id) {
            return Ok(tx.clone());
        }
        let directory = format!("{TRANSACTIONS}/{}", hash(id)?);
        let raw = self.required(&format!("{directory}/manifest.json"), MAX_MANIFEST)?;
        let manifest = self.decoded(&raw)?;
        keys(
            &manifest,
            &[
                "schema_version",
                "transaction_id",
                "base_publication",
                "plan",
                "parents",
            ],
        )?;
        if text(&manifest, "transaction_id")? != id {
            return Err(bad("native bibliographic transaction grammar"));
        }
        let base = &manifest["base_publication"];
        keys(base, &["token", "generation"])?;
        let generation = integer(base, "generation")?;
        if generation > MAX_GENERATION - 2 || base["token"].is_null() != (generation == 0) {
            return Err(bad("publication predecessor"));
        }
        if !base["token"].is_null() {
            hash(text(base, "token")?)?;
        }
        let plan = &manifest["plan"];
        let companion_home = if let Some(profile) = plan.get("path_profile") {
            keys(
                plan,
                &["authorization", "files", "new_directories", "path_profile"],
            )?;
            keys(profile, &["schema_version", "item_source_path"])?;
            let path = text(profile, "item_source_path")?;
            metadata_path(path, false)?;
            if text(profile, "schema_version")? != "tos_item_metadata_paths_v1"
                || !path.ends_with("/item.json")
                || !parent(path)?
                    .rsplit_once('/')
                    .is_some_and(|(p, _)| p.ends_with("/items"))
                || plan["authorization"]["schema_version"] != "tos_item_adoption_authorization_v1"
                || plan["authorization"]["scope"]["item_source_path"] != path
            {
                return Err(bad("exact Item transaction path profile"));
            }
            Some(parent(path)?.to_owned())
        } else {
            keys(plan, &["authorization", "files", "new_directories"])?;
            None
        };
        if text(&manifest, "schema_version")?
            != if companion_home.is_some() {
                "tos_selected_metadata_transaction_v2"
            } else {
                "tos_selected_metadata_transaction_v1"
            }
        {
            return Err(bad("transaction version/path profile mismatch"));
        }
        if !plan["authorization"].is_object() {
            return Err(bad("bounded authorization"));
        }
        let oversized = self.canonical_observation(&plan["authorization"])?.1 > 65536;
        if oversized {
            return Err(bad("bounded authorization"));
        }
        let directories = array(plan, "new_directories")?;
        if directories.len() > 64 {
            return Err(ItemRefusal::BudgetCheck {
                check: "compound new directory count",
                used: Some(directories.len() as u64),
                limit: Some(64),
            });
        }
        let dirs: Vec<_> = directories
            .iter()
            .map(|v| v.as_str().ok_or_else(|| bad("new directory")))
            .collect::<Result<_, _>>()?;
        // Three simultaneously live borrowed directory indexes: source Vec,
        // sorted Vec and uniqueness set; none owns another path payload.
        self.temporary(
            2 * std::mem::size_of::<Vec<&str>>()
                + std::mem::size_of::<BTreeSet<&&str>>()
                + dirs.len() * (2 * std::mem::size_of::<&str>() + std::mem::size_of::<&&str>()),
        )?;
        let mut sorted_dirs = dirs.clone();
        sorted_dirs.sort_by_key(|v| (v.split('/').count(), *v));
        if dirs != sorted_dirs || dirs.iter().collect::<BTreeSet<_>>().len() != dirs.len() {
            return Err(bad("new directory order/uniqueness"));
        }
        for d in &dirs {
            metadata_path(d, true)?;
        }
        let rows = array(plan, "files")?;
        if !(1..=64).contains(&rows.len()) {
            return Err(ItemRefusal::BudgetCheck {
                check: "compound plan file count (minimum 1)",
                used: Some(rows.len() as u64),
                limit: Some(64),
            });
        }
        let mut files = BTreeMap::new();
        let mut blobs = BTreeMap::<String, Vec<u8>>::new();
        let mut sides = [0usize; 2];
        let mut total = 0usize;
        let mut changed = false;
        let mut last = "";
        for row in rows {
            check(self.limits.deadline, self.cancelled)?;
            keys(row, &["path", "before", "after"])?;
            let path = text(row, "path")?;
            if companion_home.as_ref().is_some_and(|home| {
                path == format!("{home}/fixity.sha256")
                    || path == format!("{home}/forensic-report.md")
            }) {
                // The home itself passed the exact public metadata path law.
            } else {
                metadata_path(path, false)?;
            }
            if path <= last {
                return Err(bad("file path order/uniqueness"));
            }
            last = path;
            if row["before"].is_null() && row["after"].is_null() {
                return Err(bad("absent to absent file"));
            }
            changed |= row["before"] != row["after"];
            let mut bytes = [None, None];
            for (i, side) in ["before", "after"].iter().enumerate() {
                let binding = &row[*side];
                if binding.is_null() {
                    continue;
                }
                keys(binding, &["sha256", "bytes"])?;
                let sha = text(binding, "sha256")?;
                hash(sha)?;
                let size =
                    usize::try_from(integer(binding, "bytes")?).map_err(|_| ItemRefusal::Budget)?;
                sides[i] = sides[i].checked_add(size).ok_or(ItemRefusal::Budget)?;
                if size > MAX_SIDE || sides[i] > MAX_SIDE {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "compound transaction side bytes",
                        used: Some(sides[i].max(size) as u64),
                        limit: Some(MAX_SIDE as u64),
                    });
                }
                if !blobs.contains_key(sha) {
                    total = total.checked_add(size).ok_or(ItemRefusal::Budget)?;
                    if total > 2 * MAX_SIDE {
                        return Err(ItemRefusal::BudgetCheck {
                            check: "compound unique transaction blob bytes",
                            used: Some(total as u64),
                            limit: Some((2 * MAX_SIDE) as u64),
                        });
                    }
                    let raw = self.required(&format!("{directory}/{}.blob", hash(sha)?), size)?;
                    if raw.len() != size || Digest256::of_bytes(&raw).to_prefixed() != sha {
                        return Err(bad("transaction blob fixity"));
                    }
                    self.temporary(std::mem::size_of::<(String, Vec<u8>)>() + sha.len())?;
                    blobs.insert(sha.into(), raw);
                }
                // One retained file buffer; the returned transaction shares it.
                self.temporary(size)?;
                bytes[i] = Some(blobs[sha].clone());
            }
            files.insert(path.into(), (bytes[0].take(), bytes[1].take()));
        }
        if !changed {
            return Err(bad("transaction has no change"));
        }
        for a in files.keys() {
            for b in files.keys().map(String::as_str).chain(dirs.iter().copied()) {
                if a.as_str() != b && is_ancestor(a, b) {
                    return Err(bad("target ancestor collision"));
                }
            }
        }
        for d in &dirs {
            if !files
                .iter()
                .any(|(p, (before, _))| is_ancestor(d, p) && before.is_none())
            {
                return Err(bad("new directory lacks new file"));
            }
        }
        let mut parents = BTreeSet::from([HOME.to_owned()]);
        self.temporary(std::mem::size_of::<String>() + HOME.len())?;
        for p in files.keys().map(String::as_str).chain(dirs.iter().copied()) {
            let mut p = parent(p)?;
            while p == HOME || p.starts_with(&format!("{HOME}/")) {
                if !parents.contains(p) {
                    self.temporary(std::mem::size_of::<String>() + p.len())?;
                }
                parents.insert(p.into());
                if p == HOME {
                    break;
                }
                p = parent(p)?;
            }
        }
        let bindings = manifest["parents"]
            .as_object()
            .ok_or_else(|| bad("parent closure"))?;
        if bindings.keys().cloned().collect::<BTreeSet<_>>() != parents {
            return Err(bad("parent directory closure"));
        }
        for (p, binding) in bindings {
            if binding.is_null() {
                if !dirs.contains(&p.as_str()) {
                    return Err(bad("undeclared absent parent"));
                }
            } else {
                keys(binding, &["device", "inode", "mode", "uid"])?;
                for k in ["device", "inode", "mode", "uid"] {
                    integer(binding, k)?;
                }
                let mode = integer(binding, "mode")?;
                if mode & 0o170000 != 0o040000 || mode & 0o022 != 0 {
                    return Err(bad("historical directory posture"));
                }
            }
        }
        let sha = Digest256::of_bytes(&raw).to_prefixed();
        let completion = match self.optional(&format!("{directory}/completion.json"), 8192)? {
            Some(raw) => {
                let v = self.decoded(&raw)?;
                keys(&v, &["schema_version", "publication"])?;
                if text(&v, "schema_version")? != "tos_selected_metadata_completion_v1" {
                    return Err(bad("completion schema"));
                }
                let scratch = crate::record_biblio_cut::decoded_state(&v["publication"])?;
                self.temporary(scratch)?;
                state_valid_with(&v["publication"], &mut |value| {
                    self.canonical_observation(value)
                })?;
                self.release_temporary(scratch);
                let s = &v["publication"];
                if text(s, "phase")? != "ready"
                    || text(s, "transaction_id")? != id
                    || text(s, "manifest_sha256")? != sha
                    || integer(s, "generation")? != generation + 2
                {
                    return Err(bad("terminal completion binding"));
                }
                Some(self.value_copy(s)?)
            }
            None => None,
        };
        let selected = self
            .publication
            .as_ref()
            .is_some_and(|s| s["transaction_id"] == id);
        let status = if selected {
            let s = self.publication.as_ref().unwrap();
            if text(s, "manifest_sha256")? != sha {
                return Err(bad("current terminal transaction drift"));
            }
            if text(s, "phase")? == "pending" {
                if integer(s, "generation")? != generation + 1 || completion.is_some() {
                    return Err(bad("exact pending transaction binding"));
                }
                "pending".into()
            } else {
                if integer(s, "generation")? != generation + 2
                    || completion.as_ref().is_some_and(|c| c != s)
                {
                    return Err(bad("terminal generation/completion drift"));
                }
                text(s, "outcome")?.into()
            }
        } else {
            completion
                .as_ref()
                .map(|s| text(s, "outcome").map(str::to_owned))
                .transpose()?
                .unwrap_or("orphan".into())
        };
        self.temporary(
            files
                .keys()
                .try_fold(0usize, |sum, path| {
                    sum.checked_add(path.len())?
                        .checked_add(std::mem::size_of::<(
                            String,
                            (Option<Vec<u8>>, Option<Vec<u8>>),
                        )>())
                })
                .ok_or(ItemRefusal::Budget)?,
        )?;
        self.temporary(
            id.len()
                + std::mem::size_of::<(String, std::sync::Arc<Transaction>)>()
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<Transaction>()
                + sha.len()
                + status.len(),
        )?;
        let tx = std::sync::Arc::new(Transaction {
            manifest,
            manifest_sha256: sha,
            status,
            files,
        });
        Ok(tx)
    }
    fn archive(&mut self, path: &str, id: &str, receipt: &Value) -> Result<Package, ItemRefusal> {
        self.archive_typed(path, id, receipt, "record_id")
    }
    fn archive_typed(
        &mut self,
        path: &str,
        id: &str,
        receipt: &Value,
        identity_field: &str,
    ) -> Result<Package, ItemRefusal> {
        let before = self.temporary_state;
        let result = self.archive_files_inner(path, id, receipt, Some(identity_field));
        self.release_temporary_since(before);
        if let Ok(files) = &result {
            self.temporary(package_state(files)?)?;
        }
        result
    }
    fn archive_files_inner(
        &mut self,
        path: &str,
        id: &str,
        receipt: &Value,
        identity_field: Option<&str>,
    ) -> Result<Package, ItemRefusal> {
        let rev = text(receipt, "previous_revision")?;
        let home = format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(id.as_bytes()).to_hex(),
            hash(rev)?
        );
        if text(receipt, "archive_path")? != home {
            return Err(bad("archive exact locator"));
        }
        let raw = self.required(&format!("{home}/manifest.json"), MAX_FILE)?;
        let manifest = self.decoded(&raw)?;
        let v2 = text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
        let mut wanted = vec![
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ];
        // V1 Claim corrections bind a flat archive without a record basename;
        // selected V2 metadata archives retain their three-file scope.
        if v2 {
            wanted.push("publication_protocol");
        }
        keys(&manifest, &wanted)?;
        if !matches!(
            text(&manifest, "schema_version")?,
            "tos_source_package_archive_v1" | "tos_source_package_archive_v2"
        ) || v2 && text(&manifest, "publication_protocol")? != PROTOCOL
            || text(&manifest, "source_path")? != path
            || manifest["source"] != receipt["previous_source"]
            || manifest["revision"] != receipt["previous_revision"]
        {
            return Err(bad("archive metadata binding"));
        }
        let bindings = manifest["files"]
            .as_object()
            .ok_or_else(|| bad("archive files"))?;
        if bindings.len() > 64 {
            return Err(ItemRefusal::BudgetCheck {
                check: "compound archived package file count",
                used: Some(bindings.len() as u64),
                limit: Some(64),
            });
        }
        let mut files = Package::new();
        let mut expected = BTreeSet::from(["manifest.json".to_owned()]);
        let mut total = raw.len();
        for (name, b) in bindings {
            check(self.limits.deadline, self.cancelled)?;
            keys(b, &["blob", "sha256", "bytes"])?;
            if name.is_empty() || name.contains('/') || matches!(name.as_str(), "." | "..") {
                return Err(bad("flat archived filename"));
            }
            let sha = text(b, "sha256")?;
            let blob = format!("{}.blob", hash(sha)?);
            if text(b, "blob")? != blob {
                return Err(bad("archive blob name"));
            }
            let size = usize::try_from(integer(b, "bytes")?).map_err(|_| ItemRefusal::Budget)?;
            let raw = self.required(&format!("{home}/{blob}"), size.min(MAX_FILE))?;
            total = total.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?;
            if total > MAX_SIDE + MAX_FILE {
                return Err(ItemRefusal::BudgetCheck {
                    check: "compound archived package plus manifest bytes",
                    used: Some(total as u64),
                    limit: Some((MAX_SIDE + MAX_FILE) as u64),
                });
            }
            if raw.len() != size || Digest256::of_bytes(&raw).to_prefixed() != sha {
                return Err(bad("archive exact blob bytes"));
            }
            self.temporary(
                std::mem::size_of::<(String, Vec<u8>)>()
                    + name.len()
                    + std::mem::size_of::<String>()
                    + blob.len(),
            )?;
            files.insert(name.clone(), raw);
            expected.insert(blob);
        }
        let actual = self.current_member_names_under(&home, 65)?;
        self.temporary(
            actual
                .iter()
                .try_fold(0usize, |n, p: &String| {
                    n.checked_add(std::mem::size_of::<String>() + p.len())
                })
                .ok_or(ItemRefusal::Budget)?,
        )?;
        if actual != expected {
            return Err(bad("archive extra/nested/unbound files"));
        }
        if v2 {
            let names = selected_names(path)?;
            if files.keys().any(|n| !names.contains(n)) || !files.contains_key(&names[0]) {
                return Err(bad("selected archive package scope"));
            }
        }
        if self.package_revision(&files)? != rev {
            return Err(bad("archive package revision"));
        }
        let Some(identity_field) = identity_field else {
            return Ok(files);
        };
        let names = selected_names(path)?;
        let old = self.decoded(
            files
                .get(&names[0])
                .ok_or_else(|| bad("archive source missing"))?,
        )?;
        if !self.reference_matches(
            &old,
            identity_field,
            "record_version",
            &receipt["previous_source"],
        )? {
            return Err(bad("archive previous source"));
        }
        if let Some(request) = receipt.get("request") {
            let mut revised = self.value_copy(&old)?;
            let object = revised
                .as_object_mut()
                .ok_or_else(|| bad("source object"))?;
            for (k, v) in request["fields"]
                .as_object()
                .ok_or_else(|| bad("revision fields"))?
            {
                object.insert(k.clone(), v.clone());
            }
            object.insert(
                "record_version".into(),
                json!(
                    integer(&old, "record_version")?
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?
                ),
            );
            self.temporary(
                crate::record_biblio_cut::decoded_state(&revised)?
                    .saturating_sub(crate::record_biblio_cut::decoded_state(&old)?),
            )?;
            if !self.reference_matches(
                &revised,
                identity_field,
                "record_version",
                &receipt["source"],
            )? {
                return Err(bad("retained request successor"));
            }
        }
        Ok(files)
    }
    fn flat_package(&mut self, home: &str) -> Result<Package, ItemRefusal> {
        let names = self.current_member_names_under(home, 64)?;
        if !(1..=64).contains(&names.len()) || names.iter().any(|name| name.contains('/')) {
            return Err(bad("membership flat Claim package count"));
        }
        let mut result = Package::new();
        self.temporary(std::mem::size_of::<Package>())?;
        let mut total = 0usize;
        for name in names {
            let raw = self.required(&format!("{home}/{name}"), MAX_FILE)?;
            total = total.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?;
            if total > MAX_SIDE {
                return Err(ItemRefusal::BudgetCheck {
                    check: "membership current Claim package bytes",
                    used: Some(total as u64),
                    limit: Some(MAX_SIDE as u64),
                });
            }
            self.temporary(std::mem::size_of::<(String, Vec<u8>)>() + name.len())?;
            result.insert(name.into(), raw);
        }
        Ok(result)
    }
    fn single_membership_claim(&mut self, raw: &[u8], id: &str) -> Result<Value, ItemRefusal> {
        if raw.len() > 1_048_576 {
            return Err(ItemRefusal::BudgetCheck {
                check: "membership correction stream bytes",
                used: Some(raw.len() as u64),
                limit: Some(1_048_576),
            });
        }
        let mut result = None;
        for (line, _) in claim_lines(raw) {
            check(self.limits.deadline, self.cancelled)?;
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            if result.is_some() {
                return Err(bad("membership current stream has another Claim"));
            }
            let value = self.decoded(line)?;
            if text(&value, "claim_id")? != id {
                return Err(bad("membership current stream Claim identity"));
            }
            result = Some(value);
        }
        result.ok_or_else(|| bad("membership Claim stream empty"))
    }
    fn attachment_claim_initial(
        &mut self,
        path: &str,
        claim: &Value,
    ) -> Result<Vec<u8>, ItemRefusal> {
        use crate::source_forms::source_copy_kernel as kernel;
        let files = self.flat_package(parent(path)?)?;
        let id = text(claim, "claim_id")?;
        let raw = files
            .get("source-claims.jsonl")
            .ok_or_else(|| bad("membership current stream missing"))?;
        let current = self.single_membership_claim(raw, id)?;
        if &current != claim {
            return Err(bad("membership current stream differs from observed Claim"));
        }
        let history = match files.get("claim-revision-history.json") {
            Some(raw) => self.decoded(raw)?,
            None => {
                let value = json!({"schema_version":"tos_claim_revision_history_v1","source_path":path,"receipts":[]});
                self.temporary(crate::record_biblio_cut::decoded_state(&value)?)?;
                value
            }
        };
        keys(&history, &["schema_version", "source_path", "receipts"])?;
        let receipts = array(&history, "receipts")?;
        if history["schema_version"] != "tos_claim_revision_history_v1"
            || history["source_path"] != path
            || receipts.len() > MAX_HISTORY
        {
            return Err(bad("membership correction history grammar"));
        }
        if receipts.is_empty() {
            if current["claim_version"] != 1 {
                return Err(bad("membership noninitial Claim lacks history"));
            }
            self.temporary(std::mem::size_of::<Vec<u8>>() + raw.len())?;
            return Ok(raw.clone());
        }
        let formname = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(id.as_bytes()).to_hex()
        );
        self.temporary(std::mem::size_of::<String>() + formname.len())?;
        let current_forms = self.ordered_value(
            files
                .get(&formname)
                .ok_or_else(|| bad("membership current correction forms absent"))?,
        )?;
        let subject = current_forms
            .object_get("subject")
            .ok_or_else(|| bad("membership correction form subject"))?;
        let form_rows = current_forms
            .object_get("forms")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| bad("membership current forms"))?;
        let prior_rows = current_forms
            .object_get("prior_forms")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| bad("membership prior forms"))?;
        let history_scratch = (form_rows.len() + prior_rows.len())
            * std::mem::size_of::<((&str, u64), &JsonValue)>()
            + form_rows.len() * std::mem::size_of::<&str>();
        self.temporary(history_scratch)?;
        kernel::validate_history(&current_forms, subject)
            .map_err(|_| bad("membership correction form history"))?;
        self.release_temporary(history_scratch);
        let mut commands = BTreeSet::new();
        self.temporary(
            std::mem::size_of::<BTreeSet<&str>>() + receipts.len() * std::mem::size_of::<&str>(),
        )?;
        let mut expected: Option<Vec<u8>> = None;
        let mut initial = None;
        for receipt in receipts {
            check(self.limits.deadline, self.cancelled)?;
            keys(
                receipt,
                &[
                    "command_id",
                    "request_digest",
                    "principal_id",
                    "authority_ref",
                    "owner_configuration",
                    "recorded_at",
                    "reason",
                    "previous_source",
                    "source",
                    "previous_revision",
                    "archive_path",
                    "dependencies",
                    "source_bindings",
                    "changed_fields",
                    "forms",
                    "grants_admission",
                    "request",
                ],
            )?;
            let request = &receipt["request"];
            crate::retirement_rules::observed_instant_order(
                text(receipt, "recorded_at")?,
                text(receipt, "recorded_at")?,
            )
            .map_err(|_| bad("membership correction aware instant"))?;
            if !commands.insert(text(receipt, "command_id")?)
                || request["operation"] != "claim.revise"
                || text(receipt, "request_digest")? != self.canonical_observation(request)?.0
                || receipt["grants_admission"] != false
                || receipt["source"]["id"] != receipt["previous_source"]["id"]
                || integer(&receipt["source"], "version")?
                    != integer(&receipt["previous_source"], "version")?
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?
            {
                return Err(bad("membership correction receipt binding"));
            }
            for (left, right) in [
                ("command_id", "command_id"),
                ("previous_source", "expected_source"),
                ("previous_revision", "expected_revision"),
                ("owner_configuration", "expected_configuration"),
                ("dependencies", "expected_dependencies"),
                ("source_bindings", "expected_inputs"),
                ("reason", "reason"),
            ] {
                if receipt[left] != request[right] {
                    return Err(bad("membership correction request binding"));
                }
            }
            let fields = request["fields"]
                .as_object()
                .ok_or_else(|| bad("membership correction fields"))?;
            let changed = array(receipt, "changed_fields")?;
            if changed.len() != fields.len()
                || !changed
                    .iter()
                    .zip(fields.keys())
                    .all(|(value, key)| value.as_str() == Some(key.as_str()))
            {
                return Err(bad("membership correction changed fields"));
            }
            let previous_survivors = initial
                .as_ref()
                .map_or(0, |v: &Vec<u8>| std::mem::size_of::<Vec<u8>>() + v.len())
                + expected
                    .as_ref()
                    .map_or(0, |v| std::mem::size_of::<Vec<u8>>() + v.len());
            let phase = self
                .temporary_state
                .checked_sub(previous_survivors)
                .ok_or(ItemRefusal::Budget)?;
            let archived = self.archive_files_inner(
                path,
                text(&receipt["previous_source"], "id")?,
                receipt,
                None,
            )?;
            let before = archived
                .get("source-claims.jsonl")
                .ok_or_else(|| bad("membership correction archived stream"))?;
            let previous = self.single_membership_claim(before, id)?;
            if initial.is_none() {
                if previous["claim_version"] != 1 {
                    return Err(bad("membership correction lacks initial stream"));
                }
                self.temporary(std::mem::size_of::<Vec<u8>>() + before.len())?;
                initial = Some(before.clone());
            }
            if expected.as_ref().is_some_and(|raw| raw != before)
                || !self.reference_matches(
                    &previous,
                    "claim_id",
                    "claim_version",
                    &receipt["previous_source"],
                )?
            {
                return Err(bad("membership correction predecessor bytes"));
            }
            let revised = self.advance_claim(&previous, request)?;
            if !self.reference_matches(&revised, "claim_id", "claim_version", &receipt["source"])? {
                return Err(bad("membership correction successor reference"));
            }
            let selections = array(request, "forms")?;
            if !(1..=32).contains(&selections.len())
                || !selections
                    .iter()
                    .any(|s| s["field_id"] == "claim.statement")
            {
                return Err(bad("membership correction statement form"));
            }
            let mut seen = BTreeSet::new();
            self.temporary(
                std::mem::size_of::<BTreeSet<&str>>()
                    + selections.len() * std::mem::size_of::<&str>(),
            )?;
            for selection in selections {
                keys(selection, &["form_id", "field_id"])?;
                if !seen.insert(text(selection, "form_id")?) {
                    return Err(bad("membership correction duplicate form selection"));
                }
            }
            let prior = archived
                .get(&formname)
                .map(|raw| self.ordered_value(raw))
                .transpose()?;
            let revised_raw = self.buffer(self.canonical_buffer(&revised)?)?;
            let revised_ordered = self.ordered_value(&revised_raw)?;
            let (result, result_refs) = forms(
                &revised_ordered,
                prior.as_ref(),
                &request["forms"],
                text(receipt, "principal_id")?,
                true,
                self.limits
                    .max_state_bytes
                    .checked_sub(self.state)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            self.temporary(
                crate::record_biblio_cut::ordered_state(&result)?
                    .checked_add(crate::record_biblio_cut::ordered_state(&result_refs)?)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            let result_refs_raw = self.ordered_buffer(&result_refs)?;
            let expected_refs_raw = self.buffer(self.canonical_buffer(&receipt["forms"])?)?;
            if result_refs_raw != expected_refs_raw {
                return Err(bad("membership correction archived form results"));
            }
            let reference_buffers = std::mem::size_of::<Vec<u8>>() * 2
                + result_refs_raw.len()
                + expected_refs_raw.len();
            drop(result_refs_raw);
            drop(expected_refs_raw);
            self.release_temporary(reference_buffers);
            for reference in array(receipt, "forms")? {
                let mut retained = false;
                for form in form_rows.iter().chain(prior_rows) {
                    let before_form = self.temporary_state;
                    let raw = self.ordered_buffer(form)?;
                    let form = self.decoded(&raw)?;
                    let matches =
                        self.reference_matches(&form, "form_id", "form_version", reference)?;
                    drop(form);
                    drop(raw);
                    self.release_temporary_since(before_form);
                    if matches {
                        retained = true;
                        break;
                    }
                }
                if !retained {
                    return Err(bad("membership correction result forms no longer retained"));
                }
            }
            let output_size = claim_lines(before)
                .try_fold(0usize, |n, (line, ending)| {
                    n.checked_add(if line.iter().all(u8::is_ascii_whitespace) {
                        line.len() + ending.len()
                    } else {
                        revised_raw.len()
                            + if ending == b"\r\n" {
                                2
                            } else if ending == b"\n" {
                                1
                            } else {
                                0
                            }
                    })
                })
                .ok_or(ItemRefusal::Budget)?;
            self.temporary(std::mem::size_of::<Vec<u8>>() + output_size)?;
            let mut output = Vec::with_capacity(output_size);
            for (line, ending) in claim_lines(before) {
                if line.iter().all(u8::is_ascii_whitespace) {
                    output.extend_from_slice(line);
                    output.extend_from_slice(ending);
                } else {
                    output.extend_from_slice(&revised_raw);
                    if ending == b"\r\n" || ending == b"\n" {
                        output.extend_from_slice(ending);
                    }
                }
            }
            if let Some(old) = expected.replace(output) {
                let cost = std::mem::size_of::<Vec<u8>>() + old.len();
                drop(old);
                self.release_temporary(cost);
            }
            // initial+expected survive the archive/reconstruction temporary phase.
            let survivors = initial
                .as_ref()
                .map_or(0, |v| std::mem::size_of::<Vec<u8>>() + v.len())
                + expected
                    .as_ref()
                    .map_or(0, |v| std::mem::size_of::<Vec<u8>>() + v.len());
            drop(archived);
            drop(previous);
            drop(revised);
            drop(prior);
            drop(revised_ordered);
            drop(result);
            drop(result_refs);
            drop(revised_raw);
            self.release_temporary_since(phase);
            self.temporary(survivors)?;
        }
        if expected.as_ref() != Some(raw) {
            return Err(bad(
                "membership current stream differs from correction head",
            ));
        }
        initial.ok_or_else(|| bad("membership initial stream absent"))
    }
    // The two maintained attachment owners resolve their existing endpoint
    // through current/continuously archived lineage; neither creates it.
    fn attachment_endpoint_binding(
        &mut self,
        scope: &Value,
        work: &Value,
        dependencies: &Value,
        kind: CompoundKind,
    ) -> Result<JsonValue, ItemRefusal> {
        let start = self.temporary_state;
        let result = self.attachment_endpoint_binding_inner(scope, work, dependencies, kind);
        self.release_temporary_since(start);
        if let Ok(value) = &result {
            self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        }
        result
    }
    fn attachment_endpoint_binding_inner(
        &mut self,
        scope: &Value,
        work: &Value,
        dependencies: &Value,
        kind: CompoundKind,
    ) -> Result<JsonValue, ItemRefusal> {
        let path = text(scope, kind.child_path())?;
        let id = text(scope, kind.child_key())?;
        let sha = text(&dependencies["catalog_and_sources"], path)?;
        let expected = Digest256::from_hex(sha)
            .map_err(|_| bad("membership exact Work dependency raw hash"))?;
        let files = self.selected(path)?;
        let current = self.decoded(&files[kind.child_file()])?;
        if text(&current, "record_id")? != id || text(&current, "record_type")? != kind.child_kind()
        {
            return Err(bad("membership Work current typed identity"));
        }
        let history = self.history(path, &files)?;
        self.temporary(std::mem::size_of::<String>() + 71)?;
        let work_digest = self.canonical_observation(work)?.0;
        let mut matched = None;
        if Digest256::of_bytes(&files[kind.child_file()]) == expected {
            if self.canonical_observation(&current)?.0 != work_digest {
                return Err(bad("membership Work current payload binding"));
            }
            matched = Some(files[kind.child_file()].len());
        }
        for receipt in array(&history, "receipts")? {
            check(self.limits.deadline, self.cancelled)?;
            let request = &receipt["request"];
            if kind == CompoundKind::CollectionWork
                && text(request, "operation")? == "work.expression.create"
            {
                parent_receipt_shape_with(receipt, CompoundKind::WorkExpression, &mut |value| {
                    self.canonical_observation(value)
                })?;
            } else if request["schema_version"] != "tos_local_source_command_v1"
                || request["operation"] != "record.revise"
                || request["fields"].as_object().is_none_or(|fields| {
                    fields.is_empty()
                        || fields.keys().any(|key| {
                            !["preferred_label", "notes", "field_languages", "source_refs"]
                                .contains(&key.as_str())
                        })
                })
            {
                return Err(bad("membership Work undeclared metadata transition"));
            }
            let phase = self.temporary_state;
            let archived = self.archive(path, id, receipt)?;
            if let Some(publication) = receipt.get("publication") {
                let tx = self.transaction(text(publication, "transaction_id")?)?;
                let (before, after) = tx
                    .files
                    .get(path)
                    .ok_or_else(|| bad("membership Work selected transition absent"))?;
                if tx.status != "committed" || before.as_ref() != archived.get(kind.child_file()) {
                    return Err(bad("membership Work committed selected before binding"));
                }
                let after = self.decoded(
                    after
                        .as_ref()
                        .ok_or_else(|| bad("membership Work selected successor absent"))?,
                )?;
                if !self.reference_matches(
                    &after,
                    "record_id",
                    "record_version",
                    &receipt["source"],
                )? {
                    return Err(bad("membership Work committed selected successor binding"));
                }
            }
            let raw = &archived[kind.child_file()];
            if Digest256::of_bytes(raw) == expected {
                let previous = self.decoded(raw)?;
                if self.canonical_observation(&previous)?.0 != work_digest {
                    return Err(bad("membership retained Work payload binding"));
                }
                matched = Some(raw.len());
            }
            drop(archived);
            self.release_temporary_since(phase);
        }
        let size =
            matched.ok_or_else(|| bad("membership Work lacks current continuous raw binding"))?;
        // The consumed reference reuses the already compared canonical digest.
        // Admit its two actual ordered object containers and strings before
        // constructing them; the helper input Vecs coexist during collection.
        let version = integer(work, "record_version")?;
        let version_digits = work["record_version"]
            .as_number()
            .ok_or_else(|| bad("membership Work version number"))?
            .as_str()
            .len();
        let size_digits = if size == 0 {
            1
        } else {
            size.ilog10() as usize + 1
        };
        let string_state =
            |s: &str| s.len() + s.encode_utf16().count() * std::mem::size_of::<u16>();
        let keys = [
            "source_path",
            "source",
            "source_sha256",
            "source_bytes",
            "id",
            "version",
            "digest",
        ];
        let retained = std::mem::size_of::<JsonValue>()
            + keys.len() * std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>()
            + keys.iter().map(|key| string_state(key)).sum::<usize>()
            + string_state(path)
            + string_state(id)
            + 2 * string_state(&work_digest)
            + version_digits
            + size_digits;
        let scratch = 2 * std::mem::size_of::<Vec<(&str, JsonValue)>>()
            + keys.len() * std::mem::size_of::<(&str, JsonValue)>()
            + std::mem::size_of::<String>()
            + 71
            + 2 * std::mem::size_of::<Value>()
            + version_digits
            + size_digits;
        self.temporary(retained.checked_add(scratch).ok_or(ItemRefusal::Budget)?)?;
        let result = object(vec![
            ("source_path", string(path)),
            (
                "source",
                object(vec![
                    ("id", string(id)),
                    ("version", j(&json!(version))?),
                    ("digest", string(&work_digest)),
                ]),
            ),
            ("source_sha256", string(&expected.to_prefixed())),
            ("source_bytes", j(&json!(size))?),
        ]);
        self.release_temporary(scratch);
        Ok(result)
    }
    pub(crate) fn finish(self) -> (u64, Vec<PredicateRead>) {
        (self.bytes, self.reads)
    }
}
fn state_valid_with(
    v: &Value,
    observe: &mut impl FnMut(&Value) -> Result<(String, usize), ItemRefusal>,
) -> Result<(), ItemRefusal> {
    keys(
        v,
        &[
            "schema_version",
            "generation",
            "transition_id",
            "phase",
            "transaction_id",
            "manifest_sha256",
            "outcome",
            "recovery_authorization",
            "token",
        ],
    )?;
    let generation = integer(v, "generation")?;
    let transition = text(v, "transition_id")?;
    let phase = text(v, "phase")?;
    if text(v, "schema_version")? != "tos_source_metadata_publication_v1"
        || !(1..=MAX_GENERATION).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !matches!(phase, "pending" | "ready")
        || phase == "pending" && (!v["outcome"].is_null() || !v["recovery_authorization"].is_null())
        || phase == "ready" && !matches!(text(v, "outcome")?, "committed" | "rolled-back")
    {
        return Err(bad("publication state grammar"));
    }
    for k in ["transaction_id", "manifest_sha256", "token"] {
        hash(text(v, k)?)?;
    }
    if !v["recovery_authorization"].is_null()
        && (!v["recovery_authorization"].is_object()
            || observe(&v["recovery_authorization"])?.1 > 4096)
    {
        return Err(bad("recovery evidence bound"));
    }
    let mut contents = v.clone();
    contents.as_object_mut().unwrap().remove("token");
    if text(v, "token")? != observe(&contents)?.0 {
        return Err(bad("publication token digest"));
    }
    Ok(())
}

fn request_valid_with(
    request: &Value,
    kind: CompoundKind,
    observe: &mut impl FnMut(&Value) -> Result<(String, usize), ItemRefusal>,
) -> Result<(String, usize), ItemRefusal> {
    let mut request_keys = vec![
        "schema_version",
        "operation",
        kind.record_key(),
        "claim",
        "forms",
        kind.child_form_request(),
        "claim_forms",
        "reason",
        "command_id",
        "fields",
        "expected_configuration",
        "expected_source",
        "expected_revision",
        "expected_dependencies",
        "expected_publication",
    ];
    if kind.relation_attachment() {
        request_keys.retain(|key| *key != kind.child_form_request());
        request_keys.push("forms");
    }
    if kind == CompoundKind::EditionItem {
        request_keys.extend([
            "rights",
            "item_kind",
            "inventory",
            "inventory_limitation",
            "fixity_verified_at",
        ]);
        let now = text(request, "fixity_verified_at")?;
        crate::retirement_rules::observed_instant_order(now, now)
            .map_err(|_| bad("Item fixity instant"))?;
        if request["inventory"].is_null() != !request["inventory_limitation"].is_null() {
            return Err(bad("explicit inventory completeness/limitation"));
        }
    }
    keys(request, &request_keys)?;
    if text(request, "schema_version")? != kind.request_schema()
        || text(request, "operation")? != kind.operation()
    {
        return Err(bad("native bibliographic compound request grammar"));
    }
    let observed = observe(request)?;
    if observed.1 > 1_048_576 {
        return Err(bad("native bibliographic compound request grammar"));
    }
    let reason = tos_foundation::python_strip_unicode16_v1(text(request, "reason")?, MAX_SIDE)
        .map_err(|_| ItemRefusal::Budget)?;
    if reason.is_empty()
        || reason.chars().count() > 4096
        || !(1..=256).contains(&text(request, "command_id")?.chars().count())
    {
        return Err(bad("request reason/command bounds"));
    }
    for k in [
        "expected_configuration",
        "expected_revision",
        "expected_dependencies",
    ] {
        hash(text(request, k)?)?;
    }
    if !request["expected_publication"].is_null() {
        hash(text(request, "expected_publication")?)?;
    }
    keys(&request["fields"], &[kind.field()])?;
    Ok(observed)
}
fn transaction_id_with(
    request: &Value,
    kind: CompoundKind,
    observe: &mut impl FnMut(&Value) -> Result<(String, usize), ItemRefusal>,
) -> Result<String, ItemRefusal> {
    let request_digest = observe(request)?.0;
    transaction_id_with_digest(request, kind, &request_digest, observe)
}
fn transaction_id_with_digest(
    request: &Value,
    kind: CompoundKind,
    request_digest: &str,
    observe: &mut impl FnMut(&Value) -> Result<(String, usize), ItemRefusal>,
) -> Result<String, ItemRefusal> {
    observe(&json!({"operation":kind.operation(),"command_id":request["command_id"],"owner_configuration":request["expected_configuration"],"request_digest":request_digest})).map(|(digest,_)|digest)
}
fn canonical_observation(value: &Value) -> Result<(String, usize), ItemRefusal> {
    let bytes = canonical(value)?;
    Ok((Digest256::of_bytes(&bytes).to_prefixed(), bytes.len()))
}
fn transaction_id(request: &Value, kind: CompoundKind) -> Result<String, ItemRefusal> {
    transaction_id_with(request, kind, &mut canonical_observation)
}
fn parent_receipt_shape_with(
    receipt: &Value,
    kind: CompoundKind,
    observe: &mut impl FnMut(&Value) -> Result<(String, usize), ItemRefusal>,
) -> Result<(String, usize), ItemRefusal> {
    let request = &receipt["request"];
    let request_observation = request_valid_with(request, kind, observe)?;
    let publication = &receipt["publication"];
    keys(
        publication,
        &["protocol", "transaction_id", "selected_files"],
    )?;
    let refs = array(&request["fields"], kind.field())?;
    if text(publication, "protocol")? != PROTOCOL
        || text(publication, "transaction_id")?
            != transaction_id_with_digest(request, kind, &request_observation.0, observe)?
        || publication["selected_files"] != {
            let mut names = selected_names(kind.parent_file())?;
            names.sort();
            json!(names)
        }
        || receipt["changed_fields"] != json!([kind.field()])
        || request["claim"]["predicate"] != kind.predicate()
        || request["claim"]["subject_ref"] != receipt["previous_source"]["id"]
        || !kind.initial_backlink(
            &request[kind.record_key()],
            &receipt["previous_source"]["id"],
        )
        || kind == CompoundKind::ExpressionEdition
            && request[kind.record_key()]["embodies_expression_refs"]
                != json!([receipt["previous_source"]["id"]])
        || request["claim"]["object"] != request[kind.record_key()]["record_id"]
        || refs.last() != request["claim"].get("claim_id")
    {
        return Err(bad("explicit compound parent lineage"));
    }
    Ok(request_observation)
}

/// Existing `_history` law over exact read-only packages. Callers own custody.
/// Other compound parent handlers remain explicit unsupported profiles.
pub fn inspect_record_history(
    files: &BTreeMap<String, Vec<u8>>,
    record_raw: &[u8],
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
) -> Result<Value, ItemRefusal> {
    check(deadline, cancelled)?;
    let record = decode(record_raw)?;
    let history = match files.get(HISTORY) {
        Some(raw) => decode(raw)?,
        None => {
            json!({"schema_version":"tos_source_revision_history_v1","record_id":record["record_id"],"receipts":[]})
        }
    };
    validate_record_history_values(
        files,
        &record,
        &history,
        "record_id",
        deadline,
        cancelled,
        &mut canonical_observation,
    )?;
    Ok(history)
}
fn validate_record_history_values(
    files: &Package,
    record: &Value,
    history: &Value,
    identity_field: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    observe: &mut impl FnMut(&Value) -> Result<(String, usize), ItemRefusal>,
) -> Result<(), ItemRefusal> {
    let version = integer(record, "record_version")?;
    if version == 0 {
        return Err(bad("positive record version"));
    }
    let subject =
        json!({"id":text(record,identity_field)?,"version":version,"digest":observe(record)?.0});
    keys(&history, &["schema_version", "record_id", "receipts"])?;
    if !matches!(
        text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || history["record_id"] != subject["id"]
    {
        return Err(bad("history subject/version"));
    }
    let receipts = array(&history, "receipts")?;
    if receipts.len() > MAX_HISTORY || files.contains_key(HISTORY) && receipts.is_empty() {
        return Err(bad("stored history capacity/empty chain"));
    }
    let mut commands = BTreeSet::new();
    let mut previous = None;
    for receipt in receipts {
        check(deadline, cancelled)?;
        let selected = receipt.get("publication").is_some();
        let mut fields = vec![
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
        ];
        if selected {
            fields.push("publication");
        }
        keys(receipt, &fields)?;
        if selected {
            if text(&history, "schema_version")? != "tos_source_revision_history_v2" {
                return Err(bad("selected history v2 required"));
            }
            let p = &receipt["publication"];
            keys(p, &["protocol", "transaction_id", "selected_files"])?;
            let names = array(p, "selected_files")?;
            if text(p, "protocol")? != PROTOCOL
                || !p["transaction_id"].is_string()
                || names.len() != 3
                || names
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != 3
                || !names.iter().any(|n| n == HISTORY)
                || names.iter().any(|n| {
                    n.as_str()
                        .is_none_or(|s| s.contains('/') || s.is_empty() || matches!(s, "." | ".."))
                })
            {
                return Err(bad("selected history publication binding"));
            }
        }
        crate::retirement_rules::observed_instant_order(
            text(receipt, "recorded_at")?,
            text(receipt, "recorded_at")?,
        )
        .map_err(|_| bad("history aware instant"))?;
        let request = &receipt["request"];
        let observed_request = match text(request, "operation")? {
            "collection.work.attach"
            | "expression.responsibility.attach"
            | "work.expression.create"
            | "expression.edition.create"
            | "item.adopt" => Some(parent_receipt_shape_with(
                receipt,
                CompoundKind::from_operation(text(request, "operation")?)?,
                observe,
            )?),
            "record.revise" => None,
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "retained compound parent handler {other}"
                )));
            }
        };
        let fields = request["fields"]
            .as_object()
            .ok_or_else(|| bad("retained request fields"))?;
        if !commands.insert(text(receipt, "command_id")?)
            || text(receipt, "request_digest")?
                != match observed_request {
                    Some((digest, _)) => digest,
                    None => observe(request)?.0,
                }
            || receipt["command_id"] != request["command_id"]
            || receipt["previous_source"] != request["expected_source"]
            || receipt["previous_revision"] != request["expected_revision"]
            || receipt["owner_configuration"] != request["expected_configuration"]
            || receipt["dependencies"] != request["expected_dependencies"]
            || receipt["reason"] != request["reason"]
            || receipt["changed_fields"] != json!(fields.keys().collect::<Vec<_>>())
            || receipt["grants_admission"] != false
            || receipt["source"]["id"] != subject["id"]
            || receipt["previous_source"]["id"] != subject["id"]
            || integer(&receipt["source"], "version")?
                != integer(&receipt["previous_source"], "version")?
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?
            || previous.is_some_and(|p| receipt.get("previous_source") != Some(p))
        {
            return Err(bad("broken source revision chain"));
        }
        previous = receipt.get("source");
    }
    if previous.is_some_and(|p| p != &subject) {
        return Err(bad("current source differs from history head"));
    }
    check(deadline, cancelled)?;
    Ok(())
}

impl NativeCompoundReader<'_> {
    fn object_link_subject_binding(
        &mut self,
        scope: &Value,
        initial: &Value,
        expected_sha: &str,
    ) -> Result<(), ItemRefusal> {
        let path = text(scope, "subject_source_path")?;
        let id = text(scope, "subject_id")?;
        let identity = if scope["subject_record_type"] == "artifact" {
            "artifact_id"
        } else {
            "record_id"
        };
        let selected = self.selected(path)?;
        let name = path
            .rsplit('/')
            .next()
            .ok_or_else(|| bad("object-Link subject basename"))?;
        let raw = selected
            .get(name)
            .ok_or_else(|| bad("object-Link current subject absent"))?;
        let current = self.decoded(raw)?;
        if text(&current, identity)? != id {
            return Err(bad("object-Link current subject identity"));
        }
        let history = self.history_typed(path, &selected, identity)?;
        let wanted =
            Digest256::from_hex(expected_sha).map_err(|_| bad("object-Link subject raw SHA"))?;
        let mut matched = false;
        if Digest256::of_bytes(raw) == wanted {
            if &current != initial {
                return Err(bad("object-Link exact current subject payload"));
            }
            matched = true;
        }
        for receipt in array(&history, "receipts")? {
            check(self.limits.deadline, self.cancelled)?;
            let before = self.temporary_state;
            let archived = self.archive_typed(path, id, receipt, identity)?;
            if let Some(publication) = receipt.get("publication") {
                let transaction = self.transaction(text(publication, "transaction_id")?)?;
                let (prior, next) = transaction
                    .files
                    .get(path)
                    .ok_or_else(|| bad("object-Link selected subject transition"))?;
                if transaction.status != "committed"
                    || prior.as_ref() != archived.get(name)
                    || next.as_ref().is_none_or(|bytes| {
                        self.decoded(bytes).map_or(true, |value| {
                            self.reference_matches(
                                &value,
                                identity,
                                "record_version",
                                &receipt["source"],
                            )
                            .map_or(true, |valid| !valid)
                        })
                    })
                {
                    return Err(bad("object-Link committed subject transition"));
                }
            }
            let previous = archived
                .get(name)
                .ok_or_else(|| bad("object-Link archived subject absent"))?;
            if Digest256::of_bytes(previous) == wanted {
                let payload = self.decoded(previous)?;
                if &payload != initial {
                    return Err(bad("object-Link retained subject payload"));
                }
                if matched {
                    return Err(bad("object-Link ambiguous original subject bytes"));
                }
                matched = true;
            }
            drop(archived);
            self.release_temporary_since(before);
        }
        if !matched {
            return Err(bad(
                "object-Link original subject bytes not continuously retained",
            ));
        }
        Ok(())
    }
    fn object_link_initial_forms(
        &mut self,
        path: &str,
        expected: &[u8],
    ) -> Result<(), ItemRefusal> {
        use crate::source_forms::source_copy_kernel as kernel;
        let current_raw = self.required(path, MAX_FILE)?;
        let current = self.ordered_value(&current_raw)?;
        let subject = current
            .object_get("subject")
            .ok_or_else(|| bad("object-Link current form subject"))?;
        kernel::validate_history(&current, subject)
            .map_err(|_| bad("object-Link continuous source-copy Form history"))?;
        // Source _validate_history uses logical Python dict equality for the
        // retained Form, independent of published object-key insertion order.
        let current_logical = self.decoded(&current_raw)?;
        let original_logical = self.decoded(expected)?;
        let rows = array(&current_logical, "forms")?;
        let prior = array(&current_logical, "prior_forms")?;
        let initial = array(&original_logical, "forms")?;
        for form in initial {
            check(self.limits.deadline, self.cancelled)?;
            let retained = rows
                .iter()
                .chain(prior)
                .find(|candidate| {
                    candidate["form_id"] == form["form_id"]
                        && candidate["form_version"] == form["form_version"]
                })
                .ok_or_else(|| bad("object-Link original Form lost from continuous history"))?;
            if retained != form {
                return Err(bad(
                    "object-Link original Form lost from continuous history",
                ));
            }
        }
        Ok(())
    }
    fn object_link_current_lineage(
        &mut self,
        scope: &Value,
        initial: &Value,
        initial_raw: &[u8],
    ) -> Result<Value, ItemRefusal> {
        let path = text(scope, "link_source_path")?;
        let id = text(scope, "link_id")?;
        let files = self.selected(path)?;
        let current_raw = files
            .get("link.json")
            .ok_or_else(|| bad("object-Link current source absent"))?;
        let current = self.decoded(current_raw)?;
        if text(&current, "record_id")? != id || current["record_type"] != "link" {
            return Err(bad("object-Link current typed identity"));
        }
        let history = self.history(path, &files)?;
        let initial_ref = self.canonical_observation(initial)?.0;
        let mut original = current_raw.as_slice() == initial_raw;
        for (index, receipt) in array(&history, "receipts")?.iter().enumerate() {
            check(self.limits.deadline, self.cancelled)?;
            let request = &receipt["request"];
            let fields = request["fields"]
                .as_object()
                .ok_or_else(|| bad("object-Link correction fields"))?;
            if request["schema_version"] != "tos_local_source_command_v1"
                || request["operation"] != "record.revise"
                || fields.is_empty()
                || fields.keys().any(|key| {
                    ![
                        "preferred_label",
                        "variant_labels",
                        "notes",
                        "source_refs",
                        "provider_label",
                    ]
                    .contains(&key.as_str())
                })
                || receipt["publication"]["protocol"] != PROTOCOL
                || receipt["publication"]["selected_files"]
                    != json!(["link.human-forms.json", "link.json", HISTORY])
            {
                return Err(bad("object-Link descriptive selected correction"));
            }
            let before = self.temporary_state;
            let archive = self.archive(path, id, receipt)?;
            let old_raw = archive
                .get("link.json")
                .ok_or_else(|| bad("object-Link archived source absent"))?;
            let old = self.decoded(old_raw)?;
            let mut successor = self.value_copy(&old)?;
            let map = successor
                .as_object_mut()
                .ok_or_else(|| bad("object-Link record"))?;
            for (key, value) in fields {
                map.insert(key.clone(), value.clone());
            }
            map.insert(
                "record_version".into(),
                json!(
                    integer(&old, "record_version")?
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?
                ),
            );
            if fields.keys().any(|key| {
                ![
                    "preferred_label",
                    "variant_labels",
                    "notes",
                    "source_refs",
                    "provider_label",
                ]
                .contains(&key.as_str())
            }) || !self.reference_matches(
                &successor,
                "record_id",
                "record_version",
                &receipt["source"],
            )? {
                return Err(bad("object-Link correction identity/allowed fields"));
            }
            let tx = self.transaction(text(&receipt["publication"], "transaction_id")?)?;
            let (prior, after) = tx
                .files
                .get(path)
                .ok_or_else(|| bad("object-Link correction transaction source"))?;
            if tx.status != "committed"
                || prior.as_ref() != Some(old_raw)
                || after
                    .as_ref()
                    .is_none_or(|raw| self.decoded(raw).map_or(true, |value| value != successor))
            {
                return Err(bad("object-Link exact committed correction"));
            }
            if index == 0
                && old_raw.as_slice() == initial_raw
                && receipt["previous_source"] == json!({"id":id,"version":1,"digest":initial_ref})
            {
                original = true;
            }
            drop(archive);
            self.release_temporary_since(before);
        }
        if !original {
            return Err(bad(
                "object-Link committed initial source absent from continuous lineage",
            ));
        }
        if current["association_claim_refs"] != json!([scope["claim_id"]])
            || current["provenance_event_ref"] != scope["provenance_event_id"]
        {
            return Err(bad("object-Link current association closure"));
        }
        Ok(current)
    }
    fn history(
        &mut self,
        path: &str,
        files: &Package,
    ) -> Result<std::sync::Arc<Value>, ItemRefusal> {
        self.history_typed(path, files, "record_id")
    }
    fn history_typed(
        &mut self,
        path: &str,
        files: &Package,
        identity_field: &str,
    ) -> Result<std::sync::Arc<Value>, ItemRefusal> {
        self.history_typed_with_origin(path, files, identity_field, false)
            .map(|(history, _)| history)
    }

    fn history_typed_with_origin(
        &mut self,
        path: &str,
        files: &Package,
        identity_field: &str,
        capture_origin: bool,
    ) -> Result<(std::sync::Arc<Value>, Option<(String, usize)>), ItemRefusal> {
        let before = self.temporary_state;
        let result = self.history_inner(path, files, identity_field, capture_origin);
        self.release_temporary_since(before);
        result
    }

    fn history_inner(
        &mut self,
        path: &str,
        files: &Package,
        identity_field: &str,
        capture_origin: bool,
    ) -> Result<(std::sync::Arc<Value>, Option<(String, usize)>), ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        // Bind memoized lineage to the whole selected package, not its subject
        // alone: source-copy forms and history bytes participate in revision.
        let key = (path.to_owned(), self.package_revision(files)?);
        let key_state = std::mem::size_of_val(&key)
            .checked_add(key.0.len())
            .and_then(|n| n.checked_add(key.1.len()))
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(key_state)?;
        if let Some(history) = self.histories.get(&key) {
            return Ok((history.clone(), None));
        }
        let name = path
            .rsplit('/')
            .next()
            .ok_or_else(|| bad("record basename"))?;
        let raw = files
            .get(name)
            .ok_or_else(|| bad("history source absent"))?;
        let record = self.decoded(raw)?;
        let id = text(&record, identity_field)?;
        let history = match files.get(HISTORY) {
            Some(raw) => self.decoded(raw)?,
            None => {
                let value = json!({"schema_version":"tos_source_revision_history_v1","record_id":id,"receipts":[]});
                self.temporary(crate::record_biblio_cut::decoded_state(&value)?)?;
                value
            }
        };
        // The commands index borrows retained history strings. Field-name
        // vectors contain only references; the exact subject is one owned value.
        let history_validation_state = reference_state(&record, identity_field, "record_version")?
            + array(&history, "receipts")?.len() * std::mem::size_of::<&str>()
            + 17 * std::mem::size_of::<&str>();
        self.temporary(history_validation_state)?;
        validate_record_history_values(
            files,
            &record,
            &history,
            identity_field,
            self.limits.deadline,
            self.cancelled,
            &mut |value| self.canonical_observation(value),
        )?;
        self.release_temporary(history_validation_state);
        let mut origin_record = None;
        for (index, receipt) in array(&history, "receipts")?.iter().enumerate() {
            check(self.limits.deadline, self.cancelled)?;
            let previous_temporary = self.temporary_state;
            let archived = self.archive_typed(path, id, receipt, identity_field)?;
            if capture_origin && index == 0 {
                let origin_bytes = archived
                    .get(name)
                    .ok_or_else(|| bad("archive origin record absent"))?;
                let digest = Digest256::of_bytes(origin_bytes).to_hex();
                self.temporary(
                    digest
                        .len()
                        .checked_add(std::mem::size_of::<String>())
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                origin_record = Some((digest, origin_bytes.len()));
            }
            let predecessor_record = self.decoded(
                archived
                    .get(name)
                    .ok_or_else(|| bad("archive source absent"))?,
            )?;
            let predecessor = match archived.get(HISTORY) {
                Some(raw) => self.decoded(raw)?,
                None => {
                    let value = json!({"schema_version":"tos_source_revision_history_v1","record_id":id,"receipts":[]});
                    self.temporary(crate::record_biblio_cut::decoded_state(&value)?)?;
                    value
                }
            };
            let predecessor_subject_state =
                reference_state(&predecessor_record, identity_field, "record_version")?;
            let predecessor_indexes = array(&predecessor, "receipts")?.len()
                * std::mem::size_of::<&str>()
                + 17 * std::mem::size_of::<&str>();
            self.temporary(
                predecessor_subject_state
                    .checked_add(predecessor_indexes)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            validate_record_history_values(
                &archived,
                &predecessor_record,
                &predecessor,
                identity_field,
                self.limits.deadline,
                self.cancelled,
                &mut |value| self.canonical_observation(value),
            )?;
            if array(&predecessor, "receipts")? != &array(&history, "receipts")?[..index] {
                return Err(bad("retained predecessor receipt prefix"));
            }
            drop(predecessor);
            drop(predecessor_record);
            drop(archived);
            self.release_temporary_since(previous_temporary);
            if capture_origin && index == 0 {
                let origin_string_state = match origin_record.as_ref() {
                    Some((digest, _)) => digest
                        .len()
                        .checked_add(std::mem::size_of::<String>())
                        .ok_or(ItemRefusal::Budget)?,
                    None => 0,
                };
                self.temporary(origin_string_state)?;
            }
        }
        let history_state = crate::record_biblio_cut::decoded_state(&history)?
            .checked_add(
                key.0.len()
                    + key.1.len()
                    + std::mem::size_of::<((String, String), std::sync::Arc<Value>)>()
                    + 2 * std::mem::size_of::<usize>(),
            )
            .ok_or(ItemRefusal::Budget)?;
        let tree_state = crate::record_biblio_cut::decoded_state(&history)?;
        self.state -= tree_state + key_state;
        self.temporary_state -= tree_state + key_state;
        reserve(&mut self.state, history_state, self.limits.max_state_bytes)?;
        let history = std::sync::Arc::new(history);
        self.histories.insert(key, history.clone());
        Ok((history, origin_record))
    }
}

const SCOPE_KEYS: [&str; 9] = [
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_work_form_ids",
    "allowed_expression_form_ids",
    "allowed_claim_form_ids",
];
const EDITION_SCOPE_KEYS: [&str; 11] = [
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "edition_id",
    "edition_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_expression_form_ids",
    "allowed_edition_form_ids",
    "allowed_claim_form_ids",
];
fn typed_id(id: &str, kind: &str) -> bool {
    id.strip_prefix(&format!("tos.{kind}."))
        .is_some_and(segment)
}
fn segment(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
        && s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        && !s
            .as_bytes()
            .windows(2)
            .any(|p| matches!(p[0], b'.' | b'-') && matches!(p[1], b'.' | b'-'))
}
fn scope_valid(
    scope: &Value,
    request: &Value,
    authority: &Value,
    kind: CompoundKind,
) -> Result<(), ItemRefusal> {
    if kind.relation_attachment() {
        attachment_scope_valid(scope, request, kind)?;
    } else if kind == CompoundKind::EditionItem {
        item_scope_valid(scope, request)?;
    } else {
        keys(
            scope,
            if kind == CompoundKind::WorkExpression {
                &SCOPE_KEYS
            } else {
                &EDITION_SCOPE_KEYS
            },
        )?;
        for (k, kind) in [
            ("work_id", "work"),
            ("expression_id", "expression"),
            ("claim_id", "claim"),
            ("provenance_event_id", "event"),
        ] {
            if !typed_id(text(scope, k)?, kind) {
                return Err(bad("typed compound scope identities"));
            }
        }
        let work = text(scope, "work_source_path")?;
        let expression = text(scope, "expression_source_path")?;
        metadata_path(work, false)?;
        metadata_path(expression, false)?;
        if !work.starts_with("ToS/source-witnesses/works/")
            || work.split('/').count() < 5
            || !work.ends_with("/work.json")
            || !expression.ends_with("/expression.json")
        {
            return Err(bad("Work Expression home grammar"));
        }
        let child = parent(expression)?;
        let expected = format!("{}/expressions/", parent(work)?);
        if !child
            .strip_prefix(&expected)
            .is_some_and(|s| !s.contains('/') && segment(s))
        {
            return Err(bad("one exact child home"));
        }
        if kind == CompoundKind::ExpressionEdition {
            if !typed_id(text(scope, "edition_id")?, "edition") {
                return Err(bad("typed Edition identity"));
            }
            let edition = text(scope, "edition_source_path")?;
            metadata_path(edition, false)?;
            let expected = format!("{}/editions/", parent(expression)?);
            if !edition.ends_with("/edition.json")
                || !parent(edition)?
                    .strip_prefix(&expected)
                    .is_some_and(|s| !s.contains('/') && segment(s))
            {
                return Err(bad("one exact Edition home"));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for (field, allowed) in [
        (
            "forms",
            match kind {
                CompoundKind::CollectionWork => "allowed_collection_form_ids",
                CompoundKind::ExpressionResponsibility => "allowed_expression_form_ids",
                CompoundKind::WorkExpression => "allowed_work_form_ids",
                CompoundKind::ExpressionEdition => "allowed_expression_form_ids",
                CompoundKind::EditionItem => "allowed_edition_form_ids",
            },
        ),
        (
            kind.child_form_request(),
            match kind {
                CompoundKind::CollectionWork => "allowed_collection_form_ids",
                CompoundKind::ExpressionResponsibility => "allowed_expression_form_ids",
                CompoundKind::WorkExpression => "allowed_expression_form_ids",
                CompoundKind::ExpressionEdition => "allowed_edition_form_ids",
                CompoundKind::EditionItem => "allowed_item_form_ids",
            },
        ),
        ("claim_forms", "allowed_claim_form_ids"),
    ]
    .into_iter()
    .filter(|(field, _)| !kind.relation_attachment() || *field != "forms")
    .chain((kind.relation_attachment()).then_some(("forms", kind.parent_form_grant())))
    {
        let ids = array(scope, allowed)?;
        if !(1..=32).contains(&ids.len()) {
            return Err(bad("form identity bounds"));
        }
        let mut grant = BTreeSet::new();
        for id in ids {
            let id = id.as_str().ok_or_else(|| bad("form identity"))?;
            let tail = id
                .strip_prefix("tos.form.")
                .ok_or_else(|| bad("form identity prefix"))?;
            if tail.is_empty()
                || !tail.as_bytes()[0].is_ascii_alphanumeric()
                || !tail.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
                })
                || !seen.insert(id)
                || !grant.insert(id)
            {
                return Err(bad("distinct subject form identities"));
            }
        }
        let selections = array(request, field)?;
        if !(1..=32).contains(&selections.len()) {
            return Err(bad("explicit form selections"));
        }
        let mut selected = BTreeSet::new();
        for item in selections {
            keys(item, &["form_id", "field_id"])?;
            if !grant.contains(text(item, "form_id")?) || !selected.insert(text(item, "form_id")?) {
                return Err(bad("form selection exceeds retained scope"));
            }
            text(item, "field_id")?;
        }
    }
    let claim = &request["claim"];
    let record = &request[kind.record_key()];
    if record["record_id"] != scope[kind.child_key()]
        || !kind.initial_backlink(record, &scope[kind.parent_key()])
        || kind == CompoundKind::ExpressionEdition
            && record["embodies_expression_refs"] != json!([scope["expression_id"]])
        || kind == CompoundKind::EditionItem
            && record["item_manifest_ref"]
                != format!(
                    "{}/item.manifest.json",
                    parent(text(scope, "item_source_path")?)?
                )
        || claim["claim_id"] != scope["claim_id"]
        || claim["subject_ref"] != scope[kind.parent_key()]
        || claim["object"] != scope[kind.child_key()]
        || claim["provenance_event_ref"] != scope["provenance_event_id"]
        || claim["maker"]
            != json!({"maker_type":authority["maker_type"],"agent_ref":authority["principal_id"]})
    {
        return Err(bad("exact scope/request endpoints and maker"));
    }
    Ok(())
}
const COLLECTION_SCOPE_KEYS: [&str; 12] = [
    "collection_id",
    "collection_source_path",
    "work_id",
    "work_source_path",
    "predicate",
    "claim_id",
    "claim_source_path",
    "provenance_event_id",
    "allowed_collection_form_ids",
    "allowed_claim_form_ids",
    "allowed_evidence_refs",
    "retained_membership_provenance_refs",
];
const RESPONSIBILITY_SCOPE_KEYS: [&str; 11] = [
    "expression_id",
    "expression_source_path",
    "agent_id",
    "agent_source_path",
    "predicate",
    "claim_id",
    "claim_source_path",
    "provenance_event_id",
    "allowed_expression_form_ids",
    "allowed_claim_form_ids",
    "allowed_evidence_refs",
];
fn attachment_scope_valid(
    scope: &Value,
    request: &Value,
    kind: CompoundKind,
) -> Result<(), ItemRefusal> {
    keys(
        scope,
        if kind == CompoundKind::CollectionWork {
            &COLLECTION_SCOPE_KEYS[..]
        } else {
            &RESPONSIBILITY_SCOPE_KEYS[..]
        },
    )?;
    for (field, kind) in [
        (kind.parent_key(), kind.parent_kind()),
        (kind.child_key(), kind.child_kind()),
        ("claim_id", "claim"),
        ("provenance_event_id", "event"),
    ] {
        if !typed_id(text(scope, field)?, kind) {
            return Err(bad("membership typed scope identities"));
        }
    }
    let collection = text(scope, kind.parent_path())?;
    let work = text(scope, kind.child_path())?;
    let claim = text(scope, "claim_source_path")?;
    for path in [collection, work, claim] {
        metadata_path(path, false)?;
    }
    if text(scope, "predicate")? != kind.predicate()
        || if kind == CompoundKind::CollectionWork {
            !collection.starts_with("ToS/source-witnesses/collections/")
                || collection.split('/').count() < 5
                || !collection.ends_with("/collection.json")
                || !work.starts_with("ToS/source-witnesses/works/")
                || !work.ends_with("/work.json")
        } else {
            !collection.starts_with("ToS/source-witnesses/works/")
                || !collection.ends_with("/expression.json")
                || !collection.split('/').any(|part| part == "expressions")
                || !work.starts_with("ToS/source-witnesses/agents/")
                || !work.ends_with("/agent.json")
        }
        || !claim.starts_with("ToS/source-witnesses/relations/")
        || claim.split('/').count() != 5
        || !claim.ends_with("/source-claims.jsonl")
        || !segment(parent(claim)?.rsplit('/').next().unwrap())
    {
        return Err(bad("membership separate exact public homes"));
    }
    if request[kind.record_key()]["record_type"] != kind.child_kind()
        || request["claim"]["predicate"] != scope["predicate"]
    {
        return Err(bad("membership exact Work/Claim route"));
    }
    let evidence = array(scope, "allowed_evidence_refs")?;
    if !(1..=128).contains(&evidence.len()) {
        return Err(bad("membership evidence bounds"));
    }
    let mut seen = BTreeSet::new();
    for value in evidence {
        let value = value
            .as_str()
            .ok_or_else(|| bad("membership evidence strings"))?;
        if value.chars().count() > 4096
            || tos_foundation::python_strip_unicode16_v1(value, MAX_SIDE)
                .map_err(|_| ItemRefusal::Budget)?
                .is_empty()
            || !seen.insert(value)
        {
            return Err(bad("membership distinct explicit evidence"));
        }
    }
    for field in ["evidence_refs", "counterevidence_refs"] {
        if let Some(refs) = request["claim"].get(field) {
            for value in refs
                .as_array()
                .ok_or_else(|| bad("membership evidence array"))?
            {
                if !value.is_string() || !evidence.contains(value) {
                    return Err(bad("membership evidence exceeds recorded scope"));
                }
            }
        }
    }
    if kind == CompoundKind::CollectionWork {
        let provenance = array(scope, "retained_membership_provenance_refs")?;
        if provenance.len() > 32 {
            return Err(bad("membership retained provenance bounds"));
        }
        seen.clear();
        for value in provenance {
            let path = value
                .as_str()
                .ok_or_else(|| bad("membership provenance path"))?;
            metadata_path(path, false)?;
            let name = path.rsplit('/').next().unwrap();
            if !name.ends_with(".jsonl") || !name.contains("provenance") || !seen.insert(path) {
                return Err(bad("membership retained provenance grammar"));
            }
        }
    }
    Ok(())
}
const ITEM_SCOPE_KEYS: [&str; 18] = [
    "edition_id",
    "edition_source_path",
    "item_id",
    "item_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_edition_form_ids",
    "allowed_item_form_ids",
    "allowed_claim_form_ids",
    "file_id",
    "payload_basename",
    "original_basename",
    "media_type",
    "byte_size",
    "sha256",
    "rights_id",
    "acquisition_event_id",
    "inventory_event_id",
];
fn item_scope_valid(scope: &Value, request: &Value) -> Result<(), ItemRefusal> {
    keys(scope, &ITEM_SCOPE_KEYS)?;
    for (key, kind) in [
        ("edition_id", "edition"),
        ("item_id", "item"),
        ("file_id", "file"),
        ("claim_id", "claim"),
        ("provenance_event_id", "event"),
        ("rights_id", "rights"),
        ("acquisition_event_id", "event"),
        ("inventory_event_id", "event"),
    ] {
        if !typed_id(text(scope, key)?, kind) {
            return Err(bad("typed Item adoption identity"));
        }
    }
    if [
        text(scope, "provenance_event_id")?,
        text(scope, "acquisition_event_id")?,
        text(scope, "inventory_event_id")?,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>()
    .len()
        != 3
    {
        return Err(bad("separate Item copy/enumeration/serialization events"));
    }
    let edition = text(scope, "edition_source_path")?;
    let item = text(scope, "item_source_path")?;
    metadata_path(edition, false)?;
    metadata_path(item, false)?;
    let prefix = format!("{}/items/", parent(edition)?);
    if !edition.ends_with("/edition.json")
        || !edition.split('/').any(|p| p == "editions")
        || !item.ends_with("/item.json")
        || !parent(item)?
            .strip_prefix(&prefix)
            .is_some_and(|part| !part.contains('/') && segment(part))
    {
        return Err(bad("one exact Item child home"));
    }
    let payload = text(scope, "payload_basename")?;
    let original = text(scope, "original_basename")?;
    let media = text(scope, "media_type")?;
    let media_part = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-')
            })
    };
    if !(1..=201).contains(&payload.len())
        || !payload.as_bytes()[0].is_ascii_alphanumeric()
        || !payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        || !(1..=256).contains(&original.chars().count())
        || original
            .chars()
            .any(|c| matches!(c, '/' | '\\' | '\n' | '\r' | '\0'))
        || !media
            .split_once('/')
            .is_some_and(|(a, b)| media_part(a) && media_part(b))
        || !(1..=536_870_912).contains(&integer(scope, "byte_size")?)
    {
        return Err(bad("bounded Item File name/media/size"));
    }
    hash(&format!("sha256:{}", text(scope, "sha256")?))?;
    let rights = &request["rights"];
    let scopes = array(rights, "scope_refs")?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if rights["rights_id"] != scope["rights_id"]
        || scopes != BTreeSet::from([text(scope, "item_id")?, text(scope, "file_id")?])
        || rights["visibility"] != "local_only"
        || rights["review_status"] != "unreviewed"
        || !matches!(
            text(rights, "assessment_status")?,
            "not_assessed" | "copyright_not_evaluated" | "copyright_undetermined"
        )
        || rights["redistribution_posture"] != "not_authorized"
        || rights["derivative_posture"] != "local_research_only"
        || rights["permissions"] != json!([])
        || rights
            .get("layer_assessments")
            .is_some_and(|v| *v != json!([]))
        || !matches!(
            text(request, "item_kind")?,
            "born_digital" | "digitized_physical_copy" | "derived_publication" | "unknown"
        )
    {
        return Err(bad(
            "supplied Item rights remain separate unreviewed local-only observations",
        ));
    }
    Ok(())
}
fn item_payload(scope: &Value) -> Result<Value, ItemRefusal> {
    Ok(
        json!({"file_id":scope["file_id"],"relative_path":format!("payload/{}",text(scope,"payload_basename")?),
        "original_basename":scope["original_basename"],"media_type":scope["media_type"],
        "byte_size":scope["byte_size"],"sha256":scope["sha256"]}),
    )
}
fn item_companions(
    scope: &Value,
    request: &Value,
    byte_receipt: &Value,
    generator: &str,
    schemas: &mut impl CutSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<(String, Vec<u8>)>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    keys(
        byte_receipt,
        &[
            "schema_version",
            "transaction_id",
            "owner_configuration",
            "private_stage_digest",
            "recovery_configuration",
            "file",
            "started_at",
            "deposited_at",
            "observation_interval",
            "original_preserved",
            "metadata_committed",
            "grants_admission",
        ],
    )?;
    let identifier = transaction_id(request, CompoundKind::EditionItem)?;
    if byte_receipt["schema_version"] != "tos_item_deposit_receipt_v1"
        || byte_receipt["transaction_id"] != identifier
        || byte_receipt["owner_configuration"] != request["expected_configuration"]
        || byte_receipt["file"] != item_payload(scope)?
        || byte_receipt["original_preserved"] != true
        || byte_receipt["metadata_committed"] != false
        || byte_receipt["grants_admission"] != false
    {
        return Err(bad("public Item byte receipt exact File binding"));
    }
    hash(text(byte_receipt, "private_stage_digest")?)?;
    if !byte_receipt["recovery_configuration"].is_null() {
        hash(text(byte_receipt, "recovery_configuration")?)?;
    }
    let interval = &byte_receipt["observation_interval"];
    keys(interval, &["started_at", "ended_at"])?;
    let times = [
        text(interval, "started_at")?,
        text(interval, "ended_at")?,
        text(byte_receipt, "started_at")?,
        text(byte_receipt, "deposited_at")?,
    ];
    for pair in times.windows(2) {
        if crate::retirement_rules::observed_instant_order(pair[0], pair[1])
            .map_err(|_| bad("Item deposit aware chronology"))?
            == std::cmp::Ordering::Greater
        {
            return Err(bad("Item observation/deposit times reversed"));
        }
    }
    // A public retained receipt binds source-safe observations, not possession
    // of the private stage, current payload fixity or any publication grant.
    let boundary = match generator {
        "1" => {
            "resource enumeration, geometry, ordering, counts, and one-way fingerprints only; no source text, bibliographic acceptance, textual acceptance, rights clearance, translation, semantics, or canon authority"
        }
        "2" => {
            "This inventory records resource enumeration, geometry, ordering, counts and one-way fingerprints for the selected source."
        }
        _ => {
            return Err(ItemRefusal::Unsupported(
                "resource inventory generator version".into(),
            ));
        }
    };
    let home = parent(text(scope, "item_source_path")?)?;
    let locator = |name: &str| format!("{home}/{name}");
    let stamp = text(byte_receipt, "deposited_at")?;
    let payload = object(vec![
        ("file_id", j(&scope["file_id"])?),
        (
            "relative_path",
            string(&format!("payload/{}", text(scope, "payload_basename")?)),
        ),
        ("original_basename", j(&scope["original_basename"])?),
        ("media_type", j(&scope["media_type"])?),
        ("byte_size", j(&scope["byte_size"])?),
        ("sha256", j(&scope["sha256"])?),
        ("fixity_verified_at", string(stamp)),
    ]);
    let manifest = object(vec![
        ("schema_version", string("tos_source_item_manifest_v1")),
        ("item_id", j(&scope["item_id"])?),
        ("item_kind", j(&request["item_kind"])?),
        ("embodiment_ref", j(&scope["edition_id"])?),
        ("storage_posture", string("local_gitignored_payload")),
        ("payload_files", JsonValue::Array(vec![payload])),
        ("acquisition_event_ref", j(&scope["acquisition_event_id"])?),
        ("rights_ref", string(&locator("rights.json"))),
        ("provenance_ref", string(&locator("provenance.jsonl"))),
        (
            "forensic_report_ref",
            string(&locator("forensic-report.md")),
        ),
        (
            "resource_inventory_ref",
            string(&locator("resource-inventory.json")),
        ),
        ("visibility", string("local_only")),
        ("manifest_version", j(&json!(1))?),
    ]);
    let input = &request["inventory"];
    if input.is_null() || !request["inventory_limitation"].is_null() {
        return Err(bad("Item inventory unavailable"));
    }
    for (key, value) in [
        ("file_id", &scope["file_id"]),
        ("file_sha256", &scope["sha256"]),
        ("media_type", &scope["media_type"]),
    ] {
        if input.get(key) != Some(value) {
            return Err(bad("Item inventory exact granted File"));
        }
    }
    let inventory = object(vec![
        (
            "$schema",
            string(
                "https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json",
            ),
        ),
        ("schema_version", string("tos_source_resource_inventory_v1")),
        ("item_id", j(&scope["item_id"])?),
        (
            "generated_from_manifest_ref",
            string(&locator("item.manifest.json")),
        ),
        ("inventory_authority", string("mechanical_metadata_only")),
        ("source_text_included", JsonValue::Bool(false)),
        ("files", JsonValue::Array(vec![j(input)?])),
        (
            "generator",
            object(vec![
                ("name", string("build_source_resource_inventories.py")),
                ("version", string(generator)),
            ]),
        ),
        ("provenance_event_ref", j(&scope["inventory_event_id"])?),
        ("inventory_version", j(&json!(1))?),
        ("supersedes_inventory_ref", JsonValue::Null),
        ("authority_boundary", string(boundary)),
    ]);
    let rights = j(&request["rights"])?;
    let guard = |used: usize| {
        if used > limits.max_state_bytes {
            Err(ItemRefusal::BudgetCheck {
                check: "compound Item companion logical workspace",
                used: Some(used as u64),
                limit: Some(limits.max_state_bytes as u64),
            })
        } else {
            Ok(())
        }
    };
    let mut workspace = 0usize;
    for value in [&manifest, &inventory, &rights] {
        workspace = workspace
            .checked_add(crate::record_biblio_cut::ordered_state(value)?)
            .ok_or(ItemRefusal::Budget)?;
    }
    guard(workspace)?;
    for (name, leaf, value) in [
        ("source-item-manifest", "item.manifest.json", &manifest),
        (
            "source-resource-inventory",
            "resource-inventory.json",
            &inventory,
        ),
        ("rights-record", "rights.json", &rights),
    ] {
        let raw = canonical_ordered(value)?;
        guard(
            workspace
                .checked_add(raw.len())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        if !schemas.check_reusing_scalar(
            &format!("{}#compound-reconstructed", locator(leaf)),
            &raw,
            &format!("ToS/contracts/{name}.schema.json"),
            limits.deadline,
            cancelled,
        )? {
            return Err(bad("Item companion selected schema"));
        }
    }
    let inventory_raw = pretty(&inventory)?;
    let event = json!({"schema_version":"tos_provenance_event_v1","event_id":scope["acquisition_event_id"],
        "event_type":"acquisition","started_at":byte_receipt["started_at"],"ended_at":stamp,
        "agent_refs":["software:tos-source-item-commands"],
        "inputs":[{"ref":scope["file_id"],"role":"previously_acquired_local_input","sha256":scope["sha256"]}],
        "outputs":[{"ref":locator(&format!("payload/{}",text(scope,"payload_basename")?)),"role":"retained_local_witness_bytes","sha256":scope["sha256"]}],
        "method":{"maker_type":"software","name":"bounded-local-file-adoption","version":"1","configuration":{
            "transaction_id":identifier,"owner_configuration":request["expected_configuration"],"byte_receipt_ref":locator("item-deposit-receipt.json")}},
        "status":"completed_with_warnings","warnings":["Local retention only; not rights, bibliographic or textual admission."],
        "receipt_refs":[locator("edition-item-receipt.json")],"rights_basis_ref":locator("rights.json"),"event_version":1});
    workspace = workspace
        .checked_add(inventory_raw.len())
        .and_then(|n| n.checked_add(crate::record_biblio_cut::decoded_state(&event).ok()?))
        .ok_or(ItemRefusal::Budget)?;
    guard(
        workspace
            .checked_add(crate::record_biblio_cut::decoded_state(&event)?)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let mut enumeration = event.clone();
    for (key, value) in [
        ("event_id", scope["inventory_event_id"].clone()),
        ("event_type", json!("forensic_inspection")),
        ("started_at", interval["started_at"].clone()),
        ("ended_at", interval["ended_at"].clone()),
        (
            "inputs",
            json!([{"ref":scope["file_id"],"role":"resource_inventory_input","sha256":scope["sha256"]}]),
        ),
        (
            "outputs",
            json!([{"ref":locator("resource-inventory.json"),"role":"tracked_text_free_resource_inventory","sha256":Digest256::of_bytes(&inventory_raw).to_hex()}]),
        ),
        (
            "method",
            json!({"maker_type":"software","name":"build_source_resource_inventories.py","version":generator,
            "configuration":{"scope":"resource enumeration only; no text extraction or semantic reading"}}),
        ),
    ] {
        enumeration
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
    }
    workspace = workspace
        .checked_add(crate::record_biblio_cut::decoded_state(&enumeration)?)
        .ok_or(ItemRefusal::Budget)?;
    guard(workspace)?;
    let mut provenance = Vec::new();
    for (index, value) in [&event, &enumeration].into_iter().enumerate() {
        let raw = canonical(value)?;
        guard(
            workspace
                .checked_add(provenance.len())
                .and_then(|n| n.checked_add(raw.len()))
                .ok_or(ItemRefusal::Budget)?,
        )?;
        if !schemas.check_reusing_scalar(
            &format!(
                "{}:{}#compound-reconstructed",
                locator("provenance.jsonl"),
                index + 1
            ),
            &raw,
            "ToS/contracts/provenance-event.schema.json",
            limits.deadline,
            cancelled,
        )? {
            return Err(bad("Item acquisition/enumeration selected schema"));
        }
        provenance.extend(raw);
        provenance.push(b'\n');
    }
    let report = format!(
        "# Local Item adoption forensic boundary\n\nFile: {}\nSHA-256: {}\nRetains one unchanged previously acquired local file; the input is preserved.\nThe inventory enumerates resources only. No OCR, correction, translation, source reading,\ncopyright clearance, publication authorization or semantic acceptance was performed.\nCopy and metadata publication are separate stages; retained transaction evidence owns recovery.\n",
        text(scope, "file_id")?,
        text(scope, "sha256")?
    );
    let inventory_bytes = inventory_raw.len();
    let result = vec![
        ("item.manifest.json".into(), pretty(&manifest)?),
        ("rights.json".into(), pretty(&rights)?),
        ("resource-inventory.json".into(), inventory_raw),
        (
            "fixity.sha256".into(),
            format!(
                "{}  payload/{}\n",
                text(scope, "sha256")?,
                text(scope, "payload_basename")?
            )
            .into_bytes(),
        ),
        ("forensic-report.md".into(), report.into_bytes()),
        ("provenance.jsonl".into(), provenance),
    ];
    // inventory/provenance/report bytes move into these output rows, while the
    // manifest/rights/event trees above remain live until this return.
    let trees = workspace.checked_sub(inventory_bytes).unwrap_or(workspace);
    guard(
        trees
            .checked_add(rows_state(&result, true)?)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    Ok(result)
}

/// Physical Python bytes.splitlines boundaries, retaining each line ending.
pub fn claim_lines(raw: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut offset = 0usize;
    std::iter::from_fn(move || {
        if offset >= raw.len() {
            return None;
        }
        let start = offset;
        while offset < raw.len() && !matches!(raw[offset], b'\n' | b'\r') {
            offset += 1;
        }
        let end = offset;
        if offset < raw.len() {
            let marker = raw[offset];
            offset += 1;
            if marker == b'\r' && offset < raw.len() && raw[offset] == b'\n' {
                offset += 1;
            }
        }
        Some((&raw[start..end], &raw[end..offset]))
    })
}
fn advance_membership_claim(previous: &Value, request: &Value) -> Result<Value, ItemRefusal> {
    let fields = request["fields"]
        .as_object()
        .ok_or_else(|| bad("membership correction field patch"))?;
    let transition = request.get("layer_transition").filter(|v| !v.is_null());
    if fields.is_empty()
        || fields.keys().any(|key| {
            if transition.is_some() {
                key != "assertion_layer"
            } else {
                ![
                    "qualifiers",
                    "evidence_refs",
                    "counterevidence_refs",
                    "alternative_claim_refs",
                    "supporting_quotes",
                    "epistemic_status",
                    "confidence",
                    "object",
                ]
                .contains(&key.as_str())
            }
        })
    {
        return Err(bad(
            "membership correction cannot change identity/maker/endpoints",
        ));
    }
    if let Some(transition) = transition {
        keys(transition, &["from", "to"])?;
        let from = text(transition, "from")?;
        let to = text(transition, "to")?;
        let layer = |value: &str| {
            !value.is_empty()
                && value.len() <= 64
                && value.as_bytes()[0].is_ascii_lowercase()
                && value
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        };
        if !layer(from)
            || !layer(to)
            || from == to
            || previous["assertion_layer"] != from
            || fields["assertion_layer"] != to
        {
            return Err(bad("membership exact retained layer transition"));
        }
    }
    if fields.contains_key("object")
        && (!previous["object"].is_object() || !fields["object"].is_object())
    {
        return Err(bad("membership correction cannot change identity endpoint"));
    }
    let mut result = previous.clone();
    for (key, value) in fields {
        if key == "qualifiers" {
            let patch = value
                .as_object()
                .ok_or_else(|| bad("membership qualifier correction patch"))?;
            let target = result
                .as_object_mut()
                .unwrap()
                .entry(key.clone())
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| bad("membership qualifiers object"))?;
            target.extend(patch.iter().map(|(k, v)| (k.clone(), v.clone())));
        } else {
            result
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), value.clone());
        }
    }
    if &result == previous {
        return Err(bad("membership correction did not change source"));
    }
    result.as_object_mut().unwrap().insert(
        "claim_version".into(),
        json!(
            integer(previous, "claim_version")?
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?
        ),
    );
    Ok(result)
}

fn j(value: &Value) -> Result<JsonValue, ItemRefusal> {
    ordered(&serde_json::to_vec(value).map_err(|_| bad("JSON value encoding"))?)
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (tos_foundation::JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn string(s: &str) -> JsonValue {
    JsonValue::String(tos_foundation::JsonString::from_utf8(s))
}
fn set(value: &mut JsonValue, key: &str, new: JsonValue) -> Result<(), ItemRefusal> {
    let JsonValue::Object(fields) = value else {
        return Err(bad("ordered object required"));
    };
    if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = new;
    } else {
        fields.push((tos_foundation::JsonString::from_utf8(key), new));
    }
    Ok(())
}
fn ref_ordered(v: &Value, id: &str, version: &str) -> Result<JsonValue, ItemRefusal> {
    let r = reference(v, id, version)?;
    Ok(object(vec![
        ("id", j(&r["id"])?),
        ("version", j(&r["version"])?),
        ("digest", j(&r["digest"])?),
    ]))
}
fn refs_ordered<T: AsRef<[u8]>>(files: &[(String, T)]) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    tos_foundation::JsonString::from_utf8(name),
                    object(vec![
                        (
                            "sha256",
                            string(&Digest256::of_bytes(raw.as_ref()).to_prefixed()),
                        ),
                        (
                            "bytes",
                            j(&json!(raw.as_ref().len())).expect("bounded byte length"),
                        ),
                    ]),
                )
            })
            .collect(),
    )
}
fn forms(
    source: &JsonValue,
    previous: Option<&JsonValue>,
    selections: &Value,
    principal: &str,
    claim: bool,
    available: usize,
) -> Result<(JsonValue, JsonValue), ItemRefusal> {
    use crate::source_forms::source_copy_kernel as kernel;
    let fail = |e| ItemRefusal::Unsupported(format!("compound source-copy forms: {e:?}"));
    let selections = selections
        .as_array()
        .ok_or_else(|| bad("form selections"))?;
    if let Some(previous) = previous {
        let old = previous
            .object_get("forms")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| bad("previous forms"))?;
        for form in old {
            let id = form
                .object_get("form_id")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| bad("previous form id"))?;
            if !selections.iter().any(|s| s["form_id"] == id)
                || form
                    .object_get("content")
                    .and_then(|v| v.object_get("kind"))
                    .and_then(JsonValue::as_str)
                    != Some("source-copy")
            {
                return Err(bad("parent explicitly rebinds all source-copy forms"));
            }
        }
    }
    let fields = kernel::metadata_fields(source).map_err(fail)?;
    let mut fields_state = std::mem::size_of::<Vec<kernel::FormField>>();
    for field in &fields {
        let strings = field.id.len() + field.pointer.len() + field.role.len();
        let context = field
            .context
            .iter()
            .try_fold(0usize, |n, s| {
                n.checked_add(std::mem::size_of::<String>() + s.len())
            })
            .ok_or(ItemRefusal::Budget)?;
        fields_state = fields_state
            .checked_add(std::mem::size_of::<kernel::FormField>())
            .and_then(|n| n.checked_add(strings))
            .and_then(|n| n.checked_add(context))
            .and_then(|n| {
                n.checked_add(
                    crate::record_biblio_cut::ordered_state(&field.language)
                        .ok()?
                        .checked_sub(std::mem::size_of::<JsonValue>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    crate::record_biblio_cut::ordered_state(&field.script)
                        .ok()?
                        .checked_sub(std::mem::size_of::<JsonValue>())?,
                )
            })
            .ok_or(ItemRefusal::Budget)?;
    }
    let guard = |used: usize| {
        if used > available {
            Err(ItemRefusal::BudgetCheck {
                check: "compound source-copy logical workspace",
                used: Some(used as u64),
                limit: Some(available as u64),
            })
        } else {
            Ok(())
        }
    };
    // Canonical hash/equality helpers emit at most two independent buffers at
    // once. Price the actual selected source/prior bytes, not a raw multiplier.
    let mut codec_indexes = crate::record_biblio_cut::ordered_emit_state(source)?;
    guard(
        fields_state
            .checked_add(codec_indexes)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let mut codec_wire =
        canonical_count_v1(source, CanonicalProfile::SourceCommandInputV1, limits())
            .map_err(|error| ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
    let prior_codec = if let Some(previous) = previous {
        guard(
            fields_state
                .checked_add(crate::record_biblio_cut::ordered_emit_state(previous)?)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let bytes = canonical_count_v1(previous, CanonicalProfile::SourceCommandInputV1, limits())
            .map_err(|error| ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
        codec_wire = codec_wire.max(bytes);
        codec_indexes = codec_indexes.max(crate::record_biblio_cut::ordered_emit_state(previous)?);
        bytes
            .checked_add(crate::record_biblio_cut::ordered_codec_state(previous)?)
            .ok_or(ItemRefusal::Budget)?
    } else {
        0
    };
    guard(
        fields_state
            .checked_add(
                codec_wire
                    .checked_add(codec_indexes)
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let subject = kernel::metadata_subject(source).map_err(fail)?;
    let subject_state = crate::record_biblio_cut::ordered_state(&subject)?;
    let empty = if previous.is_none() {
        Some(kernel::empty_set(&subject))
    } else {
        None
    };
    let empty_state = empty
        .as_ref()
        .map(crate::record_biblio_cut::ordered_state)
        .transpose()?
        .unwrap_or(0);
    guard(
        fields_state
            .checked_add(subject_state)
            .and_then(|n| n.checked_add(empty_state))
            .and_then(|n| n.checked_add(codec_wire.checked_add(codec_indexes)?))
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let mut changes = Vec::new();
    let mut changes_state = std::mem::size_of::<Vec<JsonValue>>();
    for selection in selections {
        let field_id = text(selection, "field_id")?;
        let selected = fields
            .iter()
            .find(|field| field.id == field_id)
            .ok_or_else(|| {
                fail(kernel::FormMechanicsError::Invalid(
                    "unknown source field selector",
                ))
            })?;
        let change = kernel::prepared_change(
            previous
                .or(empty.as_ref())
                .ok_or_else(|| bad("form preparation base"))?,
            &subject,
            principal,
            text(selection, "form_id")?,
            selected,
        )
        .map_err(fail)?;
        changes_state = changes_state
            .checked_add(crate::record_biblio_cut::ordered_state(&change)?)
            .ok_or(ItemRefusal::Budget)?;
        let indexes = crate::record_biblio_cut::ordered_emit_state(&change)?;
        guard(
            fields_state
                .checked_add(changes_state)
                .and_then(|n| n.checked_add(subject_state))
                .and_then(|n| n.checked_add(empty_state))
                .and_then(|n| n.checked_add(indexes))
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let bytes = canonical_count_v1(&change, CanonicalProfile::SourceCommandInputV1, limits())
            .map_err(|error| {
            ItemRefusal::Unsupported(format!("compound canonical: {error:?}"))
        })?;
        codec_wire = codec_wire.max(bytes);
        codec_indexes = codec_indexes.max(crate::record_biblio_cut::ordered_emit_state(&change)?);
        guard(
            fields_state
                .checked_add(changes_state)
                .and_then(|n| n.checked_add(subject_state))
                .and_then(|n| n.checked_add(empty_state))
                .and_then(|n| n.checked_add(codec_wire.checked_add(codec_indexes)?))
                .ok_or(ItemRefusal::Budget)?,
        )?;
        changes.push(change);
    }
    drop(empty);
    // apply retains one successor clone plus its canonical predecessor parse;
    // newly prepared forms are already owned by changes above.
    guard(
        fields_state
            .checked_add(changes_state)
            .and_then(|n| n.checked_add(subject_state))
            .and_then(|n| n.checked_add(prior_codec))
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let result = kernel::apply_form_changes(previous, &subject, &changes).map_err(fail)?;
    let result_state = crate::record_biblio_cut::ordered_state(&result)?;
    let base = fields_state
        .checked_add(changes_state)
        .and_then(|n| n.checked_add(subject_state))
        .and_then(|n| n.checked_add(result_state))
        .ok_or(ItemRefusal::Budget)?;
    guard(
        base.checked_add(crate::record_biblio_cut::ordered_emit_state(&result)?)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let result_bytes =
        canonical_count_v1(&result, CanonicalProfile::SourceCommandInputV1, limits())
            .map_err(|error| ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
    codec_wire = codec_wire.max(result_bytes);
    codec_indexes = codec_indexes.max(crate::record_biblio_cut::ordered_emit_state(&result)?);
    let equality_buffers = codec_wire
        .checked_add(
            codec_wire
                .checked_add(codec_indexes)
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let forms = result
        .object_get("forms")
        .and_then(JsonValue::as_array)
        .map_or(0, |v| v.len());
    let prior = result
        .object_get("prior_forms")
        .and_then(JsonValue::as_array)
        .map_or(0, |v| v.len());
    // Borrowed history indexes, materializer subject and one field-match Vec.
    let indexes = (forms + prior) * std::mem::size_of::<((&str, u64), &JsonValue)>()
        + forms * std::mem::size_of::<&str>()
        + fields.len() * std::mem::size_of::<&kernel::FormField>();
    let inner = subject_state
        .checked_add(indexes)
        .and_then(|n| n.checked_add(equality_buffers))
        .ok_or(ItemRefusal::Budget)?;
    guard(base.checked_add(inner).ok_or(ItemRefusal::Budget)?)?;
    let views = kernel::materialize_source_forms_from_fields(
        source,
        &result,
        &fields,
        available
            .checked_sub(base)
            .and_then(|n| n.checked_sub(inner))
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let views_state = views
        .iter()
        .try_fold(std::mem::size_of::<Vec<JsonValue>>(), |n, v| {
            n.checked_add(crate::record_biblio_cut::ordered_state(v).ok()?)
        })
        .ok_or(ItemRefusal::Budget)?;
    guard(base.checked_add(views_state).ok_or(ItemRefusal::Budget)?)?;
    if !views
        .iter()
        .all(|v| v.object_get("state").and_then(JsonValue::as_str) == Some("ready"))
        || !views.iter().any(|v| {
            v.object_get("role").and_then(JsonValue::as_str)
                == Some(if claim { "statement" } else { "name" })
        })
    {
        return Err(bad("ready source copies require name/statement"));
    }
    let refs = changes
        .iter()
        .map(|c| {
            kernel::form_reference(
                c.object_get("form")
                    .ok_or_else(|| bad("prepared form missing"))?,
            )
            .map_err(fail)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refs = JsonValue::Array(refs);
    guard(
        base.checked_add(views_state)
            .and_then(|n| n.checked_add(crate::record_biblio_cut::ordered_state(&refs).ok()?))
            .ok_or(ItemRefusal::Budget)?,
    )?;
    Ok((result, refs))
}

fn slice_rows_state(rows: &[(String, &[u8])]) -> Result<usize, ItemRefusal> {
    rows.iter()
        .try_fold(
            std::mem::size_of::<Vec<(String, &[u8])>>(),
            |sum, (name, _)| {
                sum.checked_add(std::mem::size_of::<(String, &[u8])>())?
                    .checked_add(name.len())
            },
        )
        .ok_or(ItemRefusal::Budget)
}
fn rows_state(rows: &[(String, Vec<u8>)], payloads: bool) -> Result<usize, ItemRefusal> {
    rows.iter()
        .try_fold(
            std::mem::size_of::<Vec<(String, Vec<u8>)>>(),
            |sum, (name, raw)| {
                sum.checked_add(std::mem::size_of::<(String, Vec<u8>)>())?
                    .checked_add(name.len())?
                    .checked_add(if payloads { raw.len() } else { 0 })
            },
        )
        .ok_or(ItemRefusal::Budget)
}
fn package_state(files: &Package) -> Result<usize, ItemRefusal> {
    files
        .iter()
        .try_fold(std::mem::size_of::<Package>(), |sum, (name, raw)| {
            sum.checked_add(std::mem::size_of::<(String, Vec<u8>)>())?
                .checked_add(name.len())?
                .checked_add(raw.len())
        })
        .ok_or(ItemRefusal::Budget)
}
struct Reconstructed {
    scope: Value,
    request: Value,
    parent_receipt: Value,
    child: Package,
    receipt: Value,
}
// The one maintained byte recipe pauses after parent/child buffers exist.
// The Work writer supplies its real native capture at that boundary; retained
// readers supply the historical capture and still verify transaction custody.
struct CompoundCore<'a> {
    kind: CompoundKind,
    authority: Cow<'a, Value>,
    request: Value,
    request_digest: String,
    before: Package,
    old: Value,
    revised: Value,
    parent_receipt: Value,
    parent_receipt_ordered: JsonValue,
    parent_files: Vec<(String, Vec<u8>)>,
    child_files: Vec<(String, Vec<u8>)>,
    parent_forms: JsonValue,
    expression_forms: JsonValue,
    claim_forms: JsonValue,
    parent_refs: JsonValue,
    expression_refs: JsonValue,
    claim_refs: JsonValue,
    grammar_digests: BTreeMap<String, String>,
    id: String,
    archive_path: String,
    recorded_at: String,
}
struct WorkGrammar {
    dependencies: BTreeMap<String, Digest256>,
    digests: BTreeMap<String, String>,
    claim_sha256: Digest256,
    binding: CutExecutionBinding,
}

trait NativeCompoundSchema: CutSchemaExecutor {
    fn cut_execution_binding(&self) -> Option<CutExecutionBinding>;
}

impl<S: CutSchemaExecutor + CutSchemaReceiptRange> NativeCompoundSchema for S {
    fn cut_execution_binding(&self) -> Option<CutExecutionBinding> {
        Some(CutSchemaReceiptRange::execution_binding(self))
    }
}

impl<I: Copy + Eq> NativeCompoundSchema for CandidateCutWorkerSchemaExecutor<I> {
    fn cut_execution_binding(&self) -> Option<CutExecutionBinding> {
        None
    }
}

struct LocalClaimValidation {
    dependency_digests: BTreeMap<String, Digest256>,
    dependency_bytes_read: u64,
}

fn merge_candidate_diagnostics_cost(
    target: &mut crate::record_rules::CandidateLocalClaimDiagnosticsCost,
    source: crate::record_rules::CandidateLocalClaimDiagnosticsCost,
) -> Result<(), ItemRefusal> {
    macro_rules! add {
        ($field:ident) => {
            target.$field = target
                .$field
                .checked_add(source.$field)
                .ok_or(ItemRefusal::Budget)?;
        };
    }
    add!(completed_exchanges);
    add!(issue_count);
    add!(schema_resource_bytes);
    add!(schema_resource_buffer_bytes);
    add!(input_instance_bytes);
    add!(input_instance_buffer_bytes);
    add!(input_metadata_bytes);
    add!(request_bytes);
    add!(request_buffer_bytes);
    add!(response_bytes);
    add!(response_buffer_bytes);
    add!(worker_cpu_micros);
    add!(retained_state_bytes);
    add!(accounted_state_bytes);
    Ok(())
}
fn preparation_grammar_from_cut(
    kind: CompoundKind,
    reader: &mut NativeCompoundReader<'_>,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_raw: &[u8],
) -> Result<WorkGrammar, ItemRefusal> {
    let extra = preparation_grammar_extra(kind)?;
    let mut local_limits = reader.limits;
    local_limits.max_state_bytes = reader
        .limits
        .max_state_bytes
        .checked_sub(reader.state)
        .ok_or(ItemRefusal::Budget)?;
    local_limits.max_total_bytes = reader
        .limits
        .max_total_bytes
        .checked_sub(reader.bytes)
        .ok_or(ItemRefusal::Budget)?;
    let mut local = crate::record_rules::validate_source_claim_from_cut(
        reader.cut(),
        claim_raw,
        schemas,
        local_limits,
        reader.cancelled,
    )?;
    if !local.issues.is_empty() || local.execution_binding != schemas.execution_binding() {
        return Err(bad("Work Claim local profile/binding"));
    }
    let claim_sha256 = local.source_input_sha256;
    let binding = local.execution_binding.clone();
    let dependencies = std::mem::take(&mut local.dependency_digests);
    drop(local);
    let dependency_slots = dependencies
        .keys()
        .try_fold(0usize, |n, path| {
            n.checked_add(std::mem::size_of::<(String, Digest256)>() + path.len())
        })
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(
        dependency_slots
            .checked_add(std::mem::size_of::<BTreeMap<String, Digest256>>())
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let grammar_slots = dependencies
        .keys()
        .map(String::as_str)
        .chain(
            extra
                .iter()
                .copied()
                .filter(|path| !dependencies.contains_key(*path)),
        )
        .try_fold(
            std::mem::size_of::<BTreeMap<String, String>>(),
            |n, path| n.checked_add(std::mem::size_of::<(String, String)>() + path.len() + 64),
        )
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(grammar_slots)?;
    let mut digests = dependencies
        .iter()
        .map(|(path, sha)| (path.clone(), sha.to_hex()))
        .collect::<BTreeMap<_, _>>();
    for &path in extra {
        if !digests.contains_key(path) {
            let raw = reader.required(path, MAX_FILE)?;
            digests.insert(path.into(), Digest256::of_bytes(&raw).to_hex());
            reader.release_temporary(std::mem::size_of::<Vec<u8>>() + raw.len());
        }
    }
    Ok(WorkGrammar {
        dependencies,
        digests,
        claim_sha256,
        binding,
    })
}
struct FinishedCompound<'a> {
    kind: CompoundKind,
    authority: Cow<'a, Value>,
    request: Value,
    before: Package,
    parent_receipt: Value,
    parent_files: Vec<(String, Vec<u8>)>,
    child_files: Vec<(String, Vec<u8>)>,
    receipt: Value,
    receipt_raw: Vec<u8>,
    id: String,
    archive_path: String,
}
/// Prepared Work buffers only. This carries no authority to publish them.
pub struct WorkExpressionCore<'a> {
    reader: NativeCompoundReader<'a>,
    prepared: CompoundCore<'a>,
    schema_binding: CutExecutionBinding,
}
pub struct WorkExpressionBytes {
    pub parent: BTreeMap<String, Vec<u8>>,
    pub child: BTreeMap<String, Vec<u8>>,
    pub parent_receipt: Value,
    pub receipt: Value,
    pub transaction_id: String,
    pub archive_path: String,
    pub reads: Vec<PredicateRead>,
    pub bytes_read: u64,
}
fn native_work_environment(environment: &Value) -> Result<(), ItemRefusal> {
    keys(
        environment,
        &[
            "runtime",
            "runtime_version",
            "runtime_artifact_sha256",
            "backend",
            "hardware_target",
            "unicode_version",
            "argv_sha256",
        ],
    )?;
    if environment
        .as_object()
        .ok_or_else(|| bad("native Work environment"))?
        .values()
        .any(|v| v.as_str().is_none_or(str::is_empty))
        || environment["runtime"] != "native ELF process"
        || environment["backend"] != "tos-command source serialization"
        || environment["unicode_version"] != "16.0.0 source-command whitespace profile"
        || ["runtime_artifact_sha256", "argv_sha256"]
            .iter()
            .any(|key| {
                hash(&format!(
                    "sha256:{}",
                    environment[*key].as_str().unwrap_or("")
                ))
                .is_err()
            })
    {
        return Err(bad("native Work observed environment"));
    }
    Ok(())
}
impl WorkExpressionCore<'_> {
    pub fn authorization(&self) -> &Value {
        &self.prepared.authority
    }
    pub fn archive_path(&self) -> &str {
        &self.prepared.archive_path
    }
    pub fn transaction_id(&self) -> &str {
        &self.prepared.id
    }
    pub fn reads(&self) -> &[PredicateRead] {
        &self.reader.reads
    }
    pub fn bytes_read(&self) -> u64 {
        self.reader.bytes
    }
    /// The maintained Work grammar dependency group, not writer authority.
    pub fn grammar_digests(&self) -> &BTreeMap<String, String> {
        &self.prepared.grammar_digests
    }
    /// Only path slots are copied; the actual source buffers remain borrowed.
    pub fn outputs(&self) -> Result<Vec<(String, &[u8])>, ItemRefusal> {
        let scope = &self.prepared.authority["scope"];
        let parent_home = parent(text(scope, self.prepared.kind.parent_path())?)?;
        let child_home = self.prepared.kind.publication_home(scope)?;
        let count = self
            .prepared
            .parent_files
            .len()
            .checked_add(self.prepared.child_files.len())
            .ok_or(ItemRefusal::Budget)?;
        if count > 64 {
            return Err(ItemRefusal::Budget);
        }
        let mut slots = std::mem::size_of::<Vec<(String, &[u8])>>();
        for (home, files) in [
            (parent_home, &self.prepared.parent_files),
            (child_home, &self.prepared.child_files),
        ] {
            for (name, _) in files {
                slots = slots
                    .checked_add(std::mem::size_of::<(String, &[u8])>())
                    .and_then(|n| n.checked_add(home.len()))
                    .and_then(|n| n.checked_add(1))
                    .and_then(|n| n.checked_add(name.len()))
                    .ok_or(ItemRefusal::Budget)?;
            }
        }
        let used = self
            .reader
            .state
            .checked_add(slots)
            .ok_or(ItemRefusal::Budget)?;
        if used > self.reader.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "prepared Work output path slots",
                used: Some(used as u64),
                limit: Some(self.reader.limits.max_state_bytes as u64),
            });
        }
        let mut rows = Vec::with_capacity(count);
        for (home, files) in [
            (parent_home, &self.prepared.parent_files),
            (child_home, &self.prepared.child_files),
        ] {
            for (name, raw) in files {
                rows.push((format!("{home}/{name}"), raw.as_slice()));
            }
        }
        Ok(rows)
    }
    /// Consume only the prepared parent/child buffers for a descriptive
    /// preview. The actual byte buffers move; no capture or receipt is made.
    pub fn into_prepared_outputs(self) -> Result<BTreeMap<String, Vec<u8>>, ItemRefusal> {
        let WorkExpressionCore {
            reader, prepared, ..
        } = self;
        let CompoundCore {
            kind,
            authority,
            parent_files,
            child_files,
            ..
        } = prepared;
        let scope = &authority["scope"];
        let parent_home = parent(text(scope, kind.parent_path())?)?;
        let child_home = kind.publication_home(scope)?;
        let count = parent_files
            .len()
            .checked_add(child_files.len())
            .ok_or(ItemRefusal::Budget)?;
        if count > 64 {
            return Err(ItemRefusal::Budget);
        }
        let mut slots = std::mem::size_of::<BTreeMap<String, Vec<u8>>>();
        for (home, files) in [(parent_home, &parent_files), (child_home, &child_files)] {
            for (name, _) in files {
                slots = slots
                    .checked_add(std::mem::size_of::<(String, Vec<u8>)>())
                    .and_then(|n| n.checked_add(home.len()))
                    .and_then(|n| n.checked_add(1))
                    .and_then(|n| n.checked_add(name.len()))
                    .ok_or(ItemRefusal::Budget)?;
            }
        }
        let used = reader.state.checked_add(slots).ok_or(ItemRefusal::Budget)?;
        if used > reader.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "prepared Work output map slots",
                used: Some(used as u64),
                limit: Some(reader.limits.max_state_bytes as u64),
            });
        }
        let mut outputs = BTreeMap::new();
        for (home, files) in [(parent_home, parent_files), (child_home, child_files)] {
            for (name, raw) in files {
                if outputs.insert(format!("{home}/{name}"), raw).is_some() {
                    return Err(bad("prepared Work duplicate output path"));
                }
            }
        }
        Ok(outputs)
    }
}
/// Bind actual native capture bytes to prepared buffers. CMD must separately
/// check selected software, physical source and journal fences before publish.
pub fn finish_work_expression_bytes(
    core: WorkExpressionCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<WorkExpressionBytes, ItemRefusal> {
    finish_prepared_compound_bytes(core, environment_raw, event_raw, schemas, true)
}
fn finish_prepared_compound_bytes(
    core: WorkExpressionCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
    native_producer: bool,
) -> Result<WorkExpressionBytes, ItemRefusal> {
    let WorkExpressionCore {
        mut reader,
        prepared,
        schema_binding,
    } = core;
    check(reader.limits.deadline, reader.cancelled)?;
    if schemas.execution_binding() != schema_binding
        || environment_raw.len() > MAX_FILE
        || event_raw.len() > MAX_FILE
    {
        return Err(bad("native Work capture worker/size binding"));
    }
    let environment = reader.decoded(environment_raw)?;
    if native_producer {
        native_work_environment(&environment)?;
    }
    let mut expected_environment_raw = canonical(&environment)?;
    expected_environment_raw.push(b'\n');
    if environment_raw != expected_environment_raw {
        return Err(bad("native Work environment exact canonical bytes"));
    }
    let kind = prepared.kind;
    let finished = reader.finish_compound_core(prepared, &environment, Some(event_raw), schemas)?;
    let FinishedCompound {
        kind: _,
        authority: _,
        request: _,
        before: _,
        parent_receipt,
        parent_files,
        mut child_files,
        receipt,
        receipt_raw,
        id,
        archive_path,
    } = finished;
    if parent_files
        .iter()
        .chain(child_files.iter())
        .any(|(_, raw)| raw.len() > MAX_FILE)
        || receipt_raw.len() > MAX_FILE
    {
        return Err(ItemRefusal::Budget);
    }
    child_files.push((kind.receipt_file().into(), receipt_raw));
    let parent = parent_files.into_iter().collect::<BTreeMap<_, _>>();
    let child = child_files.into_iter().collect::<BTreeMap<_, _>>();
    Ok(WorkExpressionBytes {
        parent,
        child,
        parent_receipt,
        receipt,
        transaction_id: id,
        archive_path,
        reads: reader.reads,
        bytes_read: reader.bytes,
    })
}
/// Construct the maintained Work/Expression buffers from a separately selected
/// current Work package. The caller still owns physical publication authority.
pub fn prepare_work_expression_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    request_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    authorize: impl FnOnce(&BTreeMap<String, String>) -> Result<Value, ItemRefusal>,
) -> Result<WorkExpressionCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || request_raw.len() > MAX_FILE {
        return Err(bad("Work source/schema revision or request size"));
    }
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("Work recorded aware instant"))?;
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    prepare_native_with_reader(
        CompoundKind::WorkExpression,
        None,
        reader,
        schemas,
        scope,
        Cow::Borrowed(request_raw),
        owned_before,
        recorded_at,
        None,
        authorize,
    )
}

/// Produce a descriptive Work preview from one selected Claim grammar pass.
/// The callback builds its request and authorization from those exact digests;
/// the later create operation independently rereads the current cut.
pub fn prepare_work_expression_preview_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    proposed_claim_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    build_request_and_authorization: impl FnOnce(
        &BTreeMap<String, String>,
    ) -> Result<(Vec<u8>, Value), ItemRefusal>,
) -> Result<WorkExpressionCore<'a>, ItemRefusal> {
    prepare_typed_compound_preview_bytes(
        CompoundKind::WorkExpression,
        cut,
        schemas,
        scope,
        proposed_claim_raw,
        owned_before,
        recorded_at,
        limits,
        cancelled,
        build_request_and_authorization,
    )
}

fn prepare_typed_compound_preview_bytes<'a>(
    kind: CompoundKind,
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    proposed_claim_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    build_request_and_authorization: impl FnOnce(
        &BTreeMap<String, String>,
    ) -> Result<(Vec<u8>, Value), ItemRefusal>,
) -> Result<WorkExpressionCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || proposed_claim_raw.len() > MAX_FILE
    {
        return Err(bad("Work grammar source/schema or Claim size"));
    }
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("Work recorded aware instant"))?;
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    // The caller's proposed Claim stays live during the callback and byte
    // recipe, so its state shares the same operation envelope as the grammar.
    reader.temporary(std::mem::size_of::<Vec<u8>>() + proposed_claim_raw.len())?;
    let claim = reader.decoded(proposed_claim_raw)?;
    let mut expected = canonical(&claim)?;
    expected.push(b'\n');
    let expected = reader.buffer(expected)?;
    if proposed_claim_raw != expected {
        return Err(bad("Work proposed Claim exact canonical bytes"));
    }
    let decoded_state = crate::record_biblio_cut::decoded_state(&claim)?;
    drop(claim);
    reader.release_temporary(decoded_state);
    let encoded_state = std::mem::size_of::<Vec<u8>>() + expected.len();
    drop(expected);
    reader.release_temporary(encoded_state);
    let grammar = preparation_grammar_from_cut(kind, &mut reader, schemas, proposed_claim_raw)?;
    if grammar.binding != schemas.execution_binding() {
        return Err(bad("Work preview schema binding changed"));
    }
    let (request_raw, authority) = build_request_and_authorization(&grammar.digests)?;
    prepare_native_with_reader(
        kind,
        None,
        reader,
        schemas,
        scope,
        Cow::Owned(request_raw),
        owned_before,
        recorded_at,
        Some((proposed_claim_raw, grammar)),
        |_| Ok(authority),
    )
}

/// Typed maintained Collection membership recipe over the existing bounded core.
pub struct CollectionMembershipCore<'a> {
    inner: WorkExpressionCore<'a>,
}
pub type CollectionMembershipBytes = WorkExpressionBytes;
impl CollectionMembershipCore<'_> {
    pub fn authorization(&self) -> &Value {
        self.inner.authorization()
    }
    pub fn archive_path(&self) -> &str {
        self.inner.archive_path()
    }
    pub fn transaction_id(&self) -> &str {
        self.inner.transaction_id()
    }
    pub fn reads(&self) -> &[PredicateRead] {
        self.inner.reads()
    }
    pub fn bytes_read(&self) -> u64 {
        self.inner.bytes_read()
    }
    pub fn grammar_digests(&self) -> &BTreeMap<String, String> {
        self.inner.grammar_digests()
    }
    pub fn outputs(&self) -> Result<Vec<(String, &[u8])>, ItemRefusal> {
        self.inner.outputs()
    }
    pub fn into_prepared_outputs(self) -> Result<BTreeMap<String, Vec<u8>>, ItemRefusal> {
        self.inner.into_prepared_outputs()
    }
}
pub fn prepare_collection_membership_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    request_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    authorize: impl FnOnce(&BTreeMap<String, String>) -> Result<Value, ItemRefusal>,
) -> Result<CollectionMembershipCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || request_raw.len() > MAX_FILE {
        return Err(bad("compound source/schema revision or request size"));
    }
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("compound recorded aware instant"))?;
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    prepare_native_with_reader(
        CompoundKind::CollectionWork,
        None,
        reader,
        schemas,
        scope,
        Cow::Borrowed(request_raw),
        owned_before,
        recorded_at,
        None,
        authorize,
    )
    .map(|inner| CollectionMembershipCore { inner })
}
pub fn prepare_collection_membership_preview_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    proposed_claim_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    build_request_and_authorization: impl FnOnce(
        &BTreeMap<String, String>,
    ) -> Result<(Vec<u8>, Value), ItemRefusal>,
) -> Result<CollectionMembershipCore<'a>, ItemRefusal> {
    prepare_typed_compound_preview_bytes(
        CompoundKind::CollectionWork,
        cut,
        schemas,
        scope,
        proposed_claim_raw,
        owned_before,
        recorded_at,
        limits,
        cancelled,
        build_request_and_authorization,
    )
    .map(|inner| CollectionMembershipCore { inner })
}
/// Finish one actual native producer capture; its environment stays native-only.
pub fn finish_collection_membership_bytes(
    core: CollectionMembershipCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<CollectionMembershipBytes, ItemRefusal> {
    finish_work_expression_bytes(core.inner, environment_raw, event_raw, schemas)
}
/// Reconstruct exact retained Python/native capture after CMD proved journal custody.
pub fn restore_collection_membership_bytes(
    core: CollectionMembershipCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<CollectionMembershipBytes, ItemRefusal> {
    finish_prepared_compound_bytes(core.inner, environment_raw, event_raw, schemas, false)
}
/// Typed maintained ExpressionResponsibility recipe over the existing bounded compound kernel.
/// Its output is metadata transport, never semantic or publication admission.
pub struct ExpressionResponsibilityCore<'a> {
    inner: WorkExpressionCore<'a>,
}
pub type ExpressionResponsibilityBytes = WorkExpressionBytes;
impl ExpressionResponsibilityCore<'_> {
    pub fn authorization(&self) -> &Value {
        self.inner.authorization()
    }
    pub fn archive_path(&self) -> &str {
        self.inner.archive_path()
    }
    pub fn transaction_id(&self) -> &str {
        self.inner.transaction_id()
    }
    pub fn reads(&self) -> &[PredicateRead] {
        self.inner.reads()
    }
    pub fn bytes_read(&self) -> u64 {
        self.inner.bytes_read()
    }
    pub fn grammar_digests(&self) -> &BTreeMap<String, String> {
        self.inner.grammar_digests()
    }
    pub fn outputs(&self) -> Result<Vec<(String, &[u8])>, ItemRefusal> {
        self.inner.outputs()
    }
    pub fn into_prepared_outputs(self) -> Result<BTreeMap<String, Vec<u8>>, ItemRefusal> {
        self.inner.into_prepared_outputs()
    }
}
pub fn prepare_expression_responsibility_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    request_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    authorize: impl FnOnce(&BTreeMap<String, String>) -> Result<Value, ItemRefusal>,
) -> Result<ExpressionResponsibilityCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || request_raw.len() > MAX_FILE {
        return Err(bad("compound source/schema revision or request size"));
    }
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("compound recorded aware instant"))?;
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    prepare_native_with_reader(
        CompoundKind::ExpressionResponsibility,
        None,
        reader,
        schemas,
        scope,
        Cow::Borrowed(request_raw),
        owned_before,
        recorded_at,
        None,
        authorize,
    )
    .map(|inner| ExpressionResponsibilityCore { inner })
}
pub fn prepare_expression_responsibility_preview_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    proposed_claim_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    build_request_and_authorization: impl FnOnce(
        &BTreeMap<String, String>,
    ) -> Result<(Vec<u8>, Value), ItemRefusal>,
) -> Result<ExpressionResponsibilityCore<'a>, ItemRefusal> {
    prepare_typed_compound_preview_bytes(
        CompoundKind::ExpressionResponsibility,
        cut,
        schemas,
        scope,
        proposed_claim_raw,
        owned_before,
        recorded_at,
        limits,
        cancelled,
        build_request_and_authorization,
    )
    .map(|inner| ExpressionResponsibilityCore { inner })
}
/// Finish one actual native producer capture; its environment stays native-only.
pub fn finish_expression_responsibility_bytes(
    core: ExpressionResponsibilityCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<ExpressionResponsibilityBytes, ItemRefusal> {
    finish_work_expression_bytes(core.inner, environment_raw, event_raw, schemas)
}
/// Reconstruct exact retained Python/native capture after CMD proved journal custody.
pub fn restore_expression_responsibility_bytes(
    core: ExpressionResponsibilityCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<ExpressionResponsibilityBytes, ItemRefusal> {
    finish_prepared_compound_bytes(core.inner, environment_raw, event_raw, schemas, false)
}
/// Typed maintained ExpressionEdition recipe over the existing bounded compound kernel.
/// Its output is metadata transport, never semantic or publication admission.
pub struct ExpressionEditionCore<'a> {
    inner: WorkExpressionCore<'a>,
}
pub type ExpressionEditionBytes = WorkExpressionBytes;
impl ExpressionEditionCore<'_> {
    pub fn authorization(&self) -> &Value {
        self.inner.authorization()
    }
    pub fn archive_path(&self) -> &str {
        self.inner.archive_path()
    }
    pub fn transaction_id(&self) -> &str {
        self.inner.transaction_id()
    }
    pub fn reads(&self) -> &[PredicateRead] {
        self.inner.reads()
    }
    pub fn bytes_read(&self) -> u64 {
        self.inner.bytes_read()
    }
    pub fn grammar_digests(&self) -> &BTreeMap<String, String> {
        self.inner.grammar_digests()
    }
    pub fn outputs(&self) -> Result<Vec<(String, &[u8])>, ItemRefusal> {
        self.inner.outputs()
    }
    pub fn into_prepared_outputs(self) -> Result<BTreeMap<String, Vec<u8>>, ItemRefusal> {
        self.inner.into_prepared_outputs()
    }
}
pub fn prepare_expression_edition_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    request_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    authorize: impl FnOnce(&BTreeMap<String, String>) -> Result<Value, ItemRefusal>,
) -> Result<ExpressionEditionCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || request_raw.len() > MAX_FILE {
        return Err(bad("compound source/schema revision or request size"));
    }
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("compound recorded aware instant"))?;
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    prepare_native_with_reader(
        CompoundKind::ExpressionEdition,
        None,
        reader,
        schemas,
        scope,
        Cow::Borrowed(request_raw),
        owned_before,
        recorded_at,
        None,
        authorize,
    )
    .map(|inner| ExpressionEditionCore { inner })
}
pub fn prepare_expression_edition_preview_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    proposed_claim_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    build_request_and_authorization: impl FnOnce(
        &BTreeMap<String, String>,
    ) -> Result<(Vec<u8>, Value), ItemRefusal>,
) -> Result<ExpressionEditionCore<'a>, ItemRefusal> {
    prepare_typed_compound_preview_bytes(
        CompoundKind::ExpressionEdition,
        cut,
        schemas,
        scope,
        proposed_claim_raw,
        owned_before,
        recorded_at,
        limits,
        cancelled,
        build_request_and_authorization,
    )
    .map(|inner| ExpressionEditionCore { inner })
}
/// Finish one actual native producer capture; its environment stays native-only.
pub fn finish_expression_edition_bytes(
    core: ExpressionEditionCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<ExpressionEditionBytes, ItemRefusal> {
    finish_work_expression_bytes(core.inner, environment_raw, event_raw, schemas)
}
/// Reconstruct exact retained Python/native capture after CMD proved journal custody.
pub fn restore_expression_edition_bytes(
    core: ExpressionEditionCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<ExpressionEditionBytes, ItemRefusal> {
    finish_prepared_compound_bytes(core.inner, environment_raw, event_raw, schemas, false)
}
/// Read-only grammar closure for the maintained byte-only Item route.
/// It does not make acquired metadata, a receipt, or a publication grant.
pub struct EditionItemGrammarObservation {
    pub grammar_digests: BTreeMap<String, String>,
    pub reads: Vec<PredicateRead>,
    pub bytes_read: u64,
    pub returned_state_bytes: usize,
}
pub fn inspect_edition_item_grammar(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    edition_raw: &[u8],
    item_raw: &[u8],
    claim_raw: &[u8],
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<EditionItemGrammarObservation, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() {
        return Err(bad("Item grammar selected worker/cut"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    for raw in [edition_raw, item_raw, claim_raw] {
        if raw.len() > limits.max_member_bytes.min(MAX_FILE) {
            return Err(ItemRefusal::Budget);
        }
        reader.temporary(std::mem::size_of::<Vec<u8>>() + raw.len())?;
        account(&mut reader.bytes, raw.len(), limits.max_total_bytes)?;
    }
    let edition = reader.decoded(edition_raw)?;
    let item = reader.decoded(item_raw)?;
    let claim = reader.decoded(claim_raw)?;
    if edition["record_type"] != "edition"
        || item["record_type"] != "item"
        || !typed_id(text(&edition, "record_id")?, "edition")
        || !typed_id(text(&item, "record_id")?, "item")
        || claim["predicate"] != "exemplified_by"
        || claim["subject_ref"] != edition["record_id"]
        || claim["object"] != item["record_id"]
    {
        return Err(bad("Item grammar supplied records/exemplar endpoints"));
    }
    for (label, raw) in [
        ("item-adoption-supplied-edition", edition_raw),
        ("item-adoption-supplied-item", item_raw),
    ] {
        if !schemas.check_reusing_scalar(
            label,
            raw,
            "ToS/contracts/corpus-record.schema.json",
            limits.deadline,
            cancelled,
        )? {
            return Err(bad("Item grammar supplied corpus record schema"));
        }
    }
    let trees = crate::record_biblio_cut::decoded_state(&edition)?
        .checked_add(crate::record_biblio_cut::decoded_state(&item)?)
        .and_then(|n| n.checked_add(crate::record_biblio_cut::decoded_state(&claim).ok()?))
        .ok_or(ItemRefusal::Budget)?;
    drop(edition);
    drop(item);
    drop(claim);
    reader.release_temporary(trees);
    let grammar =
        preparation_grammar_from_cut(CompoundKind::EditionItem, &mut reader, schemas, claim_raw)?;
    reader.record_work_dependencies(grammar.dependencies, &grammar.digests)?;
    reader.release_temporary(std::mem::size_of::<BTreeMap<String, Digest256>>());
    let returned_state_bytes = grammar
        .digests
        .iter()
        .try_fold(
            std::mem::size_of::<EditionItemGrammarObservation>(),
            |n, (path, sha)| {
                n.checked_add(std::mem::size_of::<(String, String)>())?
                    .checked_add(path.len())?
                    .checked_add(sha.len())
            },
        )
        .and_then(|n| {
            reader.reads.iter().try_fold(n, |n, read| {
                n.checked_add(crate::record_biblio_cut::predicate_state(read).ok()?)
            })
        })
        .ok_or(ItemRefusal::Budget)?;
    if returned_state_bytes > limits.max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    Ok(EditionItemGrammarObservation {
        grammar_digests: grammar.digests,
        reads: reader.reads,
        bytes_read: reader.bytes,
        returned_state_bytes,
    })
}

/// Prepared Edition/Item metadata only; private payload custody remains with CMD.
pub struct EditionItemCore<'a> {
    inner: WorkExpressionCore<'a>,
    descriptive_preview: bool,
}
pub type EditionItemBytes = WorkExpressionBytes;
enum ItemPreparationInput<'a> {
    Observed {
        receipt_raw: &'a [u8],
        generator: &'a str,
    },
    Preview {
        generator: &'a str,
    },
}
impl EditionItemCore<'_> {
    pub fn authorization(&self) -> &Value {
        self.inner.authorization()
    }
    pub fn archive_path(&self) -> &str {
        self.inner.archive_path()
    }
    pub fn transaction_id(&self) -> &str {
        self.inner.transaction_id()
    }
    pub fn reads(&self) -> &[PredicateRead] {
        self.inner.reads()
    }
    pub fn bytes_read(&self) -> u64 {
        self.inner.bytes_read()
    }
    pub fn grammar_digests(&self) -> &BTreeMap<String, String> {
        self.inner.grammar_digests()
    }
    pub fn outputs(&self) -> Result<Vec<(String, &[u8])>, ItemRefusal> {
        self.inner.outputs()
    }
    pub fn into_prepared_outputs(self) -> Result<BTreeMap<String, Vec<u8>>, ItemRefusal> {
        self.inner.into_prepared_outputs()
    }
    pub fn expects_request_capture(&self) -> bool {
        true
    }
}
/// Consume the genuine CMD deposit's source-safe public receipt. This verifies
/// its metadata binding, never possession, payload fixity or publication rights.
pub fn prepare_edition_item_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    request_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    byte_receipt_raw: &[u8],
    inventory_generator: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    authorize: impl FnOnce(&BTreeMap<String, String>) -> Result<Value, ItemRefusal>,
) -> Result<EditionItemCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("Item recorded aware instant"))?;
    let reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    prepare_native_with_reader(
        CompoundKind::EditionItem,
        Some(ItemPreparationInput::Observed {
            receipt_raw: byte_receipt_raw,
            generator: inventory_generator,
        }),
        reader,
        schemas,
        scope,
        Cow::Borrowed(request_raw),
        owned_before,
        recorded_at,
        None,
        authorize,
    )
    .map(|inner| EditionItemCore {
        inner,
        descriptive_preview: false,
    })
}
/// Bind the actual native process capture to the prepared Item metadata.
pub fn finish_edition_item_bytes(
    core: EditionItemCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<EditionItemBytes, ItemRefusal> {
    if core.descriptive_preview {
        return Err(bad(
            "descriptive Item preview cannot finish a publication capture",
        ));
    }
    finish_work_expression_bytes(core.inner, environment_raw, event_raw, schemas)
}
/// Reconstruct exactly the retained Item capture, including maintained Python
/// interrupted-producer events. The normal producer finish stays native-only;
/// CMD owns retained journal/capture custody and physical recovery authority.
pub fn restore_edition_item_bytes(
    core: EditionItemCore<'_>,
    environment_raw: &[u8],
    event_raw: &[u8],
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<EditionItemBytes, ItemRefusal> {
    if core.descriptive_preview {
        return Err(bad(
            "descriptive Item preview cannot restore retained capture",
        ));
    }
    finish_prepared_compound_bytes(core.inner, environment_raw, event_raw, schemas, false)
}

fn preview_item_receipt(
    reader: &mut NativeCompoundReader<'_>,
    scope: &Value,
    request: &Value,
    request_digest: &str,
) -> Result<Vec<u8>, ItemRefusal> {
    let stamp = text(request, "fixity_verified_at")?;
    let identifier = transaction_id_with_digest(
        request,
        CompoundKind::EditionItem,
        request_digest,
        &mut |value| reader.canonical_observation(value),
    )?;
    // Exact maintained preview recipe and field order, distinct from the real
    // constructor. Admit the small ordered-tree workspace before construction.
    let strings = scope
        .as_object()
        .ok_or_else(|| bad("Item scope"))?
        .values()
        .filter_map(Value::as_str)
        .try_fold(0usize, |n, v| n.checked_add(v.len()))
        .and_then(|n| n.checked_add(stamp.len().checked_mul(4)?))
        .and_then(|n| n.checked_add(identifier.len()))
        .and_then(|n| n.checked_add(text(request, "expected_configuration").ok()?.len()))
        .ok_or(ItemRefusal::Budget)?;
    let workspace = strings
        .checked_mul(4)
        .and_then(|n| n.checked_add(8192))
        .ok_or(ItemRefusal::Budget)?;
    reader.temporary(workspace)?;
    let payload = object(vec![
        ("file_id", j(&scope["file_id"])?),
        (
            "relative_path",
            string(&format!("payload/{}", text(scope, "payload_basename")?)),
        ),
        ("original_basename", j(&scope["original_basename"])?),
        ("media_type", j(&scope["media_type"])?),
        ("byte_size", j(&scope["byte_size"])?),
        ("sha256", j(&scope["sha256"])?),
    ]);
    let receipt = object(vec![
        ("schema_version", string("tos_item_deposit_receipt_v1")),
        ("transaction_id", string(&identifier)),
        (
            "owner_configuration",
            j(&request["expected_configuration"])?,
        ),
        (
            "private_stage_digest",
            string(&format!("sha256:{}", "0".repeat(64))),
        ),
        ("recovery_configuration", JsonValue::Null),
        ("file", payload),
        ("started_at", string(stamp)),
        (
            "observation_interval",
            object(vec![
                ("started_at", string(stamp)),
                ("ended_at", string(stamp)),
            ]),
        ),
        ("deposited_at", string(stamp)),
        ("original_preserved", JsonValue::Bool(true)),
        ("metadata_committed", JsonValue::Bool(false)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let raw = reader.buffer(pretty(&receipt)?)?;
    drop(receipt);
    reader.release_temporary(workspace);
    Ok(raw)
}
fn native_preparation_procedure(kind: CompoundKind) -> Result<&'static str, ItemRefusal> {
    match kind {
        CompoundKind::CollectionWork => Ok("native-collection-membership-serialization"),
        CompoundKind::WorkExpression => Ok("native-work-expression-serialization"),
        CompoundKind::EditionItem => Ok("native-item-adoption-serialization"),
        CompoundKind::ExpressionEdition => Ok("native-expression-edition-serialization"),
        CompoundKind::ExpressionResponsibility => {
            Ok("native-expression-responsibility-serialization")
        }
        _ => Err(bad("unknown actual native preparation family")),
    }
}
fn native_preparation_purpose(kind: CompoundKind) -> Result<&'static str, ItemRefusal> {
    match kind {
        CompoundKind::CollectionWork => Ok(
            "Serialize one qualified Collection membership Claim and explicit source-copy forms without judging membership.",
        ),
        CompoundKind::WorkExpression => Ok(
            "Serialize one declared Work/Expression link and explicit source-copy forms without judging content.",
        ),
        CompoundKind::EditionItem => Ok(
            "Serialize one declared Edition/Item link and explicit source-copy forms without judging content.",
        ),
        CompoundKind::ExpressionEdition => Ok(
            "Serialize one declared Expression/Edition link and explicit source-copy forms without judging content.",
        ),
        CompoundKind::ExpressionResponsibility => Ok(
            "Serialize one qualified Expression responsibility Claim and explicit source-copy forms without judging attribution.",
        ),
        _ => Err(bad("unknown actual native preparation family")),
    }
}
fn native_preparation_warning(kind: CompoundKind) -> Result<&'static str, ItemRefusal> {
    match kind {
        CompoundKind::CollectionWork => Ok(
            "Completed in-process Collection membership buffer serialization; atomic selected-metadata publication occurs afterward.",
        ),
        CompoundKind::WorkExpression => Ok(
            "Completed in-process Work/Expression buffer serialization; atomic selected-metadata publication occurs afterward.",
        ),
        CompoundKind::EditionItem => Ok(
            "Completed in-process Edition/Item buffer serialization; atomic selected-metadata publication occurs afterward.",
        ),
        CompoundKind::ExpressionEdition => Ok(
            "Completed in-process Expression/Edition buffer serialization; atomic selected-metadata publication occurs afterward.",
        ),
        CompoundKind::ExpressionResponsibility => Ok(
            "Completed in-process Expression responsibility buffer serialization; atomic selected-metadata publication occurs afterward.",
        ),
        _ => Err(bad("unknown actual native preparation family")),
    }
}
/// Produce a descriptive Item preview from one selected Claim grammar pass.
/// The callback builds its request and authorization from those exact digests;
/// the later create operation independently rereads the current cut.
pub fn prepare_edition_item_preview_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    proposed_claim_raw: &[u8],
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    inventory_generator: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    build_request_and_authorization: impl FnOnce(
        &BTreeMap<String, String>,
    ) -> Result<(Vec<u8>, Value), ItemRefusal>,
) -> Result<EditionItemCore<'a>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || proposed_claim_raw.len() > MAX_FILE
    {
        return Err(bad("Item grammar source/schema or Claim size"));
    }
    crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
        .map_err(|_| bad("Item recorded aware instant"))?;
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    // The caller's proposed Claim stays live during the callback and byte
    // recipe, so its state shares the same operation envelope as the grammar.
    reader.temporary(std::mem::size_of::<Vec<u8>>() + proposed_claim_raw.len())?;
    let claim = reader.decoded(proposed_claim_raw)?;
    let mut expected = canonical(&claim)?;
    expected.push(b'\n');
    let expected = reader.buffer(expected)?;
    if proposed_claim_raw != expected {
        return Err(bad("Item proposed Claim exact canonical bytes"));
    }
    let decoded_state = crate::record_biblio_cut::decoded_state(&claim)?;
    drop(claim);
    reader.release_temporary(decoded_state);
    let encoded_state = std::mem::size_of::<Vec<u8>>() + expected.len();
    drop(expected);
    reader.release_temporary(encoded_state);
    let grammar = preparation_grammar_from_cut(
        CompoundKind::EditionItem,
        &mut reader,
        schemas,
        proposed_claim_raw,
    )?;
    if grammar.binding != schemas.execution_binding() {
        return Err(bad("Item preview schema binding changed"));
    }
    let (request_raw, authority) = build_request_and_authorization(&grammar.digests)?;
    prepare_native_with_reader(
        CompoundKind::EditionItem,
        Some(ItemPreparationInput::Preview {
            generator: inventory_generator,
        }),
        reader,
        schemas,
        scope,
        Cow::Owned(request_raw),
        owned_before,
        recorded_at,
        Some((proposed_claim_raw, grammar)),
        |_| Ok(authority),
    )
    .map(|inner| EditionItemCore {
        inner,
        descriptive_preview: true,
    })
}

fn prepare_native_with_reader<'a>(
    kind: CompoundKind,
    item_input: Option<ItemPreparationInput<'_>>,
    mut reader: NativeCompoundReader<'a>,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    request_raw: Cow<'_, [u8]>,
    owned_before: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
    prechecked: Option<(&[u8], WorkGrammar)>,
    authorize: impl FnOnce(&BTreeMap<String, String>) -> Result<Value, ItemRefusal>,
) -> Result<WorkExpressionCore<'a>, ItemRefusal> {
    check(reader.limits.deadline, reader.cancelled)?;
    if schemas.source_revision() != reader.cut().current().revision()
        || request_raw.len() > MAX_FILE
    {
        return Err(bad(
            "native compound source/schema revision or request size",
        ));
    }
    reader.temporary(std::mem::size_of::<Vec<u8>>() + request_raw.len())?;
    let request_raw = request_raw.into_owned();
    let request = reader.decoded(&request_raw)?;
    let observation = request_valid_with(&request, kind, &mut |value| {
        reader.canonical_observation(value)
    })?;
    let mut expected_raw = canonical(&request)?;
    expected_raw.push(b'\n');
    let expected_raw = reader.buffer(expected_raw)?;
    if request_raw != expected_raw {
        return Err(bad("native compound request exact canonical bytes"));
    }
    let expected_len = expected_raw.len();
    drop(expected_raw);
    reader.release_temporary(std::mem::size_of::<Vec<u8>>() + expected_len);
    let ordered_request = reader.ordered_value(&request_raw)?;
    let claim_ordered = ordered_request
        .object_get("claim")
        .ok_or_else(|| bad("native compound ordered Claim"))?;
    let mut claim_raw = canonical_ordered(claim_ordered)?;
    claim_raw.push(b'\n');
    let claim_raw = reader.buffer(claim_raw)?;
    let ordered_state = crate::record_biblio_cut::ordered_state(&ordered_request)?;
    drop(ordered_request);
    reader.release_temporary(ordered_state);
    let grammar = if let Some((proposed_claim_raw, grammar)) = prechecked {
        if claim_raw.as_slice() != proposed_claim_raw
            || grammar.claim_sha256 != Digest256::of_bytes(&claim_raw)
            || grammar.binding != schemas.execution_binding()
        {
            return Err(bad("native compound preview request Claim/worker changed"));
        }
        grammar
    } else {
        preparation_grammar_from_cut(kind, &mut reader, schemas, &claim_raw)?
    };
    let claim_len = claim_raw.len();
    drop(claim_raw);
    reader.release_temporary(std::mem::size_of::<Vec<u8>>() + claim_len);
    let authority = authorize(&grammar.digests)?;
    reader.temporary(crate::record_biblio_cut::decoded_state(&authority)?)?;
    keys(
        &authority,
        &[
            "schema_version",
            "scope",
            "principal_id",
            "maker_type",
            "authority_ref",
            "owner_configuration",
            "command_id",
            "request_digest",
            "dependency_bindings",
        ],
    )?;
    if text(&authority, "schema_version")? != kind.authorization_schema()
        || authority["scope"] != *scope
    {
        return Err(bad("native compound authorization profile/scope"));
    }
    keys(
        &authority["dependency_bindings"],
        &[
            "catalog_and_sources",
            "contracts",
            "implementation",
            "retained_transactions",
        ],
    )?;
    if authority["dependency_bindings"]["contracts"] != json!(grammar.digests) {
        return Err(bad(
            "native compound authorization exact Claim/form/provenance contracts",
        ));
    }
    scope_valid(scope, &request, &authority, kind)?;
    let work_path = text(scope, kind.parent_path())?;
    let before = reader.selected(work_path)?;
    if &before != owned_before {
        return Err(bad(
            "native compound protected/current selected package mismatch",
        ));
    }
    let old = reader.decoded(
        before
            .get(kind.parent_file())
            .ok_or_else(|| bad("native compound parent absent"))?,
    )?;
    if old["record_type"] != kind.parent_kind()
        || old["record_id"] != scope[kind.parent_key()]
        || !reader.reference_matches(
            &old,
            "record_id",
            "record_version",
            &request["expected_source"],
        )?
        || reader.package_revision(&before)? != text(&request, "expected_revision")?
        || authority["owner_configuration"] != request["expected_configuration"]
        || authority["command_id"] != request["command_id"]
        || text(&authority, "request_digest")? != observation.0
        || reader
            .canonical_observation(&authority["dependency_bindings"])?
            .0
            != text(&request, "expected_dependencies")?
    {
        return Err(bad(
            "native compound authorization/request/current source binding",
        ));
    }
    let preview_receipt;
    let item_companions_input = match item_input {
        Some(ItemPreparationInput::Observed {
            receipt_raw,
            generator,
        }) => {
            if receipt_raw.len() > MAX_FILE {
                return Err(ItemRefusal::Budget);
            }
            let receipt = reader.decoded(receipt_raw)?;
            if receipt["private_stage_digest"] == format!("sha256:{}", "0".repeat(64)) {
                return Err(bad("observed Item deposit cannot use preview stage marker"));
            }
            let retained = crate::record_biblio_cut::decoded_state(&receipt)?;
            drop(receipt);
            reader.release_temporary(retained);
            Some((receipt_raw, generator))
        }
        Some(ItemPreparationInput::Preview { generator }) => {
            preview_receipt = preview_item_receipt(&mut reader, scope, &request, &observation.0)?;
            Some((preview_receipt.as_slice(), generator))
        }
        None => None,
    };
    if (kind == CompoundKind::EditionItem) != item_companions_input.is_some() {
        return Err(bad("native preparation exact Item companion input"));
    }
    let cut = reader.cut;
    let cut = cut.ok_or_else(|| bad("native preparation requires a corpus cut"))?;
    let local_cancelled = reader.cancelled;
    let mut validate_local_claim =
        |claim_raw: &[u8], schemas: &mut CutWorkerSchemaExecutor, local_limits: ItemLimits| {
            let mut local = crate::record_rules::validate_source_claim_from_cut(
                cut,
                claim_raw,
                schemas,
                local_limits,
                local_cancelled,
            )?;
            if !local.issues.is_empty() {
                return Err(bad("compound Claim local owner profile"));
            }
            Ok(LocalClaimValidation {
                dependency_digests: std::mem::take(&mut local.dependency_digests),
                dependency_bytes_read: 0,
            })
        };
    let prepared = reader.prepare_compound_core(
        kind,
        Cow::Owned(authority),
        request,
        request_raw,
        observation.0,
        before,
        old,
        recorded_at,
        schemas,
        &|_| Err(bad("native compound has no external companion inputs")),
        Some(grammar),
        item_companions_input,
        &mut validate_local_claim,
    )?;
    reader.release_raw_cache();
    Ok(WorkExpressionCore {
        reader,
        prepared,
        schema_binding: schemas.execution_binding(),
    })
}
/// Mechanical ObjectLink package. No publication or admission authority is granted.
pub struct ObjectLinkBytes {
    pub scope: Value,
    pub request: Value,
    pub receipt: Value,
    pub files: BTreeMap<String, Vec<u8>>,
    pub transaction_id: String,
    pub reads: Vec<PredicateRead>,
    pub bytes_read: u64,
}

/// Compose exact new-only source buffers using the retained verifier's kernel.
/// The caller owns current delegation, dependency currentness and publication.
pub fn prepare_object_link_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    authorization: &Value,
    request_raw: &[u8],
    environment_raw: &[u8],
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
) -> Result<ObjectLinkBytes, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision()
        || request_raw.len() > MAX_FILE
        || environment_raw.len() > MAX_FILE
    {
        return Err(bad("object-Link source/schema revision or input size"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    // Reserve the bounded output envelope before any package buffer allocation.
    // Eight metadata files are produced by this exact kernel.
    reader.temporary(
        8usize
            .checked_mul(MAX_FILE)
            .and_then(|n| {
                n.checked_add(crate::record_biblio_cut::decoded_state(authorization).ok()?)
            })
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<Package>() + 8 * std::mem::size_of::<(String, Vec<u8>)>(),
                )
            })
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let prepared = reader.compose_object_link(
        scope,
        authorization,
        request_raw,
        environment_raw,
        recorded_at,
        schemas,
        None,
        None,
    )?;
    let transaction_id = text(&prepared.receipt, "transaction_id")?.to_owned();
    reader.release_raw_cache();
    Ok(ObjectLinkBytes {
        scope: prepared.scope,
        request: prepared.request,
        receipt: prepared.receipt,
        files: prepared.files,
        transaction_id,
        reads: reader.reads,
        bytes_read: reader.bytes,
    })
}

/// Native producer extension of the same ObjectLink byte composer. Capture sees
/// only prepared source/form bytes and supplies actual environment/event buffers.
pub fn prepare_native_object_link_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    authorization: &Value,
    request_raw: &[u8],
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    mut capture: impl FnMut(&[(String, &[u8])]) -> Result<(Vec<u8>, Vec<u8>), ItemRefusal>,
) -> Result<ObjectLinkBytes, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() || request_raw.len() > MAX_FILE {
        return Err(bad("object-Link source/schema revision or request size"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    reader.temporary(8usize.checked_mul(MAX_FILE).ok_or(ItemRefusal::Budget)?)?;
    let prepared = reader.compose_object_link(
        scope,
        authorization,
        request_raw,
        &[],
        recorded_at,
        schemas,
        None,
        Some(&mut capture),
    )?;
    let transaction_id = text(&prepared.receipt, "transaction_id")?.to_owned();
    reader.release_raw_cache();
    Ok(ObjectLinkBytes {
        scope: prepared.scope,
        request: prepared.request,
        receipt: prepared.receipt,
        files: prepared.files,
        transaction_id,
        reads: reader.reads,
        bytes_read: reader.bytes,
    })
}

/// Recompose retained ObjectLink bytes without synthesizing a transaction.
/// The owner must compare this package with its actual retained journal plan.
pub fn reconstruct_object_link_bytes<'a>(
    cut: &'a CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    scope: &Value,
    authorization: &Value,
    request_raw: &[u8],
    environment_raw: &[u8],
    event_raw: &[u8],
    recorded_at: &str,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
) -> Result<ObjectLinkBytes, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision()
        || [request_raw.len(), environment_raw.len(), event_raw.len()]
            .iter()
            .any(|size| *size > MAX_FILE)
    {
        return Err(bad("ObjectLink retained input/cut binding"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    reader.temporary(8usize.checked_mul(MAX_FILE).ok_or(ItemRefusal::Budget)?)?;
    let prepared = reader.compose_object_link(
        scope,
        authorization,
        request_raw,
        environment_raw,
        recorded_at,
        schemas,
        Some(event_raw),
        None,
    )?;
    let transaction_id = text(&prepared.receipt, "transaction_id")?.to_owned();
    Ok(ObjectLinkBytes {
        scope: prepared.scope,
        request: prepared.request,
        receipt: prepared.receipt,
        files: prepared.files,
        transaction_id,
        reads: reader.reads,
        bytes_read: reader.bytes,
    })
}
/// Read exact ObjectLink creation and current correction lineage with its
/// existing native verifier. This descriptive observation grants no authority.
pub fn verify_object_link_from_cut(
    cut: &CorpusCutReader,
    schemas: &mut CutWorkerSchemaExecutor,
    claim_path: &str,
    claim: &Value,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<NativeCompoundReadObservation, ItemRefusal> {
    if schemas.source_revision() != cut.current().revision()
        || claim["schema_version"] != OBJECT_LINK_CLAIM
        || !claim["predicate"]
            .as_str()
            .is_some_and(object_link_predicate)
    {
        return Err(bad("ObjectLink selected claim/cut profile"));
    }
    let mut reader = NativeCompoundReader::new(cut, limits, cancelled)?;
    let observation = reader.verify(claim_path, claim, schemas)?;
    measured_compound_observation(reader, observation)
}

struct ObjectLinkReconstructed {
    scope: Value,
    request: Value,
    receipt: Value,
    files: Package,
}
impl NativeCompoundReader<'_> {
    fn record_work_dependencies(
        &mut self,
        dependencies: BTreeMap<String, Digest256>,
        digests: &BTreeMap<String, String>,
    ) -> Result<(), ItemRefusal> {
        for (path, sha) in dependencies {
            RelativePath::parse(&path).map_err(|_| bad("Claim contract path"))?;
            let (current_digest, size) = self.current_digest_size(&path)?;
            if current_digest != sha {
                return Err(bad("Work prechecked Claim dependency membership drift"));
            }
            // Candidate digest lookup already reads and charges the member;
            // cut-backed lookup obtains metadata and retains its original charge.
            if self.cut.is_some() {
                account(
                    &mut self.bytes,
                    usize::try_from(size).map_err(|_| ItemRefusal::Budget)?,
                    self.limits.max_total_bytes,
                )?;
            }
            self.release_temporary(std::mem::size_of::<(String, Digest256)>() + path.len());
            if digests.get(&path).map(String::as_str) != Some(sha.to_hex().as_str()) {
                return Err(bad("Work prechecked Claim dependency drift"));
            }
            self.record_read(PredicateRead::ExactPath {
                path,
                digest: sha.to_prefixed(),
            })?;
        }
        Ok(())
    }
    fn reconstruct<S, F>(
        &mut self,
        tx: &Transaction,
        kind: CompoundKind,
        schemas: &mut S,
        validate_local_claim: &mut F,
    ) -> Result<Reconstructed, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        let before = self.temporary_state;
        let result = self.reconstruct_inner(tx, kind, schemas, validate_local_claim);
        self.release_temporary_since(before);
        if let Ok(value) = &result {
            let mut amount = std::mem::size_of::<Reconstructed>();
            for tree in [
                &value.scope,
                &value.request,
                &value.parent_receipt,
                &value.receipt,
            ] {
                amount = amount
                    .checked_add(
                        crate::record_biblio_cut::decoded_state(tree)?
                            .checked_sub(std::mem::size_of::<Value>())
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
            }
            amount = amount
                .checked_add(
                    package_state(&value.child)?
                        .checked_sub(std::mem::size_of::<Package>())
                        .ok_or(ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?;
            self.temporary(amount)?;
        }
        result
    }
    fn prepare_compound_core<'b, S, F>(
        &mut self,
        kind: CompoundKind,
        authority: Cow<'b, Value>,
        request: Value,
        request_raw: Vec<u8>,
        request_digest: String,
        before: Package,
        old: Value,
        recorded_at: &str,
        schemas: &mut S,
        after: &impl Fn(&str) -> Result<Vec<u8>, ItemRefusal>,
        work_grammar: Option<WorkGrammar>,
        item_input: Option<(&[u8], &str)>,
        validate_local_claim: &mut F,
    ) -> Result<CompoundCore<'b>, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        let scope = &authority["scope"];
        let work_path = text(scope, kind.parent_path())?;
        let expression_path = text(scope, kind.child_path())?;
        let home = kind.publication_home(scope)?;
        let work_home = parent(work_path)?;
        self.temporary(std::mem::size_of::<String>() + recorded_at.len())?;
        let old_raw = before
            .get(kind.parent_file())
            .ok_or_else(|| bad("retained parent input missing"))?;
        let history = self.history(work_path, &before)?;
        if array(&history, "receipts")?.len() >= MAX_HISTORY {
            return Err(bad("parent history capacity"));
        }
        let mut revised = self.value_copy(&old)?;
        let map = revised
            .as_object_mut()
            .ok_or_else(|| bad("parent object"))?;
        for (k, v) in request["fields"].as_object().unwrap() {
            map.insert(k.clone(), v.clone());
        }
        map.insert(
            "record_version".into(),
            json!(
                integer(&old, "record_version")?
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?
            ),
        );
        // Inserts replace old subtrees; charge only positive growth of the
        // retained revised tree, not another full cloned profile.
        let old_state = crate::record_biblio_cut::decoded_state(&old)?;
        let revised_state = crate::record_biblio_cut::decoded_state(&revised)?;
        self.temporary(revised_state.saturating_sub(old_state))?;
        let mut revised_ordered = self.ordered_value(old_raw)?;
        let original_state = crate::record_biblio_cut::ordered_state(&revised_ordered)?;
        set(
            &mut revised_ordered,
            kind.field(),
            j(&request["fields"][kind.field()])?,
        )?;
        set(
            &mut revised_ordered,
            "record_version",
            j(&revised["record_version"])?,
        )?;
        self.temporary(
            crate::record_biblio_cut::ordered_state(&revised_ordered)?
                .saturating_sub(original_state),
        )?;
        let expression = &request[kind.record_key()];
        let claim = &request["claim"];
        let ordered_request = self.ordered_value(&request_raw)?;
        let expression_ordered = self.ordered_copy(
            ordered_request
                .object_get(kind.record_key())
                .ok_or_else(|| bad("ordered child record"))?,
        )?;
        let claim_ordered = self.ordered_copy(
            ordered_request
                .object_get("claim")
                .ok_or_else(|| bad("ordered Claim"))?,
        )?;
        let request_raw_state = std::mem::size_of::<Vec<u8>>() + request_raw.len();
        drop(request_raw);
        self.release_temporary(request_raw_state);
        let parent_raw = self.buffer(pretty(&revised_ordered)?)?;
        let expression_raw = self.buffer(pretty(&expression_ordered)?)?;
        let mut claim_raw = canonical_ordered(&claim_ordered)?;
        claim_raw.push(b'\n');
        let claim_raw = self.buffer(claim_raw)?;
        let mut delta_limits = self.limits;
        delta_limits.max_state_bytes = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        delta_limits.max_total_bytes = self
            .limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let delta = crate::biblio_rules::inspect_bibliographic_delta(
            crate::biblio_rules::BiblioDeltaInput {
                parent_path: work_path,
                parent_before_raw: old_raw,
                parent_after_raw: &parent_raw,
                endpoint_path: expression_path,
                endpoint_raw: &expression_raw,
                claim_path: &format!("{home}/source-claims.jsonl"),
                claim_raw: &claim_raw,
            },
            delta_limits,
            self.cancelled,
            schemas,
        )?;
        if !delta.issues.is_empty() {
            return Err(bad("native bibliographic append/delta mechanics"));
        }
        let delta_reads_state = delta
            .reads
            .iter()
            .try_fold(0usize, |n, r| {
                n.checked_add(crate::record_biblio_cut::predicate_state(r).ok()?)
            })
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(delta_reads_state)?;
        for read in delta.reads {
            self.release_temporary(crate::record_biblio_cut::predicate_state(&read)?);
            // Delta validates these three supplied reconstruction buffers. Their
            // destination paths do not assert that historical bytes are current.
            // Actual selected reads and retained archive/plan custody stay paths.
            let read = match read {
                PredicateRead::ExactPath { path, digest }
                    if (path == work_path
                        && digest == Digest256::of_bytes(&parent_raw).to_prefixed())
                        || (path == expression_path
                            && digest == Digest256::of_bytes(&expression_raw).to_prefixed())
                        || (path == format!("{home}/source-claims.jsonl")
                            && digest == Digest256::of_bytes(&claim_raw).to_prefixed()) =>
                {
                    PredicateRead::ExactBytes {
                        locator: format!("reconstructed-compound-delta:{path}"),
                        digest,
                    }
                }
                other => other,
            };
            self.record_read(read)?;
        }
        if kind == CompoundKind::WorkExpression
            && (!text(expression, "language").is_ok_and(|v| !v.is_empty())
                || !text(expression, "expression_role").is_ok_and(|v| !v.is_empty()))
            || !kind.relation_attachment()
                && ["variant_labels", "external_identifiers"].iter().any(|k| {
                    !expression[*k]
                        .as_array()
                        .is_some_and(|a| a.iter().all(|v| v["status"] == "unverified"))
                })
        {
            return Err(bad("child language/role/unverified variants"));
        }
        let mut local_limits = self.limits;
        local_limits.max_state_bytes = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        local_limits.max_total_bytes = self
            .limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let prechecked_grammar = work_grammar.is_some();
        let (dependency_digests, grammar_digests, dependency_bytes_read) =
            if let Some(grammar) = work_grammar {
                preparation_grammar_extra(kind)?;
                if grammar.claim_sha256 != Digest256::of_bytes(&claim_raw)
                    || schemas.cut_execution_binding() != Some(grammar.binding)
                {
                    return Err(bad("Work prechecked Claim/worker input drift"));
                }
                (grammar.dependencies, grammar.digests, 0)
            } else {
                let local = validate_local_claim(&claim_raw, schemas, local_limits)?;
                (
                    local.dependency_digests,
                    BTreeMap::new(),
                    local.dependency_bytes_read,
                )
            };
        let dependencies_state = dependency_digests
            .keys()
            .try_fold(0usize, |n, path| {
                n.checked_add(std::mem::size_of::<(String, Digest256)>() + path.len())
            })
            .ok_or(ItemRefusal::Budget)?;
        if !prechecked_grammar {
            self.temporary(dependencies_state)?;
        }
        if prechecked_grammar {
            self.record_work_dependencies(dependency_digests, &grammar_digests)?;
            self.release_temporary(std::mem::size_of::<BTreeMap<String, Digest256>>());
            for &path in preparation_grammar_extra(kind)? {
                if !grammar_digests.contains_key(path) {
                    return Err(bad("Work grammar missing selected contract"));
                }
            }
        } else {
            if self.input.is_some() {
                account(
                    &mut self.bytes,
                    usize::try_from(dependency_bytes_read).map_err(|_| ItemRefusal::Budget)?,
                    self.limits.max_total_bytes,
                )?;
            }
            for (path, sha) in dependency_digests {
                if self.input.is_none() {
                    let relative =
                        RelativePath::parse(&path).map_err(|_| bad("Claim contract path"))?;
                    let size = self
                        .cut()
                        .current()
                        .member(&relative)
                        .ok_or_else(|| bad("Claim dependency membership"))?
                        .size_bytes;
                    account(
                        &mut self.bytes,
                        usize::try_from(size).map_err(|_| ItemRefusal::Budget)?,
                        self.limits.max_total_bytes,
                    )?;
                }
                self.release_temporary(std::mem::size_of::<(String, Digest256)>() + path.len());
                self.record_read(PredicateRead::ExactPath {
                    path,
                    digest: sha.to_prefixed(),
                })?;
            }
        }
        // The constructor's decoded registries/routes have now been dropped.
        // Acquire the remainder before allocating any generated forms,
        // Item companions, provenance event or whole output maps.
        let form_name = kind.parent_forms();
        let prior_forms = before
            .get(form_name)
            .map(|v| self.ordered_value(v))
            .transpose()?;
        let principal = text(&authority, "principal_id")?;
        let (parent_forms, parent_refs) = forms(
            &revised_ordered,
            prior_forms.as_ref(),
            &request["forms"],
            principal,
            false,
            self.limits
                .max_state_bytes
                .checked_sub(self.state)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        for value in [&parent_forms, &parent_refs] {
            self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        }
        let (expression_forms, expression_refs) = if kind.relation_attachment() {
            (JsonValue::Null, JsonValue::Null)
        } else {
            forms(
                &expression_ordered,
                None,
                &request[kind.child_form_request()],
                principal,
                false,
                self.limits
                    .max_state_bytes
                    .checked_sub(self.state)
                    .ok_or(ItemRefusal::Budget)?,
            )?
        };
        for value in [&expression_forms, &expression_refs] {
            self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        }
        let (claim_forms, claim_refs) = forms(
            &claim_ordered,
            None,
            &request["claim_forms"],
            principal,
            true,
            self.limits
                .max_state_bytes
                .checked_sub(self.state)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        for value in [&claim_forms, &claim_refs] {
            self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        }
        let id = transaction_id_with_digest(&request, kind, &request_digest, &mut |value| {
            self.canonical_observation(value)
        })?;
        let archive_path = format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(text(scope, kind.parent_key())?.as_bytes()).to_hex(),
            hash(text(&request, "expected_revision")?)?
        );
        let parent_receipt_ordered = object(vec![
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&request_digest)),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("recorded_at", string(recorded_at)),
            ("reason", j(&request["reason"])?),
            (
                "previous_source",
                ref_ordered(&old, "record_id", "record_version")?,
            ),
            (
                "source",
                ref_ordered(&revised, "record_id", "record_version")?,
            ),
            ("previous_revision", j(&request["expected_revision"])?),
            ("archive_path", string(&archive_path)),
            ("dependencies", j(&request["expected_dependencies"])?),
            ("changed_fields", j(&json!([kind.field()]))?),
            ("forms", parent_refs.clone()),
            ("grants_admission", JsonValue::Bool(false)),
            ("request", self.ordered_copy(&ordered_request)?),
            (
                "publication",
                object(vec![
                    ("protocol", string(PROTOCOL)),
                    ("transaction_id", string(&id)),
                    (
                        "selected_files",
                        j(&{
                            let mut names = selected_names(work_path)?;
                            names.sort();
                            json!(names)
                        })?,
                    ),
                ]),
            ),
        ]);
        self.temporary(crate::record_biblio_cut::ordered_state(
            &parent_receipt_ordered,
        )?)?;
        let parent_receipt_raw = self.buffer(canonical_ordered(&parent_receipt_ordered)?)?;
        let parent_receipt = self.decoded(&parent_receipt_raw)?;
        parent_receipt_shape_with(&parent_receipt, kind, &mut |value| {
            self.canonical_observation(value)
        })?;
        let (mut receipts, mut receipt_array_state) = match before.get(HISTORY) {
            Some(raw) => {
                let prior = self.ordered_value(raw)?;
                let prior_state = crate::record_biblio_cut::ordered_state(&prior)?;
                let rows = prior
                    .object_get("receipts")
                    .and_then(JsonValue::as_array)
                    .ok_or_else(|| bad("ordered receipt chain"))?;
                let cost = rows
                    .iter()
                    .try_fold(std::mem::size_of::<Vec<JsonValue>>(), |n, row| {
                        n.checked_add(crate::record_biblio_cut::ordered_state(row).ok()?)
                    })
                    .ok_or(ItemRefusal::Budget)?;
                self.temporary(cost)?;
                let rows = rows.to_vec();
                drop(prior);
                self.release_temporary(prior_state);
                (rows, cost)
            }
            None => {
                let cost = std::mem::size_of::<Vec<JsonValue>>();
                self.temporary(cost)?;
                (vec![], cost)
            }
        };
        let new_receipt_state = crate::record_biblio_cut::ordered_state(&parent_receipt_ordered)?;
        self.temporary(new_receipt_state)?;
        receipt_array_state = receipt_array_state
            .checked_add(new_receipt_state)
            .ok_or(ItemRefusal::Budget)?;
        receipts.push(parent_receipt_ordered.clone());
        // Maintained _compose creates this outer dict afresh in fixed order;
        // retained receipt object order, but not prior outer order, survives.
        let history_ordered = object(vec![
            ("schema_version", string("tos_source_revision_history_v2")),
            ("record_id", j(&scope[kind.parent_key()])?),
            ("receipts", JsonValue::Array(receipts)),
        ]);
        self.release_temporary(receipt_array_state);
        self.temporary(crate::record_biblio_cut::ordered_state(&history_ordered)?)?;
        let parent_files: Vec<(String, Vec<u8>)> = vec![
            (kind.parent_file().into(), parent_raw),
            (form_name.into(), self.buffer(pretty(&parent_forms)?)?),
            (HISTORY.into(), self.buffer(pretty(&history_ordered)?)?),
        ];
        let claim_form_name = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(text(scope, "claim_id")?.as_bytes()).to_hex()
        );
        let mut child_files: Vec<(String, Vec<u8>)> = vec![
            ("source-claims.jsonl".into(), claim_raw),
            (claim_form_name, self.buffer(pretty(&claim_forms)?)?),
        ];
        if !kind.relation_attachment() {
            child_files.insert(0, (kind.child_file().into(), expression_raw));
            child_files.insert(
                1,
                (
                    kind.child_forms().into(),
                    self.buffer(pretty(&expression_forms)?)?,
                ),
            );
        } else {
            let cost = std::mem::size_of::<Vec<u8>>() + expression_raw.len();
            drop(expression_raw);
            self.release_temporary(cost);
        }
        self.temporary(rows_state(&parent_files, false)?)?;
        self.temporary(rows_state(&child_files, false)?)?;
        if kind == CompoundKind::EditionItem {
            let byte_receipt_raw;
            let inventory_raw;
            let inventory;
            let generator = if let Some((receipt_raw, generator)) = item_input {
                if receipt_raw.len() > MAX_FILE {
                    return Err(ItemRefusal::Budget);
                }
                // Admit the real public receipt before copying; no payload is read here.
                self.temporary(std::mem::size_of::<Vec<u8>>() + receipt_raw.len())?;
                byte_receipt_raw = receipt_raw.to_vec();
                generator
            } else {
                byte_receipt_raw = self.buffer(after("item-deposit-receipt.json")?)?;
                inventory_raw = self.buffer(after("resource-inventory.json")?)?;
                inventory = self.decoded(&inventory_raw)?;
                text(&inventory["generator"], "version")?
            };
            let byte_receipt = self.decoded(&byte_receipt_raw)?;
            let mut companion_limits = self.limits;
            companion_limits.max_state_bytes = self
                .limits
                .max_state_bytes
                .checked_sub(self.state)
                .ok_or(ItemRefusal::Budget)?;
            let companions = item_companions(
                scope,
                &request,
                &byte_receipt,
                generator,
                schemas,
                companion_limits,
                self.cancelled,
            )?;
            self.temporary(rows_state(&companions, true)?)?;
            child_files.extend(companions);
            let ordered_receipt = self.ordered_value(&byte_receipt_raw)?;
            let receipt_raw = self.buffer(pretty(&ordered_receipt)?)?;
            self.temporary(
                std::mem::size_of::<(String, Vec<u8>)>() + "item-deposit-receipt.json".len(),
            )?;
            child_files.push(("item-deposit-receipt.json".into(), receipt_raw));
        }
        Ok(CompoundCore {
            kind,
            authority,
            request,
            request_digest,
            before,
            old,
            revised,
            parent_receipt,
            parent_receipt_ordered,
            parent_files,
            child_files,
            parent_forms,
            expression_forms,
            claim_forms,
            parent_refs,
            expression_refs,
            claim_refs,
            grammar_digests,
            id,
            archive_path,
            recorded_at: recorded_at.to_owned(),
        })
    }
    fn finish_compound_core<'b>(
        &mut self,
        core: CompoundCore<'b>,
        environment: &Value,
        native_event_raw: Option<&[u8]>,
        schemas: &mut impl CutSchemaExecutor,
    ) -> Result<FinishedCompound<'b>, ItemRefusal> {
        let CompoundCore {
            kind,
            authority,
            request,
            request_digest,
            before,
            old,
            revised,
            parent_receipt,
            parent_receipt_ordered,
            parent_files,
            mut child_files,
            parent_forms,
            expression_forms,
            claim_forms,
            parent_refs,
            expression_refs,
            claim_refs,
            grammar_digests: _,
            id,
            archive_path,
            recorded_at,
        } = core;
        let scope = &authority["scope"];
        let work_path = text(scope, kind.parent_path())?;
        let home = kind.publication_home(scope)?;
        let work_home = parent(work_path)?;
        let expression = &request[kind.record_key()];
        let claim = &request["claim"];
        let outputs: Vec<_> = parent_files
            .iter()
            .map(|(n, r)| (format!("{work_home}/{n}"), r.as_slice()))
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), r.as_slice())),
            )
            .collect();
        self.temporary(slice_rows_state(&outputs)?)?;
        let mut captured_native = false;
        let event = if let Some(raw) = native_event_raw {
            preparation_grammar_extra(kind)?;
            let event = self.decoded(raw)?;
            let procedure = text(&event["method"]["procedure"], "name")?.to_owned();
            match procedure.as_str() {
                name if name == native_preparation_procedure(kind)? => {
                    native_work_event(
                        kind,
                        &event,
                        scope,
                        &request,
                        &authority["dependency_bindings"],
                        &before,
                        &outputs,
                        environment,
                        &archive_path,
                        &recorded_at,
                        self.limits.deadline,
                        self.cancelled,
                        self.limits
                            .max_state_bytes
                            .checked_sub(self.state)
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    captured_native = true;
                    event
                }
                name if name == kind.procedure() => {
                    let state = crate::record_biblio_cut::decoded_state(&event)?;
                    drop(event);
                    self.release_temporary(state);
                    compound_event(
                        kind,
                        scope,
                        &request,
                        &before,
                        &outputs,
                        environment,
                        &authority["dependency_bindings"],
                        &recorded_at,
                        self.limits
                            .max_state_bytes
                            .checked_sub(self.state)
                            .ok_or(ItemRefusal::Budget)?,
                    )?
                }
                _ => return Err(bad("unknown retained Work event profile")),
            }
        } else {
            compound_event(
                kind,
                scope,
                &request,
                &before,
                &outputs,
                &environment,
                &authority["dependency_bindings"],
                &recorded_at,
                self.limits
                    .max_state_bytes
                    .checked_sub(self.state)
                    .ok_or(ItemRefusal::Budget)?,
            )?
        };
        let outputs_state = slice_rows_state(&outputs)?;
        drop(outputs);
        self.release_temporary(outputs_state);
        if !captured_native {
            self.temporary(crate::record_biblio_cut::decoded_state(&event)?)?;
        }
        let mut event_raw = canonical(&event)?;
        event_raw.push(b'\n');
        if native_event_raw.is_some_and(|raw| raw != event_raw) {
            return Err(bad("native Work event exact canonical bytes"));
        }
        let event_raw = self.buffer(event_raw)?;
        if !schemas.check_reusing_scalar(
            &format!("{home}/source-create-provenance.jsonl"),
            &event_raw,
            "ToS/contracts/provenance-event-v2.schema.json",
            self.limits.deadline,
            self.cancelled,
        )? {
            return Err(bad("reconstructed provenance schema"));
        }
        for raw in [&parent_forms, &expression_forms, &claim_forms]
            .into_iter()
            .filter(|raw| !matches!(raw, JsonValue::Null))
        {
            if !schemas.check_reusing_scalar(
                "compound-reconstructed-human-form-set",
                &canonical_ordered(raw)?,
                "ToS/contracts/human-form-set.schema.json",
                self.limits.deadline,
                self.cancelled,
            )? {
                return Err(bad("compound forms schema"));
            }
        }
        child_files.extend([
            ("source-create-request.json".into(), {
                let mut r = canonical(&request)?;
                r.push(b'\n');
                self.buffer(r)?
            }),
            ("source-create-environment.json".into(), {
                let mut r = canonical(&environment)?;
                r.push(b'\n');
                self.buffer(r)?
            }),
            ("source-create-provenance.jsonl".into(), event_raw),
        ]);
        let files: Vec<_> = parent_files
            .iter()
            .map(|(n, r)| (format!("{work_home}/{n}"), r.as_slice()))
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), r.as_slice())),
            )
            .collect();
        self.temporary(slice_rows_state(&files)?)?;
        let before_refs: Vec<_> = selected_names(work_path)?
            .iter()
            .filter_map(|n| before.get(n).map(|r| (n.clone(), r.as_slice())))
            .collect();
        self.temporary(slice_rows_state(&before_refs)?)?;
        let mut receipt_fields = vec![
            ("schema_version", string(kind.receipt_schema())),
            ("operation", string(kind.operation())),
            ("transaction_id", string(&id)),
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&request_digest)),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("recorded_at", string(&recorded_at)),
            ("scope", j(scope)?),
            ("dependencies", j(&request["expected_dependencies"])?),
            (
                "parent_before",
                ref_ordered(&old, "record_id", "record_version")?,
            ),
            (
                "parent_after",
                ref_ordered(&revised, "record_id", "record_version")?,
            ),
            ("parent_revision", j(&request["expected_revision"])?),
            ("parent_archive_ref", string(&archive_path)),
            (
                "parent_transition_sha256",
                string(
                    &Digest256::of_bytes(&canonical_ordered(&parent_receipt_ordered)?)
                        .to_prefixed(),
                ),
            ),
            ("parent_before_files", refs_ordered(&before_refs)),
            (
                kind.child_kind(),
                ref_ordered(expression, "record_id", "record_version")?,
            ),
            ("claim", ref_ordered(claim, "claim_id", "claim_version")?),
            (
                "forms",
                object(vec![
                    (kind.parent_kind(), parent_refs),
                    ("claim", claim_refs),
                ]),
            ),
            ("files", refs_ordered(&files)),
            ("grants_admission", JsonValue::Bool(false)),
        ];
        let mut binding_state = 0;
        if !kind.relation_attachment() {
            let forms = receipt_fields
                .iter_mut()
                .find(|(key, _)| *key == "forms")
                .unwrap();
            if let JsonValue::Object(ref mut fields) = forms.1 {
                fields.insert(
                    1,
                    (
                        tos_foundation::JsonString::from_utf8(kind.child_kind()),
                        expression_refs,
                    ),
                );
            }
        } else {
            let binding = self.attachment_endpoint_binding(
                scope,
                expression,
                &authority["dependency_bindings"],
                kind,
            )?;
            binding_state = crate::record_biblio_cut::ordered_state(&binding)?;
            let claim_position = receipt_fields
                .iter()
                .position(|(key, _)| *key == "claim")
                .unwrap();
            receipt_fields.insert(
                claim_position,
                (
                    if kind == CompoundKind::CollectionWork {
                        "work_source_binding"
                    } else {
                        "agent_source_binding"
                    },
                    binding,
                ),
            );
        }
        let receipt_ordered = object(receipt_fields);
        let rows_state = slice_rows_state(&files)?
            .checked_add(slice_rows_state(&before_refs)?)
            .ok_or(ItemRefusal::Budget)?;
        drop(files);
        drop(before_refs);
        self.release_temporary(rows_state);
        // The binding moved into this receipt; transfer its existing charge,
        // rather than counting a second retained tree for the same value.
        self.release_temporary(binding_state);
        self.temporary(crate::record_biblio_cut::ordered_state(&receipt_ordered)?)?;
        let expected_raw = self.buffer(pretty(&receipt_ordered)?)?;
        let receipt = self.decoded(&expected_raw)?;
        Ok(FinishedCompound {
            kind,
            authority,
            request,
            before,
            parent_receipt,
            parent_files,
            child_files,
            receipt,
            receipt_raw: expected_raw,
            id,
            archive_path,
        })
    }
    fn reconstruct_inner<S, F>(
        &mut self,
        tx: &Transaction,
        kind: CompoundKind,
        schemas: &mut S,
        validate_local_claim: &mut F,
    ) -> Result<Reconstructed, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        let plan = &tx.manifest["plan"];
        let authority = &plan["authorization"];
        keys(
            authority,
            &[
                "schema_version",
                "scope",
                "principal_id",
                "maker_type",
                "authority_ref",
                "owner_configuration",
                "command_id",
                "request_digest",
                "dependency_bindings",
            ],
        )?;
        if text(authority, "schema_version")? != kind.authorization_schema() {
            return Err(bad("native bibliographic authorization profile"));
        }
        let scope = &authority["scope"];
        let work_path = text(scope, kind.parent_path())?;
        let expression_path = text(scope, kind.child_path())?;
        let home = kind.publication_home(scope)?;
        let work_home = parent(work_path)?;
        // Charge the representations this phase actually owns. No allowance
        // for future forms/events competes with the local Claim constructor.
        let after = |name: &str| {
            tx.files
                .get(&format!("{home}/{name}"))
                .and_then(|v| v.1.as_ref())
                .cloned()
                .ok_or_else(|| bad("missing compound after buffer"))
        };
        let request_raw = self.buffer(after("source-create-request.json")?)?;
        let request = self.decoded(&request_raw)?;
        let request_observation = request_valid_with(&request, kind, &mut |value| {
            self.canonical_observation(value)
        })?;
        self.temporary(std::mem::size_of::<String>() + request_observation.0.len())?;
        let scope_scratch = if kind.relation_attachment() {
            let evidence = array(scope, "allowed_evidence_refs")?;
            let provenance = if kind == CompoundKind::CollectionWork {
                array(scope, "retained_membership_provenance_refs")?.len()
            } else {
                0
            };
            let grants = array(scope, kind.parent_form_grant())?.len()
                + array(scope, "allowed_claim_form_ids")?.len();
            let selections = array(&request, "forms")?
                .len()
                .max(array(&request, "claim_forms")?.len());
            // Collection evidence/provenance indexes are dropped before the
            // form pass. Form seen+current grant+selected indexes coexist;
            // Unicode strip is borrowed and allocates no string.
            let current_grant = array(scope, kind.parent_form_grant())?
                .len()
                .max(array(scope, "allowed_claim_form_ids")?.len());
            3 * std::mem::size_of::<BTreeSet<&str>>()
                + evidence
                    .len()
                    .max(provenance)
                    .max(grants + current_grant + selections)
                    * std::mem::size_of::<&str>()
        } else {
            0
        };
        self.temporary(scope_scratch)?;
        scope_valid(scope, &request, authority, kind)?;
        self.release_temporary(scope_scratch);
        let environment_raw = self.buffer(after("source-create-environment.json")?)?;
        let environment = self.decoded(&environment_raw)?;
        let environment_raw_state = std::mem::size_of::<Vec<u8>>() + environment_raw.len();
        drop(environment_raw);
        self.release_temporary(environment_raw_state);
        keys(
            &environment,
            &[
                "runtime",
                "runtime_version",
                "runtime_artifact_sha256",
                "backend",
                "hardware_target",
                "unicode_version",
                "argv_sha256",
            ],
        )?;
        for value in environment.as_object().unwrap().values() {
            if value.as_str().is_none_or(str::is_empty) {
                return Err(bad("retained environment fields"));
            }
        }
        for k in ["runtime_artifact_sha256", "argv_sha256"] {
            hash(&format!("sha256:{}", text(&environment, k)?))?;
        }
        let receipt_raw = self.buffer(after(kind.receipt_file())?)?;
        let actual_receipt = self.decoded(&receipt_raw)?;
        let recorded_at = text(&actual_receipt, "recorded_at")?;
        crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
            .map_err(|_| bad("compound recorded aware instant"))?;
        let mut before = Package::new();
        for name in selected_names(work_path)? {
            if let Some(raw) = tx
                .files
                .get(&format!("{work_home}/{name}"))
                .and_then(|s| s.0.clone())
            {
                let raw = self.buffer(raw)?;
                self.temporary(std::mem::size_of::<(String, Vec<u8>)>() + name.len())?;
                before.insert(name, raw);
            }
        }
        let old_raw = before
            .get(kind.parent_file())
            .ok_or_else(|| bad("retained parent input missing"))?;
        let old = self.decoded(old_raw)?;
        if !self.reference_matches(
            &old,
            "record_id",
            "record_version",
            &request["expected_source"],
        )? || self.package_revision(&before)? != text(&request, "expected_revision")?
        {
            return Err(bad("retained authorization/request/before binding"));
        }
        if authority["owner_configuration"] != request["expected_configuration"]
            || authority["command_id"] != request["command_id"]
            || text(authority, "request_digest")? != request_observation.0
            || self
                .canonical_observation(&authority["dependency_bindings"])?
                .0
                != text(&request, "expected_dependencies")?
        {
            return Err(bad("retained authorization/request/before binding"));
        }
        let dirs = array(plan, "new_directories")?;
        if kind.relation_attachment() && (dirs.len() != 1 || dirs[0].as_str() != Some(home))
            || !kind.relation_attachment()
                && !(kind == CompoundKind::EditionItem && dirs.is_empty())
                && *dirs != vec![json!(home)]
                && *dirs != vec![json!(parent(home)?), json!(home)]
        {
            return Err(bad("exact new child directories"));
        }
        let core = self.prepare_compound_core(
            kind,
            Cow::Borrowed(authority),
            request,
            request_raw,
            request_observation.0,
            before,
            old,
            recorded_at,
            schemas,
            &after,
            None,
            None,
            validate_local_claim,
        )?;
        if core.id != tx.manifest["transaction_id"] {
            return Err(bad("compound transaction request identity"));
        }
        let work_event = if matches!(
            kind,
            CompoundKind::WorkExpression
                | CompoundKind::EditionItem
                | CompoundKind::ExpressionEdition
                | CompoundKind::ExpressionResponsibility
                | CompoundKind::CollectionWork
        ) {
            Some(
                tx.files
                    .get(&format!("{home}/source-create-provenance.jsonl"))
                    .and_then(|pair| pair.1.as_deref())
                    .ok_or_else(|| bad("retained Work provenance event absent"))?,
            )
        } else {
            None
        };
        let FinishedCompound {
            kind: _,
            authority: _,
            request,
            before,
            parent_receipt,
            parent_files,
            mut child_files,
            receipt,
            receipt_raw: expected_raw,
            id: _,
            archive_path: _,
        } = self.finish_compound_core(core, &environment, work_event, schemas)?;
        if actual_receipt != receipt || receipt_raw != expected_raw {
            return Err(bad("exact reconstructed compound receipt bytes"));
        }
        child_files.push((kind.receipt_file().into(), expected_raw));
        if parent_files
            .iter()
            .chain(child_files.iter())
            .any(|(_, raw)| raw.len() > MAX_FILE)
        {
            return Err(ItemRefusal::Budget);
        }
        let expected: BTreeMap<_, _> = parent_files
            .iter()
            .map(|(name, raw)| {
                (
                    format!("{work_home}/{name}"),
                    (before.get(name).map(Vec::as_slice), Some(raw.as_slice())),
                )
            })
            .chain(
                child_files
                    .iter()
                    .map(|(name, raw)| (format!("{home}/{name}"), (None, Some(raw.as_slice())))),
            )
            .collect();
        let expected_state = expected
            .keys()
            .try_fold(0usize, |sum, path| {
                sum.checked_add(
                    std::mem::size_of::<(String, (Option<&[u8]>, Option<&[u8]>))>() + path.len(),
                )
            })
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(expected_state)?;
        if tx.files.len() != expected.len()
            || expected.iter().any(|(path, (before, after))| {
                tx.files
                    .get(path)
                    .is_none_or(|(old, new)| old.as_deref() != *before || new.as_deref() != *after)
            })
        {
            return Err(bad("exact whole retained before/after plan"));
        }
        drop(expected);
        self.release_temporary(expected_state);
        if self.archive(work_path, text(scope, kind.parent_key())?, &parent_receipt)? != before {
            return Err(bad("parent archive versus transaction inputs"));
        }
        Ok(Reconstructed {
            scope: self.value_copy(scope)?,
            request,
            parent_receipt,
            child: child_files.into_iter().collect(),
            receipt,
        })
    }
}

struct CompoundEventProfile<'a> {
    home: &'a str,
    receipt_file: &'static str,
    module: &'static str,
    warning: &'static str,
    executor: &'static str,
    procedure: &'static str,
    purpose: &'static str,
    component: &'static str,
    parent_id: Option<&'a str>,
    forensic_media: bool,
}
fn compound_event(
    kind: CompoundKind,
    scope: &Value,
    request: &Value,
    before: &Package,
    outputs: &[(String, &[u8])],
    environment: &Value,
    dependencies: &Value,
    recorded_at: &str,
    available: usize,
) -> Result<Value, ItemRefusal> {
    let profile = CompoundEventProfile {
        home: kind.publication_home(scope)?,
        receipt_file: kind.receipt_file(),
        module: kind.module(),
        warning: if kind == CompoundKind::ExpressionResponsibility {
            "A qualified attribution is supplied by the caller; serialization and URL presence do not prove source reading or its truth."
        } else if kind == CompoundKind::CollectionWork {
            "A qualified membership account is supplied by the caller; serialization and URL presence do not prove source reading or its truth."
        } else {
            "Observed denotes the declared record link, not accepted bibliographic or textual truth."
        },
        executor: kind.executor(),
        procedure: kind.procedure(),
        purpose: if kind == CompoundKind::ExpressionResponsibility {
            "Serialize one qualified translator Claim and an Expression responsibility reference without judging attribution."
        } else if kind == CompoundKind::CollectionWork {
            "Serialize one qualified membership Claim and a Collection membership reference without judging membership."
        } else {
            "Serialize one declared parent link and explicit source-copy forms without judging their content."
        },
        component: kind.component(),
        parent_id: Some(text(scope, kind.parent_key())?),
        forensic_media: kind == CompoundKind::EditionItem,
    };
    compound_event_profile(
        profile,
        scope,
        request,
        before,
        outputs,
        environment,
        dependencies,
        recorded_at,
        available,
    )
}
fn object_link_event(
    scope: &Value,
    request: &Value,
    outputs: &[(String, &[u8])],
    environment: &Value,
    dependencies: &Value,
    recorded_at: &str,
    available: usize,
) -> Result<Value, ItemRefusal> {
    let profile = CompoundEventProfile {
        home: parent(text(scope, "claim_source_path")?)?,
        receipt_file: OBJECT_LINK_RECEIPT,
        module: OBJECT_LINK_MODULE,
        warning: "The caller supplies the link observation and qualification. No remote content is fetched or rights conclusion reached.",
        executor: "software:tos-source-link-commands",
        procedure: "native-object-link-metadata-serialization",
        purpose: "Serialize one native Link and its qualified association Claim without observing a remote provider.",
        component: "ToS native object-Link adapter",
        parent_id: None,
        forensic_media: false,
    };
    compound_event_profile(
        profile,
        scope,
        request,
        &Package::new(),
        outputs,
        environment,
        dependencies,
        recorded_at,
        available,
    )
}
fn compound_event_profile(
    profile: CompoundEventProfile<'_>,
    scope: &Value,
    request: &Value,
    before: &Package,
    outputs: &[(String, &[u8])],
    environment: &Value,
    dependencies: &Value,
    recorded_at: &str,
    available: usize,
) -> Result<Value, ItemRefusal> {
    let module = profile.module;
    let home = profile.home;
    let request_ref = format!("{home}/source-create-request.json");
    let environment_ref = format!("{home}/source-create-environment.json");
    let mut request_raw = canonical(request)?;
    request_raw.push(b'\n');
    let mut environment_raw = canonical(environment)?;
    environment_raw.push(b'\n');
    let archive = if let Some(id) = profile.parent_id {
        format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(id.as_bytes()).to_hex(),
            hash(text(request, "expected_revision")?)?
        )
    } else {
        String::new()
    };
    let prior: BTreeMap<_, _> = before
        .values()
        .map(|raw| {
            (
                format!("{archive}/{}.blob", Digest256::of_bytes(raw).to_hex()),
                raw,
            )
        })
        .collect();
    let output: BTreeMap<_, _> = outputs.iter().map(|(p, r)| (p, r)).collect();
    let entity = |reference: &str, raw: &[u8], role: &str| json!({"entity_ref":reference,"role":role,"sha256":Digest256::of_bytes(raw).to_hex(),"size_bytes":raw.len(),"media_type":if reference.ends_with(".jsonl"){"application/x-ndjson"}else if profile.forensic_media && reference.ends_with("/forensic-report.md"){"text/markdown"}else if profile.forensic_media && reference.ends_with("/fixity.sha256"){"text/plain"}else{"application/json"},"availability":"owner_local","content_disclosure":"public_metadata_only","fixity_verified":false,"fixity_verified_at":null});
    let mut inputs = vec![entity(
        &request_ref,
        &request_raw,
        "caller-supplied-metadata-request",
    )];
    inputs.extend(
        prior
            .iter()
            .map(|(p, r)| entity(p, r, "retained-parent-metadata-input")),
    );
    let script = text(&dependencies["implementation"], module)?;
    hash(&format!("sha256:{script}"))?;
    let mut env = environment.clone();
    env.as_object_mut().unwrap().remove("argv_sha256");
    env.as_object_mut().unwrap().insert(
        "environment_profile_binding".into(),
        json!({"ref":environment_ref,"sha256":Digest256::of_bytes(&environment_raw).to_hex()}),
    );
    let derivation =
        text(scope, "provenance_event_id")?.replacen("tos.event.", "tos.derivation.", 1);
    let output_bytes = output
        .values()
        .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
        .ok_or(ItemRefusal::Budget)?;
    let mut scratch = request_raw
        .len()
        .checked_add(environment_raw.len())
        .and_then(|n| n.checked_add(crate::record_biblio_cut::decoded_state(&env).ok()?))
        .ok_or(ItemRefusal::Budget)?;
    scratch = scratch
        .checked_add(
            inputs
                .iter()
                .try_fold(0usize, |n, v| {
                    n.checked_add(crate::record_biblio_cut::decoded_state(v).ok()?)
                })
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    scratch = scratch
        .checked_add(
            prior
                .keys()
                .try_fold(0usize, |n, p| {
                    n.checked_add(std::mem::size_of::<(String, &Vec<u8>)>() + p.len())
                })
                .ok_or(ItemRefusal::Budget)?,
        )
        .and_then(|n| n.checked_add(output.len() * std::mem::size_of::<(&String, &&[u8])>()))
        .ok_or(ItemRefusal::Budget)?;
    let result = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json","schema_version":"tos_provenance_event_v2","event_id":scope["provenance_event_id"],"event_version":1,"supersedes_event_ref":null,
        "record_binding":{"manifest_ref":format!("{home}/{}",profile.receipt_file),"digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"},
        "activity":{"event_type":"annotation","started_at":recorded_at,"ended_at":recorded_at,"status":"completed_with_warnings","terminal_reason":null,"exit_code":0,"warnings":["Captured prepared metadata buffers; the committed transaction is a separate verification.",profile.warning]},
        "entities":{"inputs":inputs,"outputs":output.iter().map(|(p,r)|entity(p,r,"prepared-compound-source-metadata")).collect::<Vec<_>>(),"byproducts":[entity(&environment_ref,&environment_raw,"runtime-description")]},
        "derivations":output.keys().enumerate().map(|(index,p)|json!({"derivation_id":format!("{derivation}.output-{index}"),"input_entity_ref":request_ref,"output_entity_ref":p,"relation":"was_derived_from","influence_asserted":true,"description":"Technical source metadata serialization; no historical influence or textual identity is asserted."})).collect::<Vec<_>>(),
        "responsibility":[{"agent_ref":profile.executor,"agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":{"ref":module,"sha256":script},"human_evidence_status":"not_applicable"}],
        "method":{"procedure":{"name":profile.procedure,"version":"1","purpose":profile.purpose},"command_capture":{"disclosure":"withheld_digest_only","argv":null,"argv_sha256":environment["argv_sha256"],"withholding_reason":"Process arguments may contain a private owner-configuration path."},"configuration_binding":{"ref":request_ref,"sha256":Digest256::of_bytes(&request_raw).to_hex()},"software_components":[{"name":profile.component,"version":"1","role":"serialization-runner","artifact_ref":module,"artifact_sha256":script,"verification_status":"verified"}],"model_invocations":[],"environment":env},
        "manual_changes":{"status":"none_declared","change_receipts":[],"statement":"Caller authorship precedes this operation; no manual edits are performed inside serialization."},
        "measurements":[{"metric":"output_bytes","status":"measured","value":output_bytes,"unit":"bytes","method":"Sum of prepared source record, form and parent history buffers; excludes capture and receipt.","evidence_binding":null}],
        "evidence_authentication":{"capture_posture":"tool_captured","signature_status":"unsigned","signature_bindings":[],"verification_status":"unverified","producer_control_boundary":"The same unsigned local process serializes and records; hashes do not authenticate execution truth."},
        "rights_and_visibility":{"rights_record_bindings":[],"intended_uses":["local_research","public_metadata"],"content_visibility":"tracked_public_metadata","publication_authorized":false,"publication_authority_bindings":[]},
        "review_and_authority":{"mechanical_validation":"not_run","human_review_status":"not_performed","review_bindings":[],"accepted_uses":[],"promotion_authorized":false,"competence_evidence_bindings":[]},
        "reproducibility":{"classification":"partially_specified","known_gaps":["Upstream research, source reading and model invocations are outside this operation.","Runtime metadata is captured, not a complete archived execution environment."],"replay_scope":"Exact retained request, metadata and source-copy buffer construction; not bibliographic truth."},
        "authority_boundary":{"validator_role":"mechanics_and_closure_only_not_truth","claims_not_established":["execution_truth","content_truth","source_fidelity","translation_quality","semantic_correctness","rights_clearance","human_review","publication_authority","canon_authority"]}
    });
    let used = scratch
        .checked_add(crate::record_biblio_cut::decoded_state(&result)?)
        .ok_or(ItemRefusal::Budget)?;
    if used > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "compound provenance logical workspace",
            used: Some(used as u64),
            limit: Some(available as u64),
        });
    }
    Ok(result)
}

fn work_capture_entity(path: &str, raw: &[u8], role: &str) -> Value {
    json!({"entity_ref":path,"role":role,"media_type":if path.ends_with(".jsonl") {"application/x-ndjson"} else if path.ends_with("/forensic-report.md") || path.ends_with("/fixity.sha256") {"text/plain; charset=utf-8"} else {"application/json"},
        "size_bytes":raw.len(),"sha256":Digest256::of_bytes(raw).to_hex(),"availability":"owner_local",
        "content_disclosure":"public_metadata_only","fixity_verified":false,"fixity_verified_at":null})
}

/// Check a real, retained native capture against the exact Work or Item buffers. The
/// selected software and ELF observations are authenticated by the CMD owner;
/// this mechanical check cannot infer who executed a program from its event.
fn native_work_event(
    kind: CompoundKind,
    event: &Value,
    scope: &Value,
    request: &Value,
    dependencies: &Value,
    before: &Package,
    outputs: &[(String, &[u8])],
    environment: &Value,
    archive: &str,
    _recorded_at: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    available: usize,
) -> Result<(), ItemRefusal> {
    native_compound_capture_event(
        event,
        scope,
        request,
        dependencies,
        before,
        outputs,
        environment,
        archive,
        kind.publication_home(scope)?,
        kind.receipt_file(),
        native_preparation_procedure(kind)?,
        native_preparation_purpose(kind)?,
        native_preparation_warning(kind)?,
        deadline,
        cancelled,
        available,
    )
}
fn native_compound_capture_event(
    event: &Value,
    scope: &Value,
    request: &Value,
    dependencies: &Value,
    before: &Package,
    outputs: &[(String, &[u8])],
    environment: &Value,
    archive: &str,
    home: &str,
    receipt_file: &str,
    procedure: &str,
    purpose: &str,
    warning: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    available: usize,
) -> Result<(), ItemRefusal> {
    check(deadline, cancelled)?;
    native_work_environment(environment)?;
    let request_ref = format!("{home}/source-create-request.json");
    let environment_ref = format!("{home}/source-create-environment.json");
    let mut request_raw = canonical(request)?;
    request_raw.push(b'\n');
    let mut environment_raw = canonical(environment)?;
    environment_raw.push(b'\n');
    let mut prior = BTreeMap::new();
    for raw in before.values() {
        prior.insert(
            format!("{archive}/{}.blob", Digest256::of_bytes(raw).to_hex()),
            raw,
        );
    }
    let mut sorted = outputs.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    if sorted.windows(2).any(|v| v[0].0 == v[1].0) {
        return Err(bad("native Work duplicate output"));
    }
    let mut inputs = vec![work_capture_entity(
        &request_ref,
        &request_raw,
        "caller-supplied-metadata-request",
    )];
    inputs.extend(
        prior
            .iter()
            .map(|(path, raw)| work_capture_entity(path, raw, "retained-parent-metadata-input")),
    );
    let output_entities = sorted
        .iter()
        .map(|(path, raw)| work_capture_entity(path, raw, "prepared-compound-source-metadata"))
        .collect::<Vec<_>>();
    let event_id = text(scope, "provenance_event_id")?;
    let derivation = event_id.replacen("tos.event.", "tos.derivation.", 1);
    let derivations = sorted.iter().enumerate().map(|(index,(path,_))| json!({
        "derivation_id":format!("{derivation}.output-{index}"),"input_entity_ref":request_ref,
        "output_entity_ref":path,"relation":"was_derived_from","influence_asserted":true,
        "description":"Technical source metadata serialization; no historical influence or textual identity is asserted."
    })).collect::<Vec<_>>();
    let mut scratch = request_raw
        .len()
        .checked_add(environment_raw.len())
        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Value>>() * 3))
        .and_then(|n| n.checked_add(std::mem::size_of::<BTreeMap<String, &Vec<u8>>>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<(String, &[u8])>>()))
        .and_then(|n| {
            n.checked_add(
                std::mem::size_of::<String>() * 3
                    + request_ref.len()
                    + environment_ref.len()
                    + derivation.len(),
            )
        })
        .ok_or(ItemRefusal::Budget)?;
    for path in prior.keys().chain(sorted.iter().map(|(path, _)| path)) {
        scratch = scratch
            .checked_add(std::mem::size_of::<(String, &[u8])>() + path.len())
            .ok_or(ItemRefusal::Budget)?;
    }
    for value in inputs
        .iter()
        .chain(output_entities.iter())
        .chain(derivations.iter())
    {
        scratch = scratch
            .checked_add(crate::record_biblio_cut::decoded_state(value)?)
            .ok_or(ItemRefusal::Budget)?;
    }
    let mut method_environment = environment.clone();
    method_environment
        .as_object_mut()
        .ok_or_else(|| bad("native Work environment object"))?
        .remove("argv_sha256");
    method_environment["environment_profile_binding"] = json!({"ref":environment_ref,
        "sha256":Digest256::of_bytes(&environment_raw).to_hex()});
    scratch = scratch
        .checked_add(crate::record_biblio_cut::decoded_state(
            &method_environment,
        )?)
        .ok_or(ItemRefusal::Budget)?;
    if scratch > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "native Work capture reference workspace",
            used: Some(scratch as u64),
            limit: Some(available as u64),
        });
    }
    let method = &event["method"];
    let components = array(method, "software_components")?;
    let software_index = std::mem::size_of::<BTreeSet<&str>>()
        .checked_add(
            components
                .len()
                .checked_mul(std::mem::size_of::<&str>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    if scratch
        .checked_add(software_index)
        .is_none_or(|used| used > available)
    {
        return Err(ItemRefusal::BudgetCheck {
            check: "native Work software component index",
            used: scratch.checked_add(software_index).map(|n| n as u64),
            limit: Some(available as u64),
        });
    }
    let runtime_sha = text(environment, "runtime_artifact_sha256")?;
    hash(&format!("sha256:{runtime_sha}"))?;
    let runtime_rows = components
        .iter()
        .filter(|v| {
            v["artifact_ref"] == "runtime:tos-native-executable"
                && v["artifact_sha256"] == runtime_sha
                && v["role"] == "serialization-runner"
                && v["verification_status"] == "verified"
        })
        .count();
    if runtime_rows != 1
        || components.len() < 2
        || components.len() > 129
        || components
            .iter()
            .any(|v| v["artifact_ref"] == "runtime:python-executable")
        || components[..components.len() - 1].iter().any(|v| {
            v["role"] != "serialization-source-observation"
                || v["version"] != "selected-capture-bytes"
                || v["verification_status"] != "verified"
                || v["name"] != v["artifact_ref"]
                || v["artifact_ref"].as_str().is_none_or(|path| {
                    path.starts_with("ToS/") || RelativePath::parse(path).is_err()
                })
                || v["artifact_sha256"]
                    .as_str()
                    .is_none_or(|sha| hash(&format!("sha256:{sha}")).is_err())
        })
        || components.last().is_none_or(|v| {
            v["artifact_ref"] != "runtime:tos-native-executable"
                || v["name"] != "executing native process"
                || v["version"] != environment["runtime_version"]
        })
        || components[..components.len() - 1]
            .iter()
            .filter_map(|v| v["artifact_ref"].as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != components.len() - 1
    {
        return Err(bad("native Work observed software/ELF closure"));
    }
    // The retained authorization names the required historical source bytes.
    // Additional captured rows describe the actual selected producer subset;
    // they do not weaken any required implementation binding or attest a build.
    let required = dependencies["implementation"]
        .as_object()
        .ok_or_else(|| bad("native Work implementation bindings"))?;
    if required.is_empty() || required.len() > components.len() - 1 {
        return Err(bad("native Work implementation subset"));
    }
    for (path, digest) in required {
        check(deadline, cancelled)?;
        if path.starts_with("ToS/")
            || RelativePath::parse(path).is_err()
            || digest
                .as_str()
                .is_none_or(|sha| Digest256::from_hex(sha).is_err())
            || !components[..components.len() - 1].iter().any(|row| {
                row["artifact_ref"] == path.as_str()
                    && row["artifact_sha256"] == digest.as_str().unwrap_or("")
            })
        {
            return Err(bad("native Work required software source digest"));
        }
    }
    let start = text(&event["activity"], "started_at")?;
    let end = text(&event["activity"], "ended_at")?;
    if crate::retirement_rules::observed_instant_order(start, end)
        .map_err(|_| bad("native Work capture instants"))?
        == std::cmp::Ordering::Greater
    {
        return Err(bad("native Work capture chronology"));
    }
    let measurements = array(event, "measurements")?;
    if measurements.len() != 1
        || measurements[0]["metric"] != "wall_duration_ms"
        || measurements[0]["status"] != "measured"
        || measurements[0]["unit"] != "ms"
        || measurements[0]["method"]
            != "Rust monotonic Instant from capture through native executable/source observation and buffer binding; excludes commit."
        || measurements[0]["evidence_binding"] != Value::Null
        || measurements[0]["value"]
            .as_f64()
            .is_none_or(|v| !v.is_finite() || v < 0.0)
    {
        return Err(bad("native Work observed duration"));
    }
    if event["$schema"]
        != "https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json"
        || event["schema_version"] != "tos_provenance_event_v2"
        || event["event_id"] != event_id
        || event["event_version"] != 1
        || !event["supersedes_event_ref"].is_null()
        || event["record_binding"]
            != json!({"manifest_ref":format!("{home}/{}",receipt_file),
            "digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"})
        || event["entities"]["inputs"] != json!(inputs)
        || event["entities"]["outputs"] != json!(output_entities)
        || event["entities"]["byproducts"]
            != json!([work_capture_entity(
                &environment_ref,
                &environment_raw,
                "runtime-description"
            )])
        || event["derivations"] != json!(derivations)
        || method["procedure"]
            != json!({"name":procedure,"version":"1",
            "purpose":purpose})
        || method["configuration_binding"]
            != json!({"ref":request_ref,"sha256":Digest256::of_bytes(&request_raw).to_hex()})
        || method["environment"] != method_environment
        || method["command_capture"]["argv_sha256"] != environment["argv_sha256"]
        || method["command_capture"]["disclosure"] != "withheld_digest_only"
        || !method["command_capture"]["argv"].is_null()
        || event["responsibility"]
            != json!([{"agent_ref":"software:tos-native-source-commands",
            "agent_kind":"software","role":"executor","responsibility_posture":"performed",
            "evidence_binding":null,"human_evidence_status":"not_applicable"}])
        || event["activity"]["event_type"] != "annotation"
        || event["activity"]["status"] != "completed_with_warnings"
        || !event["activity"]["terminal_reason"].is_null()
        || event["activity"]["exit_code"] != 0
        || event["activity"]["warnings"]
            != json!([
                warning,
                "The declared record link is not accepted bibliographic or textual truth."
            ])
        || !method["model_invocations"]
            .as_array()
            .is_some_and(Vec::is_empty)
        || method["command_capture"]["withholding_reason"]
            != "Observed process argv may contain private paths; the exact library request is captured separately."
        || event["manual_changes"]
            != json!({"status":"none_declared","change_receipts":[],
            "statement":"Caller authorship precedes this operation; no manual edits occur inside serialization."})
        || event["rights_and_visibility"]
            != json!({"rights_record_bindings":[],
            "intended_uses":["local_research","public_metadata"],"content_visibility":"tracked_public_metadata",
            "publication_authorized":false,"publication_authority_bindings":[]})
        || event["review_and_authority"]
            != json!({"mechanical_validation":"not_run",
            "human_review_status":"not_performed","review_bindings":[],"accepted_uses":[],
            "promotion_authorized":false,"competence_evidence_bindings":[]})
        || event["evidence_authentication"]
            != json!({"capture_posture":"tool_captured",
            "signature_status":"unsigned","signature_bindings":[],"verification_status":"unverified",
            "producer_control_boundary":"The same unsigned native process serializes and observes; hashes do not authenticate execution truth."})
        || event["reproducibility"]
            != json!({"classification":"partially_specified",
            "known_gaps":[
                "Selected source capture is byte evidence only; compiler, dependencies, build and source-to-ELF relation are not attested.",
                "Upstream research, reading and model invocation are outside this serialization.",
                "Clock observations and durations are not deterministic; complete runtime environment is not archived."
            ],"replay_scope":"Exact retained request and source-copy buffers only, not bibliographic truth."})
        || event["authority_boundary"]
            != json!({"validator_role":"mechanics_and_closure_only_not_truth",
            "claims_not_established":["execution_truth","content_truth","source_fidelity",
                "translation_quality","semantic_correctness","rights_clearance","human_review",
                "publication_authority","canon_authority"]})
    {
        return Err(bad("native Work capture profile/byte references"));
    }
    Ok(())
}

impl NativeCompoundReader<'_> {
    fn reconstruct_object_link<S, F>(
        &mut self,
        tx: &Transaction,
        schemas: &mut S,
        validate_local_claim: &mut F,
    ) -> Result<ObjectLinkReconstructed, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        let plan = &tx.manifest["plan"];
        keys(plan, &["authorization", "new_directories", "files"])?;
        let authority = &plan["authorization"];
        let scope = &authority["scope"];
        let link_home = parent(text(scope, "link_source_path")?)?;
        let claim_home = parent(text(scope, "claim_source_path")?)?;
        if tx
            .files
            .values()
            .any(|(before, after)| before.is_some() || after.is_none())
        {
            return Err(bad("object-Link new-only exact plan"));
        }
        let output_state = tx
            .files
            .iter()
            .try_fold(std::mem::size_of::<Package>(), |n, (path, (_, after))| {
                n.checked_add(std::mem::size_of::<(String, Vec<u8>)>())?
                    .checked_add(path.len())?
                    .checked_add(after.as_ref()?.len())
            })
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(output_state)?;
        if plan["new_directories"]
            != json!(
                [link_home, claim_home]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            )
        {
            return Err(bad("object-Link new home plan"));
        }
        let after = |path: &str| {
            tx.files
                .get(path)
                .and_then(|(_, after)| after.as_ref())
                .ok_or_else(|| bad("object-Link new-only exact plan"))
        };
        let receipt_raw = after(&format!("{claim_home}/{OBJECT_LINK_RECEIPT}"))?;
        let actual_receipt = self.decoded(receipt_raw)?;
        let event_raw = after(&format!("{claim_home}/source-create-provenance.jsonl"))?;
        let _actual_event = self.decoded(event_raw)?;
        let result = self.compose_object_link_with_validator(
            scope,
            authority,
            after(&format!("{claim_home}/source-create-request.json"))?,
            after(&format!("{claim_home}/source-create-environment.json"))?,
            text(&actual_receipt, "recorded_at")?,
            schemas,
            validate_local_claim,
            Some(event_raw),
            None,
        )?;
        if tx.manifest["transaction_id"] != result.receipt["transaction_id"] {
            return Err(bad("object-Link transaction identity"));
        }
        if result.files[&format!("{claim_home}/source-create-provenance.jsonl")] != *event_raw {
            return Err(bad("object-Link exact source provenance event"));
        }
        if result.files[&format!("{claim_home}/{OBJECT_LINK_RECEIPT}")] != *receipt_raw
            || result.receipt != actual_receipt
        {
            return Err(bad("object-Link exact reconstructed receipt bytes"));
        }
        if result.files.len() != tx.files.len()
            || result.files.iter().any(|(path, raw)| {
                tx.files
                    .get(path)
                    .is_none_or(|(before, after)| before.is_some() || after.as_ref() != Some(raw))
            })
        {
            return Err(bad("object-Link complete before/after transaction plan"));
        }
        Ok(result)
    }
    fn compose_object_link<S: CutSchemaExecutor + CutSchemaReceiptRange>(
        &mut self,
        scope: &Value,
        authority: &Value,
        request_raw: &[u8],
        environment_raw: &[u8],
        recorded_at: &str,
        schemas: &mut S,
        retained_event: Option<&[u8]>,
        capture: Option<
            &mut dyn FnMut(&[(String, &[u8])]) -> Result<(Vec<u8>, Vec<u8>), ItemRefusal>,
        >,
    ) -> Result<ObjectLinkReconstructed, ItemRefusal> {
        let cut = self.cut;
        let cancelled = self.cancelled;
        let mut validate_local_claim = |raw: &[u8], schemas: &mut S, limits: ItemLimits| {
            let mut local = crate::record_rules::validate_source_claim_from_cut(
                cut.ok_or_else(|| bad("ObjectLink writer requires corpus cut"))?,
                raw,
                schemas,
                limits,
                cancelled,
            )?;
            if !local.issues.is_empty() {
                return Err(bad("object-Link exact local Claim profile"));
            }
            Ok(LocalClaimValidation {
                dependency_digests: std::mem::take(&mut local.dependency_digests),
                dependency_bytes_read: 0,
            })
        };
        self.compose_object_link_with_validator(
            scope,
            authority,
            request_raw,
            environment_raw,
            recorded_at,
            schemas,
            &mut validate_local_claim,
            retained_event,
            capture,
        )
    }

    fn compose_object_link_with_validator<S, F>(
        &mut self,
        scope: &Value,
        authority: &Value,
        request_raw: &[u8],
        environment_raw: &[u8],
        recorded_at: &str,
        schemas: &mut S,
        validate_local_claim: &mut F,
        retained_event: Option<&[u8]>,
        mut capture: Option<
            &mut dyn FnMut(&[(String, &[u8])]) -> Result<(Vec<u8>, Vec<u8>), ItemRefusal>,
        >,
    ) -> Result<ObjectLinkReconstructed, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        keys(
            authority,
            &[
                "schema_version",
                "scope",
                "principal_id",
                "maker_type",
                "authority_ref",
                "owner_configuration",
                "command_id",
                "request_digest",
                "dependency_bindings",
            ],
        )?;
        if authority["schema_version"] != "tos_object_link_authorization_v1" {
            return Err(bad("object-Link exact adapter authorization"));
        }
        if scope != &authority["scope"] {
            return Err(bad("object-Link exact scope binding"));
        }
        let link_path = text(scope, "link_source_path")?;
        let claim_path = text(scope, "claim_source_path")?;
        let claim_home = parent(claim_path)?;
        let link_home = parent(link_path)?;
        let request_path = format!("{claim_home}/source-create-request.json");
        let receipt_path = format!("{claim_home}/{OBJECT_LINK_RECEIPT}");
        let environment_path = format!("{claim_home}/source-create-environment.json");
        let event_path = format!("{claim_home}/source-create-provenance.jsonl");
        let request = self.decoded(request_raw)?;
        keys(
            &request,
            &[
                "schema_version",
                "operation",
                "subject",
                "link",
                "claim",
                "forms",
                "claim_forms",
                "reason",
                "command_id",
                "expected_configuration",
                "expected_dependencies",
                "expected_publication",
            ],
        )?;
        if request["schema_version"] != "tos_local_object_link_command_v1"
            || request["operation"] != OBJECT_LINK_OPERATION
            || self.canonical_observation(&request)?.1 > 1_048_576
            || text(&request, "reason")?.trim().is_empty()
            || text(&request, "reason")?.chars().count() > 4096
            || !(1..=256).contains(&text(&request, "command_id")?.chars().count())
            || request["expected_configuration"] != authority["owner_configuration"]
            || request["command_id"] != authority["command_id"]
            || self.canonical_observation(&request)?.0 != text(authority, "request_digest")?
            || self
                .canonical_observation(&authority["dependency_bindings"])?
                .0
                != text(&request, "expected_dependencies")?
        {
            return Err(bad("object-Link exact retained request"));
        }
        for field in ["expected_configuration", "expected_dependencies"] {
            hash(text(&request, field)?)?;
        }
        if !request["expected_publication"].is_null() {
            hash(text(&request, "expected_publication")?)?;
        }
        object_link_scope(scope, &request, authority)?;
        let request_digest = self.canonical_observation(&request)?.0;
        let transaction_id=self.canonical_observation(&json!({"operation":OBJECT_LINK_OPERATION,"command_id":request["command_id"],"owner_configuration":request["expected_configuration"],"request_digest":request_digest}))?.0;
        let dependencies = &authority["dependency_bindings"];
        keys(
            dependencies,
            &[
                "catalog_and_sources",
                "contracts",
                "implementation",
                "retained_transactions",
            ],
        )?;
        if dependencies["retained_transactions"]
            .as_object()
            .is_none_or(|rows| !rows.is_empty())
        {
            return Err(bad(
                "object-Link unexpected retained transaction dependency",
            ));
        }
        for group in ["catalog_and_sources", "contracts", "implementation"] {
            let rows = dependencies[group]
                .as_object()
                .ok_or_else(|| bad("object-Link dependency bindings"))?;
            for (path, digest) in rows {
                check(self.limits.deadline, self.cancelled)?;
                if Digest256::from_hex(
                    digest
                        .as_str()
                        .ok_or_else(|| bad("object-Link dependency SHA"))?,
                )
                .is_err()
                    || path.is_empty()
                {
                    return Err(bad("object-Link dependency binding"));
                }
            }
        }
        let contract_bindings = dependencies["contracts"]
            .as_object()
            .ok_or_else(|| bad("object-Link source contract bindings"))?;
        for (path, digest) in contract_bindings {
            RelativePath::parse(path).map_err(|_| bad("object-Link contract locator"))?;
            let (current_digest, _) = self.current_digest_size(path)?;
            if digest.as_str() != Some(current_digest.to_hex().as_str()) {
                return Err(bad("object-Link current source contract digest"));
            }
        }
        let expected_sha = text(
            &dependencies["catalog_and_sources"],
            text(scope, "subject_source_path")?,
        )?;
        if Digest256::from_hex(expected_sha).is_err() {
            return Err(bad("object-Link exact subject raw dependency"));
        }
        let subject = &request["subject"];
        let link = &request["link"];
        let claim = &request["claim"];
        let subject_schema = if scope["subject_record_type"] == "artifact" {
            match text(subject, "schema_version")? {
                "tos_artifact_source_witness_v1" => {
                    "ToS/contracts/artifact-source-witness.schema.json"
                }
                "tos_artifact_source_witness_v2" => {
                    "ToS/contracts/artifact-source-witness-v2.schema.json"
                }
                _ => return Err(bad("object-Link artifact native schema")),
            }
        } else {
            "ToS/contracts/corpus-record.schema.json"
        };
        for (value, contract) in [
            (subject, subject_schema),
            (link, "ToS/contracts/source-link.schema.json"),
            (claim, "ToS/contracts/object-link-claim-v2.schema.json"),
        ] {
            let raw = self.canonical_buffer(value)?;
            if !schemas.check_reusing_scalar(
                "object-link-retained-input",
                &raw,
                contract,
                self.limits.deadline,
                self.cancelled,
            )? {
                return Err(bad("object-Link selected source schema"));
            }
            let contract_path =
                RelativePath::parse(contract).map_err(|_| bad("object-Link contract path"))?;
            let (current_digest, _) = self.current_digest_size(contract_path.as_str())?;
            if dependencies["contracts"][contract] != current_digest.to_hex() {
                return Err(bad("object-Link contract fixity dependency"));
            }
        }
        let mut claim_raw = self.canonical_buffer(claim)?;
        claim_raw.push(b'\n');
        let claim_raw = self.buffer(claim_raw)?;
        let mut local_limits = self.limits;
        local_limits.max_state_bytes = local_limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        local_limits.max_total_bytes = local_limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let mut local = validate_local_claim(&claim_raw, schemas, local_limits)?;
        for (path, digest) in &local.dependency_digests {
            if contract_bindings.get(path).and_then(Value::as_str) != Some(digest.to_hex().as_str())
            {
                return Err(bad("object-Link local Claim owner contract binding"));
            }
        }
        account(
            &mut self.bytes,
            usize::try_from(local.dependency_bytes_read).map_err(|_| ItemRefusal::Budget)?,
            self.limits.max_total_bytes,
        )?;
        let dependency_digests = std::mem::take(&mut local.dependency_digests);
        drop(local);
        let dependencies_state = dependency_digests
            .keys()
            .try_fold(0usize, |n, path| {
                n.checked_add(std::mem::size_of::<(String, Digest256)>() + path.len())
            })
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(dependencies_state)?;
        for (path, sha) in dependency_digests {
            RelativePath::parse(&path).map_err(|_| bad("object-Link Claim dependency path"))?;
            let (current_digest, current_size) = self.current_digest_size(&path)?;
            if current_digest != sha {
                return Err(bad("object-Link Claim dependency changed"));
            }
            if self.cut.is_some() {
                account(
                    &mut self.bytes,
                    usize::try_from(current_size).map_err(|_| ItemRefusal::Budget)?,
                    self.limits.max_total_bytes,
                )?;
            }
            self.release_temporary(std::mem::size_of::<(String, Digest256)>() + path.len());
            if self.input.is_none() {
                self.record_read(PredicateRead::ExactPath {
                    path,
                    digest: sha.to_prefixed(),
                })?;
            }
        }
        let ordered_request = self.ordered_value(request_raw)?;
        let link_ordered = ordered_request
            .object_get("link")
            .ok_or_else(|| bad("ordered object-Link source"))?;
        let claim_ordered = ordered_request
            .object_get("claim")
            .ok_or_else(|| bad("ordered object-Link Claim"))?;
        let link_raw = pretty(link_ordered)?;
        let mut canonical_claim = canonical_ordered(claim_ordered)?;
        canonical_claim.push(b'\n');
        if canonical_claim != claim_raw {
            return Err(bad("object-Link canonical Claim bytes"));
        }
        self.release_temporary(std::mem::size_of::<Vec<u8>>() + claim_raw.len());
        drop(claim_raw);
        let principal = text(authority, "principal_id")?;
        let (link_forms, link_refs) = forms(
            link_ordered,
            None,
            &request["forms"],
            principal,
            false,
            self.limits
                .max_state_bytes
                .checked_sub(self.state)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        self.temporary(
            crate::record_biblio_cut::ordered_state(&link_forms)?
                .checked_add(crate::record_biblio_cut::ordered_state(&link_refs)?)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let (claim_forms, claim_refs) = forms(
            claim_ordered,
            None,
            &request["claim_forms"],
            principal,
            true,
            self.limits
                .max_state_bytes
                .checked_sub(self.state)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        self.temporary(
            crate::record_biblio_cut::ordered_state(&claim_forms)?
                .checked_add(crate::record_biblio_cut::ordered_state(&claim_refs)?)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let link_form_path = format!("{link_home}/link.human-forms.json");
        let claim_form_path = format!(
            "{claim_home}/source-claims.{}.human-forms.json",
            Digest256::of_bytes(text(scope, "claim_id")?.as_bytes()).to_hex()
        );
        let mut files = Package::new();
        files.insert(link_path.into(), link_raw);
        files.insert(link_form_path.clone(), pretty(&link_forms)?);
        files.insert(claim_path.into(), canonical_claim);
        files.insert(claim_form_path.clone(), pretty(&claim_forms)?);
        let native_capture_requested = capture.is_some();
        let captured;
        let (environment_raw, retained_event) = if let Some(capture) = capture.as_mut() {
            let outputs: Vec<_> = files
                .iter()
                .map(|(path, raw)| (path.clone(), raw.as_slice()))
                .collect();
            captured = capture(&outputs)?;
            if captured.0.len() > MAX_FILE || captured.1.len() > MAX_FILE {
                return Err(ItemRefusal::Budget);
            }
            self.temporary(
                captured
                    .0
                    .len()
                    .checked_add(captured.1.len())
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            (captured.0.as_slice(), Some(captured.1.as_slice()))
        } else {
            (environment_raw, retained_event)
        };
        let environment = self.decoded(environment_raw)?;
        crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
            .map_err(|_| bad("object-Link aware recorded instant"))?;
        keys(
            &environment,
            &[
                "runtime",
                "runtime_version",
                "runtime_artifact_sha256",
                "backend",
                "hardware_target",
                "unicode_version",
                "argv_sha256",
            ],
        )?;
        for field in [
            "runtime",
            "runtime_version",
            "runtime_artifact_sha256",
            "backend",
            "hardware_target",
            "unicode_version",
            "argv_sha256",
        ] {
            if text(&environment, field)?.is_empty() {
                return Err(bad("object-Link retained runtime"));
            }
        }
        for field in ["runtime_artifact_sha256", "argv_sha256"] {
            if Digest256::from_hex(text(&environment, field)?).is_err() {
                return Err(bad("object-Link runtime digest"));
            }
        }
        let outputs: Vec<_> = files
            .iter()
            .map(|(path, raw)| (path.clone(), raw.as_slice()))
            .collect();
        let outputs_state = slice_rows_state(&outputs)?;
        self.temporary(outputs_state)?;
        let native_event = retained_event.map(|raw| self.decoded(raw)).transpose()?;
        if native_capture_requested
            && native_event.as_ref().is_none_or(|event| {
                event["method"]["procedure"]["name"] != "native-object-link-serialization"
            })
        {
            return Err(bad("ObjectLink native producer capture profile"));
        }
        let event = if native_event.as_ref().is_some_and(|event| {
            event["method"]["procedure"]["name"] == "native-object-link-serialization"
        }) {
            let event = native_event.unwrap();
            native_compound_capture_event(
                &event,
                scope,
                &request,
                dependencies,
                &Package::new(),
                &outputs,
                &environment,
                "",
                claim_home,
                OBJECT_LINK_RECEIPT,
                "native-object-link-serialization",
                "Serialize one declared Object/Link association and explicit source-copy forms without judging content.",
                "Completed in-process Object/Link buffer serialization; atomic selected-metadata publication occurs afterward.",
                self.limits.deadline,
                self.cancelled,
                self.limits
                    .max_state_bytes
                    .checked_sub(self.state)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            event
        } else {
            object_link_event(
                scope,
                &request,
                &outputs,
                &environment,
                dependencies,
                recorded_at,
                self.limits
                    .max_state_bytes
                    .checked_sub(self.state)
                    .ok_or(ItemRefusal::Budget)?,
            )?
        };
        drop(outputs);
        self.release_temporary(outputs_state);
        self.temporary(crate::record_biblio_cut::decoded_state(&event)?)?;
        let mut event_bytes = self.canonical_buffer(&event)?;
        event_bytes.push(b'\n');
        if !schemas.check_reusing_scalar(
            &event_path,
            &event_bytes,
            "ToS/contracts/provenance-event-v2.schema.json",
            self.limits.deadline,
            self.cancelled,
        )? {
            return Err(bad("object-Link event schema/exact bytes"));
        }
        let mut request_bytes = self.canonical_buffer(&request)?;
        request_bytes.push(b'\n');
        let mut environment_bytes = self.canonical_buffer(&environment)?;
        environment_bytes.push(b'\n');
        files.insert(request_path.clone(), request_bytes);
        files.insert(environment_path.clone(), environment_bytes);
        files.insert(event_path.clone(), event_bytes);
        let mut scope_fields = Vec::new();
        for key in OBJECT_LINK_SCOPE {
            scope_fields.push((key, j(&scope[key])?));
        }
        scope_fields.sort_by_key(|(key, _)| *key);
        // Maintained receipt bytes retain the writer's insertion order, while
        // the transaction's file rows are sorted independently.
        let refs: Vec<_> = [
            link_path,
            &link_form_path,
            claim_path,
            &claim_form_path,
            &request_path,
            &environment_path,
            &event_path,
        ]
        .into_iter()
        .map(|path| (path.to_owned(), files[path].as_slice()))
        .collect();
        let moved_form_refs_state = crate::record_biblio_cut::ordered_state(&link_refs)?
            .checked_add(crate::record_biblio_cut::ordered_state(&claim_refs)?)
            .ok_or(ItemRefusal::Budget)?;
        let receipt_ordered = object(vec![
            ("schema_version", string("tos_object_link_receipt_v1")),
            ("operation", string(OBJECT_LINK_OPERATION)),
            ("transaction_id", string(&transaction_id)),
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&request_digest)),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            ("recorded_at", string(recorded_at)),
            ("scope", object(scope_fields)),
            ("dependencies", j(&request["expected_dependencies"])?),
            (
                "subject",
                ref_ordered(
                    subject,
                    if scope["subject_record_type"] == "artifact" {
                        "artifact_id"
                    } else {
                        "record_id"
                    },
                    "record_version",
                )?,
            ),
            ("subject_source_sha256", string(expected_sha)),
            ("link", ref_ordered(link, "record_id", "record_version")?),
            ("claim", ref_ordered(claim, "claim_id", "claim_version")?),
            (
                "forms",
                object(vec![("link", link_refs), ("claim", claim_refs)]),
            ),
            ("files", refs_ordered(&refs)),
            ("grants_admission", JsonValue::Bool(false)),
        ]);
        // The two form-reference trees moved into the receipt. Transfer their
        // existing charge before accounting the complete receipt tree.
        self.release_temporary(moved_form_refs_state);
        let receipt_tree_state = crate::record_biblio_cut::ordered_state(&receipt_ordered)?;
        self.temporary(receipt_tree_state)?;
        let receipt_bytes = pretty(&receipt_ordered)?;
        drop(receipt_ordered);
        self.release_temporary(receipt_tree_state);
        let receipt = self.decoded(&receipt_bytes)?;
        files.insert(receipt_path, receipt_bytes);
        if files.values().any(|raw| raw.len() > MAX_FILE) {
            return Err(ItemRefusal::BudgetCheck {
                check: "object-Link prepared file bytes",
                used: None,
                limit: Some(MAX_FILE as u64),
            });
        }
        let state = crate::record_biblio_cut::decoded_state(scope)?
            .checked_add(crate::record_biblio_cut::decoded_state(&request)?)
            .and_then(|n| n.checked_add(crate::record_biblio_cut::decoded_state(&receipt).ok()?))
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(state)?;
        Ok(ObjectLinkReconstructed {
            scope: scope.clone(),
            request,
            receipt,
            files,
        })
    }
}

impl NativeCompoundReader<'_> {
    pub(crate) fn verify<S>(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut S,
    ) -> Result<NativeCompoundObservation, ItemRefusal>
    where
        S: NativeCompoundSchema + CutSchemaReceiptRange,
    {
        let cut = self.cut;
        let cancelled = self.cancelled;
        let mut validate_local_claim = |raw: &[u8], schemas: &mut S, limits: ItemLimits| {
            let mut local = crate::record_rules::validate_source_claim_from_cut(
                cut.ok_or_else(|| bad("native compound requires corpus cut"))?,
                raw,
                schemas,
                limits,
                cancelled,
            )?;
            if !local.issues.is_empty() {
                return Err(bad("compound Claim local owner profile"));
            }
            Ok(LocalClaimValidation {
                dependency_digests: std::mem::take(&mut local.dependency_digests),
                dependency_bytes_read: 0,
            })
        };
        self.verify_with_local_claim_validator(path, claim, schemas, &mut validate_local_claim)
    }

    fn verify_with_local_claim_validator<S, F>(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut S,
        validate_local_claim: &mut F,
    ) -> Result<NativeCompoundObservation, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        let before = self.temporary_state;
        let result = if claim.get("schema_version").and_then(Value::as_str)
            == Some(OBJECT_LINK_CLAIM)
            && claim
                .get("predicate")
                .and_then(Value::as_str)
                .is_some_and(object_link_predicate)
        {
            self.verify_object_link(path, claim, schemas, validate_local_claim)
        } else {
            self.verify_inner(path, claim, schemas, validate_local_claim)
        };
        self.release_temporary_since(before);
        self.release_raw_cache();
        result
    }
    fn verify_object_link<S, F>(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut S,
        validate_local_claim: &mut F,
    ) -> Result<NativeCompoundObservation, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        check(self.limits.deadline, self.cancelled)?;
        if !path.ends_with("/source-claims.jsonl")
            || !object_link_predicate(text(claim, "predicate")?)
            || text(claim, "schema_version")? != OBJECT_LINK_CLAIM
        {
            return Err(bad("object-Link exact Claim carrier"));
        }
        metadata_path(path, false)?;
        let home = parent(path)?;
        let receipt_raw = self.required(&format!("{home}/{OBJECT_LINK_RECEIPT}"), MAX_FILE)?;
        let receipt = self.decoded(&receipt_raw)?;
        if receipt["schema_version"] != "tos_object_link_receipt_v1" {
            return Err(bad("object-Link native receipt"));
        }
        let id = text(&receipt, "transaction_id")?;
        let tx = self.transaction(id)?;
        let transport = match tx.status.as_str() {
            "committed" => NativeTransportState::Committed,
            "rolled-back" => NativeTransportState::RolledBack,
            "pending" => NativeTransportState::Pending,
            "orphan" => NativeTransportState::Orphan,
            _ => return Err(bad("object-Link transport state")),
        };
        let observation = NativeCompoundObservation {
            claim_path: path.into(),
            claim_id: text(claim, "claim_id")?.into(),
            transaction_id: id.into(),
            manifest_sha256: tx.manifest_sha256.clone(),
            transport,
            work_parent_transition_sha256: None,
        };
        if transport != NativeTransportState::Committed {
            return Ok(observation);
        }
        if self
            .publication
            .as_ref()
            .is_some_and(|state| state["phase"] != "ready")
        {
            return Err(bad("object-Link pending owner recovery"));
        }
        let original = self.reconstruct_object_link(&tx, schemas, validate_local_claim)?;
        let scope = &original.scope;
        if path != text(scope, "claim_source_path")?
            || receipt != original.receipt
            || receipt_raw != original.files[&format!("{home}/{OBJECT_LINK_RECEIPT}")]
        {
            return Err(bad("object-Link exact current receipt"));
        }
        for name in [
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
        ] {
            let path = format!("{home}/{name}");
            if self.required(&path, MAX_FILE)? != original.files[&path] {
                return Err(bad("object-Link immutable capture changed"));
            }
        }
        let initial = self.attachment_claim_initial(path, claim)?;
        if initial != original.files[path] {
            return Err(bad("object-Link exact original association Claim"));
        }
        let link_path = text(scope, "link_source_path")?;
        let current_link = self.object_link_current_lineage(
            scope,
            &original.request["link"],
            &original.files[link_path],
        )?;
        let link_forms = format!("{}/link.human-forms.json", parent(link_path)?);
        let claim_forms = format!(
            "{home}/source-claims.{}.human-forms.json",
            Digest256::of_bytes(text(scope, "claim_id")?.as_bytes()).to_hex()
        );
        self.object_link_initial_forms(&link_forms, &original.files[&link_forms])?;
        self.object_link_initial_forms(&claim_forms, &original.files[&claim_forms])?;
        self.object_link_subject_binding(
            scope,
            &original.request["subject"],
            text(&receipt, "subject_source_sha256")?,
        )?;
        for (value, contract) in [
            (&current_link, "ToS/contracts/source-link.schema.json"),
            (claim, "ToS/contracts/object-link-claim-v2.schema.json"),
        ] {
            let raw = self.canonical_buffer(value)?;
            if !schemas.check_reusing_scalar(
                "object-link-current",
                &raw,
                contract,
                self.limits.deadline,
                self.cancelled,
            )? {
                return Err(bad("object-Link current grammar"));
            }
        }
        // The Python owner reruns SourceClaimProfiles on the current Claim.
        // Schema validity alone does not cover relation registry, shared
        // values, display, visibility and layer predicates after correction.
        let mut current_claim_raw = self.canonical_buffer(claim)?;
        current_claim_raw.push(b'\n');
        let current_claim_raw = self.buffer(current_claim_raw)?;
        let mut current_limits = self.limits;
        current_limits.max_state_bytes = current_limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        current_limits.max_total_bytes = current_limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let mut current_local = validate_local_claim(&current_claim_raw, schemas, current_limits)?;
        account(
            &mut self.bytes,
            usize::try_from(current_local.dependency_bytes_read)
                .map_err(|_| ItemRefusal::Budget)?,
            self.limits.max_total_bytes,
        )?;
        let dependency_digests = std::mem::take(&mut current_local.dependency_digests);
        drop(current_local);
        self.release_temporary(std::mem::size_of::<Vec<u8>>() + current_claim_raw.len());
        drop(current_claim_raw);
        let dependencies_state = dependency_digests
            .keys()
            .try_fold(0usize, |n, path| {
                n.checked_add(std::mem::size_of::<(String, Digest256)>() + path.len())
            })
            .ok_or(ItemRefusal::Budget)?;
        self.temporary(dependencies_state)?;
        for (path, sha) in dependency_digests {
            RelativePath::parse(&path)
                .map_err(|_| bad("object-Link current Claim dependency path"))?;
            let (current_digest, current_size) = self.current_digest_size(&path)?;
            if current_digest != sha {
                return Err(bad("object-Link current Claim dependency changed"));
            }
            if self.cut.is_some() {
                account(
                    &mut self.bytes,
                    usize::try_from(current_size).map_err(|_| ItemRefusal::Budget)?,
                    self.limits.max_total_bytes,
                )?;
            }
            self.release_temporary(std::mem::size_of::<(String, Digest256)>() + path.len());
            if self.input.is_none() {
                self.record_read(PredicateRead::ExactPath {
                    path,
                    digest: sha.to_prefixed(),
                })?;
            }
        }
        if current_link["association_claim_refs"] != json!([scope["claim_id"]])
            || current_link["provenance_event_ref"] != scope["provenance_event_id"]
            || current_link["uri"] != scope["uri"]
            || current_link["observation_ref"] != scope["observation_ref"]
            || claim["subject_ref"] != scope["subject_id"]
            || claim["object"] != scope["link_id"]
            || claim["predicate"] != scope["predicate"]
            || claim["provenance_event_ref"] != scope["provenance_event_id"]
            || claim["qualifiers"]["availability_is_rights_conclusion"] != false
            || [
                "statement",
                "statement_language",
                "statement_script",
                "link_role",
            ]
            .iter()
            .any(|key| text(&claim["qualifiers"], key).map_or(true, |s| s.trim().is_empty()))
        {
            return Err(bad("object-Link current qualified association closure"));
        }
        check(self.limits.deadline, self.cancelled)?;
        Ok(observation)
    }
    fn verify_inner<S, F>(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut S,
        validate_local_claim: &mut F,
    ) -> Result<NativeCompoundObservation, ItemRefusal>
    where
        S: NativeCompoundSchema,
        F: FnMut(&[u8], &mut S, ItemLimits) -> Result<LocalClaimValidation, ItemRefusal>,
    {
        check(self.limits.deadline, self.cancelled)?;
        let kind = CompoundKind::from_predicate(text(claim, "predicate")?)?;
        metadata_path(path, false)?;
        if !path.ends_with("/source-claims.jsonl") {
            return Err(bad("native compound Claim carrier"));
        }
        let home = parent(path)?;
        let receipt_raw = self.required(&format!("{home}/{}", kind.receipt_file()), MAX_FILE)?;
        let receipt = self.decoded(&receipt_raw)?;
        if text(&receipt, "schema_version")? != kind.receipt_schema() {
            return Err(bad("native Claim compound receipt"));
        }
        let id = text(&receipt, "transaction_id")?;
        let tx = self.transaction(id)?;
        let transport = match tx.status.as_str() {
            "committed" => NativeTransportState::Committed,
            "rolled-back" => NativeTransportState::RolledBack,
            "pending" => NativeTransportState::Pending,
            "orphan" => NativeTransportState::Orphan,
            _ => return Err(bad("transaction outcome")),
        };
        let mut observation = NativeCompoundObservation {
            claim_path: path.into(),
            claim_id: text(claim, "claim_id")?.into(),
            transaction_id: id.into(),
            manifest_sha256: tx.manifest_sha256.clone(),
            transport,
            work_parent_transition_sha256: None,
        };
        if transport != NativeTransportState::Committed {
            return Ok(observation);
        }
        if self
            .publication
            .as_ref()
            .is_some_and(|p| p["phase"] != "ready")
        {
            return Err(bad("current source snapshot is pending owner recovery"));
        }
        let reconstructed = self.reconstruct(&tx, kind, schemas, validate_local_claim)?;
        let scope = &reconstructed.scope;
        let work = text(scope, kind.parent_path())?;
        let expression = text(scope, kind.child_path())?;
        if path != format!("{}/source-claims.jsonl", kind.publication_home(scope)?)
            || !kind.relation_attachment() && claim != &reconstructed.request["claim"]
            || receipt != reconstructed.receipt
            || receipt_raw != reconstructed.child[kind.receipt_file()]
            || !kind.relation_attachment()
                && self.required(path, MAX_FILE)? != reconstructed.child["source-claims.jsonl"]
        {
            return Err(bad("exact current compound Claim/receipt bytes"));
        }
        for name in [
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
        ] {
            if self.required(&format!("{home}/{name}"), MAX_FILE)? != reconstructed.child[name] {
                return Err(bad("immutable compound capture changed"));
            }
        }
        if kind == CompoundKind::EditionItem {
            for name in [
                "item-deposit-receipt.json",
                "item.manifest.json",
                "rights.json",
                "provenance.jsonl",
                "resource-inventory.json",
                "fixity.sha256",
                "forensic-report.md",
            ] {
                if self.required(&format!("{home}/{name}"), MAX_FILE)? != reconstructed.child[name]
                {
                    return Err(bad("immutable Item companion bytes changed"));
                }
            }
        }
        let parent_files = self.selected(work)?;
        let parent_record = self.decoded(&parent_files[kind.parent_file()])?;
        if parent_record["record_id"] != scope[kind.parent_key()]
            || parent_record["record_type"] != kind.parent_kind()
            || kind == CompoundKind::ExpressionEdition
                && parent_record["work_ref"] != scope["work_id"]
        {
            return Err(bad("current parent typed identity"));
        }
        let parent_history = self.history(work, &parent_files)?;
        if !array(&parent_history, "receipts")?.contains(&reconstructed.parent_receipt) {
            return Err(bad("compound transition missing in current parent lineage"));
        }
        observation.work_parent_transition_sha256 =
            Some(text(&reconstructed.receipt, "parent_transition_sha256")?.to_owned());
        if kind.relation_attachment() {
            let initial = self.attachment_claim_initial(path, claim)?;
            if initial != reconstructed.child["source-claims.jsonl"] {
                return Err(bad("membership exact committed initial stream"));
            }
            for key in [
                "statement",
                "statement_language",
                "statement_script",
                if kind == CompoundKind::CollectionWork {
                    "membership_scope"
                } else {
                    "attribution_scope"
                },
            ] {
                let value = text(&claim["qualifiers"], key)?;
                if tos_foundation::python_strip_unicode16_v1(value, MAX_SIDE)
                    .map_err(|_| ItemRefusal::Budget)?
                    .is_empty()
                {
                    return Err(bad("membership current qualified wording"));
                }
            }
            check(self.limits.deadline, self.cancelled)?;
            return Ok(observation);
        }
        let child_files = self.selected(expression)?;
        let child_record = self.decoded(&child_files[kind.child_file()])?;
        if child_record["record_id"] != scope[kind.child_key()]
            || child_record["record_type"] != kind.child_kind()
            || !kind.initial_backlink(&child_record, &scope[kind.parent_key()])
            || kind == CompoundKind::EditionItem
                && child_record["item_manifest_ref"]
                    != format!("{}/item.manifest.json", parent(expression)?)
        {
            return Err(bad("current compound child typed parent binding"));
        }
        let child_history = self.history(expression, &child_files)?;
        let mut initial = child_files[kind.child_file()] == reconstructed.child[kind.child_file()];
        for receipt in array(&child_history, "receipts")? {
            check(self.limits.deadline, self.cancelled)?;
            let before_archive = self.temporary_state;
            let archive = self.archive(expression, text(scope, kind.child_key())?, receipt)?;
            if receipt["previous_source"] == reconstructed.receipt[kind.child_kind()] {
                if archive[kind.child_file()] != reconstructed.child[kind.child_file()] {
                    return Err(bad("compound child initial archive bytes changed"));
                }
                initial = true;
            }
            drop(archive);
            self.release_temporary_since(before_archive);
        }
        if !initial {
            return Err(bad(
                "current compound child lacks committed initial lineage",
            ));
        }
        check(self.limits.deadline, self.cancelled)?;
        Ok(observation)
    }
}
