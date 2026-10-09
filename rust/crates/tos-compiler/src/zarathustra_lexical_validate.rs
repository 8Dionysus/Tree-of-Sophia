//! Source-closure validator for the maintained Zarathustra lexical projection.
//!
//! The validator checks mechanical counts, resource closure, provenance, and
//! optional local SQLite fixity. It does not make textual, rights, semantic,
//! canon, or publication judgments.

use crate::zarathustra_lexical::{
    LexicalCapture, LexicalSchema, array, hash_local_database, number, parse, read_file, sha, text,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub const PLAN_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/index-plan.v1.json";
pub const PROJECTION_REF: &str =
    "ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json";
const LEGACY_GENERATOR_REF: &str = "scripts/build_zarathustra_lexical_index.py";
const NATIVE_GENERATOR_REF: &str = "rust/crates/tos-compiler/src/zarathustra_lexical.rs";
const BASE_PROVENANCE_EVENT_REF: &str = "tos.event.export.zarathustra-lexical-index-v1.2026-07-29";
const LEGACY_AUTHORITY_BOUNDARY: &str = "mechanical source-observation and rebuildable local search only; no accepted German, rights clearance, lexeme, lemma, translation, sign, concept, claim, relation, graph, canon, or publication authority";
const CURRENT_AUTHORITY_BOUNDARY: &str = "This artifact records mechanical source observations and supports rebuildable local search. Source assessment and permitted uses remain explicit in their own records.";
const MAX_PROVENANCE_EVENTS: usize = 1_000_000;
const MAX_VALIDATION_STEPS: u64 = 16_000_000;
const MAX_RETAINED_GENERATOR_BYTES: usize = 1_048_576;

type Result<T> = std::result::Result<T, String>;

/// Check deadline/cancellation while walking potentially large retained rows.
pub(crate) fn check_active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err("lexical cancellation/deadline".into());
    }
    Ok(())
}

fn step(counter: &mut u64, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    *counter = counter
        .checked_add(1)
        .ok_or_else(|| "lexical validation step overflow".to_owned())?;
    if *counter > MAX_VALIDATION_STEPS {
        return Err("lexical validation step limit".into());
    }
    if *counter & 1023 == 0 {
        check_active(deadline, cancelled)?;
    }
    Ok(())
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .ok_or_else(|| format!("missing object field: {key}"))
}

fn string(value: &Value, key: &str) -> Result<String> {
    Ok(text(value, key)?.to_owned())
}

fn u64_field(value: &Value, key: &str) -> Result<u64> {
    number(value, key)
}

fn parse_object(raw: &[u8], reference: &str) -> Result<Value> {
    let value = parse(raw, raw.len().max(1))?;
    if !value.is_object() {
        return Err(format!("{reference} must contain a JSON object"));
    }
    Ok(value)
}

fn read_json(
    capture: &mut LexicalCapture<'_>,
    reference: &str,
    contract: Option<&str>,
    schema: &mut dyn LexicalSchema,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(Value, Vec<u8>)> {
    check_active(deadline, cancelled)?;
    let raw = capture
        .read(reference)
        .map_err(|error| format!("cannot read {reference}: {error}"))?;
    check_active(deadline, cancelled)?;
    if let Some(contract) = contract {
        let _contract_bytes = capture
            .read(contract)
            .map_err(|error| format!("cannot read schema {contract}: {error}"))?;
        schema
            .check(contract, &raw)
            .map_err(|error| format!("{reference} schema failed: {error}"))?;
    }
    let value = parse_object(&raw, reference)?;
    check_active(deadline, cancelled)?;
    Ok((value, raw))
}

fn validate_provenance_schema(
    raw: &[u8],
    schema: &mut dyn LexicalSchema,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let text =
        std::str::from_utf8(raw).map_err(|error| format!("invalid provenance UTF-8: {error}"))?;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        check_active(deadline, cancelled)?;
        schema
            .check(
                "ToS/contracts/provenance-event.schema.json",
                line.as_bytes(),
            )
            .map_err(|error| format!("lexical provenance schema failed: {error}"))?;
    }
    Ok(())
}

/// Read newline-delimited provenance events through the shared source capture.
pub(crate) fn load_provenance(
    capture: &mut LexicalCapture<'_>,
    reference: &str,
) -> Result<Vec<Value>> {
    let raw = capture
        .read(reference)
        .map_err(|error| format!("cannot read provenance {reference}: {error}"))?;
    let content = std::str::from_utf8(&raw)
        .map_err(|error| format!("cannot read provenance {reference}: {error}"))?;
    let mut events = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if events.len() >= MAX_PROVENANCE_EVENTS {
            return Err("lexical provenance event limit".into());
        }
        let event = parse(line.as_bytes(), line.len().max(1)).map_err(|error| {
            format!(
                "invalid provenance JSON at line {}: {error}",
                line_index + 1
            )
        })?;
        if !event.is_object() {
            return Err(format!(
                "provenance row {} must be an object",
                line_index + 1
            ));
        }
        events.push(event);
    }
    if events.is_empty() {
        return Err("lexical provenance must contain at least one event".into());
    }
    Ok(events)
}

/// Collect all object keys without recursive descent over untrusted nesting.
pub(crate) fn nested_keys(value: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    keys.insert(key.clone());
                    pending.push(child);
                }
            }
            Value::Array(items) => pending.extend(items.iter()),
            _ => {}
        }
    }
    keys
}

/// Verify current or retained exact generator bytes for a provenance digest.
pub(crate) fn recorded_generator_digest(
    capture: &mut LexicalCapture<'_>,
    reference: &str,
    expected_digest: &str,
) -> Result<String> {
    if expected_digest.len() != 64
        || !expected_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("invalid recorded generator digest: {reference}"));
    }
    if reference == NATIVE_GENERATOR_REF {
        let raw = capture
            .read(reference)
            .map_err(|_| format!("recorded generator bytes unavailable: {reference}"))?;
        if sha(&raw) == expected_digest {
            return Ok(expected_digest.to_owned());
        }
        return Err(format!("recorded generator bytes unavailable: {reference}"));
    }
    let Some(file_name) = reference.strip_prefix("scripts/") else {
        return Err(format!("recorded generator bytes unavailable: {reference}"));
    };
    let stem = file_name.strip_suffix(".py").unwrap_or("");
    if stem.is_empty()
        || !stem.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
            }
        })
    {
        return Err(format!("recorded generator bytes unavailable: {reference}"));
    }
    let archived_ref =
        format!("ToS/research-packets/retained-builder-inputs/{stem}/{expected_digest}.py");
    // Retired generators remain exact historical inputs. Their evidence must
    // not depend on keeping an executable at the former active script path.
    if let Ok(archived) = capture.read(&archived_ref) {
        if archived.len() <= MAX_RETAINED_GENERATOR_BYTES && sha(&archived) == expected_digest {
            return Ok(expected_digest.to_owned());
        }
        return Err(format!("retained generator digest mismatch: {reference}"));
    }
    let current = capture
        .read(reference)
        .map_err(|_| format!("recorded generator bytes unavailable: {reference}"))?;
    if current.len() <= MAX_RETAINED_GENERATOR_BYTES && sha(&current) == expected_digest {
        return Ok(expected_digest.to_owned());
    }
    Err(format!("recorded generator bytes unavailable: {reference}"))
}

fn latest_provenance_event(
    events: &[Value],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value> {
    let mut by_id = BTreeMap::<String, &Value>::new();
    let mut successors = BTreeMap::<String, Vec<&Value>>::new();
    let mut work = 0u64;
    for event in events {
        step(&mut work, deadline, cancelled)?;
        let event_id = text(event, "event_id")?;
        if event_id.is_empty() {
            return Err("provenance event_id is absent".into());
        }
        if by_id.insert(event_id.to_owned(), event).is_some() {
            return Err(format!("duplicate provenance event_id: {event_id}"));
        }
        if let Some(parent) = event.get("supersedes_event_ref").and_then(Value::as_str) {
            successors.entry(parent.to_owned()).or_default().push(event);
        }
    }
    let mut current = *by_id
        .get(BASE_PROVENANCE_EVENT_REF)
        .ok_or_else(|| "lexical base provenance event is absent".to_owned())?;
    let mut seen = BTreeSet::from([BASE_PROVENANCE_EVENT_REF.to_owned()]);
    loop {
        check_active(deadline, cancelled)?;
        let current_id = text(current, "event_id")?;
        let Some(current_successors) = successors.get(current_id) else {
            if seen.len() != by_id.len() {
                return Err("lexical provenance contains an orphan event".into());
            }
            return Ok(current.clone());
        };
        if current_successors.len() != 1 {
            return Err(format!(
                "ambiguous provenance supersession from {current_id}"
            ));
        }
        current = current_successors[0];
        let next_id = text(current, "event_id")?;
        if !seen.insert(next_id.to_owned()) {
            return Err("cyclic lexical provenance supersession lineage".into());
        }
    }
}

#[cfg(test)]
mod retained_generator_tests {
    use super::*;
    #[test]
    fn historical_digest_survives_active_retirement_and_refuses_tamper() {
        let root = tempfile::tempdir().unwrap();
        let raw = b"# immutable historical producer bytes\n";
        let digest = sha(raw);
        let reference = "scripts/build_retired_lexical.py";
        let archive = root.path().join(format!(
            "ToS/research-packets/retained-builder-inputs/build_retired_lexical/{digest}.py"
        ));
        std::fs::create_dir_all(archive.parent().unwrap()).unwrap();
        std::fs::write(&archive, raw).unwrap();
        let mut capture = LexicalCapture::new(
            root.path(),
            crate::zarathustra_lexical::LexicalLimits::maintained(),
        )
        .unwrap();
        assert_eq!(
            recorded_generator_digest(&mut capture, reference, &digest).unwrap(),
            digest
        );
        assert!(!root.path().join(reference).exists());
        capture.revalidate().unwrap();
        std::fs::write(&archive, b"changed").unwrap();
        assert!(capture.revalidate().is_err());
        let mut changed = LexicalCapture::new(
            root.path(),
            crate::zarathustra_lexical::LexicalLimits::maintained(),
        )
        .unwrap();
        assert!(
            recorded_generator_digest(&mut changed, reference, &digest)
                .unwrap_err()
                .contains("digest mismatch")
        );
    }
}

fn resource_maps(
    inventory: &Value,
    file_id: &str,
    work: &mut u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let files = array(inventory, "files")?;
    let mut matches = Vec::new();
    for entry in files {
        step(work, deadline, cancelled)?;
        if entry.get("file_id").and_then(Value::as_str) == Some(file_id) {
            matches.push(entry);
        }
    }
    if matches.len() != 1 {
        return Err(format!("inventory does not resolve one file: {file_id}"));
    }
    let resources = array(matches[0], "resources")?;
    let mut pages = BTreeSet::new();
    let mut sections = BTreeSet::new();
    for resource in resources {
        step(work, deadline, cancelled)?;
        let Some(id) = resource.get("resource_id").and_then(Value::as_str) else {
            continue;
        };
        match resource.get("resource_kind").and_then(Value::as_str) {
            Some("tei_page_break") => {
                pages.insert(id.to_owned());
            }
            Some("tei_division") => {
                sections.insert(id.to_owned());
            }
            _ => {}
        }
    }
    if pages.is_empty() || sections.is_empty() {
        return Err("inventory lacks TEI page/division resources".into());
    }
    Ok((pages, sections))
}

fn checked_sum(target: &mut u64, value: u64, label: &str) -> Result<()> {
    *target = target
        .checked_add(value)
        .ok_or_else(|| format!("{label} overflow"))?;
    Ok(())
}

fn validate_local_database(
    max_database_bytes: u64,
    local_root: &Path,
    projection: &Value,
    plan: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    check_active(deadline, cancelled)?;
    let receipt = required(projection, "local_projection_receipt")?;
    let relative = text(receipt, "relative_path")?;
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("local database path escapes explicit root".into());
    }
    let joined = local_root.join(relative_path);
    if !relative_path
        .components()
        .any(|component| component.as_os_str() == "local-content")
    {
        return Err("local database is outside local-content".into());
    }
    let expected_bytes = u64_field(receipt, "database_bytes")?;
    let expected_digest = text(receipt, "database_sha256")?;
    let (bytes, digest) = hash_local_database(&joined, max_database_bytes, deadline, cancelled)?;
    if bytes != expected_bytes {
        return Err("local database byte-size drift".into());
    }
    if digest != expected_digest {
        return Err("local database digest drift".into());
    }

    check_active(deadline, cancelled)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let connection = Connection::open_with_flags(&joined, flags)
        .map_err(|error| format!("cannot inspect local lexical database: {error}"))?;
    connection.progress_handler(10_000, Some(move || Instant::now() >= deadline));
    let meta = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'plan_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("cannot inspect local lexical database: {error}"))?;
    if meta != text(plan, "plan_id")? {
        return Err("local database plan identity drift".into());
    }
    let plan_digest = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'plan_sha256'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("cannot inspect local lexical database: {error}"))?;
    if plan_digest != text(projection, "plan_sha256")? {
        return Err("local database plan digest drift".into());
    }
    let allowed = BTreeSet::from(["source_items", "pages", "sections", "occurrences", "forms"]);
    let table_counts = required(receipt, "table_counts")?
        .as_object()
        .ok_or_else(|| "local table_counts must be an object".to_owned())?;
    for (table, expected) in table_counts {
        if !allowed.contains(table.as_str()) {
            return Err(format!("unsupported local database table: {table}"));
        }
        check_active(deadline, cancelled)?;
        let sql = format!("SELECT count(*) FROM {table}");
        let actual: i64 = connection
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|error| format!("cannot inspect local lexical database: {error}"))?;
        let expected = expected
            .as_u64()
            .ok_or_else(|| format!("local database table count is invalid: {table}"))?;
        if actual < 0 || actual as u64 != expected {
            return Err(format!(
                "local database {table} count drift: {actual} != {expected}"
            ));
        }
    }
    drop(connection);
    check_active(deadline, cancelled)?;
    let (post_bytes, post_digest) =
        hash_local_database(&joined, max_database_bytes, deadline, cancelled)?;
    if post_bytes != expected_bytes || post_digest != expected_digest {
        return Err("local database changed during verification".into());
    }
    Ok(())
}

fn output_pairs(event: &Value) -> Result<BTreeSet<(String, String)>> {
    let mut pairs = BTreeSet::new();
    for entry in array(event, "outputs")? {
        pairs.insert((string(entry, "ref")?, string(entry, "sha256")?));
    }
    Ok(pairs)
}

fn candidate_events(
    capture: &mut LexicalCapture<'_>,
    provenance_ref: &str,
    candidate_raw: Option<&[u8]>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<Value>> {
    let authored_raw = capture
        .read(provenance_ref)
        .map_err(|error| format!("cannot read provenance {provenance_ref}: {error}"))?;
    let authored = load_provenance(capture, provenance_ref)?;
    let Some(candidate_raw) = candidate_raw else {
        return Ok(authored);
    };
    if candidate_raw.len() > capture.limits().max_file_bytes {
        return Err("candidate provenance byte budget".into());
    }
    if !candidate_raw.starts_with(&authored_raw) {
        return Err("candidate provenance does not preserve exact authored prefix".into());
    }
    let candidate_text = std::str::from_utf8(candidate_raw)
        .map_err(|error| format!("candidate provenance is not UTF-8: {error}"))?;
    let mut events = Vec::new();
    for (line_index, line) in candidate_text.lines().enumerate() {
        check_active(deadline, cancelled)?;
        if line.trim().is_empty() {
            continue;
        }
        if events.len() >= MAX_PROVENANCE_EVENTS {
            return Err("lexical provenance event limit".into());
        }
        let event = parse(line.as_bytes(), line.len().max(1)).map_err(|error| {
            format!(
                "invalid candidate provenance JSON at line {}: {error}",
                line_index + 1
            )
        })?;
        if !event.is_object() {
            return Err(format!(
                "candidate provenance row {} must be an object",
                line_index + 1
            ));
        }
        events.push(event);
    }
    if events.len() != authored.len() + 1
        || !events
            .iter()
            .take(authored.len())
            .zip(authored.iter())
            .all(|(candidate, source)| candidate == source)
    {
        return Err(
            "candidate provenance must append exactly one event to authored lineage".into(),
        );
    }
    let prior_id = text(
        events
            .get(authored.len() - 1)
            .ok_or("authored provenance is empty")?,
        "event_id",
    )?;
    if events[authored.len()]
        .get("supersedes_event_ref")
        .and_then(Value::as_str)
        != Some(prior_id)
    {
        return Err("candidate provenance successor does not extend authored lineage".into());
    }
    Ok(events)
}

/// Validate a captured lexical plan and a tracked or disposable projection.
/// All authored paths are read through the supplied immutable capture except
/// the explicit local database root and caller-supplied projection path.
pub fn validate_with_capture(
    repo_root: &Path,
    projection_path: &Path,
    local_output_root: Option<&Path>,
    candidate_provenance_raw: Option<&[u8]>,
    capture: &mut LexicalCapture<'_>,
    schema: &mut dyn LexicalSchema,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> std::result::Result<Value, String> {
    check_active(deadline, cancelled)?;
    let (plan, plan_raw) = read_json(
        capture,
        PLAN_REF,
        Some("ToS/contracts/lexical-index-plan.schema.json"),
        schema,
        deadline,
        cancelled,
    )?;
    let projection_path = if projection_path.is_absolute() {
        projection_path.to_path_buf()
    } else {
        repo_root.join(projection_path)
    };
    let projection_raw = read_file(&projection_path, capture.limits().max_projection_bytes)
        .map_err(|error| {
            format!(
                "cannot read projection {}: {error}",
                projection_path.display()
            )
        })?;
    check_active(deadline, cancelled)?;
    let _projection_schema = capture
        .read("ToS/contracts/lexical-index-projection.schema.json")
        .map_err(|error| format!("cannot read projection schema: {error}"))?;
    schema
        .check(
            "ToS/contracts/lexical-index-projection.schema.json",
            &projection_raw,
        )
        .map_err(|error| format!("lexical projection schema failed: {error}"))?;
    let projection = parse_object(&projection_raw, "lexical projection")?;
    let plan_digest = sha(&plan_raw);
    let projection_digest = sha(&projection_raw);

    if text(&projection, "generated_or_authored")? != "generated_from_source" {
        return Err("lexical projection must declare generated_from_source".into());
    }
    if text(&projection, "plan_id")? != text(&plan, "plan_id")? {
        return Err("plan identity drift".into());
    }
    if text(&projection, "plan_ref")? != PLAN_REF {
        return Err("plan ref drift".into());
    }
    if text(&projection, "plan_sha256")? != plan_digest {
        return Err("plan digest drift".into());
    }
    let generator_ref = text(&projection, "generator_ref")?;
    if generator_ref != LEGACY_GENERATOR_REF && generator_ref != NATIVE_GENERATOR_REF {
        return Err("generator ref drift".into());
    }
    if text(required(&projection, "builder")?, "surface")? != generator_ref {
        return Err("builder surface drift".into());
    }
    let generator_digest = recorded_generator_digest(
        capture,
        generator_ref,
        text(&projection, "generator_sha256")?,
    )?;
    if text(required(&plan, "tracked_projection")?, "relative_path")? != PROJECTION_REF {
        return Err("tracked projection route drift".into());
    }
    let local_receipt = required(&projection, "local_projection_receipt")?;
    let local_plan = required(&plan, "local_projection")?;
    if text(local_receipt, "relative_path")? != text(local_plan, "relative_path")? {
        return Err("local projection route drift".into());
    }
    for field in [
        "field_posture",
        "rights_and_visibility",
        "semantic_boundary",
    ] {
        if required(&projection, field)? != required(&plan, field)? {
            return Err(format!("{field} drift"));
        }
    }
    let plan_authority_boundary = text(&plan, "authority_boundary")?;
    if plan_authority_boundary != LEGACY_AUTHORITY_BOUNDARY
        && plan_authority_boundary != CURRENT_AUTHORITY_BOUNDARY
    {
        return Err("unsupported lexical authority boundary".into());
    }
    if text(&projection, "authority_boundary")? != plan_authority_boundary {
        return Err("projection authority boundary drift".into());
    }

    let plan_items_raw = array(&plan, "source_items")?;
    let receipt_items = array(&projection, "source_items")?;
    let mut plan_items = BTreeMap::<String, &Value>::new();
    for item in plan_items_raw {
        let item_ref = text(item, "item_ref")?.to_owned();
        if plan_items.insert(item_ref.clone(), item).is_some() {
            return Err(format!("duplicate plan source item: {item_ref}"));
        }
    }
    let mut receipts = BTreeMap::<String, &Value>::new();
    for item in receipt_items {
        let item_ref = text(item, "item_ref")?.to_owned();
        if receipts.insert(item_ref.clone(), item).is_some() {
            return Err(format!("duplicate projection source item: {item_ref}"));
        }
    }
    if plan_items.keys().collect::<Vec<_>>() != receipts.keys().collect::<Vec<_>>() {
        return Err("projection source-item closure drift".into());
    }
    let mut work = 0u64;
    for (index, item) in receipt_items.iter().enumerate() {
        step(&mut work, deadline, cancelled)?;
        if u64_field(item, "part_order")? != index as u64 + 1 {
            return Err("projection part order is not contiguous".into());
        }
    }

    let mut item_resources = BTreeMap::<String, (BTreeSet<String>, BTreeSet<String>)>::new();
    for (item_ref, plan_item) in &plan_items {
        step(&mut work, deadline, cancelled)?;
        let receipt = receipts[item_ref];
        for field in [
            "part_order",
            "file_id",
            "file_sha256",
            "manifest_ref",
            "resource_inventory_ref",
            "rights_ref",
            "language",
        ] {
            if required(receipt, field)? != required(plan_item, field)? {
                return Err(format!("{item_ref} receipt drift in {field}"));
            }
        }
        let manifest_ref = text(plan_item, "manifest_ref")?;
        let inventory_ref = text(plan_item, "resource_inventory_ref")?;
        let rights_ref = text(plan_item, "rights_ref")?;
        let (manifest, _) = read_json(capture, manifest_ref, None, schema, deadline, cancelled)?;
        let (inventory, _) = read_json(capture, inventory_ref, None, schema, deadline, cancelled)?;
        let (rights, _) = read_json(capture, rights_ref, None, schema, deadline, cancelled)?;
        if manifest.get("item_id").and_then(Value::as_str) != Some(item_ref) {
            return Err(format!("{item_ref} manifest identity drift"));
        }
        let file_id = text(plan_item, "file_id")?;
        let file_digest = text(plan_item, "file_sha256")?;
        let mut matching_files = 0usize;
        for entry in array(&manifest, "payload_files")? {
            step(&mut work, deadline, cancelled)?;
            if entry.get("file_id").and_then(Value::as_str) == Some(file_id)
                && entry.get("sha256").and_then(Value::as_str) == Some(file_digest)
            {
                matching_files += 1;
            }
        }
        if matching_files != 1 {
            return Err(format!(
                "{item_ref} exact file does not close through manifest"
            ));
        }
        if required(receipt, "edition_ref")? != required(&manifest, "embodiment_ref")? {
            return Err(format!("{item_ref} edition ref drift"));
        }
        for (receipt_field, rights_field, label) in [
            (
                "rights_assessment_status",
                "assessment_status",
                "rights assessment",
            ),
            ("rights_review_status", "review_status", "rights review"),
        ] {
            if required(receipt, receipt_field)? != required(&rights, rights_field)? {
                return Err(format!("{item_ref} {label} drift"));
            }
        }
        let (pages, sections) = resource_maps(&inventory, file_id, &mut work, deadline, cancelled)?;
        if u64_field(receipt, "body_page_count")? > pages.len() as u64 {
            return Err(format!("{item_ref} body page count exceeds inventory"));
        }
        if u64_field(receipt, "section_count")? > sections.len() as u64 {
            return Err(format!("{item_ref} section count exceeds inventory"));
        }
        item_resources.insert(item_ref.clone(), (pages, sections));
    }

    let form_rows = array(&projection, "form_rows")?;
    let summary = required(&projection, "summary")?;
    if form_rows.len() as u64 != u64_field(summary, "exact_form_row_count")? {
        return Err("exact form row count drift".into());
    }
    let mut form_keys = BTreeSet::new();
    let mut exact_hashes = BTreeSet::new();
    let mut normalized_hashes = BTreeSet::new();
    let mut previous_digest: Option<&str> = None;
    for row in form_rows {
        step(&mut work, deadline, cancelled)?;
        let form_key = text(row, "form_key")?;
        let exact_hash = text(row, "exact_form_sha256")?;
        if !form_keys.insert(form_key.to_owned()) {
            return Err("duplicate tracked form key".into());
        }
        if !exact_hashes.insert(exact_hash.to_owned()) {
            return Err("duplicate tracked exact-form digest".into());
        }
        if form_key != format!("lexical-form:sha256:{exact_hash}") {
            return Err("form key does not bind exact digest".into());
        }
        if previous_digest.is_some_and(|previous| previous > exact_hash) {
            return Err("tracked form rows are not deterministic".into());
        }
        previous_digest = Some(exact_hash);
        normalized_hashes.insert(text(row, "normalized_form_sha256")?.to_owned());
    }
    if normalized_hashes.len() as u64 != u64_field(summary, "normalized_form_hash_count")? {
        return Err("normalized form hash count drift".into());
    }

    let mut token_total = 0u64;
    let mut editorial_total = 0u64;
    let mut unsectioned_total = 0u64;
    let mut per_item_totals = BTreeMap::<String, u64>::new();
    for row in form_rows {
        step(&mut work, deadline, cancelled)?;
        let occurrence_count = u64_field(row, "occurrence_count")?;
        let editorial_count = u64_field(row, "source_editorial_occurrence_count")?;
        let unsectioned_count = u64_field(row, "unsectioned_occurrence_count")?;
        if editorial_count > occurrence_count {
            return Err("editorial count exceeds form count".into());
        }
        if unsectioned_count > occurrence_count {
            return Err("unsectioned count exceeds form count".into());
        }
        let mut row_total = 0u64;
        let mut row_section_total = 0u64;
        let mut seen_items = BTreeSet::new();
        for item_hit in array(row, "source_items")? {
            step(&mut work, deadline, cancelled)?;
            let item_ref = text(item_hit, "item_ref")?.to_owned();
            if !seen_items.insert(item_ref.clone()) || !plan_items.contains_key(&item_ref) {
                return Err("form row has duplicate or unknown source item".into());
            }
            let (pages, sections) = item_resources
                .get(&item_ref)
                .ok_or_else(|| "form row resource closure missing".to_owned())?;
            let mut page_total = 0u64;
            let mut seen_pages = BTreeSet::new();
            for hit in array(item_hit, "page_hits")? {
                step(&mut work, deadline, cancelled)?;
                let resource_id = text(hit, "resource_id")?.to_owned();
                if !seen_pages.insert(resource_id.clone()) || !pages.contains(&resource_id) {
                    return Err(format!("{item_ref} form row has invalid page resource"));
                }
                checked_sum(
                    &mut page_total,
                    u64_field(hit, "occurrence_count")?,
                    "page occurrence",
                )?;
            }
            let item_occurrence_count = u64_field(item_hit, "occurrence_count")?;
            if page_total != item_occurrence_count {
                return Err(format!("{item_ref} page-hit counts do not close"));
            }
            let mut section_total = 0u64;
            let mut seen_sections = BTreeSet::new();
            for hit in array(item_hit, "section_hits")? {
                step(&mut work, deadline, cancelled)?;
                let resource_id = text(hit, "resource_id")?.to_owned();
                if !seen_sections.insert(resource_id.clone()) || !sections.contains(&resource_id) {
                    return Err(format!("{item_ref} form row has invalid section resource"));
                }
                checked_sum(
                    &mut section_total,
                    u64_field(hit, "occurrence_count")?,
                    "section occurrence",
                )?;
            }
            if section_total > item_occurrence_count {
                return Err(format!("{item_ref} section-hit counts exceed occurrences"));
            }
            checked_sum(&mut row_total, item_occurrence_count, "row occurrence")?;
            checked_sum(
                &mut row_section_total,
                section_total,
                "row section occurrence",
            )?;
            let item_total = per_item_totals.entry(item_ref).or_default();
            checked_sum(item_total, item_occurrence_count, "per-item occurrence")?;
        }
        if row_total != occurrence_count {
            return Err("form source-item counts do not close".into());
        }
        if row_total.checked_sub(row_section_total) != Some(unsectioned_count) {
            return Err("form unsectioned count does not close".into());
        }
        checked_sum(&mut token_total, row_total, "projection token total")?;
        checked_sum(
            &mut editorial_total,
            editorial_count,
            "projection editorial total",
        )?;
        checked_sum(
            &mut unsectioned_total,
            unsectioned_count,
            "projection unsectioned total",
        )?;
    }
    if token_total != u64_field(summary, "token_occurrence_count")? {
        return Err("projection token total drift".into());
    }
    if editorial_total != u64_field(summary, "source_editorial_occurrence_count")? {
        return Err("projection editorial total drift".into());
    }
    if unsectioned_total != u64_field(summary, "unsectioned_occurrence_count")? {
        return Err("projection unsectioned total drift".into());
    }
    if u64_field(summary, "source_item_count")? != plan_items.len() as u64 {
        return Err("source item summary drift".into());
    }
    let body_page_sum = receipts.values().try_fold(0u64, |sum, receipt| {
        sum.checked_add(u64_field(receipt, "body_page_count")?)
            .ok_or_else(|| "body page summary overflow".to_owned())
    })?;
    if u64_field(summary, "body_page_count")? != body_page_sum {
        return Err("body page summary drift".into());
    }
    let section_sum = receipts.values().try_fold(0u64, |sum, receipt| {
        sum.checked_add(u64_field(receipt, "section_count")?)
            .ok_or_else(|| "section summary overflow".to_owned())
    })?;
    if u64_field(summary, "section_count")? != section_sum {
        return Err("section summary drift".into());
    }
    for (item_ref, receipt) in &receipts {
        if per_item_totals.get(item_ref).copied().unwrap_or(0)
            != u64_field(receipt, "token_occurrence_count")?
        {
            return Err(format!("{item_ref} token total drift"));
        }
    }

    let expected_local_counts = json!({
        "source_items": u64_field(summary, "source_item_count")?,
        "pages": u64_field(summary, "body_page_count")?,
        "sections": u64_field(summary, "section_count")?,
        "occurrences": u64_field(summary, "token_occurrence_count")?,
        "forms": u64_field(summary, "exact_form_row_count")?,
    });
    if required(local_receipt, "table_counts")? != &expected_local_counts {
        return Err("local projection receipt count drift".into());
    }
    let probes = required(local_receipt, "query_probes")?;
    for field in [
        "exact_form",
        "normalized_form",
        "prefix",
        "phrase",
        "section",
        "page",
        "language",
        "edition",
    ] {
        if text(required(probes, field)?, "status")? != "passed" {
            return Err("materialized query probe did not pass".into());
        }
    }
    for field in ["lemma", "sign_candidate"] {
        if text(required(probes, field)?, "status")? != "blocked-not-materialized" {
            return Err("linguistic/semantic blockers drift".into());
        }
    }

    let provenance_ref = text(&plan, "provenance_ref")?;
    if candidate_provenance_raw.is_some() && provenance_ref.is_empty() {
        return Err("candidate provenance route is absent".into());
    }
    let events = candidate_events(
        capture,
        provenance_ref,
        candidate_provenance_raw,
        deadline,
        cancelled,
    )?;
    let authored_provenance_raw = capture
        .read(provenance_ref)
        .map_err(|error| format!("cannot read provenance {provenance_ref}: {error}"))?;
    let effective_provenance_raw = candidate_provenance_raw.unwrap_or(&authored_provenance_raw);
    let _provenance_contract = capture
        .read("ToS/contracts/provenance-event.schema.json")
        .map_err(|error| format!("cannot read provenance schema: {error}"))?;
    validate_provenance_schema(effective_provenance_raw, schema, deadline, cancelled)?;
    let provenance = latest_provenance_event(&events, deadline, cancelled)?;
    if text(&provenance, "event_type")? != "export" {
        return Err("lexical provenance is not an export event".into());
    }
    if text(required(&provenance, "method")?, "artifact_digest")? != generator_digest {
        return Err("provenance generator digest drift".into());
    }
    let outputs = output_pairs(&provenance)?;
    if !outputs.contains(&(PROJECTION_REF.to_owned(), projection_digest.clone())) {
        return Err("provenance does not bind tracked projection digest".into());
    }
    let local_pair = (
        text(local_plan, "relative_path")?.to_owned(),
        text(local_receipt, "database_sha256")?.to_owned(),
    );
    if !outputs.contains(&local_pair) {
        return Err("provenance does not bind private local database digest".into());
    }
    if text(&provenance, "status")? != "completed_with_warnings" {
        return Err("lexical export must preserve unresolved source/rights warnings".into());
    }

    if let Some(local_root) = local_output_root {
        validate_local_database(
            capture.limits().max_database_bytes,
            local_root,
            &projection,
            &plan,
            deadline,
            cancelled,
        )?;
    }
    let usage_context = crate::zarathustra_lexical_usage_validate::validate_usage_context(
        capture, schema, deadline, cancelled,
    )?;
    let morphology_context =
        crate::zarathustra_lexical_morphology_validate::validate_morphology_context(
            capture, schema, deadline, cancelled,
        )?;
    check_active(deadline, cancelled)?;
    capture.revalidate()?;
    let projection_recheck = read_file(&projection_path, capture.limits().max_projection_bytes)
        .map_err(|error| {
            format!(
                "cannot reread projection {}: {error}",
                projection_path.display()
            )
        })?;
    if projection_recheck != projection_raw {
        return Err("projection changed during validation".into());
    }
    check_active(deadline, cancelled)?;

    Ok(json!({
        "status": "ok",
        "projection_ref": PROJECTION_REF,
        "projection_sha256": projection_digest,
        "local_database_sha256": text(local_receipt, "database_sha256")?,
        "local_database_verified": local_output_root.is_some(),
        "summary": summary,
        "usage_context": usage_context,
        "morphology_context": morphology_context,
        "authority_boundary": plan_authority_boundary,
    }))
}

#[cfg(test)]
mod retired_validator_regressions {
    use super::*;
    use std::{
        fs,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};

    #[test]
    fn exact_unicode_tokens_and_internal_joiners_survive_normalization() {
        use crate::zarathustra_lexical::{LexicalLimits, normalize_form, word_spans};
        let joiners = BTreeSet::from(['-', '\'', '’', '‐', '‑']);
        let words = word_spans("Über-Mensch O’Connor Straße 123 -- Wort", &joiners, 1024).unwrap();
        assert_eq!(
            words.iter().map(|w| w.2.as_str()).collect::<Vec<_>>(),
            ["Über-Mensch", "O’Connor", "Straße", "Wort"]
        );
        assert_eq!(
            normalize_form(&words[0].2, LexicalLimits::maintained()).unwrap(),
            "über-mensch"
        );
        assert_eq!(
            normalize_form(&words[2].2, LexicalLimits::maintained()).unwrap(),
            "strasse"
        );
        assert_eq!(words[2].2, "Straße");
    }

    #[test]
    fn local_database_fixity_counts_and_absence_are_checked() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tos-lexical-fixity-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("local-content")).unwrap();
        let path = root.join("local-content/lexical.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE forms(id TEXT PRIMARY KEY); INSERT INTO forms VALUES('one');").unwrap();
        db.execute(
            "INSERT INTO metadata VALUES('plan_id',?1),('plan_sha256',?2)",
            ["lexical-plan:test", &"a".repeat(64)],
        )
        .unwrap();
        drop(db);
        let bytes = fs::read(&path).unwrap();
        let plan = json!({"plan_id":"lexical-plan:test"});
        let mut projection = json!({"plan_sha256":"a".repeat(64),"local_projection_receipt":{"relative_path":"local-content/lexical.db","database_bytes":bytes.len(),"database_sha256":sha(&bytes),"table_counts":{"forms":1}}});
        let deadline = Instant::now() + Duration::from_secs(10);
        let cancelled = AtomicBool::new(false);
        let check =
            |p: &Value| validate_local_database(1024 * 1024, &root, p, &plan, deadline, &cancelled);
        check(&projection).unwrap();
        projection["local_projection_receipt"]["table_counts"]["forms"] = json!(2);
        assert!(check(&projection).unwrap_err().contains("count drift"));
        projection["local_projection_receipt"]["table_counts"]["forms"] = json!(1);
        fs::write(&path, [&bytes[..], b"drift"].concat()).unwrap();
        assert!(check(&projection).unwrap_err().contains("byte-size drift"));
        fs::remove_file(&path).unwrap();
        assert!(check(&projection).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    fn probe(raw: &[u8]) -> (String, SchemaBackendProbe) {
        let v: Value = serde_json::from_slice(raw).unwrap();
        let uri = v["$id"].as_str().unwrap().to_owned();
        let probe = SchemaBackendProbe::new(
            [SchemaResource {
                uri: uri.clone(),
                raw: raw.to_vec(),
            }],
            FormatProfile::LegacyPythonObserved20260923,
        )
        .unwrap();
        (uri, probe)
    }
    #[test]
    fn recurrence_and_morphology_schemas_refuse_source_and_semantic_fields() {
        let (uri, p) = probe(include_bytes!(
            "../../../../ToS/contracts/lexical-recurrence-projection.schema.json"
        ));
        let selector = format!("{uri}#/$defs/recurrenceRow");
        let row = json!({"form_key":format!("lexical-form:sha256:{}","a".repeat(64)),"exact_form_sha256":"a".repeat(64),"normalized_form_sha256":"b".repeat(64),"occurrence_count":2,"part_range":1,"section_range":1,"page_range":1,"part_dp_millionths":500000,"maximum_part_share_millionths":1000000,"source_editorial_occurrence_count":0,"unsectioned_occurrence_count":0});
        assert!(p.is_valid_value(&selector, &row).unwrap());
        for (key, value) in [
            ("sign_score", json!(0.9)),
            ("exact_form", json!("Übermensch")),
        ] {
            let mut v = row.clone();
            v[key] = value;
            assert!(!p.is_valid_value(&selector, &v).unwrap());
        }
        let (uri, p) = probe(include_bytes!(
            "../../../../ToS/contracts/morphology-census-result-receipt.schema.json"
        ));
        let selector = format!("{uri}#/$defs/providerPosCountMap");
        assert!(p.is_valid_value(&selector, &json!({"NOUN":1})).unwrap());
        assert!(
            !p.is_valid_value(&selector, &json!({"Übermensch":1}))
                .unwrap()
        );
    }

    #[test]
    fn private_usage_row_does_not_admit_semantic_fields() {
        let (uri, p) = probe(include_bytes!(
            "../../../../ToS/contracts/lexical-usage-context-row.schema.json"
        ));
        let row = json!({"schema_version":"tos_lexical_usage_context_row_v1","context_id":format!("usage-context:sha256:{}","a".repeat(64)),"question_id":"zarathustra-work-identity-control-context-v1","form_key":format!("lexical-form:sha256:{}","a".repeat(64)),"exact_form_sha256":"a".repeat(64),"occurrence_id":"tos.occurrence.synthetic-control-000001","item_ref":"tos.item.synthetic-control","part_order":1,"source_file_sha256":"b".repeat(64),"token_ordinal":1,"page_resource_id":"tei-page:synthetic-1","section_resource_id":null,"text_node_path":"/TEI/text/body/p[1]/text()[1]","start_offset":0,"end_offset":23,"editorial_status":"witness-text","target_exact_form":"synthetic-control-token","left_exact_tokens":[],"right_exact_tokens":["neighbor"],"left_token_count":0,"right_token_count":1,"requested_window_each_side":24,"page_start_clipped":true,"page_end_clipped":true,"source_database_sha256":"a".repeat(64),"authority":"unreviewed-source-visible-method-control"});
        assert!(p.is_valid_value(&uri, &row).unwrap());
        for (key, value) in [
            ("lemma", json!("synthetic")),
            ("sign_score", json!(1.0)),
            ("concept_ref", json!("tos.concept.synthetic")),
        ] {
            let mut v = row.clone();
            v[key] = value;
            assert!(!p.is_valid_value(&uri, &v).unwrap());
        }
    }
}
