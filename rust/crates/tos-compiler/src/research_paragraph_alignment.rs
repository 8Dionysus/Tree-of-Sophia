//! Native deterministic paragraph-alignment proposal materialization.
//! Text layers remain private; machine grouping never admits translation truth.
//!
//! Entry: `tos zarathustra-de-ru-paragraph-alignment-v1 --source-root ROOT ACTION`.
//! Actions preserve the frozen builder's `--import-primary DIR`,
//! `--import-challenger DIR`, `--issue-identities`, `--build`, `--check`, and
//! `--validate-tracked` contract. `--validate-tracked` needs no private payloads;
//! `--check` reconstructs private layers and checks exact bytes and mode 0600.
//!
//! German anchors come from tracked DTA packets. Russian text is reconstructed
//! by the native Antonovsky structural compiler, joined in tracked paragraph
//! order, and bound to the tracked exact hashes. Position selectors count Unicode
//! scalar values, matching Python Unicode code-point offsets for admitted UTF-8.
//! Stable identities are loaded from the issuance packet, never recomputed from
//! text or labels. Only explicit issuance mints new opaque OS-random identifiers.
//!
//! The embedded templates own static v1 serialization constants only. Coverage,
//! challenger comparisons, risk unions, identity closure, packet schema checks,
//! and all anchors/alignments are computed from exact inputs before output.
//! Historical v1 maker references and timestamps remain unchanged for byte parity;
//! this implementation does not claim a new source-visible assessment.
//!
//! The frozen v1 maker reference remains provenance only; exact recipe bytes are
//! preserved under `ToS/research-packets/retained-builder-inputs/`. The public metadata and
//! private text maps returned by `build` stay distinct through write/check.
//! Rust unit tests cover null-side and outside-scope uncertainty, nested private
//! text rejection, serialization, and Unicode offsets. Production census/parity
//! verification belongs to the bounded owner data operation, not ordinary CI.
use crate::research_execution::ResearchExecution;
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tos_foundation::Digest256;
type Result<T> = std::result::Result<T, String>;
type Files = BTreeMap<String, Vec<u8>>;
const WORK: &str = "tos.work.friedrich-nietzsche.also-sprach-zarathustra";
const BASE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
const DE: &str = "technical-markup/dta-first-editions-parts-1-4-v1";
const RU: &str = "technical-markup/antonovsky-1911-structural-paragraph-v2";
const ROUTE: &str = "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1";
const PRIVATE: &str = "gold-sets/foundation-pilot-v1/local-content/translation-alignment-v1";
const SCHEMA: &str = "ToS/contracts/translation-alignment-packet-v1.schema.json";
const LABELS: [&str; 4] = ["I", "II", "III", "IV"];
const READINGS: [usize; 4] = [23, 22, 16, 20];
fn at(route: &str, name: &str) -> String {
    format!("{BASE}/{route}/{name}")
}
fn output(name: &str) -> String {
    at(ROUTE, name)
}
fn packet_ref(part: usize) -> String {
    output(&format!("part-{part}.translation-alignment-packet.v1.json"))
}
fn relative(reference: &str) -> Result<&Path> {
    let path = Path::new(reference);
    ensure(
        !reference.is_empty()
            && path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "source reference must be root-relative without traversal",
    )?;
    Ok(path)
}
fn source_file(repo: &ResearchExecution, reference: &str) -> Result<fs::File> {
    repo.source_file(reference, u64::MAX)
}
fn bytes(repo: &ResearchExecution, reference: &str) -> Result<Vec<u8>> {
    repo.read(reference)
}

fn load(repo: &ResearchExecution, reference: &str) -> Result<Value> {
    serde_json::from_slice(&bytes(repo, reference)?).map_err(|e| format!("{reference}: {e}"))
}
fn lines(repo: &ResearchExecution, reference: &str) -> Result<Vec<Value>> {
    let b = bytes(repo, reference)?;
    let t = std::str::from_utf8(&b).map_err(|e| e.to_string())?;
    t.lines()
        .filter(|s| !s.is_empty())
        .map(|s| {
            repo.tick(1)?;
            serde_json::from_str(s).map_err(|e| format!("{reference}: {e}"))
        })
        .collect()
}
fn sha(b: &[u8]) -> String {
    Digest256::of_bytes(b).to_hex()
}
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(k, v)| (k.clone(), sorted(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        _ => v.clone(),
    }
}
fn pretty(repo: &ResearchExecution, v: &Value) -> Result<Vec<u8>> {
    repo.check()?;
    let mut b = serde_json::to_vec_pretty(&sorted(v)).map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(b)
}
fn jsonl(repo: &ResearchExecution, v: &[Value]) -> Result<Vec<u8>> {
    repo.check()?;
    let mut b = Vec::new();
    for r in v {
        repo.tick(1)?;
        serde_json::to_writer(&mut b, &sorted(r)).map_err(|e| e.to_string())?;
        b.push(b'\n');
    }
    Ok(b)
}
fn arr(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| "expected array".into())
}
fn s(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| "expected string".into())
}
fn n(v: &Value) -> Result<usize> {
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "expected nonnegative integer".into())
}
fn flag(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| "expected boolean".into())
}
fn strings(v: &Value) -> Result<Vec<String>> {
    arr(v)?.iter().map(|v| s(v).map(str::to_owned)).collect()
}
fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn text_free(repo: &ResearchExecution, v: &Value) -> Result<()> {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                repo.tick(1)?;
                ensure(
                    ![
                        "text",
                        "source_text",
                        "target_text",
                        "content",
                        "excerpt",
                        "quote",
                    ]
                    .contains(&k.to_lowercase().as_str()),
                    "text-bearing key in metadata",
                )?;
                if ["source_text_included", "target_text_included"].contains(&k.as_str()) {
                    ensure(v == &json!(false), "text inclusion flag must be false")?;
                }
                text_free(repo, v)?;
            }
        }
        Value::Array(a) => {
            for v in a {
                repo.tick(1)?;
                text_free(repo, v)?;
            }
        }
        _ => (),
    }
    Ok(())
}
fn count(repo: &ResearchExecution, rows: &[Value], key: &str) -> Result<Value> {
    let mut m = BTreeMap::<String, usize>::new();
    for r in rows {
        repo.tick(1)?;
        *m.entry(s(&r[key])?.into()).or_default() += 1;
    }
    Ok(json!(m))
}
fn template(name: &str) -> Value {
    let all: Value =
        serde_json::from_str(include_str!("research_paragraph_alignment_templates.json"))
            .expect("embedded templates");
    all[name].clone()
}
#[derive(Clone)]
struct Layer {
    reference: String,
    payload: Vec<u8>,
    digest: String,
    records: Vec<Value>,
}
pub struct Material {
    primary: Vec<Value>,
    challenger: Vec<Value>,
    de_plan: Value,
    ru_plan: Value,
    de_packets: BTreeMap<usize, Value>,
    de_units: BTreeMap<usize, Vec<Value>>,
    pub source_anchors: BTreeMap<String, Value>,
    layers: BTreeMap<usize, Layer>,
}
/// Exact native structural read closure, derived from the selected plan/item.
/// This lists physical inputs, never a subtree or inferred historical source cut.
pub fn structural_read_dependencies(repo: &ResearchExecution, plan: &Value) -> Result<Vec<String>> {
    repo.check()?;
    let source = &plan["source_item"];
    let manifest_ref = s(&source["manifest_ref"])?;
    let manifest = load(repo, manifest_ref)?;
    let entries = arr(&manifest["payload_files"])?;
    let selected: Vec<_> = entries
        .iter()
        .filter(|r| r["file_id"] == source["file_ref"])
        .collect();
    ensure(selected.len() == 1, "structural PDF manifest closure drift")?;
    let pdf = Path::new(manifest_ref)
        .parent()
        .ok_or("structural manifest parent missing")?
        .join(relative(s(&selected[0]["relative_path"])?)?)
        .to_str()
        .ok_or("structural PDF reference UTF8")?
        .to_owned();
    Ok(vec![
        at(
            "technical-markup/antonovsky-1911-pdf-layout-v1",
            "plan.v1.json",
        ),
        manifest_ref.into(),
        s(&source["resource_inventory_ref"])?.into(),
        at(
            "technical-markup/antonovsky-1911-pdf-layout-v1",
            "citation-spine.v1.jsonl",
        ),
        at(RU, "structure-census.v2.jsonl"),
        pdf,
    ])
}
/// Reconstruct exact private paragraph bytes using the native structural compiler.
pub fn reconstruct_scoped(repo: &ResearchExecution) -> Result<Material> {
    let primary = lines(repo, &output("primary-proposals-input.v1.jsonl"))?;
    let challenger = lines(repo, &output("independent-challenger-input.v1.jsonl"))?;
    ensure(primary.len() == 3423, "primary mapping census drift")?;
    let de_plan = load(repo, &at(DE, "plan.v1.json"))?;
    let ru_plan = load(
        repo,
        &at(
            "technical-markup/antonovsky-1911-pdf-layout-v1",
            "plan.v1.json",
        ),
    )?;
    let mut de_packets = BTreeMap::new();
    let mut de_units = BTreeMap::new();
    let mut source_anchors = BTreeMap::new();
    for part in 1..=4 {
        repo.tick(1)?;
        let packet = load(
            repo,
            &at(DE, &format!("part-{part}.source-text-unit.v1.json")),
        )?;
        let reference = s(&packet["source_layer"]["text_layer_ref"])?;
        let raw = bytes(repo, reference)?;
        ensure(
            sha(&raw) == s(&packet["source_layer"]["text_layer_sha256"])?,
            "German private source layer drift",
        )?;
        mode600(repo, reference)?;
        let anchors: BTreeMap<String, Value> = arr(&packet["anchors"])?
            .iter()
            .map(|a| Ok((s(&a["anchor_ref"])?.into(), a.clone())))
            .collect::<Result<_>>()?;
        let mut units = Vec::new();
        for u in arr(&packet["units"])? {
            repo.tick(1)?;
            if u["unit_kind"] == "paragraph" {
                let refs = strings(&u["ordered_anchor_refs"])?;
                ensure(
                    refs.len() == 1,
                    "German paragraph does not have exactly one anchor",
                )?;
                let anchor = anchors
                    .get(&refs[0])
                    .ok_or("German paragraph anchor missing")?;
                source_anchors.insert(s(&u["unit_id"])?.into(), anchor.clone());
                units.push(u.clone());
            }
        }
        de_units.insert(part, units);
        de_packets.insert(part, packet);
    }
    repo.reserve_structural_reads()?;
    let model = crate::antonovsky_structural::reconstruct_from_directory(
        repo.root(),
        repo.root_directory(),
        repo.deadline(),
    )?;
    let mut charged = 0;
    repo.charge_structural(&model, &mut charged)?;
    repo.check()?;
    let tracked = lines(repo, &at(RU, "paragraph-spine.v2.jsonl"))?;
    ensure(
        tracked.len() == 3569 && tracked.len() == model.paragraphs.len(),
        "Russian paragraph census drift",
    )?;
    let row_text: BTreeMap<_, _> = model
        .rows
        .iter()
        .map(|r| (r.reference.as_str(), r.text.as_str()))
        .collect();
    let mut by_part: BTreeMap<usize, Vec<Value>> = BTreeMap::new();
    for (raw, row) in model.paragraphs.iter().zip(tracked.iter()) {
        repo.tick(1)?;
        let texts = strings(&raw["row_refs"])?;
        let text = texts
            .iter()
            .map(|r| {
                row_text
                    .get(r.as_str())
                    .copied()
                    .ok_or("Russian row reference missing")
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n");
        ensure(
            sha(text.as_bytes()) == s(&row["exact_sha256"])?,
            "Russian paragraph private-text binding drift",
        )?;
        let part = part_id(&row["part_id"])?;
        by_part.entry(part).or_default().push(json!({"unit_id":row["paragraph_unit_id"],"citation":row["display_citation"],"reading_unit_ref":row["reading_unit_ref"],"text":text,"exact_sha256":row["exact_sha256"]}));
    }
    let mut layers = BTreeMap::new();
    let mut target_ids = BTreeSet::new();
    for part in 1..=4 {
        repo.tick(1)?;
        let rows = by_part.remove(&part).ok_or("missing Russian part")?;
        let mut text = String::new();
        let mut position = 0;
        let mut records = Vec::new();
        for (i, mut row) in rows.into_iter().enumerate() {
            repo.tick(1)?;
            if i > 0 {
                text.push_str("\n\n");
                position += 2;
            }
            let start = position;
            let t = s(&row["text"])?;
            position += t.chars().count();
            text.push_str(t);
            row["ordinal"] = json!(i + 1);
            row["start"] = json!(start);
            row["end"] = json!(position);
            target_ids.insert(s(&row["unit_id"])?.to_owned());
            records.push(row);
        }
        let payload = text.into_bytes();
        layers.insert(
            part,
            Layer {
                reference: at(
                    PRIVATE,
                    &format!("antonovsky-1911-part-{part}-paragraphs.v1.txt"),
                ),
                digest: sha(&payload),
                payload,
                records,
            },
        );
    }
    exact_coverage(
        repo,
        &primary,
        "source_paragraph_refs",
        3447,
        &source_anchors.keys().cloned().collect(),
    )?;
    exact_coverage(repo, &primary, "target_paragraph_refs", 3569, &target_ids)?;
    repo.charge_structural(&model, &mut charged)?;
    Ok(Material {
        primary,
        challenger,
        de_plan,
        ru_plan,
        de_packets,
        de_units,
        source_anchors,
        layers,
    })
}
fn part_id(v: &Value) -> Result<usize> {
    s(v)?
        .split('_')
        .nth(1)
        .ok_or("invalid part id")?
        .parse()
        .map_err(|_| "invalid part id".into())
}
fn exact_coverage(
    repo: &ResearchExecution,
    rows: &[Value],
    key: &str,
    total: usize,
    expected: &BTreeSet<String>,
) -> Result<()> {
    let mut refs = Vec::new();
    for r in rows {
        repo.tick(1)?;
        refs.extend(strings(&r[key])?);
    }
    let unique: BTreeSet<_> = refs.iter().cloned().collect();
    ensure(
        refs.len() == total && unique.len() == total && &unique == expected,
        "paragraph coverage is not exact",
    )
}
fn mode600(repo: &ResearchExecution, reference: &str) -> Result<()> {
    #[cfg(unix)]
    {
        let m = source_file(repo, reference)?
            .metadata()
            .map_err(|e| e.to_string())?;
        ensure(
            m.is_file() && m.permissions().mode() & 0o7777 == 0o600,
            "private source layer mode is not 0600",
        )?;
    }
    Ok(())
}

fn bindings(repo: &ResearchExecution, m: &Material) -> Result<BTreeMap<String, Vec<String>>> {
    repo.check()?;
    let mut b = BTreeMap::new();
    b.insert(
        "packets".into(),
        (1..=4).map(|p| format!("part-{p}")).collect(),
    );
    b.insert(
        "target_anchors".into(),
        m.layers
            .values()
            .flat_map(|l| l.records.iter())
            .map(|r| s(&r["unit_id"]).map(str::to_owned))
            .collect::<Result<_>>()?,
    );
    let refs = m
        .primary
        .iter()
        .map(|r| s(&r["alignment_proposal_ref"]).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    b.insert("alignments".into(), refs.clone());
    b.insert("claims".into(), refs);
    Ok(b)
}
fn identities(
    repo: &ResearchExecution,
    m: &Material,
) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let issuance = load(repo, &output("identity-issuance.v1.json"))?;
    let mut all = BTreeSet::new();
    let mut result = BTreeMap::new();
    for (kind, expected) in bindings(repo, m)? {
        repo.tick(1)?;
        let rows = arr(&issuance["identities"][&kind])?;
        ensure(
            rows.iter()
                .map(|r| s(&r["binding"]).map(str::to_owned))
                .collect::<Result<Vec<_>>>()?
                == expected,
            "identity binding drift",
        )?;
        let mut ids = BTreeMap::new();
        for r in rows {
            repo.tick(1)?;
            let id = s(&r["id"])?.to_owned();
            ensure(all.insert(id.clone()), "identity collision")?;
            ids.insert(s(&r["binding"])?.to_owned(), id);
        }
        result.insert(kind, ids);
    }
    Ok(result)
}
fn audit(repo: &ResearchExecution, m: &Material) -> Result<Vec<Value>> {
    let mut owner = BTreeMap::new();
    for (i, r) in m.challenger.iter().enumerate() {
        repo.tick(1)?;
        for u in strings(&r["ordered_source_unit_refs"])?
            .into_iter()
            .chain(strings(&r["ordered_target_unit_refs"])?)
        {
            ensure(
                owner.insert(u, i).is_none(),
                "challenger unit coverage duplicates an identity",
            )?;
        }
    }
    let mut out = Vec::new();
    for p in &m.primary {
        repo.tick(1)?;
        let source = strings(&p["source_paragraph_refs"])?;
        let target = strings(&p["target_paragraph_refs"])?;
        let ix: BTreeSet<usize> = source
            .iter()
            .chain(target.iter())
            .filter_map(|u| owner.get(u).copied())
            .collect();
        let candidates: Vec<_> = ix.into_iter().map(|i| &m.challenger[i]).collect();
        let mut sp = BTreeSet::new();
        let mut tp = BTreeSet::new();
        let mut verse = false;
        for c in &candidates {
            repo.tick(1)?;
            for (side, dst) in [("source", &mut sp), ("target", &mut tp)] {
                repo.tick(1)?;
                let us = strings(&c[format!("ordered_{side}_unit_refs")])?;
                let ks = strings(&c[format!("{side}_unit_kinds")])?;
                ensure(
                    us.len() == ks.len(),
                    "challenger kind/ref cardinality drift",
                )?;
                for (u, k) in us.into_iter().zip(ks) {
                    repo.tick(1)?;
                    if k == "prose_paragraph" {
                        dst.insert(u);
                    }
                    if k == "verse_group" {
                        verse = true;
                    }
                }
            }
        }
        let agrees = candidates.len() == 1
            && sp == source.into_iter().collect()
            && tp == target.into_iter().collect()
            && !verse;
        let outside = p["reading_ordinal_within_part"].is_null();
        let posture = if outside {
            "outside_challenger_81_reading_scope"
        } else if agrees {
            "same_paragraph_group"
        } else if verse {
            "mixed_prose_verse_scope_disagreement"
        } else {
            "paragraph_group_disagreement"
        };
        out.push(json!({"schema_version":"tos_zarathustra_de_ru_independent_challenger_audit_v1","primary_proposal_ref":p["alignment_proposal_ref"],"part_order":p["part_order"],"reading_ordinal_within_part":p["reading_ordinal_within_part"],"challenger_alignment_refs":candidates.iter().map(|c|c["alignment_id"].clone()).collect::<Vec<_>>(),"comparison_posture":posture,"same_paragraph_group":agrees,"verse_barrier_risk":verse,"requires_source_and_target_visible_review":!agrees,"resolution":if !agrees&&!outside {"none_machine_disagreement_preserved"}else if outside{"not_applicable_outside_scope"}else{"agreement_is_not_acceptance"},"source_text_included":false,"target_text_included":false}));
    }
    ensure(out.len() == 3423, "challenger audit census drift")?;
    Ok(out)
}
fn scope(repo: &ResearchExecution) -> Result<(Vec<Value>, BTreeSet<(usize, usize)>)> {
    let de = lines(repo, &at(DE, "citation-spine.v1.jsonl"))?;
    let mut ord = BTreeMap::new();
    for part in 1..=4 {
        repo.tick(1)?;
        let rows: Vec<_> = de
            .iter()
            .filter(|r| {
                r["part_order"] == part && r["structural_role"] == "major_reading_unit_candidate"
            })
            .collect();
        ensure(
            rows.len() == READINGS[part - 1],
            "German reading-unit census drift",
        )?;
        for (i, r) in rows.iter().enumerate() {
            repo.tick(1)?;
            ord.insert(s(&r["unit_id"])?.to_owned(), i + 1);
        }
    }
    let mut de_lines = BTreeMap::<String, Vec<Value>>::new();
    for r in &de {
        repo.tick(1)?;
        if r["unit_kind"] == "verse_line" {
            de_lines
                .entry(s(&r["parent_unit_id"])?.into())
                .or_default()
                .push(r["unit_id"].clone());
        }
    }
    let mut out = Vec::new();
    let mut risks = BTreeSet::new();
    for g in de.iter().filter(|r| r["unit_kind"] == "verse_group") {
        repo.tick(1)?;
        let reading = *ord
            .get(s(&g["nearest_major_unit_id"])?)
            .ok_or("missing verse reading")?;
        let part = n(&g["part_order"])?;
        risks.insert((part, reading));
        out.push(json!({"schema_version":"tos_zarathustra_de_ru_alignment_scope_exclusion_v1","scope_exclusion_ref":format!("de-verse-group-{:03}",out.len()+1),"side":"source","part_order":part,"reading_ordinal_within_part":reading,"excluded_unit_kind":"verse_group","excluded_group_unit_ref":g["unit_id"],"ordered_excluded_verse_line_refs":de_lines.get(s(&g["unit_id"])?).cloned().unwrap_or_default(),"reason":"paragraph_packet_scope_excludes_source_attested_verse_group","used_as_structural_risk_qualification":true,"source_text_included":false,"semantic_equivalence_asserted":false}));
    }
    let structure = lines(repo, &at(RU, "structure-spine.v2.jsonl"))?;
    let mut meta = BTreeMap::new();
    for r in structure {
        repo.tick(1)?;
        if r["record_kind"] == "reading_unit_heading" {
            meta.insert(
                s(&r["structure_unit_id"])?.to_owned(),
                (
                    part_id(&r["part_id"])?,
                    n(&r["reading_unit_ordinal_within_part"])?,
                ),
            );
        }
    }
    let mut ru_lines = BTreeMap::<String, Vec<Value>>::new();
    for r in lines(repo, &at(RU, "verse-line-spine.v2.jsonl"))? {
        repo.tick(1)?;
        ru_lines
            .entry(s(&r["verse_group_unit_ref"])?.into())
            .or_default()
            .push(r["verse_line_unit_id"].clone());
    }
    for (i, g) in lines(repo, &at(RU, "verse-group-spine.v2.jsonl"))?
        .iter()
        .enumerate()
    {
        let (part, reading) = *meta
            .get(s(&g["reading_unit_ref"])?)
            .ok_or("missing Russian verse reading")?;
        risks.insert((part, reading));
        out.push(json!({"schema_version":"tos_zarathustra_de_ru_alignment_scope_exclusion_v1","scope_exclusion_ref":format!("ru-verse-group-{:03}",i+1),"side":"target","part_order":part,"reading_ordinal_within_part":reading,"excluded_unit_kind":"continuous_verse_group","excluded_group_unit_ref":g["verse_group_unit_id"],"ordered_excluded_verse_line_refs":ru_lines.get(s(&g["verse_group_unit_id"])?).cloned().unwrap_or_default(),"reason":"paragraph_packet_scope_excludes_source_visible_continuous_verse_group","used_as_structural_risk_qualification":true,"source_text_included":false,"semantic_equivalence_asserted":false}));
    }
    exclusion_census(repo, &out)?;
    Ok((out, risks))
}
fn exclusion_census(repo: &ResearchExecution, rows: &[Value]) -> Result<()> {
    for (side, groups, lines) in [("source", 39, 368), ("target", 13, 359)] {
        repo.tick(1)?;
        let r: Vec<_> = rows.iter().filter(|r| r["side"] == side).collect();
        let total = r
            .iter()
            .map(|r| arr(&r["ordered_excluded_verse_line_refs"]).map(Vec::len))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .sum::<usize>();
        ensure(
            r.len() == groups && total == lines,
            "verse scope-exclusion census drift",
        )?;
    }
    Ok(())
}
fn cross_risk(p: &Value, a: &Value) -> Result<bool> {
    Ok(flag(&a["verse_barrier_risk"])?
        || ((arr(&p["source_paragraph_refs"])?.is_empty()
            || arr(&p["target_paragraph_refs"])?.is_empty())
            && p["part_order"] == 3
            && p["reading_ordinal_within_part"] == 15))
}
fn posture<'a>(p: &'a Value, a: &Value) -> Result<(&'static str, &'a str, &'static str)> {
    let shape = s(&p["mapping_type"])?;
    if p["reading_ordinal_within_part"].is_null() {
        return Ok((
            "deferred",
            "unresolved",
            "source-only in bounded 81-reading technical scope; not evidence of translator omission",
        ));
    }
    if arr(&p["source_paragraph_refs"])?.is_empty() || arr(&p["target_paragraph_refs"])?.is_empty()
    {
        return Ok((
            "ambiguous",
            shape,
            if cross_risk(p, a)? {
                "cross-kind/out-of-paragraph-scope candidate; not evidence of translator omission"
            } else {
                "structural-role mismatch; not evidence of translator omission"
            },
        ));
    }
    if !flag(&a["same_paragraph_group"])? {
        return Ok((
            "ambiguous",
            shape,
            "independent mixed-content challenger disagrees; machine ambiguity preserved without resolution",
        ));
    }
    if flag(&a["verse_barrier_risk"])? {
        return Ok((
            "ambiguous",
            shape,
            "excluded verse boundary qualifies this paragraph grouping; machine ambiguity preserved",
        ));
    }
    if s(&p["resolution_status"])?.starts_with("unresolved") || p["confidence_band"] == "low" {
        return Ok((
            "ambiguous",
            shape,
            "length-profile result is unstable or low-fit; source-and-target-visible review required",
        ));
    }
    Ok((
        "proposed",
        shape,
        "Machine grouping agrees across the compared technical profiles; the grouping retains proposal status pending source-and-target-visible translation assessment.",
    ))
}
/// Full reconstruction, machine qualifications and deterministic artifact serialization.
pub fn build_scoped(repo: &ResearchExecution, m: &Material) -> Result<(Files, Files)> {
    let ids = identities(repo, m)?;
    let audit = audit(repo, m)?;
    let (exclusions, risk_readings) = scope(repo)?;
    let method_sha = sha(&bytes(repo, &output("primary-method-input.v1.json"))?);
    let mut target_anchors = BTreeMap::new();
    for layer in m.layers.values() {
        repo.tick(1)?;
        for r in &layer.records {
            repo.tick(1)?;
            let unit = s(&r["unit_id"])?;
            target_anchors.insert(unit.to_owned(),json!({"anchor_ref":ids["target_anchors"][unit],"ordinal":r["ordinal"],"text_layer_ref":layer.reference,"text_layer_sha256":layer.digest,"selector":{"type":"text_position","start":r["start"],"end":r["end"],"position_unit":"unicode_code_point","interval":"half_open"},"exact_sha256":r["exact_sha256"],"source_return":{"required":true,"locator_ref":format!("{}#unit={unit}",at(RU,"paragraph-spine.v2.jsonl"))}}));
        }
    }
    let mut spine = Vec::new();
    let mut conflicts = Vec::new();
    let mut alignments = BTreeMap::<usize, Vec<Value>>::new();
    for (p, a) in m.primary.iter().zip(audit.iter()) {
        repo.tick(1)?;
        let reference = s(&p["alignment_proposal_ref"])?;
        let (status, shape, reason) = posture(p, a)?;
        let cross = cross_risk(p, a)?;
        let sr = strings(&p["source_paragraph_refs"])?
            .iter()
            .map(|u| {
                m.source_anchors
                    .get(u)
                    .map(|a| a["anchor_ref"].clone())
                    .ok_or_else(|| "missing source anchor".into())
            })
            .collect::<Result<Vec<Value>>>()?;
        let tr = strings(&p["target_paragraph_refs"])?
            .iter()
            .map(|u| {
                target_anchors
                    .get(u)
                    .map(|a| a["anchor_ref"].clone())
                    .ok_or_else(|| "missing target anchor".into())
            })
            .collect::<Result<Vec<Value>>>()?;
        let aid = &ids["alignments"][reference];
        let cid = &ids["claims"][reference];
        let mut al = template("alignment");
        al["alignment_id"] = json!(aid);
        al["claim_id"] = json!(cid);
        al["correspondence_shape"] = json!(shape);
        al["order_posture"] = json!(if !sr.is_empty() && !tr.is_empty() {
            "monotonic"
        } else {
            "not_applicable"
        });
        al["ordered_source_anchor_refs"] = json!(sr);
        al["ordered_target_anchor_refs"] = json!(tr);
        al["epistemic_status"] = json!(if status == "proposed" {
            "inferred"
        } else {
            "uncertain"
        });
        al["certainty"]["value"] = json!(if status == "proposed" { 0.5 } else { 0.0 });
        al["status"] = json!(status);
        al["status_reason"] = json!(reason);
        al["maker"]["configuration_sha256"] = json!(method_sha);
        al["evidence"] = json!([{"evidence_ref":format!("{}#{reference}",output("independent-challenger-audit.v1.jsonl")),"role":if status=="proposed"{"method_input"}else{"qualification"},"source_anchor_refs":sr,"target_anchor_refs":tr,"description":reason}]);
        alignments.entry(n(&p["part_order"])?).or_default().push(al);
        spine.push(json!({"schema_version":"tos_zarathustra_de_ru_paragraph_alignment_spine_v1","primary_proposal_ref":reference,"alignment_id":aid,"claim_id":cid,"part_order":p["part_order"],"reading_ordinal_within_part":p["reading_ordinal_within_part"],"technical_subpart_ordinal":p["technical_subpart_ordinal"],"correspondence_shape":shape,"ordered_source_anchor_refs":sr,"ordered_target_anchor_refs":tr,"source_paragraph_unit_refs":p["source_paragraph_refs"],"target_paragraph_unit_refs":p["target_paragraph_refs"],"status":status,"status_reason":reason,"primary_profile_disagreement":!flag(&p["primary_challenger_same_group"])?,"independent_challenger_same_paragraph_group":a["same_paragraph_group"],"independent_challenger_refs":a["challenger_alignment_refs"],"verse_barrier_risk":a["verse_barrier_risk"],"cross_kind_or_verse_scope_risk":cross,"human_acceptance":false,"source_text_included":false,"semantic_equivalence_asserted":false,"canon_effect":false}));
        if status != "proposed" {
            let mut kinds = Vec::new();
            if !flag(&a["same_paragraph_group"])? {
                kinds.push("independent_challenger_group_disagreement");
            }
            if cross {
                kinds.push("verse_barrier_or_cross_kind_risk");
            }
            if sr.is_empty() || tr.is_empty() {
                kinds.push("null_side_transition");
            }
            if !flag(&p["primary_challenger_same_group"])? {
                kinds.push("primary_length_profile_instability");
            }
            if p["confidence_band"] == "low" {
                kinds.push("low_length_fit");
            }
            conflicts.push(json!({"schema_version":"tos_zarathustra_de_ru_paragraph_alignment_conflict_v1","conflict_ref":format!("alignment-conflict-{:04}",conflicts.len()+1),"alignment_ref":aid,"primary_proposal_ref":reference,"part_order":p["part_order"],"reading_ordinal_within_part":p["reading_ordinal_within_part"],"conflict_kinds":kinds,"preserved_status":status,"reason":reason,"machine_resolution":"none","requires_source_and_target_visible_review":true,"source_text_included":false,"semantic_equivalence_asserted":false}));
        }
    }
    let mut packets = BTreeMap::new();
    for part in 1..=4 {
        repo.tick(1)?;
        let cfg = arr(&m.de_plan["source_items"])?
            .iter()
            .find(|r| r["part_order"] == part)
            .ok_or("missing German source config")?;
        let ru = &m.ru_plan["source_item"];
        let mut packet = template("packet");
        packet["$schema"] = json!(format!("https://tree-of-sophia.local/{SCHEMA}"));
        packet["packet_id"] = json!(ids["packets"][&format!("part-{part}")]);
        packet["alignments"] = json!(alignments.get(&part).cloned().unwrap_or_default());
        for (side, config) in [("source_side", cfg), ("target_side", ru)] {
            repo.tick(1)?;
            packet[side]["work_ref"] = json!(WORK);
            for key in [
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
                "file_sha256",
            ] {
                packet[side][key] = config[key].clone();
            }
            packet[side]["rights_refs"] = json!([config["rights_ref"]]);
        }
        let source = &m.de_packets[&part];
        packet["source_side"]["text_layer_ref"] = source["source_layer"]["text_layer_ref"].clone();
        packet["source_side"]["text_layer_sha256"] =
            source["source_layer"]["text_layer_sha256"].clone();
        let packet_path = at(DE, &format!("part-{part}.source-text-unit.v1.json"));
        packet["source_side"]["segmentation"]["artifact_ref"] = json!(packet_path);
        packet["source_side"]["segmentation"]["sha256"] = json!(sha(&bytes(repo, &packet_path)?));
        let mut anchors = Vec::new();
        for unit in &m.de_units[&part] {
            repo.tick(1)?;
            let a = &m.source_anchors[s(&unit["unit_id"])?];
            let mut view = serde_json::Map::new();
            for key in [
                "anchor_ref",
                "ordinal",
                "text_layer_ref",
                "text_layer_sha256",
                "selector",
                "exact_sha256",
                "source_return",
            ] {
                view.insert(key.into(), a[key].clone());
            }
            anchors.push(Value::Object(view));
        }
        packet["source_side"]["anchors"] = json!(anchors);
        let layer = &m.layers[&part];
        packet["target_side"]["text_layer_ref"] = json!(layer.reference);
        packet["target_side"]["text_layer_sha256"] = json!(layer.digest);
        let paragraph_ref = at(RU, "paragraph-spine.v2.jsonl");
        packet["target_side"]["segmentation"]["artifact_ref"] = json!(paragraph_ref);
        packet["target_side"]["segmentation"]["sha256"] = json!(sha(&bytes(repo, &paragraph_ref)?));
        packet["target_side"]["anchors"] = json!(
            layer
                .records
                .iter()
                .map(|r| target_anchors[s(&r["unit_id"]).expect("validated id")].clone())
                .collect::<Vec<_>>()
        );
        packet["rights_and_visibility"]["rights_record_refs"] =
            json!([cfg["rights_ref"], ru["rights_ref"]]);
        validate_packet(repo, &packet)?;
        packets.insert(packet_ref(part), pretty(repo, &packet)?);
    }
    let same = audit
        .iter()
        .filter(|r| r["same_paragraph_group"] == true)
        .count();
    ensure(
        same == 3311 && audit.len() - same == 112,
        "independent comparison denominator drift",
    )?;
    let statuses = count(repo, &spine, "status")?;
    let shapes = count(repo, &spine, "correspondence_shape")?;
    let disagreement: BTreeSet<String> = spine
        .iter()
        .filter(|r| r["independent_challenger_same_paragraph_group"] == false)
        .map(|r| s(&r["primary_proposal_ref"]).map(str::to_owned))
        .collect::<Result<_>>()?;
    let risk: BTreeSet<String> = spine
        .iter()
        .filter(|r| r["status"] != "proposed")
        .map(|r| s(&r["primary_proposal_ref"]).map(str::to_owned))
        .collect::<Result<_>>()?;
    let profile: BTreeSet<String> = spine
        .iter()
        .filter(|r| r["primary_profile_disagreement"] == true)
        .map(|r| s(&r["primary_proposal_ref"]).map(str::to_owned))
        .collect::<Result<_>>()?;
    let overlap = profile.intersection(&disagreement).count();
    let primary_only = risk.difference(&disagreement).count();
    let mut affected = BTreeSet::new();
    let mut by_part = [0usize; 4];
    for r in &spine {
        repo.tick(1)?;
        if disagreement.contains(s(&r["primary_proposal_ref"])?) {
            by_part[n(&r["part_order"])? - 1] += 1;
            affected.extend(strings(&r["source_paragraph_unit_refs"])?);
            affected.extend(strings(&r["target_paragraph_unit_refs"])?);
        }
        ensure(
            !(r["status"] == "proposed"
                && (r["independent_challenger_same_paragraph_group"] == false
                    || r["primary_profile_disagreement"] == true
                    || r["verse_barrier_risk"] == true)),
            "final risk/status posture drift",
        )?;
    }
    ensure(
        affected.len() == 263 && by_part == [2, 8, 53, 49],
        "independent affected-unit/part drift",
    )?;
    ensure(
        profile.len() == 20 && overlap == 12 && primary_only == 8,
        "primary-profile risk union drift",
    )?;
    ensure(
        statuses["deferred"] == 9 && conflicts.len() == risk.len(),
        "final status/conflict census drift",
    )?;
    exact_coverage(
        repo,
        &spine,
        "source_paragraph_unit_refs",
        3447,
        &m.source_anchors.keys().cloned().collect(),
    )?;
    exact_coverage(
        repo,
        &spine,
        "target_paragraph_unit_refs",
        3569,
        &target_anchors.keys().cloned().collect(),
    )?;
    let mut diagnostics = Vec::new();
    for part in 1..=4 {
        repo.tick(1)?;
        for reading in 1..=READINGS[part - 1] {
            repo.tick(1)?;
            let rows: Vec<Value> = spine
                .iter()
                .filter(|r| r["part_order"] == part && r["reading_ordinal_within_part"] == reading)
                .cloned()
                .collect();
            let source_count = rows
                .iter()
                .map(|r| arr(&r["source_paragraph_unit_refs"]).map(Vec::len))
                .collect::<Result<Vec<_>>>()?
                .iter()
                .sum::<usize>();
            let target_count = rows
                .iter()
                .map(|r| arr(&r["target_paragraph_unit_refs"]).map(Vec::len))
                .collect::<Result<Vec<_>>>()?
                .iter()
                .sum::<usize>();
            diagnostics.push(json!({"schema_version":"tos_zarathustra_de_ru_paragraph_alignment_reading_diagnostic_v1","part_order":part,"part_label":LABELS[part-1],"reading_ordinal_within_part":reading,"mapping_count":rows.len(),"source_paragraph_count":source_count,"target_paragraph_count":target_count,"status_counts":count(repo, &rows,"status")?,"correspondence_shape_counts":count(repo, &rows,"correspondence_shape")?,"independent_challenger_disagreement_count":rows.iter().filter(|r|r["independent_challenger_same_paragraph_group"]==false).count(),"verse_scope_exclusion_present":risk_readings.contains(&(part,reading)),"verse_barrier_risk_mapping_count":rows.iter().filter(|r|r["cross_kind_or_verse_scope_risk"]==true).count(),"order_posture":"monotonic_machine_proposal_within_reading","source_text_included":false,"semantic_equivalence_asserted":false}));
        }
    }
    ensure(diagnostics.len() == 81, "reading diagnostic census drift")?;
    let part_counts = json!({"I":by_part[0],"II":by_part[1],"III":by_part[2],"IV":by_part[3]});
    let mut coverage = template("coverage");
    for (key, value) in [
        ("reading_pair_count", 81),
        ("alignment_count", spine.len()),
        ("source_paragraph_expected", 3447),
        ("source_paragraph_covered", 3447),
        ("target_paragraph_expected", 3569),
        ("target_paragraph_covered", 3569),
        (
            "independent_challenger_disagreement_paragraph_unit_count",
            affected.len(),
        ),
        ("primary_profile_disagreement_count", profile.len()),
        (
            "primary_profile_and_independent_disagreement_overlap_count",
            overlap,
        ),
        ("primary_only_machine_risk_count", primary_only),
        ("machine_risk_union_count", risk.len()),
        ("conflict_ledger_count", conflicts.len()),
        ("source_verse_group_exclusion_count", 39),
        ("source_verse_line_exclusion_count", 368),
        ("target_verse_group_exclusion_count", 13),
        ("target_verse_line_exclusion_count", 359),
    ] {
        coverage[key] = json!(value);
    }
    coverage["reading_pairs_by_part"] = json!({"I":23,"II":22,"III":16,"IV":20});
    coverage["status_counts"] = statuses.clone();
    coverage["correspondence_shape_counts"] = shapes;
    coverage["independent_challenger_disagreement_by_part"] = part_counts.clone();
    coverage["preserved_ambiguous_count"] = statuses["ambiguous"].clone();
    coverage["preserved_deferred_count"] = statuses["deferred"].clone();
    let mut summary = template("summary");
    summary["alignment_count"] = json!(spine.len());
    summary["source_paragraph_count"] = json!(3447);
    summary["target_paragraph_count"] = json!(3569);
    summary["status_counts"] = statuses;
    summary["independent_challenger_comparison"]["affected_paragraph_units"] =
        json!(affected.len());
    summary["independent_challenger_comparison"]["disagreement_by_part"] = part_counts;
    summary["machine_risk_union"]["mapping_count"] = json!(risk.len());
    summary["machine_risk_union"]["primary_profile_disagreement_count"] = json!(profile.len());
    summary["machine_risk_union"]["primary_profile_and_independent_overlap_count"] = json!(overlap);
    summary["machine_risk_union"]["primary_only_risk_count"] = json!(primary_only);
    let mut provenance = Vec::new();
    for (name, kind, reference) in [
        (
            "primary-import",
            "primary_machine_proposal_import",
            output("primary-input-receipt.v1.json"),
        ),
        (
            "challenger-import",
            "independent_mixed_content_challenger_import",
            output("independent-challenger-input-receipt.v1.json"),
        ),
        (
            "target-layer",
            "private_russian_paragraph_layer_derivation",
            at(RU, "paragraph-spine.v2.jsonl"),
        ),
        (
            "build",
            "translation_alignment_packet_proposal_materialization",
            SCHEMA.into(),
        ),
    ] {
        provenance.push(json!({"event_id":format!("tos.event.zarathustra-de-ru-paragraph-alignment-v1.{name}"),"event_kind":kind,"input_ref":reference,"schema_version":"tos_zarathustra_de_ru_paragraph_alignment_provenance_v1","event_date":"2026-09-01","ended_at":"2026-09-01T21:00:00-06:00","method_output_posture":"proposal_not_truth","human_review":false,"source_text_included":false,"semantic_equivalence_asserted":false,"canon_effect":false}));
    }
    let mut public = packets;
    for (name, rows) in [
        ("alignment-spine.v1.jsonl", spine),
        ("per-reading-diagnostics.v1.jsonl", diagnostics),
        ("conflict-ledger.v1.jsonl", conflicts),
        ("scope-exclusions.v1.jsonl", exclusions),
        ("independent-challenger-audit.v1.jsonl", audit),
        ("provenance.jsonl", provenance),
    ] {
        for r in &rows {
            repo.tick(1)?;
            text_free(repo, r)?;
        }
        public.insert(output(name), jsonl(repo, &rows)?);
    }
    for (name, value) in [
        ("coverage-receipt.v1.json", coverage),
        ("agent-verification.v1.json", template("verification")),
        ("summary.v1.json", summary),
    ] {
        text_free(repo, &value)?;
        public.insert(output(name), pretty(repo, &value)?);
    }
    let mut static_inputs = serde_json::Map::new();
    for reference in [
        output("primary-proposals-input.v1.jsonl"),
        output("primary-diagnostics-input.v1.jsonl"),
        output("primary-method-input.v1.json"),
        output("primary-input-receipt.v1.json"),
        output("independent-challenger-input.v1.jsonl"),
        output("independent-challenger-input-receipt.v1.json"),
        output("identity-issuance.v1.json"),
        SCHEMA.into(),
        at(DE, "plan.v1.json"),
        at(
            "technical-markup/antonovsky-1911-pdf-layout-v1",
            "plan.v1.json",
        ),
    ] {
        let raw = bytes(repo, &reference)?;
        static_inputs.insert(reference, json!({"sha256":sha(&raw),"byte_size":raw.len()}));
    }
    let generated: BTreeMap<_, _> = public
        .iter()
        .map(|(r, b)| (r.clone(), json!({"sha256":sha(b),"byte_size":b.len()})))
        .collect();
    let private: Files = m
        .layers
        .values()
        .map(|l| (l.reference.clone(), l.payload.clone()))
        .collect();
    let private_receipts: BTreeMap<_, _> = m
        .layers
        .values()
        .map(|l| {
            (
                l.reference.clone(),
                json!({"sha256":l.digest,"byte_size":l.payload.len(),"required_mode":"0600"}),
            )
        })
        .collect();
    let mut manifest = template("manifest");
    manifest["static_inputs"] = Value::Object(static_inputs);
    manifest["generated_outputs"] = json!(generated);
    manifest["private_outputs"] = json!(private_receipts);
    public.insert(output("manifest.v1.json"), pretty(repo, &manifest)?);
    Ok((public, private))
}
fn validate_packet(repo: &ResearchExecution, p: &Value) -> Result<()> {
    use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};
    let uri = format!("https://tree-of-sophia.local/{SCHEMA}");
    let probe = SchemaBackendProbe::new(
        [SchemaResource {
            uri: uri.clone(),
            raw: bytes(repo, SCHEMA)?,
        }],
        FormatProfile::LegacyPythonObserved20260923,
    )
    .map_err(|e| format!("schema backend: {e:?}"))?; // The schema backend admits at most 1 MiB per instance. Validate bounded
    // packet slices while retaining every scalar and every original array element.
    // The three sliced arrays have only minItems:1 and item constraints in v1;
    // explicit nonempty checks preserve their global cardinality law.
    let sources = arr(&p["source_side"]["anchors"])?;
    let targets = arr(&p["target_side"]["anchors"])?;
    let alignments = arr(&p["alignments"])?;
    ensure(
        !sources.is_empty() && !targets.is_empty() && !alignments.is_empty(),
        "packet arrays must be nonempty",
    )?;
    let slices = sources
        .len()
        .max(targets.len())
        .max(alignments.len())
        .div_ceil(100);
    for i in 0..slices {
        repo.tick(1)?;
        let mut chunk = p.clone();
        for (side, rows) in [("source_side", sources), ("target_side", targets)] {
            repo.tick(1)?;
            let start = i * 100;
            chunk[side]["anchors"] = json!(if start < rows.len() {
                &rows[start..(start + 100).min(rows.len())]
            } else {
                &rows[..1]
            });
        }
        let start = i * 100;
        chunk["alignments"] = json!(if start < alignments.len() {
            &alignments[start..(start + 100).min(alignments.len())]
        } else {
            &alignments[..1]
        });
        ensure(
            probe
                .is_valid_raw(&uri, &pretty(repo, &chunk)?)
                .map_err(|e| format!("packet schema: {e:?}"))?,
            "packet schema failure",
        )?;
        repo.check()?;
    }
    text_free(repo, p)?;
    ensure(
        arr(&p["reviews"])?.is_empty() && arr(&p["projections"])?.is_empty(),
        "reviews and projections must remain empty",
    )?;
    let mut anchors = BTreeSet::new();
    for side in ["source_side", "target_side"] {
        repo.tick(1)?;
        for a in arr(&p[side]["anchors"])? {
            repo.tick(1)?;
            anchors.insert(s(&a["anchor_ref"])?.to_owned());
        }
    }
    for a in arr(&p["alignments"])? {
        repo.tick(1)?;
        ensure(
            ["proposed", "ambiguous", "deferred"].contains(&s(&a["status"])?),
            "machine packet contains a decided status",
        )?;
        for r in strings(&a["ordered_source_anchor_refs"])?
            .into_iter()
            .chain(strings(&a["ordered_target_anchor_refs"])?)
        {
            ensure(
                anchors.contains(&r),
                "packet alignment anchor closure failure",
            )?;
        }
        ensure(
            arr(&a["review_refs"])?.is_empty(),
            "machine proposal has review refs",
        )?;
        ensure(
            !(["source_omission", "target_addition", "unresolved"]
                .contains(&s(&a["correspondence_shape"])?)
                && a["status"] == "proposed"),
            "null or unresolved mapping cannot be proposed",
        )?;
    }
    Ok(())
}
fn manifest_check(repo: &ResearchExecution, manifest: &Value) -> Result<()> {
    for group in ["static_inputs", "generated_outputs"] {
        repo.tick(1)?;
        for (reference, r) in manifest[group]
            .as_object()
            .ok_or("invalid manifest group")?
        {
            let b = bytes(repo, reference)?;
            ensure(
                sha(&b) == s(&r["sha256"])? && b.len() == n(&r["byte_size"])?,
                &format!("manifest {group} drift: {reference}"),
            )?;
        }
    }
    Ok(())
}
/// Validate tracked proposal metadata without reading private payload layers.
pub fn validate_tracked_scoped(repo: &ResearchExecution) -> Result<Value> {
    manifest_check(repo, &load(repo, &output("manifest.v1.json"))?)?;
    let mut alignment_ids = BTreeSet::new();
    let mut claim_ids = BTreeSet::new();
    for part in 1..=4 {
        repo.tick(1)?;
        let p = load(repo, &packet_ref(part))?;
        validate_packet(repo, &p)?;
        for a in arr(&p["alignments"])? {
            repo.tick(1)?;
            ensure(
                alignment_ids.insert(s(&a["alignment_id"])?.to_owned()),
                "alignment identity collision",
            )?;
            ensure(
                claim_ids.insert(s(&a["claim_id"])?.to_owned()),
                "claim identity collision",
            )?;
        }
    }
    let spine = lines(repo, &output("alignment-spine.v1.jsonl"))?;
    let audit = lines(repo, &output("independent-challenger-audit.v1.jsonl"))?;
    let conflicts = lines(repo, &output("conflict-ledger.v1.jsonl"))?;
    let exclusions = lines(repo, &output("scope-exclusions.v1.jsonl"))?;
    let diagnostics = lines(repo, &output("per-reading-diagnostics.v1.jsonl"))?;
    let coverage = load(repo, &output("coverage-receipt.v1.json"))?;
    let verification = load(repo, &output("agent-verification.v1.json"))?;
    for rows in [&spine, &audit, &conflicts, &exclusions, &diagnostics] {
        repo.tick(1)?;
        for r in rows {
            repo.tick(1)?;
            text_free(repo, r)?;
        }
    }
    text_free(repo, &coverage)?;
    text_free(repo, &verification)?;
    ensure(
        spine.len() == 3423 && audit.len() == spine.len(),
        "alignment/audit census drift",
    )?;
    let statuses = count(repo, &spine, "status")?;
    ensure(
        statuses == coverage["status_counts"],
        "tracked status census differs from coverage receipt",
    )?;
    let same = audit
        .iter()
        .filter(|r| r["same_paragraph_group"] == true)
        .count();
    ensure(
        same == 3311 && audit.len() - same == 112,
        "tracked independent comparison denominator drift",
    )?;
    let mut risk = BTreeSet::new();
    for r in &spine {
        repo.tick(1)?;
        ensure(
            !(r["status"] == "proposed"
                && r["independent_challenger_same_paragraph_group"] == false),
            "independent disagreement leaked into proposed status",
        )?;
        if r["status"] != "proposed" {
            risk.insert(s(&r["primary_proposal_ref"])?.to_owned());
        }
    }
    let conflict_refs = conflicts
        .iter()
        .map(|r| s(&r["primary_proposal_ref"]).map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure(
        conflict_refs == risk
            && conflicts.len() == n(&coverage["machine_risk_union_count"])?
            && conflicts.iter().all(|r| r["machine_resolution"] == "none"),
        "conflict ambiguity was not preserved",
    )?;
    for (key, total) in [
        ("source_paragraph_unit_refs", 3447),
        ("target_paragraph_unit_refs", 3569),
    ] {
        let mut all = Vec::new();
        for r in &spine {
            repo.tick(1)?;
            all.extend(strings(&r[key])?);
        }
        ensure(
            all.len() == total && all.into_iter().collect::<BTreeSet<_>>().len() == total,
            "tracked paragraph exact-once coverage failure",
        )?;
    }
    ensure(
        diagnostics.len() == 81,
        "tracked reading diagnostic census drift",
    )?;
    exclusion_census(repo, &exclusions)?;
    ensure(
        coverage["accepted_alignment_count"] == 0 && verification["human_acceptance"] == false,
        "acceptance boundary drift",
    )?;
    Ok(
        json!({"status":"pass","packets":4,"reading_pairs":81,"alignments":spine.len(),"source_paragraphs":3447,"target_paragraphs":3569,"proposed":statuses["proposed"],"ambiguous":statuses["ambiguous"],"deferred":statuses["deferred"],"accepted":0,"machine_risk_union":risk.len(),"independent_agree":3311,"independent_disagree":112}),
    )
}
fn write_file(
    repo: &ResearchExecution,
    reference: &str,
    payload: &[u8],
    private: bool,
    exclusive: bool,
) -> Result<()> {
    repo.write(
        reference,
        payload,
        if private { 0o600 } else { 0o644 },
        exclusive,
    )
}

fn write_outputs(repo: &ResearchExecution, public: &Files, private: &Files) -> Result<()> {
    for (r, b) in public {
        repo.tick(1)?;
        write_file(repo, r, b, false, false)?;
    }
    for (r, b) in private {
        repo.tick(1)?;
        write_file(repo, r, b, true, false)?;
    }
    Ok(())
}
fn check_outputs(repo: &ResearchExecution, public: &Files, private: &Files) -> Result<()> {
    let mut drift = Vec::new();
    for (r, b) in public {
        repo.tick(1)?;
        if bytes(repo, r).ok().as_ref() != Some(b) {
            drift.push(r.as_str());
        }
    }
    ensure(
        drift.is_empty(),
        &format!("tracked generated drift: {}", drift.join(", ")),
    )?;
    let mut drift = Vec::new();
    for (r, b) in private {
        repo.tick(1)?;
        if bytes(repo, r).ok().as_ref() != Some(b) || mode600(repo, r).is_err() {
            drift.push(r.as_str());
        }
    }
    ensure(
        drift.is_empty(),
        &format!("private layer drift or mode failure: {}", drift.join(", ")),
    )
}
fn issue(repo: &ResearchExecution, m: &Material) -> Result<()> {
    let reference = output("identity-issuance.v1.json");
    ensure(
        !repo.join(&reference).exists(),
        "identity issuance already exists; refusing remint",
    )?;
    let mut entropy = fs::File::open("/dev/urandom").map_err(|e| e.to_string())?;
    let mut records = BTreeMap::<String, Vec<Value>>::new();
    let mut ids = BTreeSet::new();
    for (kind, bindings) in bindings(repo, m)? {
        repo.tick(1)?;
        let prefix = match kind.as_str() {
            "packets" => "tos.translation-alignment-packet",
            "target_anchors" => "tos.anchor",
            "alignments" => "tos.translation-alignment",
            "claims" => "tos.translation-alignment-claim",
            _ => return Err("unknown identity kind".into()),
        };
        for binding in bindings {
            repo.tick(1)?;
            let mut raw = [0u8; 16];
            repo.read_exact(&mut entropy, &mut raw)?;
            let hex = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let id = format!("{prefix}.sid-{hex}");
            ensure(ids.insert(id.clone()), "identity collision")?;
            records
                .entry(kind.clone())
                .or_default()
                .push(json!({"binding":binding,"id":id}));
        }
    }
    let v = json!({"schema_version":"tos_zarathustra_de_ru_paragraph_alignment_identity_issuance_v1","issuance_id":"tos.identity-issuance.zarathustra-de-ru-paragraph-alignment-v1","issued_on":"2026-09-01","opaque_identity":true,"binding_is_not_identity":true,"identities":records});
    write_file(repo, &reference, &pretty(repo, &v)?, false, true)
}
fn import_bytes(repo: &ResearchExecution, directory: &Path, name: &str) -> Result<Vec<u8>> {
    repo.check()?;
    let dir = tos_fd_open::open_absolute_directory(directory).map_err(|e| e.to_string())?;
    let mut file =
        tos_fd_open::open_regular_at(&dir, relative(name)?).map_err(|e| e.to_string())?;
    repo.read_file(&mut file, 256 * 1024 * 1024)
}

fn import(repo: &ResearchExecution, directory: &Path, primary: bool) -> Result<()> {
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let targets = if primary {
        vec![
            "primary-proposals-input.v1.jsonl",
            "primary-diagnostics-input.v1.jsonl",
            "primary-method-input.v1.json",
            "primary-input-receipt.v1.json",
        ]
    } else {
        vec![
            "independent-challenger-input.v1.jsonl",
            "independent-challenger-input-receipt.v1.json",
        ]
    };
    for t in &targets {
        repo.tick(1)?;
        ensure(
            !repo.join(output(t)).exists(),
            if primary {
                "primary inputs already exist; refusing replacement"
            } else {
                "challenger inputs already exist; refusing replacement"
            },
        )?;
    }
    let mappings = if primary {
        vec![
            (
                "primary-proposals-input.v1.jsonl",
                "alignment-proposals.jsonl",
            ),
            (
                "primary-diagnostics-input.v1.jsonl",
                "per-reading-diagnostics.jsonl",
            ),
            ("primary-method-input.v1.json", "method.json"),
        ]
    } else {
        vec![(
            "independent-challenger-input.v1.jsonl",
            "mapping.proposal.v1.jsonl",
        )]
    };
    let mut pending = Vec::new();
    for (target, source) in mappings {
        repo.tick(1)?;
        let raw = import_bytes(repo, &directory, source)?;
        if source.ends_with(".jsonl") {
            let text = std::str::from_utf8(&raw).map_err(|e| e.to_string())?;
            for line in text.lines().filter(|l| !l.is_empty()) {
                repo.tick(1)?;
                text_free(
                    repo,
                    &serde_json::from_str::<Value>(line).map_err(|e| e.to_string())?,
                )?;
            }
        } else {
            text_free(
                repo,
                &serde_json::from_slice::<Value>(&raw).map_err(|e| e.to_string())?,
            )?;
        }
        pending.push((output(target), raw));
    }
    let names = if primary {
        vec![
            "alignment-proposals.jsonl",
            "per-reading-diagnostics.jsonl",
            "anomaly-conflict-ledger.jsonl",
            "coverage-receipt.json",
            "method.json",
            "report.json",
            "manifest.json",
            "build_alignment.py",
        ]
    } else {
        vec![
            "mapping.proposal.v1.jsonl",
            "chapter-summary.v1.jsonl",
            "low-confidence-anomalies.v1.jsonl",
            "coverage.v1.json",
            "invariant-audit.v1.json",
            "configuration.v1.json",
            "manifest.v1.json",
            "run_challenger.py",
        ]
    };
    let mut digests = BTreeMap::new();
    for name in names {
        repo.tick(1)?;
        let raw = import_bytes(repo, &directory, name)?;
        digests.insert(name, sha(&raw));
    }
    let receipt = if primary {
        json!({"schema_version":"tos_zarathustra_de_ru_primary_input_receipt_v1","role":"machine_primary_proposal_input_not_truth","artifact_sha256":digests,"source_text_included":false,"human_acceptance":false})
    } else {
        json!({"schema_version":"tos_zarathustra_de_ru_independent_challenger_input_receipt_v1","role":"mixed_prose_verse_machine_challenger_not_truth","artifact_sha256":digests,"source_text_included":false,"target_text_included":false,"accepted_alignment_count":0})
    };
    pending.push((
        output(targets.last().ok_or("missing receipt name")?),
        pretty(repo, &receipt)?,
    ));
    for (reference, raw) in pending {
        repo.tick(1)?;
        write_file(repo, &reference, &raw, false, true)?;
    }
    Ok(())
}
/// Args are the legacy action flags, after the access adapter removes --source-root.
pub fn run_scoped(repo: &ResearchExecution, args: &[String]) -> Result<Value> {
    repo.check()?;
    let action = args
        .first()
        .map(String::as_str)
        .ok_or("required alignment action")?;
    let result = match action {
        "--import-primary" | "--import-challenger" => {
            ensure(
                args.len() == 2,
                "import action requires exactly one directory",
            )?;
            let primary = action == "--import-primary";
            import(repo, Path::new(&args[1]), primary)?;
            Ok(
                json!({"status":"imported","input":if primary{"primary"}else{"independent_challenger"}}),
            )
        }
        "--validate-tracked" => {
            ensure(args.len() == 1, "unexpected alignment argument")?;
            validate_tracked_scoped(repo)
        }
        "--issue-identities" | "--build" | "--check" => {
            ensure(args.len() == 1, "unexpected alignment argument")?;
            let m = reconstruct_scoped(repo)?;
            if action == "--issue-identities" {
                issue(repo, &m)?;
                repo.check()?;
                return Ok(
                    json!({"status":"issued","identity_ref":output("identity-issuance.v1.json")}),
                );
            }
            let (public, private) = build_scoped(repo, &m)?;
            if action == "--build" {
                write_outputs(repo, &public, &private)?;
            } else {
                check_outputs(repo, &public, &private)?;
            }
            serde_json::from_slice(&public[&output("summary.v1.json")]).map_err(|e| e.to_string())
        }
        _ => Err(format!("unknown alignment action: {action}")),
    };
    repo.check()?;
    result
}
/// Compatibility API; production dispatch carries one context through run_scoped.
pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    let ctx = ResearchExecution::new(root, 180)?;
    run_scoped(&ctx, args)
}
pub fn reconstruct(root: &Path) -> Result<Material> {
    let ctx = ResearchExecution::new(root, 180)?;
    reconstruct_scoped(&ctx)
}
pub fn build(root: &Path, m: &Material) -> Result<(Files, Files)> {
    let ctx = ResearchExecution::new(root, 180)?;
    build_scoped(&ctx, m)
}
pub fn validate_tracked(root: &Path) -> Result<Value> {
    let ctx = ResearchExecution::new(root, 180)?;
    validate_tracked_scoped(&ctx)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn test_context() -> ResearchExecution {
        ResearchExecution::new_with_scratch(Path::new("/"), 180, 16 * 1024 * 1024).unwrap()
    }
    #[test]
    fn null_side_keeps_machine_uncertainty() {
        let p = json!({"source_paragraph_refs":["s"],"target_paragraph_refs":[],"mapping_type":"source_omission","part_order":3,"reading_ordinal_within_part":15});
        let a = json!({"verse_barrier_risk":false,"same_paragraph_group":false});
        let (status, shape, reason) = posture(&p, &a).unwrap();
        assert_eq!((status, shape), ("ambiguous", "source_omission"));
        assert!(reason.contains("not evidence of translator omission"));
        assert!(cross_risk(&p, &a).unwrap());
    }
    #[test]
    fn outside_scope_is_first_class_deferred() {
        let p = json!({"mapping_type":"source_omission","reading_ordinal_within_part":null});
        assert_eq!(posture(&p, &json!({})).unwrap().0, "deferred");
    }
    #[test]
    fn metadata_rejects_nested_text_and_inclusion_flags() {
        assert!(text_free(&test_context(), &json!({"nested":[{"Text":"private"}]})).is_err());
        assert!(text_free(&test_context(), &json!({"source_text_included":null})).is_err());
        assert!(
            text_free(
                &test_context(),
                &json!({"source_text_included":false,"exact_sha256":"bound"})
            )
            .is_ok()
        );
    }
    #[test]
    fn unicode_positions_and_serialization_are_python_compatible() {
        assert_eq!("Я🙂e\u{301}".chars().count(), 4);
        assert_eq!(
            pretty(&test_context(), &json!({"b":false,"a":"Я"})).unwrap(),
            "{\n  \"a\": \"Я\",\n  \"b\": false\n}\n".as_bytes()
        );
        assert_eq!(
            jsonl(&test_context(), &[json!({"z":1,"a":"Я"})]).unwrap(),
            "{\"a\":\"Я\",\"z\":1}\n".as_bytes()
        );
    }
    #[test]
    fn primary_only_risk_remains_ambiguous() {
        let p = json!({"source_paragraph_refs":["s"],"target_paragraph_refs":["t"],"mapping_type":"one_to_one","part_order":1,"reading_ordinal_within_part":1,"resolution_status":"unresolved_low_fit","confidence_band":"low"});
        let a = json!({"verse_barrier_risk":false,"same_paragraph_group":true});
        assert_eq!(posture(&p, &a).unwrap().0, "ambiguous");
    }
    #[test]
    fn import_rejects_private_text_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let input = tempfile::tempdir().unwrap();
        fs::write(
            input.path().join("mapping.proposal.v1.jsonl"),
            b"{\"text\":\"private\"}\n",
        )
        .unwrap();
        assert!(
            import(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                input.path(),
                false
            )
            .is_err()
        );
        assert!(
            !root
                .path()
                .join(output("independent-challenger-input.v1.jsonl"))
                .exists()
        );
    }
    #[test]
    fn import_refuses_existing_input_even_if_receipt_absent() {
        let root = tempfile::tempdir().unwrap();
        let reference = output("independent-challenger-input.v1.jsonl");
        write_file(
            &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
            &reference,
            b"original",
            false,
            true,
        )
        .unwrap();
        assert!(
            import(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                root.path(),
                false
            )
            .is_err()
        );
        assert_eq!(
            bytes(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                &reference
            )
            .unwrap(),
            b"original"
        );
    }
    #[test]
    #[cfg(unix)]
    fn private_check_rejects_permissions_drift() {
        let root = tempfile::tempdir().unwrap();
        write_file(
            &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
            "private/layer",
            b"private",
            true,
            false,
        )
        .unwrap();
        assert!(
            mode600(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "private/layer"
            )
            .is_ok()
        );
        fs::set_permissions(
            root.path().join("private/layer"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(
            mode600(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "private/layer"
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_root_rejects_traversal_and_absolute_refs() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            bytes(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "../outside"
            )
            .is_err()
        );
        assert!(
            write_file(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "../outside",
                b"x",
                true,
                false
            )
            .is_err()
        );
        assert!(
            bytes(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "/etc/passwd"
            )
            .is_err()
        );
    }
    #[test]
    #[cfg(unix)]
    fn symlink_parent_cannot_escape_output_root() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(other.path(), root.path().join("link")).unwrap();
        assert!(
            write_file(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "link/escaped",
                b"x",
                true,
                false
            )
            .is_err()
        );
        assert!(!other.path().join("escaped").exists());
    }
    #[test]
    fn exclusive_issuance_never_replaces_existing_bytes() {
        let root = tempfile::tempdir().unwrap();
        write_file(
            &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
            "issued.json",
            b"first",
            false,
            true,
        )
        .unwrap();
        assert!(
            write_file(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "issued.json",
                b"second",
                false,
                true
            )
            .is_err()
        );
        assert_eq!(
            bytes(
                &ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap(),
                "issued.json"
            )
            .unwrap(),
            b"first"
        );
    }
    #[test]
    fn tracked_current_route_preserves_census_and_authority_ceiling() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
        let summary = validate_tracked(&root).expect("tracked alignment route validates");
        assert_eq!(summary["status"], "pass");
        assert_eq!(summary["packets"], 4);
        assert_eq!(summary["reading_pairs"], 81);
        assert_eq!(summary["alignments"], 3423);
        assert_eq!(summary["source_paragraphs"], 3447);
        assert_eq!(summary["target_paragraphs"], 3569);
        assert_eq!(summary["proposed"], 3303);
        assert_eq!(summary["ambiguous"], 111);
        assert_eq!(summary["deferred"], 9);
        assert_eq!(summary["accepted"], 0);
        assert_eq!(summary["machine_risk_union"], 120);
        assert_eq!(summary["independent_agree"], 3311);
        assert_eq!(summary["independent_disagree"], 112);
    }

    #[test]
    fn present_private_layers_rebuild_exactly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
        let repo = ResearchExecution::new(&root, 180).unwrap();
        let private_refs = (1..=4).flat_map(|part| {
            let packet = load(&repo, &packet_ref(part)).unwrap();
            ["source_side", "target_side"].into_iter().map(move |side| {
                s(&packet[side]["text_layer_ref"]).unwrap().to_owned()
            })
        }).collect::<Vec<_>>();
        if !private_refs.iter().all(|reference| root.join(reference).is_file()) {
            eprintln!("skip: exact local German and Russian private layers are not present");
            return;
        }
        let result = run(&root, &["--check".to_owned()]).expect("native exact private rebuild parity");
        assert_eq!(result["status"], "pass");
    }

    #[test]
    fn scoped_work_exhaustion_stops_before_reading_source() {
        let root = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new_with_scratch(root.path(), 180, 16 * 1024 * 1024).unwrap();
        ctx.tick(100_000_000).unwrap();
        let error = run_scoped(&ctx, &["--validate-tracked".to_owned()]).unwrap_err();
        assert!(error.contains("work budget exceeded"));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
