//! Registry navigation over explicit owner records. A candidate match is never
//! an identity merge; recorded acquisition and live custody are distinct views.
use crate::{
    kag_release::{self, invalid},
    source_registry::{Context, member},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::AtomicI32,
};
use tos_compiler::source_registry::{
    self as rules, array, canonical_url, folded, string, text, truth, urls,
};
fn arr(v: &Value) -> io::Result<&Vec<Value>> {
    array(v).map_err(invalid)
}
fn txt(v: &Value) -> io::Result<&str> {
    text(v).map_err(invalid)
}
fn strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(s) => out.push(s),
        Value::Array(v) => {
            for v in v {
                strings(v, out)
            }
        }
        Value::Object(v) => {
            for v in v.values() {
                strings(v, out)
            }
        }
        _ => {}
    }
}
fn paths(
    context: &mut Context<'_>,
    root: &Path,
    name: &str,
    recursive: bool,
) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    kag_release::directory(root)?;
    let mut todo = vec![root.to_path_buf()];
    let mut result = Vec::new();
    let mut count = 0;
    while let Some(path) = todo.pop() {
        for entry in fs::read_dir(path)? {
            context.check(1).map_err(invalid)?;
            count += 1;
            if count > 100_000 {
                return Err(invalid("registry navigation entry budget"));
            }
            let entry = entry?;
            let typ = entry.file_type()?;
            if typ.is_symlink() {
                return Err(invalid("symlink in registry navigation inputs"));
            }
            if typ.is_dir() && recursive {
                todo.push(entry.path())
            } else if typ.is_file()
                && (entry.file_name() == name
                    || name == "*.jsonl" && entry.path().extension().is_some_and(|e| e == "jsonl"))
            {
                result.push(entry.path())
            }
        }
    }
    result.sort();
    Ok(result)
}
fn relative(root: &Path, path: &Path) -> io::Result<String> {
    path.strip_prefix(root)
        .map_err(|e| invalid(e.to_string()))?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("non-UTF8 registry path"))
}
fn json_lines(context: &mut Context<'_>, path: &Path) -> io::Result<Vec<Value>> {
    let raw = context.read(path)?;
    let source = std::str::from_utf8(&raw).map_err(|e| invalid(e.to_string()))?;
    source
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| rules::decoded(s.as_bytes(), false).map_err(invalid))
        .collect()
}
pub fn reconciliation(root: &Path, cancel: &AtomicI32) -> io::Result<Value> {
    let root = kag_release::safe_absolute(root)?;
    kag_release::directory(&root)?;
    let mut context = Context::new(cancel);
    let (snapshot, summary, documents) =
        crate::source_registry::current(&mut context, &root.join(rules::PACKET))?;
    let mut by_url = BTreeMap::<String, BTreeSet<String>>::new();
    let mut by_label = by_url.clone();
    let mut by_identifier = by_url.clone();
    let mut sources = BTreeMap::new();
    for catalog in paths(
        &mut context,
        &root.join("ToS/source-witnesses/catalog"),
        "*.jsonl",
        false,
    )? {
        if catalog.file_name().is_some_and(|s| s == "claims.jsonl") {
            continue;
        }
        for entry in json_lines(&mut context, &catalog)? {
            let reference = txt(&entry["source_record_ref"])?;
            if sources.contains_key(reference) {
                continue;
            }
            let raw = context.read(&member(&root, &entry["source_record_ref"])?)?;
            let record = rules::decoded(&raw, false).map_err(invalid)?;
            let mut canonical = rules::encoded(&record).map_err(invalid)?;
            canonical.pop();
            if entry["record_sha256"] != rules::digest(&canonical) {
                return Err(invalid(format!(
                    "catalog navigation stale against owner record: {reference}"
                )));
            }
            let mut source = json!({"record_id":entry["record_id"],"record_type":entry["record_type"],"path":reference,"sha256":rules::digest(&raw),"preferred_label":entry["preferred_label"],"declared_identity_status":record.get("identity_status").unwrap_or(&entry["identity_status"])});
            if truth(&record["item_manifest_ref"]) {
                let manifest_ref = txt(&record["item_manifest_ref"])?;
                let manifest = context.json(&member(&root, &record["item_manifest_ref"])?)?;
                let mut files = Vec::new();
                for f in arr(&manifest["payload_files"])? {
                    let rel = kag_release::relative(&f["relative_path"])?;
                    let parent = Path::new(manifest_ref)
                        .parent()
                        .ok_or_else(|| invalid("manifest parent absent"))?;
                    let path = parent.join(rel);
                    files.push(json!({"path":path.to_str().ok_or_else(||invalid("non-UTF8 payload ref"))?,"expected_sha256":f["sha256"]}));
                }
                source["item_files"] = json!(files);
            }
            sources.insert(reference.to_owned(), source);
            let mut texts = Vec::new();
            strings(&record, &mut texts);
            for s in texts {
                for url in urls(&json!(s)).map_err(invalid)? {
                    by_url
                        .entry(canonical_url(&url))
                        .or_default()
                        .insert(reference.into());
                }
            }
            let mut labels = vec![record["preferred_label"].clone()];
            for v in record["variant_labels"].as_array().into_iter().flatten() {
                labels.push(v["value"].clone())
            }
            for label in labels {
                if truth(&label) {
                    by_label
                        .entry(folded(&label).map_err(invalid)?)
                        .or_default()
                        .insert(reference.into());
                }
            }
            for id in record["external_identifiers"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let s = txt(&id["value"])?;
                let s = tos_foundation::python_strip_unicode16_v1(s, s.len())
                    .map_err(|e| invalid(e.to_string()))?;
                by_identifier
                    .entry(s.into())
                    .or_default()
                    .insert(reference.into());
            }
        }
    }
    let mut records = Vec::new();
    let mut statuses = BTreeMap::<String, usize>::new();
    let mut field_statuses = BTreeMap::<(String, String), usize>::new();
    let mut matched = 0;
    for (d, name) in documents.iter().zip(arr(&summary["documents"])?) {
        for record in arr(&d["records"])? {
            context.check(1).map_err(invalid)?;
            let raw = arr(&record["raw_fields"])?
                .iter()
                .map(|f| Ok((txt(&f["source_field"])?, &f["value"])))
                .collect::<io::Result<BTreeMap<_, _>>>()?;
            let mut matches = BTreeMap::<String, BTreeSet<String>>::new();
            for value in raw.values() {
                for url in urls(value).map_err(invalid)? {
                    for reference in by_url.get(&canonical_url(&url)).into_iter().flatten() {
                        matches
                            .entry(reference.into())
                            .or_default()
                            .insert("same_reported_url".into());
                    }
                }
            }
            for field in arr(&record["reported_fields"])? {
                let target = txt(&field["target"])?;
                *field_statuses
                    .entry((target.into(), txt(&field["normalization_status"])?.into()))
                    .or_default() += 1;
                let value = &field["value"];
                if truth(value) {
                    let lookup = if matches!(target, "subject.title" | "subject.label") {
                        Some((
                            &by_label,
                            folded(value).map_err(invalid)?,
                            "same_reported_label",
                        ))
                    } else if matches!(
                        target,
                        "source.stable_identifier" | "source.witness_or_catalog_identifier"
                    ) {
                        let v = string(value).map_err(invalid)?;
                        Some((
                            &by_identifier,
                            tos_foundation::python_strip_unicode16_v1(&v, v.len())
                                .map_err(|e| invalid(e.to_string()))?
                                .into(),
                            "same_reported_identifier_string",
                        ))
                    } else {
                        None
                    };
                    if let Some((index, key, basis)) = lookup {
                        for reference in index.get(&key).into_iter().flatten() {
                            matches
                                .entry(reference.into())
                                .or_default()
                                .insert(basis.into());
                        }
                    }
                }
            }
            matched += usize::from(!matches.is_empty());
            let unresolved = arr(&record["reported_fields"])?
                .iter()
                .filter(|f| {
                    matches!(
                        f["normalization_status"].as_str(),
                        Some("partial" | "reported_unparsed")
                    )
                })
                .count();
            *statuses
                .entry(
                    if unresolved > 0 {
                        "with_unresolved_fields"
                    } else {
                        "all_fields_accounted_without_unparsed_fragments"
                    }
                    .into(),
                )
                .or_default() += 1;
            let value = |name: &str| raw.get(name).copied().unwrap_or(&Value::Null);
            let candidates=matches.iter().map(|(owner,basis)|json!({"owner_ref":owner,"basis":basis,"status":"possible_correspondence_requires_identity_review"})).collect::<Vec<_>>();
            let files = matches
                .keys()
                .filter(|r| truth(&sources[*r]["item_files"]))
                .collect::<Vec<_>>();
            records.push(json!({"record_id":record["record_id"],"source_record_id":record["source_record_id"],"corpus_id":record["corpus_id"],"document_id":record["document_id"],"kind":record["kind"],"source_record_ref":relative(&root,&snapshot.join(txt(name)?))?,"owner_matches":candidates,"readiness":{"version":{"status":"unreviewed","reported":value("version_edition_or_translation")},"access":{"status":"unverified_current","reported":value("access_mode"),"reported_checked_at":value("checked_at")},"rights":{"status":"review_required","reported_use":value("tos_use")},"file":{"status":"identity_unresolved","possible_owner_file_refs":files},"branch":{"status":"linkage_unreviewed","reported_atlas_row":value("tos_row_id")}},"unresolved_field_count":unresolved,"next_owner":"ToS/source-witnesses/discovery/","next_condition":"Review exact version and intended use against owner evidence before acquisition; matches are navigation only."}));
        }
    }
    let mut counts = json!({"records":records.len(),"records_with_owner_match_candidates":matched,"owner_records_inspected":sources.len()});
    for (key, value) in statuses {
        counts[key] = json!(value)
    }
    Ok(
        json!({"schema_version":"tos_source_registry_reconciliation_v1","snapshot_id":summary["snapshot_id"],"semantic_ceiling":"review_preparation_no_identity_or_rights_admission","owner_sources":sources.into_values().collect::<Vec<_>>(),"summary":counts,"field_status_counts":field_statuses.into_iter().map(|((target,status),count)|json!({"target":target,"status":status,"count":count})).collect::<Vec<_>>(),"records":records}),
    )
}
type Plantings = BTreeMap<String, Vec<(String, Value)>>;
pub fn assess_target(
    context: &mut Context<'_>,
    root: &Path,
    target: &Value,
    plantings: &Plantings,
    verify_local: bool,
) -> io::Result<Value> {
    let ids = &target["ids"];
    let paths = &target["paths"];
    let manifest_ref = format!("{}/item.manifest.json", txt(&paths["item_root"])?);
    let mut result = json!({"work_id":ids["work"],"edition_id":ids["edition"],"item_id":ids["item"],"title":target["title"],"work_ref":paths["work"],"item_manifest_ref":manifest_ref,"branch_planting_refs":[],"file_count":0,"intake_evidence":"not_installed","status":"prepared_version_not_installed","all_versions_or_corpus_complete":false});
    let manifest_path = member(root, &json!(manifest_ref))?;
    if !manifest_path.is_file() {
        return Ok(result);
    }
    for kind in ["work", "expression", "edition", "item"] {
        let record = context.json(&member(root, &paths[kind])?)?;
        if record["record_id"] != ids[kind] {
            return Err(invalid(
                "installed source identity differs from reviewed target",
            ));
        }
        if kind == "expression" && record["work_ref"] != ids["work"] {
            return Err(invalid("installed expression belongs to another work"));
        }
        if kind == "edition"
            && !arr(&record["embodies_expression_refs"])?.contains(&ids["expression"])
        {
            return Err(invalid("installed edition omits selected expression"));
        }
    }
    let manifest = context.json(&manifest_path)?;
    if manifest["item_id"] != ids["item"] || manifest["embodiment_ref"] != ids["edition"] {
        return Err(invalid("installed Item/Edition chain differs"));
    }
    let files = arr(&manifest["payload_files"])?;
    if files.is_empty() || files.len() != arr(&target["files"])?.len() {
        return Err(invalid("declared file set incomplete against preparation"));
    }
    let expected = arr(&target["files"])?
        .iter()
        .map(|f| Ok((txt(&f["basename"])?, f)))
        .collect::<io::Result<BTreeMap<_, _>>>()?;
    if files
        .iter()
        .map(|f| txt(&f["original_basename"]))
        .collect::<io::Result<BTreeSet<_>>>()?
        != expected.keys().copied().collect()
    {
        return Err(invalid(
            "declared filenames differ from prepared exact file set",
        ));
    }
    let events = json_lines(context, &member(root, &manifest["provenance_ref"])?)?;
    let acquired = events
        .iter()
        .filter(|e| {
            e["event_id"] == manifest["acquisition_event_ref"]
                && e["event_type"] == "acquisition"
                && matches!(
                    e["status"].as_str(),
                    Some("completed" | "completed_with_warnings")
                )
        })
        .collect::<Vec<_>>();
    if acquired.len() != 1 {
        return Err(invalid(
            "Item lacks one recorded completed acquisition event",
        ));
    }
    let output_files = arr(&acquired[0]["outputs"])?
        .iter()
        .filter(|o| truth(&o["sha256"]))
        .map(|o| Ok((txt(&o["ref"])?, txt(&o["sha256"])?)))
        .collect::<io::Result<BTreeSet<_>>>()?;
    let mut local = Vec::new();
    for f in files {
        if !output_files.contains(&(txt(&f["file_id"])?, txt(&f["sha256"])?))
            || f["byte_size"] != expected[txt(&f["original_basename"])?]["byte_size"]
        {
            return Err(invalid(
                "File is not bound by prepared size and acquired output digest",
            ));
        }
        if verify_local {
            let rel = kag_release::relative(&f["relative_path"])?;
            let path = member(
                root,
                &json!(
                    Path::new(&manifest_ref)
                        .parent()
                        .unwrap()
                        .join(rel)
                        .to_str()
                        .ok_or_else(|| invalid("non-UTF8 file ref"))?
                ),
            )?;
            let state = if !path.is_file() {
                "missing_in_this_checkout"
            } else if rules::digest(&context.read(&path)?) == f["sha256"] {
                "verified"
            } else {
                "fixity_mismatch"
            };
            local.push(json!({"path":relative(root,&path)?,"state":state}));
        }
    }
    result["file_count"] = json!(files.len());
    result["intake_evidence"] = json!("completed_acquisition_recorded");
    result["acquisition_event_ref"] = manifest["acquisition_event_ref"].clone();
    let mut branches = Vec::new();
    for (reference, planting) in plantings.get(txt(&ids["work"])?).into_iter().flatten() {
        if planting["source_witness"]["record_ref"] != paths["work"]
            || planting["status"] != "source_witness_planted"
        {
            continue;
        }
        let Some(discovery_ref) = planting["discovery_ref"].as_str().filter(|v| !v.is_empty())
        else {
            continue;
        };
        let path = member(root, &json!(discovery_ref))?;
        if !path.is_file() {
            continue;
        }
        let discovery = context.json(&path)?;
        if discovery["target"]["known_tos_refs"]
            .as_array()
            .is_some_and(|v| v.contains(&ids["item"]))
            && discovery["provenance_event_refs"]
                .as_array()
                .is_some_and(|v| v.contains(&manifest["acquisition_event_ref"]))
        {
            branches.push(reference.clone())
        }
    }
    branches.sort();
    result["status"] = json!(if branches.is_empty() {
        "acquired_version_needs_branch"
    } else {
        "selected_version_planted"
    });
    result["branch_planting_refs"] = json!(branches);
    if verify_local {
        result["local_now"] = json!({"scope":"this checkout only","state":if local.iter().all(|f|f["state"]=="verified"){"verified"}else{"needs_attention"},"files":local});
    }
    Ok(result)
}
pub fn coverage(root: &Path, verify_local: bool, cancel: &AtomicI32) -> io::Result<Value> {
    let root = kag_release::safe_absolute(root)?;
    kag_release::directory(&root)?;
    let mut context = Context::new(cancel);
    let packet = root.join(rules::PACKET);
    let reconciliation = context.json(&packet.join("reconciliation.current.json.gz"))?;
    if reconciliation["snapshot_id"] != context.json(&packet.join("current.json"))?["snapshot_id"] {
        return Err(invalid(
            "reconciliation is not tied to current registry snapshot",
        ));
    }
    let mut plantings = Plantings::new();
    let mut planting_count = 0;
    for path in paths(
        &mut context,
        &root.join("ToS/philosophy"),
        "source-planting.json",
        true,
    )? {
        let p = context.json(&path)?;
        planting_count += 1;
        if let Some(id) = p["source_witness"]["work_id"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            plantings
                .entry(id.into())
                .or_default()
                .push((relative(&root, &path)?, p));
        }
    }
    let mut selected = BTreeMap::<String, BTreeMap<String, Value>>::new();
    let mut targets = BTreeMap::new();
    for path in paths(
        &mut context,
        &root.join("ToS/source-witnesses/discovery"),
        "manifest.json",
        true,
    )? {
        let raw = context.read(&path)?;
        let manifest = rules::decoded(&raw, false).map_err(invalid)?;
        if manifest["schema_version"] != "tos_registry_first_planting_preparation_v1" {
            continue;
        }
        let checkpoint = path
            .parent()
            .unwrap()
            .join("preparation-checkpoint-receipt.json");
        if !checkpoint.is_file() {
            continue;
        }
        let receipt = context.json(&checkpoint)?;
        if receipt["status"] != "passed"
            || receipt["manifest_sha256"] != rules::digest(&raw)
            || !truth(&receipt["checkpoint_review_ref"])
        {
            return Err(invalid(
                "selected manifest lacks exact reviewed preparation receipt",
            ));
        }
        for target in arr(&manifest["targets"])? {
            let mut observed =
                assess_target(&mut context, &root, target, &plantings, verify_local)?;
            observed["preparation_ref"] = json!(relative(&root, &path)?);
            observed["review_ref"] = receipt["checkpoint_review_ref"].clone();
            let key = txt(&target["ids"]["item"])?;
            if targets.get(key).is_some_and(|v| v != &observed) {
                return Err(invalid(
                    "same exact Item has conflicting reviewed target coverage",
                ));
            }
            targets.insert(key.to_owned(), observed.clone());
            for lead in arr(&target["registry_sources"])? {
                selected
                    .entry(txt(&lead["entry_id"])?.into())
                    .or_default()
                    .insert(key.into(), observed.clone());
            }
        }
    }
    let mut records = Vec::new();
    let mut statuses = BTreeMap::<String, usize>::new();
    let mut known = BTreeSet::new();
    for r in arr(&reconciliation["records"])? {
        let id = txt(&r["record_id"])?;
        known.insert(id.to_owned());
        let selection = selected
            .get(id)
            .map(|v| v.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let planted = selection
            .iter()
            .any(|t| t["status"] == "selected_version_planted");
        let state = if planted {
            "selected_versions_planted"
        } else if !selection.is_empty() {
            "selected_versions_pending"
        } else if truth(&r["owner_matches"]) {
            "possible_owner_correspondence"
        } else {
            "not_yet_reconciled"
        };
        *statuses.entry(state.into()).or_default() += 1;
        records.push(json!({"record_id":r["record_id"],"source_record_id":r["source_record_id"],"corpus_id":r["corpus_id"],"document_id":r["document_id"],"kind":r["kind"],"status":state,"selected_targets":selection,"possible_owner_refs":r["owner_matches"].as_array().into_iter().flatten().map(|m|&m["owner_ref"]).collect::<Vec<_>>(),"lead_scope_exhausted":false,"next_condition":if planted{"Review remaining works/versions and gaps independently; a selected version never closes the whole lead."}else{"Review exact owner correspondence or finish the selected target; absence of this link is not proof that the work is absent from ToS."}}));
    }
    if selected.keys().any(|k| !known.contains(k)) {
        return Err(invalid(
            "reviewed target references absent normalized registry record",
        ));
    }
    let works = arr(&reconciliation["owner_sources"])?
        .iter()
        .filter(|r| r["record_type"] == "work")
        .map(|r| txt(&r["record_id"]))
        .collect::<io::Result<BTreeSet<_>>>()?
        .len();
    Ok(
        json!({"schema_version":"tos_registry_planting_coverage_v1","snapshot_id":reconciliation["snapshot_id"],"semantic_ceiling":"Derived navigation over registry records, acquisition evidence and current owner sources.","custody_scope":if verify_local{"live file hashes in this checkout"}else{"recorded acquisition evidence; current local existence not asserted"},"summary":{"registry_records":records.iter().filter(|r|r["kind"]=="registry").count(),"gap_records":records.iter().filter(|r|r["kind"]=="gaps").count(),"catalogued_works":works,"all_branch_plantings":planting_count,"selected_items":targets.len(),"selected_items_planted":targets.values().filter(|t|t["status"]=="selected_version_planted").count(),"selected_files_with_intake_evidence":targets.values().map(|t|t["file_count"].as_u64().unwrap_or(0)).sum::<u64>(),"record_status_counts":statuses},"targets":targets.into_values().collect::<Vec<_>>(),"records":records}),
    )
}
pub fn markdown(value: &Value) -> io::Result<String> {
    let s = &value["summary"];
    let link = |v: &Value| -> io::Result<String> {
        let reference = kag_release::relative(v)?;
        let from = rules::PACKET.split('/').collect::<Vec<_>>();
        let to = reference.split('/').collect::<Vec<_>>();
        let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
        Ok(std::iter::repeat_n("..", from.len() - common)
            .chain(to[common..].iter().copied())
            .collect::<Vec<_>>()
            .join("/"))
    };
    let mut lines=vec!["# Покрытие реестра посадками".into(),"".into(),"Проекция по исходным owner-записям, точным подготовкам и связям ветвей. Перестраивается командой `tos-source-registry coverage --source-root SOURCE_ROOT --output-root OUTPUT_ROOT`.".into(),"".into(),format!("Каталог: {} Work. Связей посадки во всём Древе: {}. В просмотренных партиях реестра: {} посаженных версий, {} файлов с записью о приобретении.",s["catalogued_works"],s["all_branch_plantings"],s["selected_items_planted"],s["selected_files_with_intake_evidence"]),"".into(),"Это отметка о сохранённом свидетельстве приобретения. Текущее наличие и SHA-256 файлов в явно выбранном source root проверяет `tos-source-registry coverage --source-root SOURCE_ROOT --verify-local`.".into(),"".into(),"Одна посаженная версия не закрывает весь корпус, все переводы или все издания. «Ещё не сопоставлено» означает отсутствие проверенной связи с этой строкой реестра; произведение может уже находиться в Древе под другой записью.".into(),"".into(),"## Посаженные выбранные версии".into(),"".into(),"| Произведение | Точная версия | Файлов | Ветвь |".into(),"| --- | --- | ---: | --- |".into()];
    for t in arr(&value["targets"])? {
        let branches = arr(&t["branch_planting_refs"])?
            .iter()
            .map(|v| Ok(format!("[посадка]({})", link(v)?)))
            .collect::<io::Result<Vec<_>>>()?
            .join(", ");
        let branch = if branches.is_empty() {
            "ожидает связи"
        } else {
            &branches
        };
        let edition = txt(&t["edition_id"])?;
        let item = if t["status"] == "prepared_version_not_installed" {
            if truth(&t["preparation_ref"]) {
                format!(
                    "{edition} ([подготовка]({}), не установлено)",
                    link(&t["preparation_ref"])?
                )
            } else {
                format!("{edition} (подготовлено, не установлено)")
            }
        } else {
            format!("[{edition}]({})", link(&t["item_manifest_ref"])?)
        };
        lines.push(format!(
            "| [{}]({}) | {item} | {} | {branch} |",
            txt(&t["title"])?,
            link(&t["work_ref"])?,
            t["file_count"]
        ));
    }
    lines.extend(["".into(),"## Где продолжать".into(),"".into(),"Только строки без подтверждённой выбранной посадки: `tos-source-registry coverage --source-root SOURCE_ROOT --remaining --document A25`. Без `--document` команда выводит все такие строки JSONL. Возможные совпадения требуют проверки идентичности; они не принимаются автоматически.".into(),"".into(),"Полное состояние всех строк и выбранных версий: `coverage.current.json.gz`. Пробелы исходного исследования (`gaps`) сохраняются отдельно от строк `registry`.".into(),"".into(),"| Досье | Строк реестра | С выбранными посадками | Возможные совпадения | Ещё не сопоставлено | Пробелов исследования |".into(),"| --- | ---: | ---: | ---: | ---: | ---: |".into()]);
    let mut counts = BTreeMap::<String, BTreeMap<String, usize>>::new();
    for r in arr(&value["records"])? {
        let c = counts.entry(txt(&r["document_id"])?.into()).or_default();
        *c.entry(txt(&r["kind"])?.into()).or_default() += 1;
        if r["kind"] == "registry" {
            *c.entry(txt(&r["status"])?.into()).or_default() += 1;
        }
    }
    for (doc, c) in counts {
        let n = |k: &str| c.get(k).copied().unwrap_or(0);
        lines.push(format!(
            "| {doc} | {} | {} | {} | {} | {} |",
            n("registry"),
            n("selected_versions_planted"),
            n("possible_owner_correspondence"),
            n("not_yet_reconciled"),
            n("gaps")
        ));
    }
    Ok(lines.join("\n") + "\n")
}
pub fn publish_view(
    source: &Path,
    output: &Path,
    value: &Value,
    coverage: bool,
    check: bool,
    cancel: &AtomicI32,
) -> io::Result<()> {
    use std::io::Write;
    let source = kag_release::safe_absolute(source)?;
    kag_release::directory(&source)?;
    let output = kag_release::safe_absolute(output)?.join(rules::PACKET);
    kag_release::safe_absolute(&output)?;
    if output.starts_with(&source) || source.starts_with(&output) {
        return Err(invalid("output root would mutate selected source root"));
    }
    let name = if coverage {
        "coverage.current.json.gz"
    } else {
        "reconciliation.current.json.gz"
    };
    let mut outputs = vec![(
        name,
        rules::compressed(&rules::encoded(value).map_err(invalid)?).map_err(invalid)?,
    )];
    if coverage {
        outputs.push(("COVERAGE.md", markdown(value)?.into_bytes()));
    }
    let mut context = Context::new(cancel);
    for (name, body) in outputs {
        let path = output.join(name);
        context.check(body.len() as u64).map_err(invalid)?;
        if check {
            if !crate::source_registry::outputs_equal(&mut context, &path, &body)? {
                return Err(invalid(format!(
                    "registry navigation projection is stale: {name}"
                )));
            }
        } else {
            fs::create_dir_all(&output)?;
            let temp = output.join(format!(".{name}.{}.tmp", std::process::id()));
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            f.write_all(&body)?;
            f.sync_all()?;
            fs::rename(temp, &path)?;
            fs::File::open(&output)?.sync_all()?;
        }
    }
    Ok(())
}
