//! Native preflight for the reviewed source-acquisition batch contract.
//!
//! This route validates selected metadata and exact Item bindings. It does not
//! admit source, decide rights, or accept semantic interpretation.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tos_foundation::{Digest256, RelativePath};
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};

const MANIFEST_SCHEMA: &str = "ToS/contracts/acquisition-batch.schema.json";
const DELTA_SCHEMA: &str = "ToS/contracts/acquisition-provenance-delta.schema.json";
const CORPUS_SCHEMA: &str = "ToS/contracts/corpus-record.schema.json";
const ITEM_MANIFEST_SCHEMA: &str = "ToS/contracts/source-item-manifest.schema.json";
const RESOURCE_INVENTORY_SCHEMA: &str = "ToS/contracts/source-resource-inventory.schema.json";
const RIGHTS_SCHEMA: &str = "ToS/contracts/rights-record.schema.json";
const EVENT_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const DISCOVERY_SCHEMA: &str = "ToS/contracts/material-discovery-record.schema.json";
const DISCOVERY_ROOT: &str = "ToS/source-witnesses/discovery/runs";
const MAX_PAYLOAD_BYTES: u64 = 300 * 1024 * 1024;
const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;

type Result<T> = std::result::Result<T, String>;

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| format!("missing field: {key}"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("{key} must be text"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    field(value, key)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{key} must be an array"))
}

fn sha256_file(path: &Path, cap: u64) -> Result<String> {
    Ok(Digest256::of_bytes(&read_file(path, cap)?).to_hex())
}

fn read_file(path: &Path, cap: u64) -> Result<Vec<u8>> {
    crate::source_acquisition_batch::read_bytes(path, None, false, false, cap)
}

fn strict_value(raw: &[u8], label: &str) -> Result<Value> {
    let parsed =
        crate::source_command::parse(raw).map_err(|_| format!("{label} is not strict JSON"))?;
    let canonical = crate::source_command::canonical(&parsed)
        .map_err(|_| format!("{label} is not representable as JSON"))?;
    serde_json::from_slice(&canonical).map_err(|_| format!("{label} is not JSON"))
}

fn schema_path(repo: &Path, reference: &str) -> Result<PathBuf> {
    let relative = RelativePath::parse(reference)
        .map_err(|_| format!("unsafe schema reference: {reference}"))?;
    if !reference.starts_with("ToS/contracts/") || !reference.ends_with(".schema.json") {
        return Err(format!(
            "schema reference is outside the local contract home: {reference}"
        ));
    }
    let root = repo
        .canonicalize()
        .map_err(|error| format!("schema repository root unavailable: {error}"))?;
    let path = root.join(relative.as_str());
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("schema is unavailable: {reference}: {error}"))?;
    if !canonical.starts_with(&root) || canonical != path {
        return Err(format!(
            "schema path is linked or outside repository: {reference}"
        ));
    }
    Ok(path)
}

fn schema_uri(reference: &str, schema: &Value) -> Result<String> {
    let uri = text(schema, "$id")?;
    let local = format!("https://tree-of-sophia.local/{reference}");
    let alias = format!("https://treeofsophia.local/{reference}");
    if uri != local && uri != alias {
        return Err(format!(
            "schema identity differs from owner path: {reference}"
        ));
    }
    Ok(uri.to_owned())
}

fn reference_path(reference: &str, current: &str) -> Result<Option<String>> {
    let base = reference.split('#').next().unwrap_or("");
    if base.is_empty() {
        return Ok(None);
    }
    let relative = if let Some(path) = base.strip_prefix("https://tree-of-sophia.local/") {
        path.to_owned()
    } else if let Some(path) = base.strip_prefix("https://treeofsophia.local/") {
        path.to_owned()
    } else if base.contains("://") || base.starts_with('/') {
        return Err(format!(
            "schema dependency is outside local contracts: {reference}"
        ));
    } else {
        let parent = current
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        if parent.is_empty() {
            base.to_owned()
        } else {
            format!("{parent}/{base}")
        }
    };
    RelativePath::parse(&relative)
        .map_err(|_| format!("unsafe local schema dependency: {reference}"))?;
    if !relative.starts_with("ToS/contracts/") || !relative.ends_with(".schema.json") {
        return Err(format!(
            "schema dependency is outside contract home: {reference}"
        ));
    }
    Ok(Some(relative))
}

fn collect_schema_references(value: &Value, output: &mut Vec<String>) {
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(object) => {
                for (key, child) in object {
                    if matches!(key.as_str(), "$ref" | "$dynamicRef" | "$recursiveRef") {
                        if let Some(reference) = child.as_str() {
                            output.push(reference.to_owned());
                        }
                    }
                    stack.push(child);
                }
            }
            Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
}

fn schema_probe(repo: &Path, root_ref: &str) -> Result<(SchemaBackendProbe, String)> {
    let mut pending = vec![root_ref.to_owned()];
    let mut seen = BTreeSet::new();
    let mut resources = Vec::new();
    let mut total = 0usize;
    while let Some(reference) = pending.pop() {
        if !seen.insert(reference.clone()) {
            continue;
        }
        if seen.len() > SchemaBackendProbe::MAX_RESOURCES {
            return Err("schema resource count exceeds native bound".into());
        }
        let path = schema_path(repo, &reference)?;
        let raw = read_file(&path, SchemaBackendProbe::MAX_RESOURCE_BYTES as u64)?;
        total = total
            .checked_add(raw.len())
            .filter(|used| *used <= SchemaBackendProbe::MAX_TOTAL_BYTES)
            .ok_or("schema bytes exceed native bound")?;
        let schema = strict_value(&raw, "local schema")?;
        let uri = schema_uri(&reference, &schema)?;
        let mut refs = Vec::new();
        collect_schema_references(&schema, &mut refs);
        for reference_in_schema in refs {
            if let Some(dependency) = reference_path(&reference_in_schema, &reference)? {
                pending.push(dependency);
            }
        }
        resources.push(SchemaResource { uri, raw });
    }
    let root = resources
        .iter()
        .find(|resource| {
            // The order is not significant; the root path is the input route.
            resource.uri == format!("https://tree-of-sophia.local/{root_ref}")
                || resource.uri == format!("https://treeofsophia.local/{root_ref}")
        })
        .map(|resource| resource.uri.clone())
        .ok_or_else(|| format!("root schema was not loaded: {root_ref}"))?;
    let probe = SchemaBackendProbe::new(resources, FormatProfile::AssertedSourceCandidateV1)
        .map_err(|error| format!("local schema set is invalid: {error:?}"))?;
    probe
        .compile_all()
        .map_err(|error| format!("local schema compilation failed: {error:?}"))?;
    Ok((probe, root))
}

/// Validate an already strictly decoded value against one exact local ToS
/// contract and its declared local `$ref` closure.
pub fn validate_schema(repo: &Path, schema_ref: &str, value: &Value) -> Result<()> {
    let (probe, root) = schema_probe(repo, schema_ref)?;
    let valid = probe
        .is_valid_value(&root, value)
        .map_err(|error| format!("schema execution failed for {schema_ref}: {error:?}"))?;
    if valid {
        Ok(())
    } else {
        Err(format!("value does not satisfy {schema_ref}"))
    }
}

fn safe_ref(value: &str, label: &str, prefix: Option<&str>) -> Result<()> {
    RelativePath::parse(value).map_err(|_| format!("unsafe {label}: {value}"))?;
    if prefix.is_some_and(|prefix| !value.starts_with(prefix)) {
        return Err(format!("{label} leaves {}: {value}", prefix.unwrap()));
    }
    Ok(())
}

fn source_member(reference: &str) -> Result<bool> {
    RelativePath::parse(reference).map_err(|_| {
        format!("record reference is not a valid corpus source member: {reference}")
    })?;
    Ok(tos_source_store::is_authored_source_path_v1(reference))
}

fn validate_public_url(value: &str, label: &str) -> Result<()> {
    let scheme_end = value
        .find("://")
        .ok_or_else(|| format!("{label} is malformed"))?;
    let scheme = &value[..scheme_end];
    let authority = value[scheme_end + 3..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if authority.is_empty() {
        return Err(format!("{label} is malformed"));
    }
    let query = value
        .split_once('?')
        .map(|(_, rest)| rest.split('#').next().unwrap_or(""));
    let fragment = value.split_once('#').map(|(_, rest)| rest);
    if authority.contains('@')
        || query.is_some_and(|part| !part.is_empty())
        || fragment.is_some_and(|part| !part.is_empty())
    {
        return Err(format!(
            "{label} must not contain userinfo, a query, or a fragment"
        ));
    }
    if !matches!(scheme, "http" | "https") || value.chars().any(char::is_whitespace) {
        return Err(format!("{label} is malformed"));
    }
    Ok(())
}

fn tos_item(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.item.") else {
        return false;
    };
    let mut segment_nonempty = false;
    for byte in rest.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => segment_nonempty = true,
            b'.' | b'-' if segment_nonempty => segment_nonempty = false,
            _ => return false,
        }
    }
    segment_nonempty
}

fn set_texts(value: &Value, key: &str) -> Result<BTreeSet<String>> {
    array(value, key)?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{key} entries must be text"))
        })
        .collect()
}

/// Validate the acquisition manifest schema and all selection/cross-record
/// invariants before any selected record is copied or provider is contacted.
pub fn validate_manifest(repo: &Path, manifest: &Value) -> Result<()> {
    validate_schema(repo, MANIFEST_SCHEMA, manifest)
        .map_err(|error| format!("acquisition batch schema validation failed: {error}"))?;
    let base_revision = text(manifest, "base_revision")?;
    let delta = field(manifest, "provenance_delta")?;
    if text(delta, "base_revision")? != base_revision {
        return Err("provenance delta base revision differs from batch".into());
    }
    let mut seen_items = BTreeSet::new();
    let mut seen_destinations = BTreeSet::new();
    let mut record_refs = BTreeSet::new();
    let mut record_classifications = BTreeMap::<String, (String, String)>::new();
    let mut payload_refs = BTreeSet::new();
    let mut payload_descriptors = BTreeMap::<String, (String, u64, String)>::new();
    for selection in array(manifest, "selection")? {
        let item_ref = text(selection, "item_ref")?;
        if !tos_item(item_ref) || !seen_items.insert(item_ref.to_owned()) {
            return Err(format!("duplicate or invalid Item selection: {item_ref}"));
        }
        let item_root = text(selection, "item_root_ref")?;
        safe_ref(item_root, "Item root", Some("ToS/source-witnesses/"))?;
        let provider = field(selection, "provider")?;
        validate_public_url(text(provider, "source_url")?, "provider source_url")?;
        let mut record_by_ref = BTreeMap::<String, &Value>::new();
        for record in array(selection, "records")? {
            let reference = text(record, "ref")?;
            safe_ref(reference, "record reference", Some("ToS/"))?;
            if !source_member(reference)? {
                return Err(format!(
                    "record is outside the corpus source admission boundary: {reference}"
                ));
            }
            if format!("/{reference}/").contains("/payload/") {
                return Err(format!(
                    "payload cannot be selected as metadata: {reference}"
                ));
            }
            if record_by_ref.insert(reference.to_owned(), record).is_some() {
                return Err(format!("duplicate record reference: {reference}"));
            }
            let classification = (
                text(record, "kind")?.to_owned(),
                text(record, "sha256")?.to_owned(),
            );
            if record_classifications
                .get(reference)
                .is_some_and(|prior| prior != &classification)
            {
                return Err(format!(
                    "record reference has conflicting kind or digest: {reference}"
                ));
            }
            record_classifications.insert(reference.to_owned(), classification);
            record_refs.insert(reference.to_owned());
        }
        let rights = field(selection, "rights")?;
        let rights_ref = text(rights, "ref")?;
        let rights_record = record_by_ref.get(rights_ref).copied();
        if rights_record.is_none_or(|record| record["kind"] != "rights") {
            return Err(format!("rights record is not selected for {item_ref}"));
        }
        if text(rights_record.unwrap(), "sha256")? != text(rights, "sha256")? {
            return Err(format!("rights record digest differs for {item_ref}"));
        }
        let mut item_records = Vec::new();
        let mut manifests = BTreeSet::new();
        let mut provenance = false;
        for record in array(selection, "records")? {
            match text(record, "kind")? {
                "item" => item_records.push(text(record, "ref")?),
                "manifest" => {
                    manifests.insert(text(record, "ref")?);
                }
                "provenance" => provenance = true,
                _ => {}
            }
        }
        if item_records.len() != 1
            || item_records[0] != format!("{item_root}/item.json")
            || !manifests.contains(&format!("{item_root}/item.manifest.json"))
            || !provenance
        {
            return Err(format!(
                "selection must contain one Item record: {item_ref}"
            ));
        }
        let provider_revision = text(provider, "revision")?;
        let provider_source_id = text(provider, "source_id")?;
        let mut item_file_refs = BTreeSet::new();
        for payload in array(selection, "payload_files")? {
            validate_public_url(text(payload, "provider_url")?, "payload provider_url")?;
            if text(payload, "item_ref")? != item_ref
                || text(payload, "item_root_ref")? != item_root
            {
                return Err(format!("payload Item binding differs: {item_ref}"));
            }
            let file_ref = text(payload, "file_ref")?;
            let sha = text(payload, "sha256")?;
            if file_ref != format!("tos.file.sha256.{sha}") {
                return Err(format!("payload File ID is not SHA-bound: {file_ref}"));
            }
            if text(payload, "provider_revision")? != provider_revision {
                return Err(format!("provider revision differs for {file_ref}"));
            }
            if text(payload, "provider_source_id")? != provider_source_id {
                return Err(format!("provider source ID differs for {file_ref}"));
            }
            let byte_size = field(payload, "byte_size")?
                .as_u64()
                .ok_or("payload byte_size must be a positive integer")?;
            if byte_size > MAX_PAYLOAD_BYTES {
                return Err(format!(
                    "payload exceeds bounded transfer limit: {file_ref}"
                ));
            }
            if !item_file_refs.insert(file_ref.to_owned()) {
                return Err(format!("duplicate payload File ID within Item: {file_ref}"));
            }
            let descriptor = (
                sha.to_owned(),
                byte_size,
                text(payload, "media_type")?.to_owned(),
            );
            if payload_descriptors
                .get(file_ref)
                .is_some_and(|prior| prior != &descriptor)
            {
                return Err(format!(
                    "payload File descriptor differs across Items: {file_ref}"
                ));
            }
            payload_descriptors.insert(file_ref.to_owned(), descriptor);
            let destination = format!("{item_root}/{}", text(payload, "relative_path")?);
            if !seen_destinations.insert(destination.clone()) {
                return Err(format!("duplicate payload destination: {destination}"));
            }
            payload_refs.insert(file_ref.to_owned());
        }
    }
    if set_texts(delta, "record_refs")? != record_refs {
        return Err("provenance delta does not close over selected records".into());
    }
    if set_texts(delta, "payload_file_refs")? != payload_refs {
        return Err("provenance delta does not close over selected payloads".into());
    }
    Ok(())
}

fn source_path(source_root: &Path, reference: &str) -> Result<PathBuf> {
    safe_ref(reference, "prepared source reference", Some("ToS/"))?;
    let root = source_root
        .canonicalize()
        .map_err(|error| format!("prepared source root unavailable: {error}"))?;
    let path = root.join(reference);
    let canonical = path.canonicalize().map_err(|error| {
        format!("prepared selected record is unavailable: {reference}: {error}")
    })?;
    if canonical != path || !canonical.starts_with(&root) {
        return Err(format!(
            "prepared selected record path is linked: {reference}"
        ));
    }
    Ok(path)
}

fn record_value(source_root: &Path, reference: &str, digest: &str) -> Result<Value> {
    let path = source_path(source_root, reference)?;
    if sha256_file(&path, MAX_RECORD_BYTES)? != digest {
        return Err(format!(
            "prepared selected record bytes differ: {reference}"
        ));
    }
    strict_value(&read_file(&path, MAX_RECORD_BYTES)?, reference)
}

fn check_record_schema(
    repo: &Path,
    _source_root: &Path,
    record: &Value,
    schema: &str,
    label: &str,
    item_ref: &str,
) -> Result<()> {
    validate_schema(repo, schema, record)
        .map_err(|_| format!("prepared Item {label} does not satisfy its schema: {item_ref}"))
}

fn discovery_path(reference: &str, kind: &str) -> Result<()> {
    let path = RelativePath::parse(reference).map_err(|_| {
        format!("selected discovery run has an unsupported path or kind: {reference}")
    })?;
    if path.as_str().rsplit_once('/').map(|(parent, _)| parent) != Some(DISCOVERY_ROOT)
        || !reference.ends_with(".json")
        || kind != "discovery"
    {
        return Err(format!(
            "selected discovery run has an unsupported path or kind: {reference}"
        ));
    }
    Ok(())
}

/// Recheck the exact Item/manifest/rights/provenance/File closure copied into
/// the preparation root before acquisition or handoff.
pub fn verify_item_bindings(repo: &Path, source: &Path, manifest: &Value) -> Result<()> {
    let mut batch_claim_ids = BTreeMap::<String, String>::new();
    let mut validated_claim_refs = BTreeSet::new();
    let mut batch_identity_refs = BTreeMap::<String, String>::new();
    let mut batch_event_ids = BTreeSet::new();
    for selection in array(manifest, "selection")? {
        let item_ref = text(selection, "item_ref")?;
        let item_root = text(selection, "item_root_ref")?;
        let records = array(selection, "records")?;
        let record_map = records
            .iter()
            .map(|record| Ok((text(record, "ref")?.to_owned(), record)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut validated = BTreeSet::new();
        for record in records {
            let reference = text(record, "ref")?;
            let kind = text(record, "kind")?;
            if kind == "claim" {
                if Path::new(reference)
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some("source-claims.jsonl")
                {
                    return Err(format!(
                        "selected Claim carrier must use source-claims.jsonl: {reference}"
                    ));
                }
                if validated_claim_refs.insert(reference.to_owned()) {
                    let path = source_path(source, reference)?;
                    let raw = read_file(&path, MAX_RECORD_BYTES)?;
                    if Digest256::of_bytes(&raw).to_hex() != text(record, "sha256")? {
                        return Err(format!(
                            "prepared selected source Claim bytes differ: {reference}"
                        ));
                    }
                    let mut read_claim_dependency =
                        |dependency_ref: &str, cap: usize| -> Result<Vec<u8>> {
                            let dependency_path =
                                RelativePath::parse(dependency_ref).map_err(|_| {
                                    format!("unsafe local Claim dependency: {dependency_ref}")
                                })?;
                            let path = repo.join(dependency_path.as_str());
                            crate::source_acquisition_batch::read_bytes_under(
                                repo, &path, None, None, false, cap as u64,
                            )
                            .map(|(bytes, _)| bytes)
                        };
                    let identities =
                        tos_validation::record_rules::validate_acquisition_source_claims(
                            reference,
                            &raw,
                            &mut read_claim_dependency,
                        )
                        .map_err(|error| {
                            format!("selected source Claim carrier violates its profile: {reference}: {error}")
                        })?;
                    for (line, claim_id) in identities {
                        if batch_claim_ids
                            .insert(claim_id.clone(), format!("{reference}:{line}"))
                            .is_some()
                        {
                            return Err(format!(
                                "selected source Claim identity is duplicated: {claim_id}"
                            ));
                        }
                    }
                }
                validated.insert(reference.to_owned());
                continue;
            }
            if reference.starts_with(&format!("{DISCOVERY_ROOT}/")) {
                discovery_path(reference, kind)?;
                let value = record_value(source, reference, text(record, "sha256")?)?;
                check_record_schema(
                    repo,
                    source,
                    &value,
                    DISCOVERY_SCHEMA,
                    "selected discovery record",
                    item_ref,
                )?;
                let discovery_issues =
                    tos_validation::source_foundation_discovery::material_discovery_semantic_issues(
                        &value,
                    );
                if let Some(issue) = discovery_issues.first() {
                    return Err(format!(
                        "selected discovery record violates foundation semantics: {reference}: {issue}"
                    ));
                }
                validated.insert(reference.to_owned());
                continue;
            }
            if matches!(kind, "work" | "expression" | "edition") {
                let value = record_value(source, reference, text(record, "sha256")?)?;
                check_record_schema(
                    repo,
                    source,
                    &value,
                    CORPUS_SCHEMA,
                    &format!("selected {kind} record"),
                    item_ref,
                )?;
                if value.get("record_type").and_then(Value::as_str) != Some(kind) {
                    return Err(format!(
                        "selected {kind} record has record_type {}: {reference}",
                        value.get("record_type").unwrap_or(&Value::Null)
                    ));
                }
                let identity = text(&value, "record_id")?;
                if batch_identity_refs
                    .get(identity)
                    .is_some_and(|prior| prior != reference)
                {
                    return Err(format!(
                        "selected corpus identity is bound to multiple records: {identity}"
                    ));
                }
                batch_identity_refs.insert(identity.to_owned(), reference.to_owned());
                validated.insert(reference.to_owned());
                continue;
            }
            if !matches!(
                kind,
                "item" | "rights" | "provenance" | "manifest" | "discovery"
            ) {
                return Err(format!(
                    "selected record kind has no validation route: {kind}: {reference}"
                ));
            }
        }
        let manifest_ref = format!("{item_root}/item.manifest.json");
        let manifest_record = record_map.get(&manifest_ref).copied();
        if manifest_record.is_none_or(|record| record["kind"] != "manifest") {
            return Err(format!(
                "selection has no selected Item manifest record: {item_ref}"
            ));
        }
        let manifest_record = manifest_record.unwrap();
        let item_manifest = record_value(source, &manifest_ref, text(manifest_record, "sha256")?)?;
        let item_ref_path = format!("{item_root}/item.json");
        let item_record = record_value(
            source,
            &item_ref_path,
            text(
                record_map
                    .get(&item_ref_path)
                    .copied()
                    .ok_or("selected Item record absent")?,
                "sha256",
            )?,
        )?;
        if text(&item_record, "record_id")? != item_ref
            || text(&item_record, "record_type")? != "item"
            || text(&item_record, "item_manifest_ref")? != manifest_ref
        {
            return Err(format!(
                "prepared Item record identity, type, or manifest binding differs: {item_ref}"
            ));
        }
        check_record_schema(
            repo,
            source,
            &item_record,
            CORPUS_SCHEMA,
            "record",
            item_ref,
        )?;
        if batch_identity_refs
            .get(item_ref)
            .is_some_and(|prior| prior != &item_ref_path)
        {
            return Err(format!(
                "selected corpus identity is bound to multiple records: {item_ref}"
            ));
        }
        batch_identity_refs.insert(item_ref.to_owned(), item_ref_path);
        validated.insert(format!("{item_root}/item.json"));

        check_record_schema(
            repo,
            source,
            &item_manifest,
            ITEM_MANIFEST_SCHEMA,
            "manifest",
            item_ref,
        )?;
        if text(&item_manifest, "schema_version")? != "tos_source_item_manifest_v1"
            || text(&item_manifest, "item_id")? != item_ref
            || text(&item_manifest, "rights_ref")? != text(field(selection, "rights")?, "ref")?
        {
            return Err(format!(
                "Item manifest identity or rights/provenance binding differs: {item_ref}"
            ));
        }
        let provenance_ref = text(&item_manifest, "provenance_ref")?;
        let provenance_record = record_map.get(provenance_ref).copied();
        if provenance_record.is_none_or(|record| record["kind"] != "provenance") {
            return Err(format!(
                "Item manifest identity or rights/provenance binding differs: {item_ref}"
            ));
        }
        validated.insert(manifest_ref.clone());

        for (field_name, expected) in [
            (
                "forensic_report_ref",
                format!("{item_root}/forensic-report.md"),
            ),
            (
                "resource_inventory_ref",
                format!("{item_root}/resource-inventory.json"),
            ),
        ] {
            let companion_ref = text(&item_manifest, field_name)?;
            let companion = record_map.get(companion_ref).copied();
            if companion.is_none_or(|record| record["kind"] != "discovery")
                || companion_ref != expected
            {
                return Err(format!(
                    "Item manifest {field_name} is outside the Item companion route: {item_ref}"
                ));
            }
            let path = source_path(source, companion_ref)?;
            let digest = sha256_file(&path, MAX_RECORD_BYTES)?;
            if digest != text(companion.unwrap(), "sha256")? {
                return Err(format!(
                    "prepared Item {field_name} bytes differ: {item_ref}"
                ));
            }
            if field_name == "forensic_report_ref" {
                let body = read_file(&path, MAX_RECORD_BYTES)?;
                let report = std::str::from_utf8(&body)
                    .map_err(|_| format!("Item forensic report is not UTF-8 text: {item_ref}"))?;
                if report.contains('\0') {
                    return Err(format!(
                        "Item forensic report contains a NUL byte: {item_ref}"
                    ));
                }
            }
            validated.insert(companion_ref.to_owned());
        }

        let fixity_ref = format!("{item_root}/fixity.sha256");
        let fixity_record = record_map.get(&fixity_ref).copied();
        if fixity_record.is_none_or(|record| record["kind"] != "discovery") {
            return Err(format!(
                "Item fixity companion is not selected as discovery metadata: {item_ref}"
            ));
        }
        let fixity_path = source_path(source, &fixity_ref)?;
        if sha256_file(&fixity_path, MAX_RECORD_BYTES)? != text(fixity_record.unwrap(), "sha256")? {
            return Err(format!("prepared Item fixity bytes differ: {item_ref}"));
        }
        let fixity = String::from_utf8(read_file(&fixity_path, MAX_RECORD_BYTES)?)
            .map_err(|_| format!("Item fixity companion is not readable: {item_ref}"))?;
        validated.insert(fixity_ref);

        let manifest_payloads = array(&item_manifest, "payload_files")?;
        let mut manifest_by_file = BTreeMap::new();
        for payload in manifest_payloads {
            let file_id = text(payload, "file_id")?;
            if manifest_by_file
                .insert(file_id.to_owned(), payload)
                .is_some()
            {
                return Err("Item manifest payload file IDs are not unique".into());
            }
        }
        let selected_payloads = array(selection, "payload_files")?
            .iter()
            .map(|payload| Ok((text(payload, "file_ref")?.to_owned(), payload)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        if manifest_by_file.keys().collect::<BTreeSet<_>>()
            != selected_payloads.keys().collect::<BTreeSet<_>>()
        {
            return Err(format!("Item manifest payload closure differs: {item_ref}"));
        }

        let inventory_ref = text(&item_manifest, "resource_inventory_ref")?;
        let inventory_record = record_map
            .get(inventory_ref)
            .copied()
            .ok_or("selected resource inventory absent")?;
        let inventory_path = source_path(source, inventory_ref)?;
        if sha256_file(&inventory_path, MAX_RECORD_BYTES)? != text(inventory_record, "sha256")? {
            return Err(format!(
                "prepared Item resource inventory bytes differ: {item_ref}"
            ));
        }
        let inventory = strict_value(
            &read_file(&inventory_path, MAX_RECORD_BYTES)?,
            "resource inventory",
        )?;
        check_record_schema(
            repo,
            source,
            &inventory,
            RESOURCE_INVENTORY_SCHEMA,
            "resource inventory",
            item_ref,
        )?;
        let inventory_files = array(&inventory, "files")?;
        for file in inventory_files {
            let resources = array(file, "resources")?;
            if field(file, "summary")?
                .get("resource_count")
                .and_then(Value::as_u64)
                != Some(resources.len() as u64)
            {
                return Err(format!(
                    "Item resource inventory resource_count differs from resources: {item_ref}"
                ));
            }
            let mut resource_ids = BTreeSet::new();
            for resource in resources {
                if !resource_ids.insert(text(resource, "resource_id")?) {
                    return Err(format!(
                        "Item resource inventory has duplicate resource_id: {}",
                        text(file, "file_id")?
                    ));
                }
            }
        }
        let expected_inventory = manifest_payloads
            .iter()
            .map(|payload| {
                (
                    text(payload, "file_id").unwrap_or("").to_owned(),
                    text(payload, "sha256").unwrap_or("").to_owned(),
                    text(payload, "media_type").unwrap_or("").to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let actual_inventory = inventory_files
            .iter()
            .map(|file| {
                (
                    text(file, "file_id").unwrap_or("").to_owned(),
                    text(file, "file_sha256").unwrap_or("").to_owned(),
                    text(file, "media_type").unwrap_or("").to_owned(),
                )
            })
            .collect::<Vec<_>>();
        if text(&inventory, "item_id")? != item_ref
            || text(&inventory, "generated_from_manifest_ref")? != manifest_ref
            || actual_inventory != expected_inventory
        {
            return Err(format!(
                "Item resource inventory does not close over manifest payloads: {item_ref}"
            ));
        }
        validated.insert(inventory_ref.to_owned());

        let expected_fixity = manifest_payloads
            .iter()
            .map(|payload| {
                Ok(format!(
                    "{}  {}\n",
                    text(payload, "sha256")?,
                    text(payload, "relative_path")?
                ))
            })
            .collect::<Result<String>>()?;
        if fixity != expected_fixity {
            return Err(format!(
                "Item fixity companion differs from manifest payloads: {item_ref}"
            ));
        }

        let rights = field(selection, "rights")?;
        let rights_ref = text(rights, "ref")?;
        let rights_record = record_map
            .get(rights_ref)
            .copied()
            .ok_or("selected rights record is missing")?;
        if rights_record["kind"] != "rights" {
            return Err(format!("selected rights record is missing: {item_ref}"));
        }
        let rights_path = source_path(source, rights_ref)?;
        let rights_digest = sha256_file(&rights_path, MAX_RECORD_BYTES)?;
        if rights_digest != text(rights, "sha256")?
            || rights_digest != text(rights_record, "sha256")?
        {
            return Err(format!("prepared rights bytes differ: {item_ref}"));
        }
        let rights_value =
            strict_value(&read_file(&rights_path, MAX_RECORD_BYTES)?, "rights record")?;
        check_record_schema(
            repo,
            source,
            &rights_value,
            RIGHTS_SCHEMA,
            "rights record",
            item_ref,
        )?;
        let mut layer_ids = BTreeSet::new();
        for layer in rights_value
            .get("layer_assessments")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !layer_ids.insert(text(layer, "layer_id")?) {
                return Err(format!(
                    "prepared Item rights record contains duplicate layer_id: {item_ref}"
                ));
            }
        }
        if text(field(selection, "rights")?, "posture").ok() == Some("public_payload")
            && (text(&rights_value, "visibility").ok() != Some("public_payload")
                || !matches!(
                    text(&rights_value, "redistribution_posture").ok(),
                    Some("authorized" | "authorized_with_conditions")
                ))
        {
            return Err(format!(
                "declared rights posture public_payload is not supported by selected rights record: {item_ref}"
            ));
        }
        if text(&rights_value, "visibility")? != text(&item_manifest, "visibility")? {
            return Err(format!(
                "rights visibility differs from Item manifest visibility: {item_ref}"
            ));
        }
        let scopes = set_texts(&rights_value, "scope_refs")?;
        if !scopes.contains(item_ref) || selected_payloads.keys().any(|id| !scopes.contains(id)) {
            return Err(format!(
                "Item rights scope does not cover selected Item and payloads: {item_ref}"
            ));
        }
        validated.insert(rights_ref.to_owned());

        let provenance_path = source_path(source, provenance_ref)?;
        let provenance_digest = text(provenance_record.unwrap(), "sha256")?;
        if sha256_file(&provenance_path, MAX_RECORD_BYTES)? != provenance_digest {
            return Err(format!("prepared Item provenance bytes differ: {item_ref}"));
        }
        let raw = read_file(&provenance_path, MAX_RECORD_BYTES)?;
        let mut events = Vec::new();
        for line in raw.split(|byte| *byte == b'\n') {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            events.push(strict_value(line, "prepared Item provenance")?);
        }
        if events.is_empty() {
            return Err("prepared Item provenance is empty".into());
        }
        for (index, event) in events.iter().enumerate() {
            check_record_schema(
                repo,
                source,
                event,
                EVENT_SCHEMA,
                &format!("provenance event {}", index + 1),
                item_ref,
            )?;
        }
        let mut event_ids = BTreeSet::new();
        for event in &events {
            let id = text(event, "event_id")?;
            if !event_ids.insert(id.to_owned()) {
                return Err(format!(
                    "Item provenance contains duplicate event_id: {item_ref}"
                ));
            }
            if !batch_event_ids.insert(id.to_owned()) {
                return Err(format!(
                    "batch provenance contains duplicate event_id: {id}"
                ));
            }
        }
        let inventory_event_ref = text(&inventory, "provenance_event_ref")?;
        let inventory_events = events
            .iter()
            .filter(|event| {
                event.get("event_id").and_then(Value::as_str) == Some(inventory_event_ref)
            })
            .collect::<Vec<_>>();
        if inventory_events.len() != 1 {
            return Err(format!(
                "Item resource inventory provenance event is missing or ambiguous: {item_ref}"
            ));
        }
        let inventory_digest = sha256_file(&inventory_path, MAX_RECORD_BYTES)?;
        let inventory_output_present =
            array(inventory_events[0], "outputs")?.iter().any(|output| {
                output.get("ref").and_then(Value::as_str) == Some(inventory_ref)
                    && output.get("role").and_then(Value::as_str)
                        == Some("tracked_text_free_resource_inventory")
                    && output.get("sha256").and_then(Value::as_str)
                        == Some(inventory_digest.as_str())
            });
        if !inventory_output_present {
            return Err(format!(
                "Item resource inventory provenance output is not digest-bound: {item_ref}"
            ));
        }
        let acquisition_ref = text(&item_manifest, "acquisition_event_ref")?;
        let selected_events = events
            .iter()
            .filter(|event| event.get("event_id").and_then(Value::as_str) == Some(acquisition_ref))
            .collect::<Vec<_>>();
        if selected_events.len() != 1
            || selected_events[0].get("event_type").and_then(Value::as_str) != Some("acquisition")
            || !matches!(
                selected_events[0].get("status").and_then(Value::as_str),
                Some("completed" | "completed_with_warnings")
            )
            || selected_events[0]
                .get("rights_basis_ref")
                .and_then(Value::as_str)
                != Some(rights_ref)
        {
            return Err(format!(
                "Item provenance does not name the selected acquisition event: {item_ref}"
            ));
        }
        let acquisition_outputs = array(selected_events[0], "outputs")?;
        for (file_ref, payload) in &selected_payloads {
            let manifest_payload = manifest_by_file
                .get(file_ref)
                .copied()
                .ok_or("Item manifest payload absent")?;
            for (key, expected) in [
                ("file_id", file_ref.as_str()),
                ("relative_path", text(payload, "relative_path")?),
                ("sha256", text(payload, "sha256")?),
                ("media_type", text(payload, "media_type")?),
            ] {
                if manifest_payload.get(key).and_then(Value::as_str) != Some(expected) {
                    return Err(format!("Item manifest payload binding differs: {file_ref}"));
                }
            }
            if manifest_payload.get("byte_size").and_then(Value::as_u64)
                != payload.get("byte_size").and_then(Value::as_u64)
            {
                return Err(format!("Item manifest payload binding differs: {file_ref}"));
            }
            let destination = format!("{item_root}/{}", text(payload, "relative_path")?);
            if !acquisition_outputs.iter().any(|output| {
                let reference = output.get("ref").and_then(Value::as_str);
                (reference == Some(file_ref) || reference == Some(destination.as_str()))
                    && output.get("sha256").and_then(Value::as_str)
                        == payload.get("sha256").and_then(Value::as_str)
            }) {
                return Err(format!(
                    "Item provenance does not bind acquired payload: {file_ref}"
                ));
            }
        }
        validated.insert(provenance_ref.to_owned());
        let unvalidated = record_map
            .keys()
            .filter(|reference| !validated.contains(*reference))
            .collect::<Vec<_>>();
        if let Some(reference) = unvalidated.first() {
            return Err(format!(
                "selected metadata record is outside the validated Item/discovery closure: {reference}"
            ));
        }
    }
    Ok(())
}
