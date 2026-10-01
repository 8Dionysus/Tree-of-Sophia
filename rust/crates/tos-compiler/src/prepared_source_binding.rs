//! Private retained source roots paired with the prepared publication.
//! All operations borrow the whole caller's transaction; errors require rollback.
//! Root metadata is addressing evidence, never source closure or admission.
use crate::prepared_catalog_index::CatalogMaintenanceLimits;
use crate::prepared_catalog_semantics::CatalogInputs;
use crate::prepared_maintenance::{self, MaintenanceReceipt};
use crate::prepared_semantic_index::SemanticMaintenanceLimits;
use crate::{Error, Result, local_prepared as prepared};
use rusqlite::{Transaction, params};
use serde_json::Value;
use std::path::Path;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

const MAX_STATE_BYTES: usize = 1_048_576;
const ROOT_BYTES: usize = 262_144;

fn invalid<T>() -> Result<T> {
    Err(Error::Invalid("private source selection"))
}
fn exact(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or(Error::Invalid("source object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return invalid();
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("source string"))
}
fn sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn name(value: &str, source: bool) -> bool {
    !value.is_empty()
        && (!source || value.len() <= 128)
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b == b'_'
                || (source && (b == b'.' || b == b'-'))
        })
}
fn canonical(value: &JsonValue, cap: usize) -> Result<Vec<u8>> {
    let limits =
        JsonLimits::new(cap, 128, 1_000_000, 4300).map_err(|e| Error::Source(e.to_string()))?;
    let mut raw = canonical_bytes_v1(value, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    if raw.len() >= cap {
        return Err(Error::Budget("private source canonical bytes"));
    }
    raw.push(b'\n');
    Ok(raw)
}
fn strict(raw: &[u8], cap: usize) -> Result<(JsonValue, Value)> {
    if raw.len() > cap {
        return Err(Error::Budget("private source bytes"));
    }
    let limits =
        JsonLimits::new(cap, 128, 1_000_000, 4300).map_err(|e| Error::Source(e.to_string()))?;
    let typed = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?
        .root()
        .clone();
    let value = serde_json::from_slice(raw).map_err(|_| Error::Invalid("source JSON"))?;
    Ok((typed, value))
}
fn collection_slot(header: &mut Value, parts: &[&str]) -> Result<()> {
    let object = header
        .as_object_mut()
        .ok_or(Error::Invalid("collection overlaps metadata"))?;
    if parts.len() == 1 {
        if object.contains_key(parts[0]) {
            return invalid();
        }
        object.insert(parts[0].to_owned(), Value::Null);
    } else {
        let next = object
            .entry(parts[0].to_owned())
            .or_insert_with(|| Value::Object(Default::default()));
        collection_slot(next, &parts[1..])?;
    }
    Ok(())
}
fn root_profile(raw: &str, path: &str) -> Result<Value> {
    let (_, root) = strict(raw.as_bytes(), ROOT_BYTES)?;
    exact(
        &root,
        &[
            "schema_version",
            "logical_schema",
            "header",
            "limits",
            "collections",
        ],
    )?;
    if text(&root, "schema_version")? != "tos_partitioned_projection_v1"
        || text(&root, "logical_schema")?.is_empty()
        || root["header"].as_object().is_none()
        || root["header"].get("schema_version") != root.get("logical_schema")
    {
        return invalid();
    }
    if root["limits"]
        != serde_json::json!({"root_bytes":262144,"index_bytes":131072,"part_bytes":8388608,"key_bytes":4096})
    {
        return invalid();
    }
    let collections = root["collections"]
        .as_object()
        .filter(|o| !o.is_empty())
        .ok_or(Error::Invalid("source collections"))?;
    let mut header = root["header"].clone();
    let mut profile = collections.clone();
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    for (collection, spec) in collections {
        if !collection.split('/').all(|part| name(part, false)) {
            return invalid();
        }
        exact(spec, &["key_field", "order_fields", "root"])?;
        let key = &spec["key_field"];
        let valid_key = key.is_null()
            || key.as_str().is_some_and(|s| !s.is_empty())
            || key
                .as_array()
                .is_some_and(|a| a.iter().all(|v| v.as_str().is_some_and(|s| !s.is_empty())));
        let order = spec["order_fields"]
            .as_array()
            .ok_or(Error::Invalid("source order"))?;
        if !valid_key
            || order.iter().any(|v| v.as_str().is_none_or(str::is_empty))
            || (key.as_array().is_some_and(Vec::is_empty) && !order.is_empty())
        {
            return invalid();
        }
        collection_slot(&mut header, &collection.split('/').collect::<Vec<_>>())?;
        let d = &spec["root"];
        exact(
            d,
            &[
                "kind",
                "prefix",
                "path",
                "sha256",
                "size_bytes",
                "decoded_bytes",
                "decoded_sha256",
                "count",
            ],
        )?;
        let kind = text(d, "kind")?;
        let digest = text(d, "sha256")?;
        if !matches!(kind, "data" | "index")
            || text(d, "prefix")? != ""
            || !sha(digest)
            || !sha(text(d, "decoded_sha256")?)
        {
            return invalid();
        }
        let bound = if kind == "data" { 8_388_608 } else { 131_072 };
        for key in ["size_bytes", "decoded_bytes"] {
            if d[key].as_u64().is_none() {
                return invalid();
            }
        }
        // Python counts are nonnegative integers, including arbitrary precision.
        if !d["count"].as_number().is_some_and(|n| {
            let raw = n.to_string();
            raw == "-0" || (!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()))
        }) {
            return invalid();
        }
        if d["size_bytes"].as_u64().unwrap() > bound + 65536
            || d["decoded_bytes"].as_u64().unwrap() > bound
        {
            return invalid();
        }
        let suffix = if kind == "data" {
            ".jsonl.gz"
        } else {
            ".index.json"
        };
        if text(d, "path")? != format!("{stem}.parts/{}/{digest}{suffix}", &digest[..2]) {
            return invalid();
        }
        profile
            .get_mut(collection)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("root");
    }
    Ok(
        serde_json::json!({"logical_schema":root["logical_schema"],"limits":root["limits"],"collections":profile}),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceProjectionRoot {
    pub namespace_path: String,
    pub root_bytes: Vec<u8>,
    pub snapshot_sha256: String,
}
#[derive(Clone, Debug)]
pub struct PreparedSourceInputs {
    raw: Vec<u8>,
    digest: String,
    source_revision: String,
    roots: std::collections::BTreeMap<String, (String, Value)>,
    retained_roots: std::collections::BTreeMap<String, SourceProjectionRoot>,
}
impl PreparedSourceInputs {
    pub fn parse(raw: &[u8], limits: prepared::PublicationLimits) -> Result<Self> {
        limits.validate()?;
        let cap = MAX_STATE_BYTES.min(limits.max_metadata_bytes);
        let (typed, value) = strict(raw, cap)?;
        exact(
            &value,
            &[
                "schema",
                "source_revision",
                "source_publication",
                "dependencies",
                "roots",
            ],
        )?;
        let revision = text(&value, "source_revision")?;
        if text(&value, "schema")? != "tos_prepared_source_inputs_v1" || !sha(revision) {
            return invalid();
        }
        if !value["source_publication"].is_null()
            && !value["source_publication"]
                .as_str()
                .is_some_and(|s| s.strip_prefix("sha256:").is_some_and(sha))
        {
            return invalid();
        }
        let dependencies = value["dependencies"]
            .as_object()
            .ok_or(Error::Invalid("source dependencies"))?;
        if dependencies.len() > 1024
            || dependencies
                .iter()
                .any(|(k, v)| k.is_empty() || k.len() > 4096 || !v.as_str().is_some_and(sha))
        {
            return invalid();
        }
        let roots = value["roots"]
            .as_object()
            .ok_or(Error::Invalid("source roots"))?;
        if roots.is_empty() || roots.len() > 16 {
            return invalid();
        }
        let mut profiles = std::collections::BTreeMap::new();
        let mut retained_roots = std::collections::BTreeMap::new();
        for (key, root) in roots {
            if !name(key, true) {
                return invalid();
            }
            exact(root, &["namespace_path", "root_json", "snapshot_sha256"])?;
            let path = text(root, "namespace_path")?;
            if path.len() > 4096 || !path.starts_with('/') || path.split('/').any(|s| s == "..") {
                return invalid();
            }
            // pathlib normalizes redundant separators and '.' before canonical reconstruction.
            if Path::new(path)
                .components()
                .any(|c| matches!(c, std::path::Component::CurDir))
                || path.contains("/./")
                || path.ends_with("/.")
                || path.trim_start_matches('/').contains("//")
                || path.starts_with("///")
                || (path.ends_with('/') && path != "/" && path != "//")
            {
                return invalid();
            }
            let root_raw = text(root, "root_json")?;
            let digest = text(root, "snapshot_sha256")?;
            if !sha(digest) || Digest256::of_bytes(root_raw.as_bytes()).to_hex() != digest {
                return invalid();
            }
            profiles.insert(
                key.clone(),
                (path.to_owned(), root_profile(root_raw, path)?),
            );
            retained_roots.insert(
                key.clone(),
                SourceProjectionRoot {
                    namespace_path: path.to_owned(),
                    root_bytes: root_raw.as_bytes().to_vec(),
                    snapshot_sha256: digest.to_owned(),
                },
            );
        }
        if canonical(&typed, cap)? != raw {
            return Err(Error::Invalid("source canonical storage"));
        }
        Ok(Self {
            raw: raw.to_vec(),
            digest: Digest256::of_bytes(raw).to_hex(),
            source_revision: revision.to_owned(),
            roots: profiles,
            retained_roots,
        })
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    /// A detached typed copy for the whole assembler's successor construction.
    pub fn value(&self) -> Result<JsonValue> {
        Ok(strict(&self.raw, MAX_STATE_BYTES)?.0)
    }
    pub fn roots(&self) -> &std::collections::BTreeMap<String, SourceProjectionRoot> {
        &self.retained_roots
    }
}

pub fn read_prepared_source_inputs_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    catalog: &CatalogInputs,
    limits: prepared::PublicationLimits,
) -> Result<PreparedSourceInputs> {
    prepared_maintenance::selected(tx, expected, catalog, limits)?;
    let cap = MAX_STATE_BYTES.min(limits.max_metadata_bytes);
    let mut stmt=tx.prepare("SELECT CASE WHEN typeof(binding)='text' AND length(CAST(binding AS BLOB))<=? THEN binding END,CASE WHEN typeof(inputs)='text' AND length(CAST(inputs AS BLOB))<=? THEN inputs END,CASE WHEN typeof(sha256)='text' AND length(sha256)=64 THEN sha256 END FROM prepared_source_state WHERE singleton=1 LIMIT 2")?;
    let mut rows = stmt.query(params![cap, cap])?;
    let row = rows
        .next()?
        .ok_or(Error::Invalid("source selection absent"))?;
    let binding: Option<String> = row.get(0)?;
    let inputs: Option<String> = row.get(1)?;
    let digest: Option<String> = row.get(2)?;
    let binding = binding.ok_or(Error::Invalid("source binding bytes"))?;
    let inputs = inputs.ok_or(Error::Invalid("source inputs bytes"))?;
    let digest = digest.ok_or(Error::Invalid("source digest"))?;
    if rows.next()?.is_some() || binding.as_bytes() != canonical(expected, cap)? {
        return invalid();
    }
    let parsed = PreparedSourceInputs::parse(inputs.as_bytes(), limits)?;
    if parsed.digest() != digest
        || expected
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(parsed.source_revision())
    {
        return invalid();
    }
    Ok(parsed)
}

#[derive(Clone, Debug)]
pub struct SourceMaintenanceReceipt {
    pub publication: MaintenanceReceipt,
    /// Exact finalized header plus the original registry/lens/order inputs.
    pub final_catalog: CatalogInputs,
    pub source_inputs_sha256: String,
    pub sql_mutations: u64,
    pub roots_paired_in_caller_transaction: bool,
    pub source_transition_verified: bool,
    pub target_closure_verified: bool,
    pub semantic_acceptance: bool,
    pub consumer_switched: bool,
}
pub fn apply_source_bound_prepared_delta_transaction<
    I: IntoIterator<Item = Result<prepared::PreparedChange>>,
>(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before_source: &PreparedSourceInputs,
    after_source: &PreparedSourceInputs,
    before: &CatalogInputs,
    after: &CatalogInputs,
    changes: I,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    normalization_processor_sha256: &str,
) -> Result<SourceMaintenanceReceipt> {
    limits.validate()?;
    if limits.max_mutations < 2 {
        return Err(Error::Budget("source pairing mutations"));
    }
    let predecessor = read_prepared_source_inputs_transaction(tx, expected, before, limits)?;
    if predecessor.raw() != before_source.raw() {
        return Err(Error::Invalid("source predecessor CAS"));
    }
    let successor = PreparedSourceInputs::parse(after_source.raw(), limits)?;
    if predecessor.roots != successor.roots
        || after
            .header
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(successor.source_revision())
    {
        return Err(Error::Invalid("source logical profile requires bootstrap"));
    }
    let start = tx.total_changes();
    let mut engine_limits = limits;
    engine_limits.max_mutations -= 1;
    let publication = prepared_maintenance::apply_semantic_prepared_delta_transaction(
        tx,
        expected,
        before,
        after,
        changes,
        engine_limits,
        catalog_limits,
        semantic_limits,
        normalization_processor_sha256,
    )?;
    finish_pair(
        tx,
        &predecessor,
        &successor,
        after,
        publication,
        limits,
        start,
    )
}
/// Add one independent addressing root without changing existing roots, source
/// publication, normalized rows or execution profiles. The stronger source
/// owner proves membership; this helper does not confer admission or commit.
pub fn bootstrap_prepared_source_root_extension_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before_source: &PreparedSourceInputs,
    after_source: &PreparedSourceInputs,
    added_root: &str,
    before: &CatalogInputs,
    after: &CatalogInputs,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    normalization_processor_sha256: &str,
) -> Result<SourceMaintenanceReceipt> {
    limits.validate()?;
    if limits.max_mutations < 2 {
        return Err(Error::Budget("source extension pairing mutations"));
    }
    let predecessor = read_prepared_source_inputs_transaction(tx, expected, before, limits)?;
    if predecessor.raw() != before_source.raw() {
        return Err(Error::Invalid("source extension predecessor CAS"));
    }
    let successor = PreparedSourceInputs::parse(after_source.raw(), limits)?;
    let (_, old) = strict(predecessor.raw(), MAX_STATE_BYTES)?;
    let (_, new) = strict(successor.raw(), MAX_STATE_BYTES)?;
    if !name(added_root, true)
        || predecessor.retained_roots.contains_key(added_root)
        || successor.retained_roots.len() != predecessor.retained_roots.len() + 1
        || !successor.retained_roots.contains_key(added_root)
        || predecessor
            .retained_roots
            .iter()
            .any(|(key, root)| successor.retained_roots.get(key) != Some(root))
        || old["source_publication"] != new["source_publication"]
        || old["dependencies"] != new["dependencies"]
        || predecessor.source_revision() == successor.source_revision()
    {
        return Err(Error::Invalid(
            "source extension changes predecessor profile",
        ));
    }
    let namespace = &successor.retained_roots[added_root].namespace_path;
    if predecessor
        .retained_roots
        .values()
        .any(|root| &root.namespace_path == namespace)
    {
        return Err(Error::Invalid(
            "source extension namespace already retained",
        ));
    }
    let (_, mut old_header) = strict(
        &canonical(&before.header, limits.max_metadata_bytes)?,
        limits.max_metadata_bytes,
    )?;
    let (_, mut new_header) = strict(
        &canonical(&after.header, limits.max_metadata_bytes)?,
        limits.max_metadata_bytes,
    )?;
    if before.binding()? != after.binding()?
        || old_header["source_revision"].as_str() != Some(predecessor.source_revision())
        || new_header["source_revision"].as_str() != Some(successor.source_revision())
    {
        return Err(Error::Invalid("source extension catalog profile/revision"));
    }
    old_header
        .as_object_mut()
        .ok_or(Error::Invalid("source extension old header"))?
        .remove("source_revision");
    new_header
        .as_object_mut()
        .ok_or(Error::Invalid("source extension new header"))?
        .remove("source_revision");
    if old_header != new_header {
        return Err(Error::Invalid("source extension changes reader semantics"));
    }
    let start = tx.total_changes();
    let mut engine_limits = limits;
    engine_limits.max_mutations -= 1;
    let publication = prepared_maintenance::apply_semantic_prepared_delta_transaction(
        tx,
        expected,
        before,
        after,
        std::iter::empty::<Result<prepared::PreparedChange>>(),
        engine_limits,
        catalog_limits,
        semantic_limits,
        normalization_processor_sha256,
    )?;
    finish_pair(
        tx,
        &predecessor,
        &successor,
        after,
        publication,
        limits,
        start,
    )
}

fn finish_pair(
    tx: &Transaction<'_>,
    predecessor: &PreparedSourceInputs,
    successor: &PreparedSourceInputs,
    after: &CatalogInputs,
    publication: MaintenanceReceipt,
    limits: prepared::PublicationLimits,
    start: u64,
) -> Result<SourceMaintenanceReceipt> {
    let cap = MAX_STATE_BYTES.min(limits.max_metadata_bytes);
    let binding = String::from_utf8(canonical(&publication.binding, cap)?)
        .map_err(|_| Error::Invalid("source binding UTF8"))?;
    let raw =
        std::str::from_utf8(successor.raw()).map_err(|_| Error::Invalid("source inputs UTF8"))?;
    if tx.execute("UPDATE prepared_source_state SET binding=?,inputs=?,sha256=? WHERE singleton=1 AND sha256=?",params![binding,raw,successor.digest(),predecessor.digest()])? != 1 { return Err(Error::Invalid("source state CAS")); }
    // Maintenance finalizes catalog/semantic counts; the proposal header is
    // not the selected descriptor after publication. Preserve its other inputs.
    let mut final_catalog = after.clone();
    final_catalog.header = publication
        .source_header
        .clone()
        .ok_or(Error::Invalid("source pairing finalized header absent"))?;
    if read_prepared_source_inputs_transaction(tx, &publication.binding, &final_catalog, limits)?
        .raw()
        != successor.raw()
    {
        return Err(Error::Invalid("source state readback"));
    }
    let writes = tx
        .total_changes()
        .checked_sub(start)
        .ok_or(Error::Invalid("source mutation counter"))?;
    if writes > limits.max_mutations {
        return Err(Error::Budget("source pairing cumulative mutations"));
    }
    Ok(SourceMaintenanceReceipt {
        publication,
        final_catalog,
        source_inputs_sha256: successor.digest().to_owned(),
        sql_mutations: writes,
        roots_paired_in_caller_transaction: true,
        source_transition_verified: false,
        target_closure_verified: false,
        semantic_acceptance: false,
        consumer_switched: false,
    })
}

/// Explicit implementation-profile transition before any source command writes.
/// Naming a review does not prove algorithm compatibility or source admission;
/// the whole caller owns that evidence and rolls back every paired state.
pub fn transition_prepared_source_profiles_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before_source: &PreparedSourceInputs,
    after_source: &PreparedSourceInputs,
    before: &CatalogInputs,
    after: &CatalogInputs,
    reviewed: &prepared::ReviewedNormalizationTransition<'_>,
    reviewed_dependencies: &std::collections::BTreeMap<String, (String, String)>,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
) -> Result<SourceMaintenanceReceipt> {
    limits.validate()?;
    if limits.max_mutations < 2 {
        return Err(Error::Budget("source profile transition mutations"));
    }
    let predecessor = read_prepared_source_inputs_transaction(tx, expected, before, limits)?;
    if predecessor.raw() != before_source.raw() {
        return Err(Error::Invalid("source profile predecessor CAS"));
    }
    let successor = PreparedSourceInputs::parse(after_source.raw(), limits)?;
    let (_, old) = strict(predecessor.raw(), MAX_STATE_BYTES)?;
    let (_, new) = strict(successor.raw(), MAX_STATE_BYTES)?;
    if predecessor.retained_roots != successor.retained_roots
        || old["source_publication"] != new["source_publication"]
        || predecessor.source_revision() == successor.source_revision()
        || after
            .header
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(successor.source_revision())
    {
        return Err(Error::Invalid(
            "source profile transition must preserve roots/publication",
        ));
    }
    let old_dependencies = old["dependencies"]
        .as_object()
        .ok_or(Error::Invalid("source old dependencies"))?;
    let new_dependencies = new["dependencies"]
        .as_object()
        .ok_or(Error::Invalid("source new dependencies"))?;
    if old_dependencies.keys().ne(new_dependencies.keys())
        || reviewed_dependencies.is_empty()
        || reviewed_dependencies.len() > 4
    {
        return Err(Error::Invalid(
            "named source dependency transition required",
        ));
    }
    let mut changed = 0usize;
    for (key, previous) in old_dependencies {
        let next = &new_dependencies[key];
        if previous != next {
            changed += 1;
            if !matches!(
                key.as_str(),
                "normalization"
                    | "declaration-profile"
                    | "agent-publication-profile"
                    | "claim-publication-profile"
            ) {
                return Err(Error::Invalid(
                    "source dependency outside reviewed profile transition",
                ));
            }
            let (reviewed_before, reviewed_after) = reviewed_dependencies
                .get(key)
                .ok_or(Error::Invalid("source dependency transition not named"))?;
            if previous.as_str() != Some(reviewed_before.as_str())
                || next.as_str() != Some(reviewed_after.as_str())
            {
                return Err(Error::Invalid(
                    "source dependency transition exact digests differ",
                ));
            }
        }
    }
    if changed != reviewed_dependencies.len() {
        return Err(Error::Invalid(
            "source dependency transition has extra or unchanged names",
        ));
    }
    for (inputs, dependencies) in [(before, old_dependencies), (after, new_dependencies)] {
        let binding = inputs
            .header
            .object_get("normalization_binding")
            .ok_or(Error::Invalid("source normalization binding"))?;
        let (_, binding) = strict(&canonical(binding, MAX_STATE_BYTES)?, MAX_STATE_BYTES)?;
        if dependencies.get("normalization").and_then(Value::as_str)
            != Some(crate::knowledge_normalization::stable_digest(&binding)?.as_str())
        {
            return Err(Error::Invalid(
                "source normalization dependency differs from selected header",
            ));
        }
    }
    let start = tx.total_changes();
    let mut engine_limits = limits;
    engine_limits.max_mutations -= 1;
    let publication = prepared_maintenance::transition_prepared_normalization_transaction(
        tx,
        expected,
        before,
        after,
        reviewed,
        engine_limits,
        catalog_limits,
        semantic_limits,
    )?;
    finish_pair(
        tx,
        &predecessor,
        &successor,
        after,
        publication,
        limits,
        start,
    )
}
