//! Bounded reconstruction of retained direct-visual retrieval evidence.
//! This records a laboratory result; it never invokes an embedding provider.
use crate::{
    jenseits_numbered_structure::Held,
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, s, sha},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, Metadata},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
};
type Result<T> = std::result::Result<T, String>;
const META: u64 = 8 * 1024 * 1024;
const ALL_META: usize = 64 * 1024 * 1024;
const OWNER: &str = "/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab";
const EXPERIMENT: &str = "tos-visual-retrieval-foundation-v1";
const BASE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/";
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/visual-retrieval-plan.v1.json";
const RECEIPT: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/visual-retrieval-result.c-qwen3-vl-embedding-2b.v1.json";
const OLD_RECEIPT_SHA: &str = "9e07d5581e600220efa885aeddc3c5870e5669b4a68d80f72e7ac92e1906e7da";
const BUILDER: &str = "rust/crates/tos-compiler/src/research_visual_result.rs";
const DIGESTS: [(&str, &str); 7] = [
    (
        "tree_plan",
        "27e142883d4385b8b69857ab7ebcbee7facb16454e54649ef6ed1df15e3b7c25",
    ),
    (
        "source_sample_plan",
        "a813f0a19361ccc773d1427ab696a76296993c999abb2ff35376bae422bbb9d1",
    ),
    (
        "visual_sample_plan",
        "3fd111023cf2ffdbee935d771f36f314d8087677d57e9ef34a64e19e27ddae06",
    ),
    (
        "query_plan",
        "481e1f8ea32b9f7b40eb73a59e7bc3e614d08a3b8958bebd7b0c7fce94fed7a4",
    ),
    (
        "query_content",
        "5c9ce96fb362512a9ef775bd7d7212b5df6b00570226961a96d51478ba980a1e",
    ),
    (
        "render_manifest",
        "734e7474923ec2697a243b1e1b322c43c4e2a8b82b6336df57ab82f3bbc8fec7",
    ),
    (
        "runtime_manifest",
        "ff730fe5780216fe84f26b7fbed6c57d0f0de58cabac5026d6af403e094632ba",
    ),
];
fn digest(key: &str) -> &'static str {
    DIGESTS.iter().find(|(k, _)| *k == key).unwrap().1
}
fn eq(v: &Value, want: Value, label: &str) -> Result<()> {
    ensure(v == &want, &format!("visual {label} drift"))
}
// Only paths from the reconstructed, source-free receipt enter diagnostics.
// Values and unrecognized keys from an input never enter the error text.
fn receipt_differences(observed: &Value, retained: &Value, path: &str, out: &mut Vec<String>) {
    if observed == retained || out.len() >= 20 {
        return;
    }
    match (observed, retained) {
        // The retained recorder read floating observations as IEEE doubles.
        // arbitrary_precision preserves exponent spelling (e-07 vs e-7),
        // which is not a change in that observation. No tolerance is applied.
        (Value::Number(a), Value::Number(b))
            if a.is_f64() && b.is_f64() && a.as_f64() == b.as_f64() => {}
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() || a.keys().any(|key| !b.contains_key(key)) {
                out.push(format!("{path}/<fields>"));
            }
            for (key, value) in a {
                receipt_differences(
                    value,
                    b.get(key).unwrap_or(&Value::Null),
                    &format!("{path}/{key}"),
                    out,
                );
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                receipt_differences(a, b, &format!("{path}/{i}"), out);
            }
        }
        _ => out.push(path.into()),
    }
}
fn arr(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("visual array required".into())
}
fn uint(v: &Value) -> Result<u64> {
    v.as_u64().ok_or("visual unsigned integer required".into())
}
fn number(v: &Value) -> Result<f64> {
    let n = v.as_f64().ok_or("visual finite number required")?;
    ensure(n.is_finite(), "visual finite number required")?;
    Ok(n)
}
fn canonical(mut value: Value) -> Result<Vec<u8>> {
    value.sort_all_objects();
    let mut raw = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    raw.push(b'\n');
    ensure(raw.len() <= META as usize, "visual encoded metadata bound")?;
    Ok(raw)
}
fn pathsafe(value: &str) -> Result<()> {
    let p = tos_foundation::RelativePath::parse(value).map_err(|e| e.to_string())?;
    let _ = p;
    ensure(value.len() <= 4096, "visual reference length")
}
fn owner_ref(value: &str) -> Result<String> {
    let rel = Path::new(value)
        .strip_prefix(OWNER)
        .map_err(|_| "visual evidence is outside declared logical artifact owner")?
        .to_str()
        .ok_or("visual owner path encoding")?;
    pathsafe(rel)?;
    Ok(rel.into())
}
fn run_ref(value: &str) -> Result<()> {
    pathsafe(value)?;
    let p = value.split('/').collect::<Vec<_>>();
    ensure(
        p.len() == 3 && p[0] == EXPERIMENT && p[2] == "variant-C",
        "visual experiment/variant route drift",
    )
}
fn map_by<'a>(
    values: &'a [Value],
    key: &str,
    expected: usize,
) -> Result<BTreeMap<String, &'a Value>> {
    let mut out = BTreeMap::new();
    for row in values {
        ensure(
            out.insert(s(&row[key])?.into(), row).is_none(),
            "visual duplicate identity",
        )?;
    }
    ensure(out.len() == expected, "visual identity count drift")?;
    Ok(out)
}
fn same_metadata(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.size() == b.size()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
struct Files<'a> {
    ctx: &'a ResearchExecution,
    held: BTreeMap<String, Held>,
    raw: BTreeMap<String, Vec<u8>>,
    meta_bytes: usize,
}
impl<'a> Files<'a> {
    fn new(ctx: &'a ResearchExecution) -> Self {
        Self {
            ctx,
            held: BTreeMap::new(),
            raw: BTreeMap::new(),
            meta_bytes: 0,
        }
    }
    fn bind(&mut self, reference: &str, cap: u64) -> Result<&mut Held> {
        pathsafe(reference)?;
        if !self.held.contains_key(reference) {
            ensure(self.held.len() < 512, "visual held input count bound")?;
            let h = Held::open(self.ctx, reference, cap)?;
            self.held.insert(reference.into(), h);
        }
        let h = self.held.get_mut(reference).unwrap();
        ensure(h.metadata.len() <= cap, "visual reused input bound")?;
        Ok(h)
    }
    fn bytes(&mut self, reference: &str) -> Result<&[u8]> {
        if !self.raw.contains_key(reference) {
            let ctx = self.ctx;
            let h = self.bind(reference, META)?;
            let bytes = ctx.read_file(&mut h.file, META)?;
            ensure(
                bytes.len() <= ALL_META - self.meta_bytes,
                "visual aggregate metadata byte bound",
            )?;
            self.meta_bytes += bytes.len();
            self.raw.insert(reference.into(), bytes);
        }
        Ok(&self.raw[reference])
    }
    fn json(&mut self, reference: &str) -> Result<Value> {
        let raw = self.bytes(reference)?;
        let v = crate::zarathustra_lexical::parse(raw, META as usize)?;
        ensure(v.is_object(), "visual metadata object required")?;
        Ok(v)
    }
    fn fix(&mut self, reference: &str, expected: &str, cap: u64) -> Result<()> {
        ensure(
            self.bind(reference, cap)?.digest == expected,
            "visual input digest drift",
        )
    }
    fn record(&mut self, reference: &str, source: bool) -> Result<Value> {
        let h = self.bind(reference, META)?;
        let m = h.metadata.permissions().mode() & 0o777;
        ensure([0o600, 0o644].contains(&m), "visual evidence mode drift")?;
        Ok(
            json!({"sha256":h.digest,"bytes":h.metadata.len(),"mode":format!("{m:04o}"),"source_bearing":source}),
        )
    }
    fn verify(&mut self) -> Result<()> {
        for h in self.held.values_mut() {
            h.verify(self.ctx)?;
        }
        Ok(())
    }
}
fn crosswalk(source: &Value, visual: &Value) -> Result<BTreeMap<String, String>> {
    let mut sources = BTreeMap::new();
    for group in arr(&source["source_groups"])? {
        for v in arr(&group["samples"])? {
            ensure(
                sources
                    .insert(
                        s(&v["sample_id"])?.to_owned(),
                        s(&v["anchor_ref"])?.to_owned(),
                    )
                    .is_none(),
                "duplicate source sample",
            )?;
        }
    }
    let mut result = BTreeMap::new();
    let mut count = 0;
    for group in arr(&visual["source_groups"])? {
        for v in arr(&group["samples"])? {
            let from = sources
                .get(s(&v["source_sample_id"])?)
                .ok_or("missing source sample")?;
            ensure(
                result
                    .insert(from.clone(), s(&v["anchor_ref"])?.into())
                    .is_none(),
                "duplicate source/visual crosswalk",
            )?;
            count += 1;
        }
    }
    ensure(
        sources.len() == 36 && count == 36 && result.len() == 36,
        "visual source crosswalk count drift",
    )?;
    Ok(result)
}
fn pngs(files: &mut Files<'_>, render: &Value, input: &Value) -> Result<Value> {
    let rendered = map_by(arr(&render["renders"])?, "sample_id", 36)?;
    let inputs = map_by(arr(&input["page_images"])?, "id", 36)?;
    let mut records = Vec::new();
    let mut total = 0u64;
    for (id, r) in rendered {
        let observed = inputs.get(&id).ok_or("render missing from visual input")?;
        let path = Path::new(s(&r["png_ref"])?);
        let logical = if path.is_absolute() {
            path.to_owned()
        } else {
            Path::new(s(&render["artifact_root"])?).join(path)
        };
        let rel = owner_ref(logical.to_str().ok_or("PNG path encoding")?)?;
        let ctx = files.ctx;
        let h = files.bind(&rel, 64 * 1024 * 1024)?;
        ensure(h.metadata.len() >= 24, "PNG header incomplete")?;
        let mut header = [0u8; 24];
        ctx.read_exact(&mut h.file, &mut header)?;
        ensure(&header[..8] == b"\x89PNG\r\n\x1a\n", "PNG signature drift")?;
        let width = u32::from_be_bytes(header[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(header[20..24].try_into().unwrap());
        eq(&r["width_pixels"], json!(width), "PNG width")?;
        eq(&r["height_pixels"], json!(height), "PNG height")?;
        eq(&r["png_sha256"], json!(h.digest), "PNG digest")?;
        eq(&r["png_bytes"], json!(h.metadata.len()), "PNG bytes")?;
        for k in ["png_sha256", "png_bytes", "page", "item_ref", "language"] {
            eq(&observed[k], r[k].clone(), "PNG input binding")?;
        }
        eq(
            &observed["visual_anchor_ref"],
            r["anchor_ref"].clone(),
            "PNG visual anchor",
        )?;
        total = total
            .checked_add(h.metadata.len())
            .ok_or("PNG total overflow")?;
        records.push(json!({"sample_id":id,"sha256":h.digest,"bytes":h.metadata.len()}));
    }
    eq(&render["total_png_bytes"], json!(total), "PNG total")?;
    Ok(json!({"count":36,"total_bytes":total,"set_sha256":sha(&canonical(json!(records))?)}))
}
fn controls(
    files: &mut Files<'_>,
    fixed: &Value,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>> {
    eq(&fixed["rerun_performed"], json!(false), "control rerun")?;
    eq(&fixed["source_text_copied"], json!(false), "control text")?;
    let controls = map_by(arr(&fixed["controls"])?, "variant", 2)?;
    ensure(
        controls.keys().map(String::as_str).eq(["A", "B"]),
        "fixed control variants",
    )?;
    let mut out = BTreeMap::new();
    for (variant, c) in controls {
        let root = owner_ref(s(&c["run_ref"])?)?;
        files.fix(
            &format!("{root}/run.receipt.json"),
            s(&c["run_receipt_sha256"])?,
            META,
        )?;
        let mut rows = BTreeMap::new();
        for r in arr(&c["result_digests"])? {
            let id = s(&r["query_id"])?;
            pathsafe(id)?;
            ensure(!id.contains('/'), "query filename required")?;
            let rel = format!("{root}/raw-output/query-results/{id}.json");
            files.fix(&rel, s(&r["sha256"])?, META)?;
            ensure(
                rows.insert(id.into(), files.json(&rel)?).is_none(),
                "duplicate control query",
            )?;
        }
        ensure(rows.len() == 20, "control query count drift")?;
        out.insert(variant, rows);
    }
    Ok(out)
}
fn norm(v: &Value) -> Result<f64> {
    let values = arr(v)?;
    ensure(values.len() == 2048, "vector dimension drift")?;
    let (mut sum, mut correction) = (0.0f64, 0.0f64);
    for v in values {
        let x = number(v)?;
        let value = x * x;
        ensure(value.is_finite(), "vector square overflow")?;
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    let result = (sum + correction).sqrt();
    ensure(
        result.is_finite() && (result - 1.0).abs() <= 0.00001,
        "serialized vector norm exceeds limit",
    )?;
    Ok(result)
}
fn index<'a>(
    files: &mut Files<'_>,
    path: &str,
    v: &'a Value,
    input: &Value,
    lifecycle: &Value,
) -> Result<(BTreeMap<String, &'a Value>, Vec<f64>)> {
    for (k, w) in [
        ("vector_dimension", json!(2048)),
        ("normalization", json!("L2")),
        ("distance", json!("cosine-via-normalized-dot-product")),
        ("source_or_query_text_included", json!(false)),
    ] {
        eq(&v[k], w, k)?;
    }
    let inputs = map_by(arr(&input["page_images"])?, "id", 36)?;
    let indexed = map_by(arr(&v["images"])?, "visual_sample_id", 36)?;
    let mut norms = Vec::new();
    // The audit's first36 records follow the serialized index order.
    for record in arr(&v["images"])? {
        let id = s(&record["visual_sample_id"])?;
        let expected = inputs.get(id).ok_or("indexed image absent from input")?;
        norms.push(norm(&record["vector"])?);
        for k in [
            "png_sha256",
            "source_anchor_ref",
            "visual_anchor_ref",
            "source_sample_id",
            "item_ref",
            "page",
            "language",
        ] {
            eq(&record[k], expected[k].clone(), "index/input binding")?;
        }
    }
    let h = files.bind(path, META)?;
    for stage in ["first_materialization", "rebuild"] {
        eq(
            &lifecycle[stage]["sha256"],
            json!(h.digest),
            "index lifecycle digest",
        )?;
        eq(
            &lifecycle[stage]["bytes"],
            json!(h.metadata.len()),
            "index lifecycle bytes",
        )?;
    }
    eq(
        &lifecycle["digest_stable"],
        json!(true),
        "index digest stability",
    )?;
    eq(
        &lifecycle["deletion_proof"]["absent_after_delete"],
        json!(true),
        "index deletion proof",
    )?;
    Ok((indexed, norms))
}
fn normalization(
    runtime: &Value,
    input: &Value,
    query_ids: &[String],
    norms: &[f64],
) -> Result<Value> {
    let audit = &runtime["bridge_normalization_audit"];
    let records = arr(&audit["records"])?;
    ensure(records.len() == 76, "normalization record count")?;
    eq(&audit["record_count"], json!(76), "normalization count")?;
    let mut ids = arr(&input["page_images"])?
        .iter()
        .map(|r| s(&r["id"]).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    ids.extend_from_slice(query_ids);
    ids.extend_from_slice(query_ids);
    let (mut pre, mut post) = (Vec::new(), Vec::new());
    for (i, r) in records.iter().enumerate() {
        let o = r.as_object().ok_or("normalization object required")?;
        ensure(
            o.len() == 5
                && [
                    "ordinal",
                    "role",
                    "id",
                    "pre_serialization_norm",
                    "post_serialization_norm",
                ]
                .iter()
                .all(|k| o.contains_key(*k)),
            "normalization fields drift",
        )?;
        eq(&r["ordinal"], json!(i + 1), "normalization ordinal")?;
        eq(&r["id"], json!(ids[i]), "normalization identity")?;
        eq(
            &r["role"],
            json!(if i < 36 {
                "page-image"
            } else if i < 56 {
                "query-first"
            } else {
                "query-warm"
            }),
            "normalization role",
        )?;
        pre.push(number(&r["pre_serialization_norm"])?);
        post.push(number(&r["post_serialization_norm"])?);
    }
    let min = |v: &[f64]| v.iter().copied().reduce(f64::min).unwrap();
    let max = |v: &[f64]| v.iter().copied().reduce(f64::max).unwrap();
    let error = |v: &[f64]| v.iter().map(|x| (x - 1.0).abs()).reduce(f64::max).unwrap();
    for (k, n) in [
        ("pre_serialization_norm_min", min(&pre)),
        ("pre_serialization_norm_max", max(&pre)),
        ("max_abs_pre_serialization_norm_error", error(&pre)),
        ("post_serialization_norm_min", min(&post)),
        ("post_serialization_norm_max", max(&post)),
        ("max_abs_post_serialization_norm_error", error(&post)),
    ] {
        ensure(number(&audit[k])? == n, "normalization summary drift")?;
    }
    ensure(
        error(&pre) <= 0.02 && error(&post) <= 0.00001,
        "normalization limit exceeded",
    )?;
    ensure(norms.len() == 36, "page norm count")?;
    for (i, n) in norms.iter().enumerate() {
        ensure((n - post[i]).abs() <= 0.000001, "page norm/audit drift")?;
    }
    Ok(
        json!({"maximum_pre_normalization_error":error(&pre),"maximum_post_normalization_error":error(&post)}),
    )
}
fn anchors(variant: &str, v: &Value) -> Result<Vec<Value>> {
    arr(&v[if variant == "A" {
        "results"
    } else {
        "reranked_results"
    }])?
    .iter()
    .map(|r| {
        let anchor = if variant == "A" {
            &r["source_anchor_ref"]
        } else {
            &r["payload"]["source_anchor_ref"]
        };
        s(anchor)?;
        Ok(anchor.clone())
    })
    .collect()
}
struct QueryResults {
    summary: Value,
    paths: Vec<String>,
}
fn queries(
    files: &mut Files<'_>,
    run: &str,
    prior: &str,
    plan: &Value,
    content: &Value,
    cross: &BTreeMap<String, String>,
    controls: &BTreeMap<String, BTreeMap<String, Value>>,
    index: &BTreeMap<String, &Value>,
    input: &Value,
    comparison: &Value,
    metrics: &Value,
) -> Result<QueryResults> {
    let plans = map_by(arr(&plan["queries"])?, "query_id", 20)?;
    let contents = arr(&content["queries"])?;
    map_by(contents, "query_id", 20)?;
    let ids = contents
        .iter()
        .map(|r| s(&r["query_id"]).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    let ordered = arr(&plan["queries"])?
        .iter()
        .map(|r| s(&r["query_id"]).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    ensure(ids == ordered, "query identity/order drift")?;
    let comparisons = map_by(arr(&comparison["queries"])?, "query_id", 20)?;
    let input_visual = arr(&input["page_images"])?
        .iter()
        .map(|r| s(&r["visual_anchor_ref"]))
        .collect::<Result<BTreeSet<_>>>()?;
    let input_source = arr(&input["page_images"])?
        .iter()
        .map(|r| s(&r["source_anchor_ref"]))
        .collect::<Result<BTreeSet<_>>>()?;
    let (
        mut evaluable,
        mut hits,
        mut recovery,
        mut hard_slots,
        mut hard_presence,
        mut cross_queries,
        mut cross_hits,
        mut identical,
    ) = (0, 0, 0, 0, 0, 0, 0, 0);
    let (mut hard_outranks, mut paths) = (Vec::new(), Vec::new());
    let mut triggers = BTreeSet::new();
    for (i, id) in ids.iter().enumerate() {
        pathsafe(id)?;
        ensure(!id.contains('/'), "query filename required")?;
        let path = format!("{run}/raw-output/query-results/{id}.json");
        let result = files.json(&path)?;
        paths.push(path);
        let p = plans[id];
        let query_text = s(&contents[i]["text"])?;
        eq(
            &result["query_text_sha256"],
            json!(sha(query_text.as_bytes())),
            "query text digest",
        )?;
        let expected = arr(&p["expected_source_anchor_refs"])?;
        let hard = arr(&p["hard_negative_anchor_refs"])?;
        let convert = |values: &[Value]| -> Result<Vec<Value>> {
            values
                .iter()
                .map(|v| {
                    cross
                        .get(s(v)?)
                        .map(|s| json!(s))
                        .ok_or("query anchor absent from crosswalk".into())
                })
                .collect()
        };
        let expected_visual = convert(expected)?;
        let hard_visual = convert(hard)?;
        for (k, w) in [
            (
                "model_proposed_expected_source_anchor_refs",
                json!(expected),
            ),
            (
                "model_proposed_expected_visual_anchor_refs",
                json!(expected_visual),
            ),
            (
                "model_proposed_hard_negative_source_anchor_refs",
                json!(hard),
            ),
            (
                "model_proposed_hard_negative_visual_anchor_refs",
                json!(hard_visual),
            ),
        ] {
            eq(&result[k], w, k)?;
        }
        let ranked = arr(&result["results"])?;
        ensure(ranked.len() == 10, "ranked result count")?;
        let mut visual = Vec::new();
        let mut source = Vec::new();
        let mut scores = Vec::new();
        let mut seen = BTreeSet::new();
        for (rank, r) in ranked.iter().enumerate() {
            eq(&r["rank"], json!(rank + 1), "rank order")?;
            let va = s(&r["visual_anchor_ref"])?;
            let sa = s(&r["source_anchor_ref"])?;
            ensure(
                input_visual.contains(va) && input_source.contains(sa) && seen.insert(va),
                "ranked anchor resolution/uniqueness",
            )?;
            ensure(
                cross.get(sa).is_some_and(|v| v == va),
                "ranked source/visual crosswalk",
            )?;
            let ir = index
                .get(s(&r["visual_sample_id"])?)
                .ok_or("ranked sample absent from index")?;
            eq(&ir["visual_anchor_ref"], json!(va), "ranked index anchor")?;
            visual.push(json!(va));
            source.push(json!(sa));
            scores.push(number(&r["score"])?);
        }
        ensure(
            scores.windows(2).all(|v| v[0] >= v[1]),
            "ranking scores not descending",
        )?;
        for k in [
            "result_visual_anchor_refs",
            "warm_result_visual_anchor_refs",
        ] {
            eq(&result[k], json!(visual), k)?;
        }
        eq(
            &result["ranking_stable_across_repeat"],
            json!(true),
            "repeat ranking",
        )?;
        let mut expected_ranks = BTreeMap::new();
        for anchor in &expected_visual {
            if let Some(pos) = visual.iter().position(|v| v == anchor) {
                expected_ranks.insert(s(anchor)?.to_owned(), pos + 1);
            }
        }
        eq(
            &result["model_proposed_expected_visual_ranks"],
            json!(expected_ranks),
            "expected ranks",
        )?;
        let c_hit = !expected_ranks.is_empty();
        if p["expected_behavior"] != "coverage-failure" {
            evaluable += 1;
            hits += usize::from(c_hit);
        }
        let hard_ranks = hard_visual
            .iter()
            .filter_map(|a| visual.iter().position(|v| v == a).map(|p| p + 1))
            .collect::<Vec<_>>();
        hard_slots += hard_visual.len();
        hard_presence += hard_ranks.len();
        if let (Some(hr), Some(er)) = (hard_ranks.iter().min(), expected_ranks.values().min()) {
            if hr < er {
                hard_outranks.push(id.clone());
                triggers.insert(id.clone());
            }
        }
        if p["category"] == "cross-lingual" {
            cross_queries += 1;
            cross_hits += usize::from(c_hit);
            triggers.insert(id.clone());
        }
        let a = anchors("A", controls["A"].get(id).ok_or("missing A query")?)?;
        let b = anchors("B", controls["B"].get(id).ok_or("missing B query")?)?;
        let a_hit = expected.iter().any(|v| a.contains(v));
        let b_hit = expected.iter().any(|v| b.contains(v));
        if !expected_visual.is_empty() && c_hit && !a_hit && !b_hit {
            recovery += 1;
            triggers.insert(id.clone());
        }
        let row = comparisons.get(id).ok_or("missing query comparison")?;
        for (k, w) in [
            ("A_model_proposed_target_present", json!(a_hit)),
            ("A_source_anchor_refs", json!(a)),
            ("B_model_proposed_target_present", json!(b_hit)),
            ("B_source_anchor_refs", json!(b)),
            ("C_model_proposed_target_present", json!(c_hit)),
            ("C_source_anchor_refs", json!(source)),
            ("C_visual_anchor_refs", json!(visual)),
            (
                "model_proposed_expected_source_anchor_refs",
                json!(expected),
            ),
            (
                "model_proposed_expected_visual_anchor_refs",
                json!(expected_visual),
            ),
        ] {
            eq(&row[k], w, k)?;
        }
        let old = files.json(&format!("{prior}/raw-output/query-results/{id}.json"))?;
        let old_scores = arr(&old["results"])?
            .iter()
            .map(|r| number(&r["score"]))
            .collect::<Result<Vec<_>>>()?;
        if old["result_visual_anchor_refs"] == json!(visual) && old_scores == scores {
            identical += 1;
        }
    }
    eq(
        &metrics["repeat_ranking_stability"]["stable_queries"],
        json!(20),
        "stable ranking metric",
    )?;
    eq(
        &metrics["model_proposed_target_at_10"],
        json!({"present":hits,"evaluable_queries":evaluable,"status":"advisory-nonhuman-not-a-quality-score"}),
        "target metric",
    )?;
    for (v, n, label) in [
        (
            &metrics["coverage_recovery_advisory"]["C_hit_where_A_and_B_missed"],
            recovery,
            "coverage recovery",
        ),
        (
            &metrics["model_proposed_hard_negative_presence"]["present"],
            hard_presence,
            "hard-negative presence",
        ),
        (
            &metrics["model_proposed_hard_negative_presence"]["declared_slots"],
            hard_slots,
            "hard-negative slots",
        ),
        (
            &metrics["cross_lingual"]["queries"],
            cross_queries,
            "cross-language queries",
        ),
        (
            &metrics["cross_lingual"]["model_proposed_expected_hits"],
            cross_hits,
            "cross-language hits",
        ),
        (
            &metrics["source_anchor_resolution"]["resolved_ranked_results"],
            200,
            "resolved results",
        ),
    ] {
        eq(v, json!(n), label)?;
    }
    ensure(identical == 20, "prior/current ranking parity")?;
    let trigger_ids = ids
        .iter()
        .filter(|id| triggers.contains(*id))
        .collect::<Vec<_>>();
    eq(
        &json!(trigger_ids),
        json!([
            "tos-query-003",
            "tos-query-009",
            "tos-query-010",
            "tos-query-011",
            "tos-query-020"
        ]),
        "trigger query set",
    )?;
    Ok(QueryResults {
        paths,
        summary: json!({"stable":20,"evaluable":evaluable,"expected_hits":hits,"coverage_recovery":recovery,"hard_slots":hard_slots,"hard_presence":hard_presence,"hard_outranks":hard_outranks,"cross_queries":cross_queries,"cross_hits":cross_hits,"resolved":200,"r8_r9_identical":identical,"trigger_query_ids":trigger_ids}),
    })
}
const ARTIFACTS: [(&str, &str); 12] = [
    ("run_receipt", "run.receipt.json"),
    ("experiment_spec", "experiment.spec.json"),
    ("preflight", "receipts/preflight.json"),
    (
        "input_manifest",
        "inputs/visual-retrieval-input-manifest.json",
    ),
    ("fixed_controls", "inputs/fixed-controls.json"),
    ("vector_index", "derived-index/qwen3-vl-page-vectors.json"),
    ("abc_comparison", "raw-output/abc-anchor-comparison.json"),
    ("index_lifecycle", "receipts/qwen-vl-index-lifecycle.json"),
    ("invocation", "receipts/qwen-vl-invocation.json"),
    (
        "runtime_and_model",
        "receipts/qwen-vl-runtime-and-model.json",
    ),
    ("metrics", "metrics/qwen-vl-visual-retrieval-summary.json"),
    (
        "host_resource_launch",
        "receipts/abyss-machine-resource-launch.json",
    ),
];
fn runtime_metadata(runtime: &Value, invocation: &Value) -> Result<Value> {
    for (k, v) in [
        ("schema_version", json!("tos_qwen_vl_runtime_and_model_v2")),
        ("network_used_during_inference", json!(false)),
        ("unreviewed_remote_code_executed", json!(false)),
        ("reviewed_upstream_helper_executed", json!(true)),
    ] {
        eq(&runtime[k], v, k)?;
    }
    eq(
        &invocation["runtime_manifest_sha256"],
        json!(digest("runtime_manifest")),
        "runtime manifest",
    )?;
    ensure(
        Path::new(s(&invocation["argv"][0])?)
            .file_name()
            .and_then(|s| s.to_str())
            == Some("tos_foundation_lab.py"),
        "host laboratory CLI identity",
    )?;
    let guard = &runtime["bridge_offline_guard"];
    eq(&guard["active"], json!(true), "offline guard")?;
    eq(
        &guard["attempted_events"],
        json!([]),
        "offline attempted events",
    )?;
    eq(
        &guard["permitted_local_events"],
        json!([{"event":"socket.bind","host":"::1","port":0,"scope":"loopback-ephemeral-capability-probe"}]),
        "offline permitted events",
    )?;
    let mut records = BTreeMap::new();
    let mut bytes = 0u64;
    for a in arr(&runtime["model"]["artifacts"])? {
        let rel = model_ref(a)?;
        let n = uint(&a["bytes"])?;
        ensure(n <= 8 * 1024 * 1024 * 1024, "model artifact byte bound")?;
        bytes = bytes.checked_add(n).ok_or("model byte overflow")?;
        let digest = s(&a["sha256"])?;
        ensure(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x)),
            "model digest syntax",
        )?;
        ensure(
            records
                .insert(rel.clone(), json!({"ref":rel,"sha256":digest,"bytes":n}))
                .is_none(),
            "duplicate model artifact",
        )?;
    }
    ensure(
        records.len() == 18 && bytes <= 8 * 1024 * 1024 * 1024,
        "model artifact set bound",
    )?;
    eq(
        &runtime["model"]["artifact_count"],
        json!(records.len()),
        "model count",
    )?;
    eq(
        &runtime["model"]["artifact_bytes"],
        json!(bytes),
        "model bytes",
    )?;
    Ok(
        json!({"model_artifact_count":records.len(),"model_artifact_bytes":bytes,"model_artifact_set_sha256":sha(&canonical(json!(records.into_values().collect::<Vec<_>>()))?),"runner_sha256":invocation["runner_sha256"],"bridge_sha256":invocation["bridge_sha256"],"reviewed_helper_sha256":invocation["upstream_helper_sha256"]}),
    )
}
fn model_ref(a: &Value) -> Result<String> {
    for k in ["relative_path", "path", "name"] {
        if let Some(r) = a[k].as_str().filter(|v| !v.is_empty()) {
            pathsafe(r)?;
            return Ok(r.into());
        }
    }
    Err("model artifact reference required".into())
}
fn verify_model(
    model: &mut Files<'_>,
    implementation: &mut Files<'_>,
    runtime: &Value,
    invocation: &Value,
) -> Result<()> {
    for (r, k) in [
        ("qwen_vl_visual_retrieval.py", "runner_sha256"),
        ("qwen_vl_embedding_bridge.py", "bridge_sha256"),
    ] {
        implementation.fix(r, s(&invocation[k])?, META)?;
    }
    model.fix(
        s(&runtime["model"]["helper_relative_path"])?,
        s(&invocation["upstream_helper_sha256"])?,
        META,
    )?;
    for a in arr(&runtime["model"]["artifacts"])? {
        let rel = model_ref(a)?;
        let h = model.bind(&rel, 8 * 1024 * 1024 * 1024)?;
        eq(
            &json!(h.metadata.len()),
            a["bytes"].clone(),
            "model file bytes",
        )?;
        eq(&json!(h.digest), a["sha256"].clone(), "model file digest")?;
    }
    Ok(())
}
struct Directories(Vec<(File, Metadata)>);
impl Directories {
    fn verify(&self) -> Result<()> {
        for (f, m) in &self.0 {
            ensure(
                same_metadata(m, &f.metadata().map_err(|e| e.to_string())?),
                "visual scanned directory changed",
            )?;
        }
        Ok(())
    }
}
fn private_patterns(content: &Value) -> Result<regex::bytes::RegexSet> {
    let mut patterns = Vec::new();
    let mut total = 0usize;
    for q in arr(&content["queries"])? {
        for k in ["text", "fts5_query"] {
            if let Some(v) = q[k].as_str().filter(|v| !v.is_empty()) {
                total += v.len();
                ensure(total <= 65536, "private query pattern byte bound")?;
                patterns.push(regex::escape(v));
            }
        }
    }
    ensure(
        !patterns.is_empty() && patterns.len() <= 40,
        "private query pattern count",
    )?;
    regex::bytes::RegexSetBuilder::new(patterns)
        .unicode(false)
        .size_limit(4 * 1024 * 1024)
        .build()
        .map_err(|e| e.to_string())
}
fn withholding(
    files: &mut Files<'_>,
    run: &str,
    patterns: &regex::bytes::RegexSet,
) -> Result<Directories> {
    fn walk(
        files: &mut Files<'_>,
        dir: File,
        relative: &str,
        depth: usize,
        count: &mut usize,
        held: &mut Directories,
        patterns: &regex::bytes::RegexSet,
    ) -> Result<()> {
        ensure(
            depth <= 12 && held.0.len() < 64,
            "visual run directory bound",
        )?;
        let metadata = dir.metadata().map_err(|e| e.to_string())?;
        let mut children = Vec::new();
        for e in std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))
            .map_err(|e| e.to_string())?
        {
            files.ctx.tick(1)?;
            *count += 1;
            ensure(*count <= 256, "visual run entry bound")?;
            let e = e.map_err(|e| e.to_string())?;
            let name = e
                .file_name()
                .into_string()
                .map_err(|_| "visual entry encoding")?;
            pathsafe(&name)?;
            ensure(!name.contains('/'), "visual leaf required")?;
            children.push((name, e.file_type().map_err(|e| e.to_string())?));
        }
        children.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, kind) in children {
            let rel = format!("{relative}/{name}");
            if kind.is_dir() {
                let child = tos_fd_open::open_directory_at(&dir, Path::new(&name))
                    .map_err(|e| e.to_string())?;
                walk(files, child, &rel, depth + 1, count, held, patterns)?;
            } else {
                ensure(
                    kind.is_file(),
                    "visual run refuses symlink or special entry",
                )?;
                let ext = Path::new(&name)
                    .extension()
                    .and_then(|v| v.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                ensure(
                    !["png", "jpg", "jpeg", "pdf", "epub", "tif", "tiff"].contains(&ext.as_str()),
                    "copied source or image file in run",
                )?;
                if ["json", "jsonl"].contains(&ext.as_str()) {
                    let ctx = files.ctx;
                    let raw = files.bytes(&rel)?;
                    ctx.tick(raw.len() as u64)?;
                    ensure(!patterns.is_match(raw), "private query text found in run")?;
                }
            }
        }
        ensure(
            same_metadata(&metadata, &dir.metadata().map_err(|e| e.to_string())?),
            "visual run changed while scanning",
        )?;
        held.0.push((dir, metadata));
        Ok(())
    }
    let mut dir =
        tos_fd_open::reopen_directory(files.ctx.root_directory()).map_err(|e| e.to_string())?;
    for part in run.split('/') {
        dir = tos_fd_open::open_directory_at(&dir, Path::new(part)).map_err(|e| e.to_string())?;
    }
    let mut held = Directories(Vec::new());
    walk(files, dir, run, 0, &mut 0, &mut held, patterns)?;
    Ok(held)
}
fn file_set_record(files: &mut Files<'_>, root: &str, paths: &[String]) -> Result<Value> {
    ensure(
        !paths.is_empty() && paths.len() <= 256,
        "visual file set count bound",
    )?;
    let mut ordered = paths.to_vec();
    ordered.sort();
    ensure(
        !ordered.windows(2).any(|v| v[0] == v[1]),
        "visual duplicate file set entry",
    )?;
    let mut records = Vec::new();
    let mut bytes = 0u64;
    for r in ordered {
        let h = files.bind(&r, META)?;
        bytes = bytes
            .checked_add(h.metadata.len())
            .ok_or("visual file set byte overflow")?;
        records.push(json!({"ref":r.strip_prefix(&format!("{root}/")).ok_or("query run prefix")?,"sha256":h.digest,"bytes":h.metadata.len()}));
    }
    Ok(
        json!({"file_count":records.len(),"total_bytes":bytes,"set_sha256":sha(&canonical(json!(records))?),"source_bearing":false}),
    )
}
pub struct Options<'a> {
    pub inspect: bool,
    pub build: bool,
    pub artifact_root: &'a Path,
    pub run: &'a str,
    pub prior_run: &'a str,
    pub query_content: &'a Path,
    pub model_root: Option<&'a Path>,
    pub implementation_root: Option<&'a Path>,
    pub output_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, opt: Options<'_>) -> Result<Value> {
    run_ref(opt.run)?;
    run_ref(opt.prior_run)?;
    ensure(
        opt.run != opt.prior_run,
        "visual current/prior runs must differ",
    )?;
    ensure(
        !opt.build || opt.generation.is_some(),
        "build requires a distinct generation",
    )?;
    if let Some(g) = opt.generation {
        ensure(
            !g.is_empty()
                && g.len() <= 48
                && g.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                && g.as_bytes()[0].is_ascii_alphanumeric()
                && g.as_bytes()[g.len() - 1].is_ascii_alphanumeric(),
            "visual generation syntax",
        )?;
    }
    let query_parent = opt.query_content.parent().ok_or("query parent required")?;
    let query_name = opt
        .query_content
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("query filename required")?;
    if let Some(output) = opt.output_root {
        for root in [
            Some(ctx.root()),
            Some(opt.artifact_root),
            Some(query_parent),
            opt.model_root,
            opt.implementation_root,
        ]
        .into_iter()
        .flatten()
        {
            ensure(
                !output.starts_with(root) && !root.starts_with(output),
                "visual output must be separate from all inputs",
            )?;
        }
    }
    let artifact_ctx = ctx.select_directory(opt.artifact_root)?;
    let query_ctx = ctx.select_directory(query_parent)?;
    let mut sources = Files::new(ctx);
    let mut files = Files::new(&artifact_ctx);
    let mut private = Files::new(&query_ctx);
    let mut source = BTreeMap::new();
    for (k, r) in [
        ("tree_plan", PLAN.into()),
        ("source_sample_plan", format!("{BASE}sample-plan.json")),
        (
            "visual_sample_plan",
            format!("{BASE}ocr-visual-samples.json"),
        ),
        ("query_plan", format!("{BASE}retrieval-queries.json")),
    ] {
        sources.fix(&r, digest(k), META)?;
        source.insert(k, sources.json(&r)?);
    }
    private.fix(query_name, digest("query_content"), META)?;
    let query_content = private.json(query_name)?;
    let input_mode = private
        .bind(query_name, META)?
        .metadata
        .permissions()
        .mode()
        & 0o777;
    ensure(
        [0o600, 0o644].contains(&input_mode),
        "private query content mode",
    )?;
    let mut values = BTreeMap::new();
    let mut private_artifacts = json!({});
    for (k, r) in ARTIFACTS {
        let rel = format!("{}/{r}", opt.run);
        private_artifacts[k] = files.record(&rel, false)?;
        values.insert(k, files.json(&rel)?);
    }
    let run_receipt = &values["run_receipt"];
    let input = &values["input_manifest"];
    let runtime = &values["runtime_and_model"];
    let invocation = &values["invocation"];
    let metrics = &values["metrics"];
    let comparison = &values["abc_comparison"];
    let resource_launch = &values["host_resource_launch"];
    let plan = &source["tree_plan"];
    for (k, v) in [
        ("experiment_id", json!(EXPERIMENT)),
        ("variant", json!("C")),
        ("status", json!("awaiting-triggered-review")),
        ("errors", json!([])),
        ("retention_decision", json!("pending")),
        ("run_id", json!(opt.run.split('/').nth(1).unwrap())),
    ] {
        eq(&run_receipt[k], v, k)?;
    }
    for k in [
        "tree_plan",
        "source_sample_plan",
        "visual_sample_plan",
        "query_plan",
        "query_content",
        "render_manifest",
    ] {
        eq(&input[k]["sha256"], json!(digest(k)), k)?;
    }
    eq(
        &input["source_or_query_content_copied"],
        json!(false),
        "input content posture",
    )?;
    let render_ref = owner_ref(s(&input["render_manifest"]["ref"])?)?;
    files.fix(&render_ref, digest("render_manifest"), META)?;
    let render = files.json(&render_ref)?;
    let cross = crosswalk(&source["source_sample_plan"], &source["visual_sample_plan"])?;
    let pngs = pngs(&mut files, &render, input)?;
    let controls = controls(&mut files, &values["fixed_controls"])?;
    let (indexed, norms) = index(
        &mut files,
        &format!("{}/derived-index/qwen3-vl-page-vectors.json", opt.run),
        &values["vector_index"],
        input,
        &values["index_lifecycle"],
    )?;
    let model = runtime_metadata(runtime, invocation)?;
    let query_ids = arr(&query_content["queries"])?
        .iter()
        .map(|r| s(&r["query_id"]).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    let normalization = normalization(runtime, input, &query_ids, &norms)?;
    let query = queries(
        &mut files,
        opt.run,
        opt.prior_run,
        &source["query_plan"],
        &query_content,
        &cross,
        &controls,
        &indexed,
        input,
        comparison,
        metrics,
    )?;
    let query_evidence = &query.summary;
    let patterns = private_patterns(&query_content)?;
    let directories = withholding(&mut files, opt.run, &patterns)?;
    let prior_path = format!("{}/run.receipt.json", opt.prior_run);
    let prior_run_receipt = files.json(&prior_path)?;
    let prior_runtime = files.json(&format!(
        "{}/receipts/qwen-vl-runtime-and-model.json",
        opt.prior_run
    ))?;
    eq(
        &prior_runtime["schema_version"],
        json!("tos_qwen_vl_runtime_and_model_v1"),
        "prior runtime schema",
    )?;
    ensure(
        prior_runtime.get("bridge_normalization_audit").is_none(),
        "prior normalization audit must be absent",
    )?;
    eq(
        &prior_run_receipt["run_id"],
        json!(opt.prior_run.split('/').nth(1).unwrap()),
        "prior run identity",
    )?;
    for v in [comparison, metrics] {
        eq(&v["promotion_authorized"], json!(false), "promotion")?;
        eq(&v["winner"], Value::Null, "winner")?;
    }
    eq(
        &comparison["human_relevance_status"],
        json!("not_started"),
        "human relevance",
    )?;
    eq(
        &metrics["quality"],
        json!({"human_ndcg_at_10":null,"human_hard_negative_error_rate":null,"reason":"no declared human review trigger has opened"}),
        "human quality",
    )?;
    for (k, v) in [
        ("ok", json!(true)),
        ("dry_run", json!(false)),
        ("blocked_reasons", json!([])),
        ("denied_reasons", json!([])),
    ] {
        eq(&resource_launch[k], v, k)?;
    }
    eq(
        &resource_launch["request"]["force"],
        json!(false),
        "host force",
    )?;
    ensure(
        number(&resource_launch["request"]["memory_demand_mib"])? == 12288.,
        "host memory demand",
    )?;
    eq(
        &resource_launch["request"]["demand_key"],
        json!("tos-qwen-vl-visual-c"),
        "host demand key",
    )?;
    eq(
        &resource_launch["request"]["command"][2],
        json!(format!("{OWNER}/{}", opt.run)),
        "host run root",
    )?;
    eq(
        &resource_launch["execution"]["returncode"],
        json!(0),
        "host return code",
    )?;
    eq(
        &resource_launch["execution"]["systemd"]["result"],
        json!("success"),
        "host result",
    )?;
    let peaks = &resource_launch["startup_admission"]["demand_observation"]["peaks"];
    eq(&peaks["ok"], json!(true), "host peaks")?;
    private_artifacts["query_results"] = file_set_record(&mut files, opt.run, &query.paths)?;
    sources.fix(
        BUILDER,
        &sha(include_bytes!("research_visual_result.rs")),
        META,
    )?;
    let generator = json!({"ref":BUILDER,"sha256":sources.bind(BUILDER,META)?.digest});
    let prior_digest = files.bind(&prior_path, META)?.digest.clone();
    let mut receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/visual-retrieval-result-receipt.schema.json","schema_version":"tos_visual_retrieval_result_receipt_v1","generated_or_authored":"generated_from_private_direct_visual_retrieval_run","receipt_id":"visual-retrieval-result:zarathustra-foundation-pilot-v1.c.qwen3-vl-embedding-2b.v1","recorded_at_utc":run_receipt["finished_at_utc"],"status":"c-executed-triggered-review-open-unscheduled","experiment_id":EXPERIMENT,"variant":"C","plan":{"plan_id":plan["plan_id"],"ref":PLAN,"sha256":digest("tree_plan"),"frozen_before_challenger_outputs":true},"generator":generator,"private_run":{"artifact_owner":"abyss-machine:storage/artifacts/tree-of-sophia-foundation-lab","relative_ref":opt.run,"run_id":run_receipt["run_id"],"status":run_receipt["status"],"started_at_utc":run_receipt["started_at_utc"],"finished_at_utc":run_receipt["finished_at_utc"],"retention_decision":"retain","visibility":"owner-local-only","manual_review_count":0,"model_inspection_count":1},"prior_audit_incomplete_run":{"relative_ref":opt.prior_run,"run_id":prior_run_receipt["run_id"],"classification":"executed-audit-incomplete","run_receipt_sha256":prior_digest,"missing_evidence":"persisted-normalization-audit","rankings_and_scores_identical_query_count":query_evidence["r8_r9_identical"],"query_count":20,"retained":true,"promotion_authorized":false},"method":{"model_id":"Qwen/Qwen3-VL-Embedding-2B","repository_revision":"9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda","vector_dimension":2048,"distance":"cosine-via-normalized-dot-product","normalization":"L2","runtime_route":"isolated-offline-python-3.12-cpu","network_used":false,"unreviewed_remote_code_executed":false},"inputs":{"work_ref":plan["work_ref"],"query_count":20,"page_image_count":36,"tree_plan_sha256":digest("tree_plan"),"source_sample_plan_sha256":digest("source_sample_plan"),"visual_sample_plan_sha256":digest("visual_sample_plan"),"query_plan_sha256":digest("query_plan"),"query_content_sha256":digest("query_content"),"render_manifest_sha256":digest("render_manifest"),"png_set_sha256":pngs["set_sha256"],"png_total_bytes":pngs["total_bytes"],"source_or_query_content_copied":false},"runtime":{"runtime_id":runtime["runtime"]["runtime_id"],"manifest_sha256":digest("runtime_manifest"),"artifact_set_sha256":runtime["runtime"]["artifact_set_sha256"],"normalization_audit_schema":runtime["schema_version"],"normalization_record_count":76,"offline_attempted_event_count":arr(&runtime["bridge_offline_guard"]["attempted_events"])?.len(),"permitted_loopback_bind_count":arr(&runtime["bridge_offline_guard"]["permitted_local_events"])?.len()},"private_artifacts":private_artifacts,"mechanical_reconstruction":{"status":"completed-without-unresolved-mechanical-issue","performed_by":"software:tos-native-rust","frozen_digest_checks":7,"png_fixity_checks":36,"control_fixity_checks":42,"model_artifact_fixity_checks":model["model_artifact_count"],"page_vector_count":36,"vector_dimension":2048,"normalization_record_count":76,"resolved_ranked_result_count":query_evidence["resolved"],"private_text_leak_count":0,"copied_source_or_image_file_count":0,"r8_r9_identical_rankings_and_scores":query_evidence["r8_r9_identical"],"query_vector_recomputation":"not-possible-query-vectors-not-persisted"},"advisory_results":{"stable_rankings":query_evidence["stable"],"query_count":20,"model_proposed_target_at_10":query_evidence["expected_hits"],"evaluable_query_count":query_evidence["evaluable"],"coverage_recovery":query_evidence["coverage_recovery"],"hard_negative_presence":query_evidence["hard_presence"],"hard_negative_slots":query_evidence["hard_slots"],"hard_negative_outranks_expected":arr(&query_evidence["hard_outranks"])?.len(),"cross_language_expected_hits":query_evidence["cross_hits"],"cross_language_query_count":query_evidence["cross_queries"],"resolved_ranked_results":query_evidence["resolved"],"advisory_only":true},"performance":{"model_load_seconds":metrics["model_load_seconds"],"image_encoding_seconds_total":metrics["image_encoding_seconds_total"],"first_query_latency_ms_median":metrics["first_query_end_to_end_latency_ms_median"],"warm_query_latency_ms_median":metrics["warm_query_end_to_end_latency_ms_median"],"total_runner_seconds":metrics["total_runner_seconds"],"host_elapsed_seconds":resource_launch["elapsed_sec"],"index_bytes":metrics["index_bytes"],"packet_bytes_before_host_receipt":metrics["packet_bytes"],"bridge_peak_rss_bytes":metrics["bridge_peak_rss_bytes"],"host_runner_peak_rss_bytes":metrics["host_runner_peak_rss_bytes"],"host_cgroup_memory_peak_bytes":peaks["memory_peak_bytes"],"host_cgroup_swap_peak_bytes":peaks["memory_swap_peak_bytes"],"host_cgroup_footprint_peak_mib":peaks["footprint_peak_mib"]},"triggered_review":{"status":"open-unscheduled","routine_review_scheduled":false,"review_opened":true,"human_judgment_status":"not_started","human_debt_count":0,"trigger_conditions_met":["challenger-materially-changes-a-source-return-route","hard-negative-or-cross-language-behavior-remains-ambiguous"],"query_ids":query_evidence["trigger_query_ids"],"query_count":arr(&query_evidence["trigger_query_ids"])?.len(),"review_scope":"criteria-only-source-visible-ranking-review-no-retyping","review_interface_materialized":false},"quality_and_promotion":{"human_ndcg_at_10":null,"human_hard_negative_error_rate":null,"human_review_minutes":null,"winner":null,"method_adoption_under_consideration":false,"promotion_authorized":false},"rights_and_visibility":{"private_source_and_query_content":"owner-local-only","tracked_receipt_contains_source_strings":false,"tracked_receipt_contains_query_strings":false,"tracked_receipt_contains_vectors":false,"public_page_images_authorized":false,"publication_authorized":false},"authority_boundary":"This receipt records one exact offline page-image retrieval run, its frozen inputs, private artifact fixity, normalization audit, source-anchor closure, resource cost and stated trigger decision."});
    receipt["runtime"]
        .as_object_mut()
        .unwrap()
        .extend(model.as_object().unwrap().clone());
    receipt["mechanical_reconstruction"]
        .as_object_mut()
        .unwrap()
        .extend(normalization.as_object().unwrap().clone());
    let suffix = opt
        .generation
        .map(|g| format!(".native-{g}"))
        .unwrap_or_default();
    receipt["receipt_id"] = json!(format!(
        "visual-retrieval-result:zarathustra-foundation-pilot-v1.c.qwen3-vl-embedding-2b.v1{suffix}"
    ));
    if opt.generation.is_none() {
        sources.fix(RECEIPT, OLD_RECEIPT_SHA, META)?;
        let old = sources.json(RECEIPT)?;
        receipt["generator"] = old["generator"].clone();
        receipt["mechanical_reconstruction"]["performed_by"] =
            old["mechanical_reconstruction"]["performed_by"].clone();
        // Preserve the authored boundary of the pinned historical receipt.
        // Fresh generations use the current boundary above.
        receipt["authority_boundary"] = old["authority_boundary"].clone();
        if receipt != old {
            let mut paths = Vec::new();
            receipt_differences(&receipt, &old, "", &mut paths);
            if !paths.is_empty() {
                return Err(format!(
                    "visual historical result drift at {}",
                    paths.join(", ")
                ));
            }
        }
    }
    let schema_ref = "ToS/contracts/visual-retrieval-result-receipt.schema.json";
    let uri = format!("https://tree-of-sophia.local/{schema_ref}");
    let schemas = tos_validation::SchemaBackendProbe::new(
        vec![tos_validation::SchemaResource {
            uri: uri.clone(),
            raw: sources.bytes(schema_ref)?.to_vec(),
        }],
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("visual schema: {e:?}"))?;
    let encoded = canonical(receipt.clone())?;
    ensure(
        schemas
            .is_valid_raw(&uri, &encoded)
            .map_err(|e| format!("visual schema: {e:?}"))?,
        "visual result schema refused",
    )?;
    ensure(!patterns.is_match(&encoded), "private query text in result")?;
    if opt.inspect {
        files.verify()?;
        sources.verify()?;
        private.verify()?;
        directories.verify()?;
        ctx.check()?;
        return Ok(
            json!({"status":"retained-run-evidence-verified-model-files-unverified","receipt_projection_matches_retained":true,"full_result_registration":false,"model_and_implementation_fixity":"not-performed","run":opt.run,"png_fixity_checks":36,"page_vector_count":36,"normalization_record_count":76,"query_count":20,"resolved_ranked_result_count":200,"private_text_leak_count":0,"copied_source_or_image_file_count":0,"advisory_results":query.summary,"normalization":normalization,"budgets":ctx.budget_report()}),
        );
    }
    let model_ctx = ctx.select_directory(opt.model_root.ok_or("explicit model root required")?)?;
    let impl_ctx = ctx.select_directory(
        opt.implementation_root
            .ok_or("explicit implementation root required")?,
    )?;
    let mut model_files = Files::new(&model_ctx);
    let mut implementation = Files::new(&impl_ctx);
    verify_model(&mut model_files, &mut implementation, runtime, invocation)?;
    files.verify()?;
    sources.verify()?;
    private.verify()?;
    model_files.verify()?;
    implementation.verify()?;
    directories.verify()?;
    let receipt_ref = opt
        .generation
        .map(|g| format!("{}.native-{g}.json", RECEIPT.strip_suffix(".json").unwrap()))
        .unwrap_or(RECEIPT.into());
    if opt.generation.is_some() {
        let output = ctx.select_output_directory(
            opt.output_root.ok_or("explicit output root required")?,
            opt.build,
        )?;
        match output.source_file(&receipt_ref, META) {
            Ok(mut file) => {
                let mode = file
                    .metadata()
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o777;
                ensure(mode == 0o644, "visual existing output mode")?;
                ensure(
                    output.read_file(&mut file, META)? == encoded,
                    "visual output differs; generation conflict",
                )?;
            }
            Err(_) => {
                ensure(opt.build, "visual output absent")?;
                output.write(&receipt_ref, &encoded, 0o644, true)?;
            }
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":"visual-result-registered","receipt_ref":receipt_ref,"generation":opt.generation,"model_artifact_fixity_checks":model["model_artifact_count"],"resolved_ranked_result_count":200,"promotion_authorized":false,"budgets":ctx.budget_report()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_floating_observation_preserves_value_not_exponent_spelling() {
        let old: Value = serde_json::from_str("{\"error\":3.5762786865234375e-07}").unwrap();
        let current = json!({"error":3.5762786865234375e-7});
        let mut differences = Vec::new();
        receipt_differences(&current, &old, "", &mut differences);
        assert!(differences.is_empty());
        receipt_differences(
            &json!({"error":3.576278686523438e-7}),
            &old,
            "",
            &mut differences,
        );
        assert_eq!(differences, ["/error"]);
        differences.clear();
        receipt_differences(
            &json!({"count":9_007_199_254_740_993u64}),
            &json!({"count":9_007_199_254_740_992u64}),
            "",
            &mut differences,
        );
        assert_eq!(differences, ["/count"]);
    }
    #[test]
    fn fixed_vector_and_private_withholding_controls() {
        let mut vector = vec![json!(0.0); 2048];
        vector[0] = json!(1.0);
        assert_eq!(norm(&json!(vector)).unwrap(), 1.0);
        vector[1] = json!(0.1);
        assert!(norm(&json!(vector)).is_err());
        assert!(norm(&json!([1])).is_err());
        let content = json!({"queries":[{"text":"private needle","fts5_query":"secret.*(exact)"}]});
        let p = private_patterns(&content).unwrap();
        assert!(p.is_match(b"private needle"));
        assert!(p.is_match(b"secret.*(exact)"));
        assert!(!p.is_match(b"secretyyexact"));
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("run")).unwrap();
        std::fs::write(
            t.path().join("run/result.json"),
            b"{\"private\":\"private needle\"}",
        )
        .unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        assert!(withholding(&mut Files::new(&ctx), "run", &p).is_err());
        std::fs::write(
            t.path().join("run/result.json"),
            b"{\"source_bearing\":false}",
        )
        .unwrap();
        let mut files = Files::new(&ctx);
        let dirs = withholding(&mut files, "run", &p).unwrap();
        files.verify().unwrap();
        dirs.verify().unwrap();
        std::fs::write(t.path().join("run/copied.png"), b"image").unwrap();
        assert!(dirs.verify().is_err());
        assert!(withholding(&mut Files::new(&ctx), "run", &p).is_err());
    }
    #[test]
    fn explicit_model_fixity_and_original_input_reverification() {
        let m = tempfile::tempdir().unwrap();
        let imp = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new_visual_result(m.path(), 30, None).unwrap();
        let ic = ctx.select_directory(imp.path()).unwrap();
        for r in ["qwen_vl_visual_retrieval.py", "qwen_vl_embedding_bridge.py"] {
            std::fs::write(imp.path().join(r), b"reviewed").unwrap();
        }
        std::fs::write(m.path().join("helper.py"), b"helper").unwrap();
        std::fs::write(m.path().join("weights"), b"model").unwrap();
        let runtime = json!({"model":{"helper_relative_path":"helper.py","artifacts":[{"relative_path":"weights","sha256":sha(b"model"),"bytes":5}]}});
        let invocation = json!({"runner_sha256":sha(b"reviewed"),"bridge_sha256":sha(b"reviewed"),"upstream_helper_sha256":sha(b"helper")});
        let mut mf = Files::new(&ctx);
        let mut implementations = Files::new(&ic);
        verify_model(&mut mf, &mut implementations, &runtime, &invocation).unwrap();
        mf.verify().unwrap();
        implementations.verify().unwrap();
        std::fs::write(m.path().join("weights"), b"other").unwrap();
        assert!(mf.verify().is_err());
        assert!(
            verify_model(
                &mut Files::new(&ctx),
                &mut Files::new(&ic),
                &runtime,
                &invocation
            )
            .is_err()
        );
    }
}
#[cfg(test)]
mod retained_contract_tests {
    use super::*;
    #[test]
    fn retained_and_native_receipts_preserve_review_and_private_boundaries() {
        let raw = include_bytes!(
            "../../../../ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/visual-retrieval-result.c-qwen3-vl-embedding-2b.v1.json"
        );
        assert_eq!(sha(raw), OLD_RECEIPT_SHA);
        let mut receipt: Value = serde_json::from_slice(raw).unwrap();
        let uri = "https://tree-of-sophia.local/ToS/contracts/visual-retrieval-result-receipt.schema.json";
        let validator = tos_validation::SchemaBackendProbe::new(
            vec![tos_validation::SchemaResource {
                uri: uri.into(),
                raw: include_bytes!(
                    "../../../../ToS/contracts/visual-retrieval-result-receipt.schema.json"
                )
                .to_vec(),
            }],
            tos_validation::FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let valid = |v: &Value| {
            validator
                .is_valid_raw(uri, &canonical(v.clone()).unwrap())
                .unwrap()
        };
        assert!(valid(&receipt));
        assert_eq!(
            receipt["triggered_review"]["query_ids"],
            json!([
                "tos-query-003",
                "tos-query-009",
                "tos-query-010",
                "tos-query-011",
                "tos-query-020"
            ])
        );
        for (k, n) in [
            ("model_proposed_target_at_10", 19),
            ("evaluable_query_count", 19),
            ("coverage_recovery", 1),
            ("hard_negative_presence", 9),
            ("hard_negative_slots", 10),
            ("hard_negative_outranks_expected", 4),
        ] {
            assert_eq!(receipt["advisory_results"][k], json!(n));
        }
        for k in [
            "human_ndcg_at_10",
            "human_hard_negative_error_rate",
            "human_review_minutes",
            "winner",
        ] {
            assert!(receipt["quality_and_promotion"][k].is_null());
        }
        assert_eq!(
            receipt["quality_and_promotion"]["promotion_authorized"],
            false
        );
        assert_eq!(receipt["triggered_review"]["human_debt_count"], 0);
        assert_eq!(
            receipt["triggered_review"]["routine_review_scheduled"],
            false
        );
        assert_eq!(
            receipt["triggered_review"]["review_interface_materialized"],
            false
        );
        let encoded = std::str::from_utf8(raw).unwrap();
        assert!(!encoded.contains("/srv/") && !encoded.contains("/home/"));
        receipt["receipt_id"] = json!(format!(
            "{}.native-test-1",
            s(&receipt["receipt_id"]).unwrap()
        ));
        receipt["generator"] =
            json!({"ref":BUILDER,"sha256":sha(include_bytes!("research_visual_result.rs"))});
        receipt["mechanical_reconstruction"]["performed_by"] = json!("software:tos-native-rust");
        assert!(valid(&receipt));
        receipt["triggered_review"]["human_judgment_status"] = json!("completed");
        assert!(!valid(&receipt));
    }
    #[test]
    fn file_set_output_withholds_paths_and_source_strings() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("run")).unwrap();
        std::fs::write(t.path().join("run/a.json"), b"{\"query_id\":\"q1\"}\n").unwrap();
        std::fs::write(t.path().join("run/b.json"), b"{\"query_id\":\"q2\"}\n").unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        let mut files = Files::new(&ctx);
        let set = file_set_record(
            &mut files,
            "run",
            &["run/b.json".into(), "run/a.json".into()],
        )
        .unwrap();
        assert_eq!(set["file_count"], 2);
        let raw = canonical(set).unwrap();
        let text = std::str::from_utf8(&raw).unwrap();
        assert!(!text.contains("q1") && !text.contains("a.json"));
        files.verify().unwrap();
    }
}
