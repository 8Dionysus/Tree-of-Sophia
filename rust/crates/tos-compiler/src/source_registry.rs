//! Reproducible research registry snapshots. Records remain reported and
//! unreviewed; correspondences never admit identity, access or rights.
pub mod ooxml;
pub mod values;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use tos_foundation::{CanonicalProfile, Digest256Hasher, JsonLimits, canonical_raw_bytes_v1};

pub type Result<T> = std::result::Result<T, String>;
pub type Check<'a> = &'a mut dyn FnMut(u64) -> Result<()>;
pub const PACKET: &str = "ToS/research-packets/source-registries";
pub const MAX_FILE: usize = 256 * 1024 * 1024;
pub fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| "expected array".into())
}
pub fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| "expected string".into())
}
pub fn digest(raw: &[u8]) -> String {
    let mut h = Digest256Hasher::new();
    h.update(raw);
    h.finalize().to_hex()
}
pub fn encoded(v: &Value) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_FILE,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| e.to_string())
}
pub fn decoded(raw: &[u8], gzip: bool) -> Result<Value> {
    let body = if gzip {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(raw)
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string())?;
        out
    } else {
        raw.to_vec()
    };
    if body.len() > MAX_FILE {
        return Err("registry JSON decoded byte limit".into());
    }
    canonical_raw_bytes_v1(
        &body,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_FILE,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| e.to_string())?;
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}
pub fn compressed(raw: &[u8]) -> Result<Vec<u8>> {
    let mut gzip = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::best());
    gzip.write_all(raw).map_err(|e| e.to_string())?;
    gzip.finish().map_err(|e| e.to_string())
}
pub fn string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        _ => v.to_string(),
    }
}
pub fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
        Value::Number(v) => v.as_f64() != Some(0.0),
    }
}
pub fn folded(v: &Value) -> Result<String> {
    let s = values::fold(&string(v))?;
    Ok(tos_foundation::python_strip_unicode16_v1(&s, s.len())
        .map_err(|e| e.to_string())?
        .into())
}
pub fn canonical_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.into();
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return url.into();
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() {
        return url.into();
    }
    let normalized = match authority.rsplit_once('@') {
        Some((user, host)) => format!("{user}@{}", host.to_lowercase()),
        None => authority.to_lowercase(),
    };
    format!(
        "{}://{normalized}{}",
        scheme.to_ascii_lowercase(),
        &rest[end..]
    )
}
pub fn urls(v: &Value) -> Result<Vec<String>> {
    let s = if truth(v) { string(v) } else { String::new() };
    Ok(array(&values::links(&s)?["value"])?
        .iter()
        .map(|v| text(&v["url"]).map(str::to_owned))
        .collect::<Result<_>>()?)
}
pub struct DocumentInput {
    pub spec: Value,
    pub files: Value,
    pub xlsx: Vec<u8>,
    pub docx: Vec<u8>,
    pub profile: Value,
}
pub struct Snapshot {
    pub summary: Value,
    pub outputs: BTreeMap<String, Vec<u8>>,
}
pub fn delta(old: &[Value], new: &[Value]) -> Result<Value> {
    fn documents(values: &[Value]) -> Result<BTreeMap<(String, String), &Value>> {
        values
            .iter()
            .map(|v| {
                Ok((
                    (
                        text(&v["corpus_id"])?.into(),
                        text(&v["document_id"])?.into(),
                    ),
                    v,
                ))
            })
            .collect()
    }
    fn records(values: &[Value]) -> Result<BTreeMap<String, &Value>> {
        let mut result = BTreeMap::new();
        for d in values {
            for r in array(&d["records"])? {
                let id = text(&r["record_id"])?;
                if result.insert(id.into(), r).is_some() {
                    return Err("duplicate record in snapshot".into());
                }
            }
        }
        Ok(result)
    }
    fn blocks(v: Option<&&Value>) -> Result<BTreeMap<(String, String), Value>> {
        let mut result = BTreeMap::new();
        if let Some(v) = v {
            for p in array(&v["report_parts"])? {
                for b in array(&p["blocks"])? {
                    let mut block = b.as_object().ok_or("invalid block")?.clone();
                    block.remove("occurrence_id");
                    result.insert(
                        (text(&p["part"])?.into(), text(&b["xml_path"])?.into()),
                        Value::Object(block),
                    );
                }
            }
        }
        Ok(result)
    }
    fn fields(v: &Value) -> Result<BTreeMap<String, Value>> {
        content_fields(array(&v["raw_fields"])?)
            .into_iter()
            .map(|v| Ok((text(&v["source_field"])?.into(), v)))
            .collect()
    }
    let (before, after) = (records(old)?, records(new)?);
    let (old_docs, new_docs) = (documents(old)?, documents(new)?);
    let mut files = Vec::new();
    let mut reports = Vec::new();
    for key in old_docs
        .keys()
        .chain(new_docs.keys())
        .collect::<BTreeSet<_>>()
    {
        let old = old_docs.get(key);
        let new = new_docs.get(key);
        for kind in ["xlsx", "docx"] {
            let b = old.map_or(&Value::Null, |v| &v["files"][kind]["sha256"]);
            let a = new.map_or(&Value::Null, |v| &v["files"][kind]["sha256"]);
            if b != a {
                files.push(json!({"corpus_id":key.0,"document_id":key.1,"kind":kind,"before_sha256":b,"after_sha256":a}));
            }
        }
        let (b, a) = (blocks(old)?, blocks(new)?);
        let changed = b
            .keys()
            .chain(a.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|k| b.get(*k) != a.get(*k))
            .map(|k| json!([k.0, k.1]))
            .collect::<Vec<_>>();
        if !changed.is_empty() {
            reports.push(
                json!({"corpus_id":key.0,"document_id":key.1,"changed_block_locators":changed}),
            );
        }
    }
    let mut changed = Vec::new();
    let mut relocated = Vec::new();
    let mut unchanged = 0;
    for (id, a) in &after {
        if let Some(b) = before.get(id) {
            if a["content_sha256"] == b["content_sha256"] {
                unchanged += 1;
                if a["source"]["row"] != b["source"]["row"] {
                    relocated.push(id);
                }
            } else {
                let (bf, af) = (fields(b)?, fields(a)?);
                let fields = bf
                    .keys()
                    .chain(af.keys())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .filter(|k| bf.get(*k) != af.get(*k))
                    .collect::<Vec<_>>();
                changed.push(json!({"record_id":id,"previous_occurrence":b["occurrence_id"],"current_occurrence":a["occurrence_id"],"changed_fields":fields}));
            }
        }
    }
    Ok(
        json!({"files":files,"reports":reports,"added":after.keys().filter(|k|!before.contains_key(*k)).collect::<Vec<_>>(),"removed":before.keys().filter(|k|!after.contains_key(*k)).collect::<Vec<_>>(),"changed":changed,"relocated":relocated,"unchanged_count":unchanged}),
    )
}
fn content_fields(fields: &[Value]) -> Vec<Value> {
    fields
        .iter()
        .map(|field| {
            let mut f = field.as_object().expect("constructed field").clone();
            f.remove("cell");
            f.remove("style");
            Value::Object(f)
        })
        .collect()
}
fn word(c: char) -> bool {
    c == '_' || c == '-' || tos_foundation::python_word_unicode16_v1(c)
}
fn mentions(input: &str, source_map: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut ids = source_map.keys().collect::<Vec<_>>();
    ids.sort_by_key(|v| std::cmp::Reverse(v.len()));
    let mut found = BTreeSet::new();
    let mut skip = 0;
    for (at, _) in input.char_indices() {
        if at < skip || input[..at].chars().next_back().is_some_and(word) {
            continue;
        }
        for id in &ids {
            if input[at..].starts_with(id.as_str())
                && input[at + id.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !word(c))
            {
                found.extend(source_map[*id].iter().cloned());
                skip = at + id.len();
                break;
            }
        }
    }
    found
}
pub fn normalize_document(input: DocumentInput, check: Check<'_>) -> Result<Value> {
    let DocumentInput {
        spec,
        files,
        xlsx,
        docx,
        profile,
    } = input;
    values::validate_profile(&profile)?;
    let rules = array(&spec["sheets"])?;
    let mut sheet_rules = BTreeMap::new();
    for rule in rules {
        if sheet_rules.insert(text(&rule["name"])?, rule).is_some() {
            return Err("duplicate sheet rule".into());
        }
    }
    let mut records = Vec::new();
    let mut sheets = Vec::new();
    let mut explicit = BTreeSet::new();
    let mut anonymous = BTreeMap::<(String, String), usize>::new();
    let workbook = ooxml::workbook(&xlsx, check)?;
    for sheet in array(&workbook)? {
        check(1)?;
        let name = text(&sheet["name"])?;
        let rule = sheet_rules
            .get(name)
            .ok_or_else(|| format!("unmapped sheet: {name}"))?;
        let header_number = rule["header_row"].as_u64().unwrap_or(1);
        let header = array(&sheet["rows"])?
            .iter()
            .find(|r| r["row"].as_u64() == Some(header_number))
            .ok_or("missing header row")?;
        let mut headers = BTreeMap::new();
        let mut names = BTreeSet::new();
        for cell in array(&header["cells"])? {
            let name = &cell["value"];
            if !name.is_null() && name != "" && !names.insert(string(name)) {
                return Err("duplicate source field header".into());
            }
            headers.insert(ooxml::column_index(text(&cell["cell"])?)?, name);
        }
        let kind = text(&rule["kind"])?;
        let start = records.len();
        for row in array(&sheet["rows"])? {
            check(1)?;
            if row["row"].as_u64().ok_or("invalid row")? <= header_number
                || !array(&row["cells"])?
                    .iter()
                    .any(|c| !c["value"].is_null() || !c["formula"].is_null())
            {
                continue;
            }
            let cells = array(&row["cells"])?
                .iter()
                .map(|c| Ok((ooxml::column_index(text(&c["cell"])?)?, c)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            let columns = headers
                .keys()
                .chain(cells.keys())
                .copied()
                .collect::<BTreeSet<_>>();
            let mut raw = Vec::new();
            let mut fields = Vec::new();
            let mut dates = BTreeSet::new();
            for col in columns {
                let name = headers.get(&col).copied().unwrap_or(&Value::Null);
                let cell=cells.get(&col).map(|c|(*c).clone()).unwrap_or_else(||json!({"cell":format!("{}{}",ooxml::column_name(col),row["row"]),"value":null,"type":"absent","xml_value":null,"style":null,"formula":null}));
                if name.is_null() || name == "" {
                    if !cell["value"].is_null() || !cell["formula"].is_null() {
                        return Err("populated unnamed field".into());
                    }
                    continue;
                }
                let name = text(name)?.to_owned();
                if cell["type"] == "excel_datetime" {
                    dates.insert(name.clone());
                }
                raw.push((name.clone(), cell["value"].clone()));
                let mut field = cell.as_object().ok_or("invalid cell")?.clone();
                field.insert("source_field".into(), json!(name));
                fields.push(Value::Object(field));
            }
            let values = values::normalize(kind, &raw, &profile, &dates)?;
            let source_ids = profile[kind]
                .as_object()
                .ok_or("missing profile kind")?
                .iter()
                .filter(|(_, r)| r["target"] == "identity.source_record_id")
                .filter_map(|(k, _)| raw.iter().find(|(n, _)| n == k).map(|(_, v)| v))
                .filter(|v| !v.is_null() && *v != "")
                .map(string)
                .collect::<BTreeSet<_>>();
            if source_ids.len() > 1 {
                return Err("conflicting source ids".into());
            }
            let content = digest(&encoded(&json!(content_fields(&fields)))?);
            let scope = format!(
                "{}/{}/{kind}",
                text(&spec["corpus_id"])?,
                text(&spec["document_id"])?
            );
            let mut issues = array(&values["issues"])?.clone();
            let source_id = source_ids.into_iter().next();
            let mut local = if let Some(id) = &source_id {
                if !explicit.insert((scope.clone(), id.clone())) {
                    return Err(format!("duplicate source record id: {scope}/{id}"));
                }
                format!("source-{}", &digest(id.as_bytes())[..24])
            } else {
                let n = anonymous
                    .entry((scope.clone(), content.clone()))
                    .or_default();
                *n += 1;
                issues.push(json!("source_record_id_absent_content_identity_only"));
                format!("anonymous-{}-{n}", &content[..24])
            };
            let n = anonymous
                .get(&(scope.clone(), content.clone()))
                .copied()
                .unwrap_or(1);
            if let Some(id) = spec["identity_overrides"][format!("{kind}:{content}:{n}")].as_str() {
                local = id.into()
            }
            let locator = json!({"original_sha256":files["xlsx"]["sha256"],"part":sheet["part"],"sheet":sheet["name"],"row":row["row"]});
            records.push(json!({"record_id":format!("tos-registry:{scope}/{local}"),"source_record_id":source_id,"corpus_id":spec["corpus_id"],"document_id":spec["document_id"],"kind":kind,"occurrence_id":format!("tos-occurrence:{}",digest(&encoded(&locator)?)),"source":locator,"row_attributes":row["attributes"],"content_sha256":content,"raw_fields":fields,"reported_fields":values["reported_fields"],"issues":issues,"assessment_status":"reported_unreviewed"}));
        }
        sheets.push(json!({"name":name,"part":sheet["part"],"kind":kind,"state":sheet["state"],"header_cells":header["cells"],"hyperlinks":sheet["hyperlinks"],"columns":sheet["columns"],"record_count":records.len()-start,"merges":sheet["merges"]}));
    }
    if sheet_rules.keys().copied().collect::<BTreeSet<_>>()
        != sheets
            .iter()
            .map(|s| text(&s["name"]))
            .collect::<Result<BTreeSet<_>>>()?
    {
        return Err("declared worksheet absent".into());
    }
    for r in &mut records {
        let scope = format!(
            "{}/{}/{}",
            text(&spec["corpus_id"])?,
            text(&spec["document_id"])?,
            text(&r["kind"])?
        );
        if r["source_record_id"].is_null()
            && anonymous
                .get(&(scope, text(&r["content_sha256"])?.into()))
                .is_some_and(|n| *n > 1)
        {
            r["issues"]
                .as_array_mut()
                .unwrap()
                .push(json!("indistinguishable_anonymous_occurrences"));
        }
    }
    let mut reports = ooxml::docx(&docx, check)?;
    let mut source_map = BTreeMap::<String, Vec<String>>::new();
    for r in &records {
        if let Some(id) = r["source_record_id"].as_str().filter(|s| !s.is_empty()) {
            source_map
                .entry(id.into())
                .or_default()
                .push(text(&r["record_id"])?.into())
        }
    }
    for part in reports.as_array_mut().ok_or("invalid reports")? {
        let name = part["part"].clone();
        for block in part["blocks"].as_array_mut().ok_or("invalid blocks")? {
            check(1)?;
            block["occurrence_id"] = json!(format!(
                "tos-occurrence:{}",
                digest(&encoded(&json!([
                    files["docx"]["sha256"],
                    name,
                    block["xml_path"]
                ]))?)
            ));
            block["record_mentions"] = json!(mentions(text(&block["text"])?, &source_map));
        }
    }
    Ok(
        json!({"corpus_id":spec["corpus_id"],"document_id":spec["document_id"],"files":files,"sheets":sheets,"records":records,"report_parts":reports,"pair_relation":"manifest_declared_research_companions"}),
    )
}
pub fn build(
    manifest: Value,
    profiles: Value,
    processor: Value,
    inputs: Vec<DocumentInput>,
    check: Check<'_>,
) -> Result<Snapshot> {
    let mut corpora = BTreeSet::new();
    for c in array(&manifest["corpora"])? {
        if !corpora.insert(text(&c["corpus_id"])?.to_owned()) {
            return Err("duplicate corpus namespace".into());
        }
    }
    let namespace =
        regex::Regex::new(r"\A[A-Za-z0-9][A-Za-z0-9.-]*\z").map_err(|e| e.to_string())?;
    let mut seen = BTreeSet::new();
    let mut documents = Vec::new();
    let mut originals = BTreeSet::new();
    for input in inputs {
        let corpus = text(&input.spec["corpus_id"])?;
        let id = text(&input.spec["document_id"])?;
        if !corpora.contains(corpus) || !namespace.is_match(corpus) || !namespace.is_match(id) {
            return Err("invalid corpus/document namespace".into());
        }
        if !seen.insert((corpus.to_owned(), id.to_owned())) {
            return Err("duplicate document identity".into());
        }
        for kind in ["xlsx", "docx"] {
            originals.insert(text(&input.files[kind]["original_path"])?.to_owned());
        }
        documents.push(normalize_document(input, check)?);
    }
    let mut links = BTreeMap::<String, Vec<Value>>::new();
    let mut titles = BTreeMap::<String, BTreeSet<String>>::new();
    let mut ids = BTreeSet::new();
    let mut counts = BTreeMap::<String, usize>::new();
    let mut paragraphs = 0;
    let mut tables = 0;
    for document in &documents {
        for r in array(&document["records"])? {
            check(1)?;
            let id = text(&r["record_id"])?;
            if !ids.insert(id) {
                return Err("duplicate normalized record identity".into());
            }
            *counts.entry(text(&r["kind"])?.into()).or_default() += 1;
            for field in array(&r["raw_fields"])? {
                for url in urls(&field["value"])? {
                    links.entry(canonical_url(&url)).or_default().push(json!({"record_id":id,"source_field":field["source_field"],"cell":field["cell"],"original_url":url}));
                }
            }
            for f in array(&r["reported_fields"])? {
                if matches!(
                    f["target"].as_str(),
                    Some("subject.title" | "subject.label")
                ) && truth(&f["value"])
                {
                    titles
                        .entry(folded(&f["value"])?)
                        .or_default()
                        .insert(id.into());
                }
            }
        }
        for p in array(&document["report_parts"])? {
            for b in array(&p["blocks"])? {
                paragraphs += usize::from(b["kind"] == "paragraph");
                tables += usize::from(b["kind"] == "table");
            }
        }
    }
    let link_index=links.into_iter().map(|(url,occurrences)|json!({"link_id":format!("tos-research-link:{}",digest(url.as_bytes())),"url":url,"occurrences":occurrences,"assessment_status":"reported_unreviewed"})).collect::<Vec<_>>();
    let correspondences=titles.into_iter().filter(|(_,ids)|ids.len()>1).map(|(label,ids)|json!({"basis":"same_reported_title","label":label,"record_ids":ids,"status":"unreviewed_possible_correspondence_no_identity_merge"})).collect::<Vec<_>>();
    let snapshot_id = digest(&encoded(
        &json!({"manifest":manifest,"profiles":profiles,"processor":processor,"originals":originals}),
    )?);
    counts.extend([
        ("documents".into(), documents.len()),
        ("docx".into(), documents.len()),
        ("paragraphs".into(), paragraphs),
        ("tables".into(), tables),
        ("links".into(), link_index.len()),
    ]);
    let names = documents
        .iter()
        .map(|d| {
            Ok(format!(
                "documents/{}/{}.json.gz",
                text(&d["corpus_id"])?,
                text(&d["document_id"])?
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let summary = json!({"schema_version":"tos_source_registry_snapshot_v1","snapshot_id":snapshot_id,"semantic_ceiling":"research_reported_unreviewed","manifest":manifest,"profiles":profiles,"processor_sha256":processor,"counts":counts,"documents":names});
    let mut outputs = BTreeMap::from([
        ("snapshot.json".into(), encoded(&summary)?),
        (
            "links.json.gz".into(),
            compressed(&encoded(&json!(link_index))?)?,
        ),
        (
            "correspondences.json.gz".into(),
            compressed(&encoded(&json!(correspondences))?)?,
        ),
    ]);
    for (document, name) in documents.iter().zip(names) {
        let body = encoded(document)?;
        check(body.len() as u64)?;
        outputs.insert(name, compressed(&body)?);
    }
    Ok(Snapshot { summary, outputs })
}
