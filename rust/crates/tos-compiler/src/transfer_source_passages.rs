//! Exact private German layer slices for the frozen transfer route frame.
//! Authored image marker returns remain evidence, never new textual acceptance.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{
        Node, ensure, fresh_or_matching_limit, load, private_boundary, s, schema, sha, utc_now,
        xml_with_doctype,
    },
    transfer_target_passages::{
        Poppler, descendants, encode, f, flat_map, json_lines, jsonl, key, n, node_text,
        read_optional, round,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufReader, Cursor, Read},
    os::unix::fs::PermissionsExt,
    path::Path,
};
#[path = "transfer_source_passages/constants.rs"]
mod constants;
#[path = "transfer_source_passages/records.rs"]
mod records;
pub use crate::transfer_target_passages::Action;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: usize = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/transfer_source_passages.rs";
const LEGACY_EVENT_SHA: &str = "fff2ce883dbe9db7148e5b0ab873a9e6d5a38445f7ed1f029a165f766dd54119";
const LEGACY_BUILDER_SHA: &str = "f7fc1d941ea8a6fb7331c7a2ac9b85af7902fe3aa600d129450e7bc2cb4d6898";
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("required array".into())
}
fn order(a: &Value, b: &Value) -> std::cmp::Ordering {
    f(&a["y_min"])
        .unwrap()
        .total_cmp(&f(&b["y_min"]).unwrap())
        .then(f(&a["x_min"]).unwrap().total_cmp(&f(&b["x_min"]).unwrap()))
        .then(n(&a["order"]).unwrap().cmp(&n(&b["order"]).unwrap()))
}
fn attribute(node: &Node, name: &str) -> Result<f64> {
    let v = node
        .attrs
        .get(name)
        .ok_or_else(|| format!("XML coordinate absent: {name}"))?
        .parse::<f64>()
        .map_err(|_| "invalid XML coordinate")?;
    ensure(v.is_finite(), "XML coordinate is nonfinite")?;
    Ok(v)
}
fn normalized(text: &str, abbyy: bool) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || (abbyy && c == '»') {
            for lower in c.to_lowercase() {
                out.push(if abbyy {
                    lower
                } else {
                    match lower {
                        'i' | 'l' | 'x' => '1',
                        'o' => '0',
                        's' => '5',
                        'z' => '2',
                        _ => lower,
                    }
                })
            }
        }
    }
    out
}
fn pdf_records(pages: BTreeMap<usize, Value>) -> Result<BTreeMap<usize, Value>> {
    pages.into_iter().map(|(page,v)|{let mut records=vec![];for line in array(&v["lines"])?{if !(50.0..540.0).contains(&f(&line["y_min"])?) {continue}records.push(json!({"page":page,"order":line["source_order"],"x_min":line["x_min"],"y_min":line["y_min"],"x_max":line["x_max"],"y_max":line["y_max"],"text":line["text"],"words":line["words"]}));}Ok((page,json!({"page":page,"width":v["width"],"height":v["height"],"records":records})))}).collect()
}
fn xml_page(node: &Node, number: usize, abbyy: bool, body_y_min: f64) -> Result<Value> {
    let width = attribute(node, "width")?;
    let height = attribute(node, "height")?;
    ensure(width > 0.0 && height > 0.0, "source XML page dimensions")?;
    let mut records = vec![];
    let mut nodes = vec![];
    descendants(node, if abbyy { "par" } else { "LINE" }, &mut nodes);
    for (ord, node) in nodes.into_iter().enumerate() {
        if abbyy {
            let mut chars = vec![];
            descendants(node, "charParams", &mut chars);
            let text = chars
                .into_iter()
                .map(node_text)
                .collect::<String>()
                .trim()
                .to_owned();
            let mut lines = vec![];
            descendants(node, "line", &mut lines);
            if text.is_empty() || lines.is_empty() {
                continue;
            }
            let bound = |name: &str, min: bool| -> Result<i64> {
                let mut v = if min { i64::MAX } else { i64::MIN };
                for line in &lines {
                    let n = line
                        .attrs
                        .get(name)
                        .ok_or("ABBYY coordinate absent")?
                        .parse::<i64>()
                        .map_err(|_| "invalid ABBYY integer coordinate")?;
                    v = if min { v.min(n) } else { v.max(n) };
                }
                Ok(v)
            };
            let x = bound("l", true)?;
            let y = bound("t", true)?;
            if !(400..3500).contains(&y) {
                continue;
            }
            records.push(json!({"page":number,"order":ord,"x_min":x,"y_min":y,"x_max":bound("r",false)?,"y_max":bound("b",false)?,"text":text,"words":text.split_whitespace().collect::<Vec<_>>(),"compact":normalized(&text,true)}));
        } else {
            let mut words = vec![];
            for word in node.children("WORD") {
                let coords = word
                    .attrs
                    .get("coords")
                    .ok_or("DjVu word coordinates absent")?
                    .split(',')
                    .map(|v| {
                        v.parse::<f64>()
                            .map_err(|_| "DjVu word coordinate invalid".to_owned())
                    })
                    .collect::<Result<Vec<_>>>()?;
                if coords.len() < 4 {
                    continue;
                }
                ensure(
                    coords.iter().all(|v| v.is_finite()),
                    "nonfinite DjVu geometry",
                )?;
                let text = node_text(word).trim().to_owned();
                if text.is_empty() {
                    continue;
                }
                words.push(json!({"text":text,"x_min":coords[0],"y_min":coords[3],"x_max":coords[2],"y_max":coords[1]}));
            }
            if words.is_empty() {
                continue;
            }
            let bound = |name: &str, min: bool| -> Result<f64> {
                let mut v = if min {
                    f64::INFINITY
                } else {
                    f64::NEG_INFINITY
                };
                for word in &words {
                    let n = f(&word[name])?;
                    v = if min { v.min(n) } else { v.max(n) };
                }
                Ok(v)
            };
            let y = bound("y_min", true)?;
            if !(body_y_min..height - 300.0).contains(&y) {
                continue;
            }
            let texts = words
                .iter()
                .map(|v| s(&v["text"]))
                .collect::<Result<Vec<_>>>()?;
            records.push(json!({"page":number,"order":ord,"x_min":bound("x_min",true)?,"y_min":y,"x_max":bound("x_max",false)?,"y_max":bound("y_max",false)?,"text":texts.join(" "),"words":texts,"word_records":words}));
        }
    }
    if abbyy {
        records.sort_by(order)
    }
    Ok(json!({"page":number,"width":width,"height":height,"records":records}))
}
// The compressed bytes are held by the exact file owner. The XML stream is
// bounded independently and only requested page trees survive a page boundary.
fn xml_pages(ctx: &ResearchExecution,reader:impl Read,abbyy:bool,expected:usize,wanted:&BTreeSet<usize>,body_y_min:f64)->Result<BTreeMap<usize,Value>>{
 let mut result=BTreeMap::new();visit_xml_pages(ctx,reader,abbyy,expected,wanted,|page,node|{result.insert(page,xml_page(node,page,abbyy,body_y_min)?);Ok(())})?;Ok(result)
}
pub(super) fn visit_xml_pages(
    ctx: &ResearchExecution,
    reader: impl Read,
    abbyy: bool,
    expected: usize,
    wanted: &BTreeSet<usize>,
    mut visit: impl FnMut(usize, &Node) -> Result<()>,
) -> Result<()> {
    use quick_xml::{Reader, Writer, events::Event};
    struct Bounded<'a, R> {
        inner: R,
        ctx: &'a ResearchExecution,
        used: usize,
    }
    impl<R: Read> Read for Bounded<'_, R> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            self.ctx.check().map_err(std::io::Error::other)?;
            let left = (192 * 1024 * 1024usize).saturating_sub(self.used);
            if left == 0 {
                return Err(std::io::Error::other("source XML expanded byte limit"));
            }
            let cap = out.len().min(left).min(65536);
            let n = self.inner.read(&mut out[..cap])?;
            self.used += n;
            Ok(n)
        }
    }
    let mut reader = Reader::from_reader(BufReader::with_capacity(
        65536,
        Bounded {
            inner: reader,
            ctx,
            used: 0,
        },
    ));
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    let mut page = 0usize;
    let mut depth = 0usize;
    let mut page_depth = None;
    let mut writer: Option<Writer<Vec<u8>>> = None;
    let mut visited = 0usize;
    let mut namespace = false;
    let mut djvu_root_seen = false;
    loop {
        ctx.tick(1)?;
        buf.clear();
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?;
        match &event {
            Event::Start(e) => {
                depth += 1;
                ensure(depth <= 128, "source XML depth")?;
                if !abbyy && depth == 1 {
                    ensure(
                        !djvu_root_seen && e.local_name().as_ref() == b"DjVuXML",
                        "DjVuXML root drift",
                    )?;
                    djvu_root_seen = true;
                }
                if abbyy && depth == 1 {
                    namespace = e.attributes().filter_map(|a| a.ok()).any(|a| {
                        a.key.as_ref() == b"xmlns"
                            && a.value.as_ref()
                                == b"http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml"
                    });
                    ensure(namespace, "ABBYY namespace drift")?;
                }
                if e.local_name().as_ref()
                    == if abbyy {
                        b"page".as_slice()
                    } else {
                        b"OBJECT".as_slice()
                    }
                {
                    ensure(page_depth.is_none(), "nested source page")?;
                    page += 1;
                    ensure(page <= expected, "source page count drift")?;
                    page_depth = Some(depth);
                    if wanted.contains(&page) {
                        writer = Some(Writer::new(Vec::new()))
                    }
                }
            }
            Event::Empty(_) if !abbyy && depth == 0 => {
                return Err("DjVuXML extra or empty root".into());
            }
            Event::Empty(e)
                if e.local_name().as_ref()
                    == if abbyy {
                        b"page".as_slice()
                    } else {
                        b"OBJECT".as_slice()
                    } =>
            {
                ensure(page_depth.is_none(), "nested empty source page")?;
                page += 1;
                ensure(page <= expected, "source empty page count drift")?;
                if wanted.contains(&page) {
                    let mut w = Writer::new(Vec::new());
                    w.write_event(event.clone()).map_err(|e| e.to_string())?;
                    let node = xml_with_doctype(ctx, &w.into_inner(), false)?;
                    visit(page, &node)?;
                    visited += 1;
                }
            }
            Event::DocType(e) => {
                // The retained DjVuXML carrier declares an external DTD. This
                // parser never resolves it; internal/entity declarations fail.
                let declaration = std::str::from_utf8(e.as_ref()).map_err(|e| e.to_string())?;
                ensure(
                    !abbyy
                        && declaration.starts_with("DjVuXML")
                        && !declaration.contains('[')
                        && !declaration.contains("ENTITY"),
                    "source XML DTD/entity refused",
                )?;
            }
            Event::Eof => {
                ensure(
                    depth == 0 && page_depth.is_none(),
                    "source XML unclosed page",
                )?;
                break;
            }
            _ => {}
        }
        if let Some(w) = writer.as_mut() {
            w.write_event(event.clone()).map_err(|e| e.to_string())?;
            ensure(
                w.get_ref().len() <= CAP,
                "single source XML page byte bound",
            )?;
        }
        if matches!(event, Event::End(_)) {
            if page_depth == Some(depth) {
                if let Some(w) = writer.take() {
                    let node = xml_with_doctype(ctx, &w.into_inner(), false)?;
                    visit(page, &node)?;
                    visited += 1;
                }
                page_depth = None;
            }
            depth = depth.checked_sub(1).ok_or("source XML closing depth")?;
        }
    }
    ensure(
        page == expected && visited == wanted.len() && if abbyy { namespace } else { djvu_root_seen },
        "source XML page closure drift",
    )?;
    Ok(())
}
fn marker(pages: &BTreeMap<usize, Value>, page: usize, unit: &str, layer: &str) -> Result<Value> {
    let p = pages.get(&page).ok_or("source boundary page absent")?;
    let mut matches = vec![];
    for record in array(&p["records"])? {
        let found = match layer {
            "abbyy" => s(&record["compact"])? == if unit == "188" { "i8s" } else { unit },
            "pdf" => {
                (0.3..=0.7).contains(&(f(&record["x_min"])? / f(&p["width"])?))
                    && normalized(s(&record["text"])?, false) == unit
            }
            "djvu" => {
                let mut found = false;
                for w in array(&record["word_records"])? {
                    if (0.32..=0.68).contains(&(f(&w["x_min"])? / f(&p["width"])?))
                        && normalized(s(&w["text"])?, false) == unit
                    {
                        found = true;
                        break;
                    }
                }
                found
            }
            _ => return Err("unknown source layer".into()),
        };
        if found {
            matches.push(record.clone())
        }
    }
    ensure(
        matches.len() == 1,
        "source number boundary absent or ambiguous",
    )?;
    let mut r = matches.remove(0);
    r["marker_basis"] = json!(match layer {
        "abbyy" if unit == "188" => "source-visible-label-over-abbyy-ocr-token",
        "abbyy" => "exact-expected-label-in-abbyy-paragraph-layer",
        "pdf" => "exact-normalized-label-in-poppler-pdf-bbox-layer",
        _ => "exact-normalized-label-in-djvu-xml-word-layer",
    });
    Ok(r)
}
fn visible_marker(
    pages: &BTreeMap<usize, Value>,
    slug: &str,
    page: usize,
    unit: &str,
    role: &str,
    inventory: Option<&Value>,
) -> Result<(Value, Value)> {
    let is_pdf = inventory.is_none();
    let returns: Value = serde_json::from_str(if is_pdf {
        PDF_VISIBLE_MARKER_RETURNS
    } else {
        ANTI_JP2_MARKER_RETURNS
    })
    .map_err(|e| e.to_string())?;
    let selected = array(&returns)?.iter().find(|v| {
        v["key"]
            == if is_pdf {
                json!([slug, page, unit])
            } else {
                json!([page, unit])
            }
    });
    let Some(reviewed) = selected else {
        return Ok((
            marker(pages, page, unit, if is_pdf { "pdf" } else { "djvu" })?,
            Value::Null,
        ));
    };
    let p = pages.get(&page).ok_or("visible boundary page absent")?;
    let matches = array(&p["records"])?
        .iter()
        .filter(|v| v["order"] == reviewed["record_order"])
        .collect::<Vec<_>>();
    ensure(matches.len() == 1, "reviewed following line absent")?;
    let mut marker = matches[0].clone();
    let bbox = json!([
        marker["x_min"],
        marker["y_min"],
        marker["x_max"],
        marker["y_max"]
    ]);
    ensure(
        bbox == reviewed["line_bbox"],
        "reviewed following line geometry drift",
    )?;
    let mut evidence = json!({"boundary_role":role,"expected_unit_key":unit});
    let dims = if is_pdf {
        (
            f(&reviewed["image_width_pixels"])?,
            f(&reviewed["image_height_pixels"])?,
        )
    } else {
        (f(&p["width"])?, f(&p["height"])?)
    };
    if is_pdf {
        evidence["pdf_page"] = json!(page);
        evidence["pdf_resource_id"] = json!(format!("pdf-page-{page:04}"));
        for k in [
            "image_object_number",
            "image_object_generation",
            "image_width_pixels",
            "image_height_pixels",
        ] {
            evidence[k] = reviewed[k].clone()
        }
        evidence["image_kind"] = json!("jbig2-mask");
    } else {
        let inv = inventory.unwrap();
        let files = array(&inv["files"])?;
        for (profile, id) in [
            ("jp2_zip_pages_v1", format!("jp2-page-{page:04}")),
            ("scandata_pages_v1", format!("scandata-page-{page:04}")),
        ] {
            let file = files
                .iter()
                .find(|v| v["profile"] == profile)
                .ok_or("source image inventory profile absent")?;
            let resource = array(&file["resources"])?
                .iter()
                .find(|v| v["resource_id"] == id)
                .ok_or("source image resource absent")?;
            let loc = &resource["locator"];
            ensure(
                loc["page_index"] == page && loc["leaf_number"] == reviewed["leaf_number"],
                "source image leaf/page relation drift",
            )?;
            if profile == "jp2_zip_pages_v1" {
                ensure(
                    loc["member_path"] == reviewed["member_path"],
                    "JP2 member path drift",
                )?
            } else {
                ensure(
                    f(&loc["width_pixels"])? == dims.0 && f(&loc["height_pixels"])? == dims.1,
                    "scandata page geometry drift",
                )?
            }
        }
        evidence["navigation_page"] = json!(page);
        evidence["leaf_number"] = reviewed["leaf_number"].clone();
        evidence["jp2_resource_id"] = json!(format!("jp2-page-{page:04}"));
        evidence["scandata_resource_id"] = json!(format!("scandata-page-{page:04}"));
        evidence["member_path"] = reviewed["member_path"].clone();
    }
    let b = array(&reviewed["pixel_bbox"])?;
    ensure(b.len() == 4, "reviewed marker bbox length")?;
    evidence["pixel_bbox"] =
        json!({"x":b[0],"y":b[1],"width":b[2],"height":b[3],"coordinate_space":"top_left_pixels"});
    evidence["normalized_bbox"] = json!({"x":round(f(&b[0])?/dims.0,8),"y":round(f(&b[1])?/dims.1,8),"width":round(f(&b[2])?/dims.0,8),"height":round(f(&b[3])?/dims.1,8),"coordinate_space":"normalized_0_1"});
    if is_pdf {
        evidence["following_poppler_record_order"] = reviewed["record_order"].clone();
        evidence["following_poppler_line_bbox"] = json!({"x_min":marker["x_min"],"y_min":marker["y_min"],"x_max":marker["x_max"],"y_max":marker["y_max"]});
    } else {
        evidence["following_djvu_xml_record_order"] = reviewed["record_order"].clone()
    }
    evidence["maker_type"] = json!("model");
    evidence["human_review_performed"] = json!(false);
    marker["marker_basis"] = json!(if is_pdf {
        "model-visible-pdf-image-mask-number-marker-plus-first-following-poppler-pdf-bbox-line"
    } else {
        "model-visible-jp2-number-marker-plus-first-following-djvu-xml-line"
    });
    Ok((marker, evidence))
}
fn point(pages: &BTreeMap<usize, Value>, marker: &Value) -> Result<Value> {
    let p = pages
        .get(&n(&marker["page"])?)
        .ok_or("marker page absent")?;
    let w = f(&p["width"])?;
    let h = f(&p["height"])?;
    Ok(
        json!({"page":marker["page"],"x":round(f(&marker["x_min"])?/w,8),"y":round(f(&marker["y_min"])?/h,8),"width":round((f(&marker["x_max"])?-f(&marker["x_min"])?)/w,8),"height":round((f(&marker["y_max"])?-f(&marker["y_min"])?)/h,8),"coordinate_space":"normalized_0_1","marker_basis":marker["marker_basis"]}),
    )
}
fn extract(
    ctx: &ResearchExecution,
    pages: &BTreeMap<usize, Value>,
    start: usize,
    a: &Value,
    end: usize,
    b: &Value,
) -> Result<(Vec<Value>, String, Vec<Value>)> {
    ensure(start <= end && end - start < 64, "source passage span")?;
    ensure(
        start != end || order(a, b).is_lt(),
        "inverted source boundaries",
    )?;
    let mut selected = vec![];
    let mut regions = vec![];
    for page in start..=end {
        ctx.tick(1)?;
        let p = pages.get(&page).ok_or("source passage page absent")?;
        let mut rows = vec![];
        for row in array(&p["records"])? {
            if (page == start && order(row, a).is_lt()) || (page == end && !order(row, b).is_lt()) {
                continue;
            }
            rows.push(row);
            selected.push(row.clone())
        }
        if !rows.is_empty() {
            let bound = |name: &str, min: bool| -> Result<f64> {
                let mut v = if min {
                    f64::INFINITY
                } else {
                    f64::NEG_INFINITY
                };
                for row in &rows {
                    let n = f(&row[name])?;
                    v = if min { v.min(n) } else { v.max(n) };
                }
                Ok(v)
            };
            let x = bound("x_min", true)?;
            let y = bound("y_min", true)?;
            regions.push(json!({"type":"page_region","page":page,"x":round(x/f(&p["width"])?,8),"y":round(y/f(&p["height"])?,8),"width":round((bound("x_max",false)?-x)/f(&p["width"])?,8),"height":round((bound("y_max",false)?-y)/f(&p["height"])?,8),"coordinate_space":"normalized_0_1"}));
        }
    }
    ensure(!selected.is_empty(), "source passage slice empty")?;
    let mut text = String::new();
    for row in &selected {
        text.push_str(s(&row["text"])?);
        text.push('\n');
        ensure(text.len() <= CAP, "source passage text bound")?;
    }
    Ok((selected, text, regions))
}
struct Inputs {
    raw: BTreeMap<String, Vec<u8>>,
    values: BTreeMap<String, Value>,
    bindings: Vec<Value>,
}
impl Inputs {
    fn read(ctx: &ResearchExecution) -> Result<Self> {
        let mut raw = BTreeMap::new();
        let mut values = BTreeMap::new();
        let mut bindings = vec![];
        for &(r, role) in INPUTS {
            let mut file = ctx.source_file(r, CAP as u64)?;
            let bytes = ctx.read_file(&mut file, CAP as u64)?;
            if r.ends_with(".json") {
                tos_foundation::parse_json(
                    &bytes,
                    tos_foundation::JsonMode::PublishedStrict,
                    tos_foundation::JsonLimits::default(),
                )
                .map_err(|e| e.to_string())?;
                values.insert(
                    r.into(),
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
                );
            }
            bindings.push(json!({"ref":r,"role":role,"sha256":sha(&bytes)}));
            raw.insert(r.into(), bytes);
        }
        Ok(Self {
            raw,
            values,
            bindings,
        })
    }
    fn get(&self, r: &str) -> &Value {
        &self.values[r]
    }
    fn verify(&self, ctx: &ResearchExecution) -> Result<()> {
        for (r, bytes) in &self.raw {
            ensure(
                ctx.read(r)? == *bytes,
                "source metadata changed before publication",
            )?;
        }
        Ok(())
    }
    fn digest(&self, r: &str) -> String {
        sha(&self.raw[r])
    }
    fn rights(&self) -> Result<()> {
        for dir in [
            JENSEITS_ITEM_DIR,
            GENE_ITEM_DIR,
            ANTI_ADDRESS_ITEM_DIR,
            ANTI_NAV_ITEM_DIR,
        ] {
            let m = self.get(&format!("{dir}/item.manifest.json"));
            let r = self.get(&format!("{dir}/rights.json"));
            let refs = array(&r["scope_refs"])?;
            ensure(
                r["visibility"] == "local_only"
                    && r["redistribution_posture"] == "not_authorized"
                    && r["derivative_posture"] == "local_research_only"
                    && refs.contains(&m["item_id"]),
                "source private rights posture/scope drift",
            )?;
            for f in array(&m["payload_files"])? {
                ensure(
                    refs.contains(&f["file_id"]),
                    "source file outside rights scope",
                )?;
            }
        }
        Ok(())
    }
    fn entry(&self, dir: &str, media: &str) -> Result<&Value> {
        let m = self.get(&format!("{dir}/item.manifest.json"));
        let rows = array(&m["payload_files"])?
            .iter()
            .filter(|v| v["media_type"] == media)
            .collect::<Vec<_>>();
        ensure(
            rows.len() == 1,
            "source manifest media type ambiguous/absent",
        )?;
        Ok(rows[0])
    }
    fn witness(&self, dir: &str, media: &str) -> Result<Value> {
        let m = self.get(&format!("{dir}/item.manifest.json"));
        let f = self.entry(dir, media)?;
        Ok(
            json!({"item_ref":m["item_id"],"file_ref":f["file_id"],"file_sha256":f["sha256"],"rights_ref":format!("{dir}/rights.json")}),
        )
    }
}
struct Payload {
    file: File,
    entry: Value,
}
impl Payload {
    fn open(input: &Inputs, ctx: &ResearchExecution, dir: &str, media: &str) -> Result<Self> {
        let entry = input.entry(dir, media)?.clone();
        let reference = format!("{dir}/{}", s(&entry["relative_path"])?);
        let mut file = ctx.source_file(&reference, 128 * 1024 * 1024)?;
        ensure(
            file.metadata().map_err(|e| e.to_string())?.len() == n(&entry["byte_size"])? as u64
                && ctx.hash_file(&mut file, 128 * 1024 * 1024)? == s(&entry["sha256"])?,
            "source payload fixity drift",
        )?;
        Ok(Self { file, entry })
    }
    fn bytes(&mut self, ctx: &ResearchExecution) -> Result<Vec<u8>> {
        ctx.read_file(&mut self.file, n(&self.entry["byte_size"])? as u64)
    }
    fn verify(&mut self, ctx: &ResearchExecution) -> Result<()> {
        ensure(
            ctx.hash_file(&mut self.file, 128 * 1024 * 1024)? == s(&self.entry["sha256"])?,
            "source payload changed before output",
        )
    }
}
fn slug(v: &Value) -> Result<&'static str> {
    let work = s(&v["work_ref"])?;
    if work.ends_with(".jenseits-von-gut-und-boese") {
        Ok("jenseits")
    } else if work.ends_with(".zur-genealogie-der-moral") {
        Ok("genealogie")
    } else if work.ends_with(".der-antichrist") {
        Ok("antichrist")
    } else {
        Err("unsupported transfer source work".into())
    }
}
fn map_ref(slug: &str) -> &'static str {
    match slug {
        "jenseits" => JENSEITS_MAP,
        "genealogie" => GENE_MAP,
        _ => ANTI_MAP,
    }
}
fn passage(anchor: &str) -> String {
    anchor
        .replace("tos.anchor.", "tos.passage.")
        .replace(".pdf-start-page", "")
        .replace(".source-start-page", "")
}
fn layer_for(slug: &str, unit: &str) -> &'static str {
    match (slug, unit) {
        ("jenseits", "32") => "pdf-visible-marker-plus-poppler-pdf-bbox-line",
        ("jenseits", "201") => "poppler-pdf-bbox-line",
        ("jenseits", "202") => "djvu-xml-line",
        ("jenseits", _) => "abbyy-xml-paragraph",
        ("genealogie", "essay-1:10") => "pdf-visible-marker-plus-poppler-pdf-bbox-line",
        ("genealogie", _) => "poppler-pdf-bbox-line",
        (_, "main:8" | "main:9" | "main:43" | "main:44") => "jp2-visible-marker-plus-djvu-xml-line",
        _ => "djvu-xml-line",
    }
}
fn selected_boundaries<'a>(
    maps: &'a BTreeMap<String, BTreeMap<String, Value>>,
    candidate: &Value,
) -> Result<(&'static str, &'a Value, usize, usize)> {
    let slug = slug(candidate)?;
    let unit = maps[slug]
        .get(s(&candidate["qualified_unit_key"])?)
        .ok_or("source map misses frozen route")?;
    let page_field = if slug == "jenseits" {
        "pdf_page"
    } else {
        "navigation_page"
    };
    let start = n(&unit["start"][page_field])?;
    let end = n(&unit["next"][page_field])?;
    ensure(
        start > 0 && end >= start && end - start < 64,
        "source selected page range",
    )?;
    Ok((slug, unit, start, end))
}
pub struct Options<'a> {
    pub action: Action,
    pub input_root: Option<&'a Path>,
    pub output_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
    pub event_id: Option<&'a str>,
}
struct Paths {
    output: String,
    anchors: String,
    private: String,
    event: String,
    set_id: String,
    generation: String,
    historical: bool,
}
impl Paths {
    fn selected(generation: Option<&str>, event: Option<&str>) -> Result<Self> {
        let generation = generation.unwrap_or("v1");
        ensure(
            generation.len() <= 64
                && !generation.is_empty()
                && generation.as_bytes()[0].is_ascii_alphanumeric()
                && generation
                    .as_bytes()
                    .last()
                    .unwrap()
                    .is_ascii_alphanumeric()
                && generation
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "generation syntax",
        )?;
        let historical = generation == "v1";
        let event = event.unwrap_or(EVENT_ID);
        ensure(
            (event == EVENT_ID) == historical
                && event.len() < 512
                && regex::Regex::new(r"^tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*$")
                    .unwrap()
                    .is_match(event),
            "fresh source generation requires its own event ID",
        )?;
        Ok(Self {
            output: if historical {
                OUTPUT_PATH.into()
            } else {
                format!("{GOLD_ROOT}/transfer-source-passage-candidates.{generation}.json")
            },
            anchors: if historical {
                ANCHOR_PATH.into()
            } else {
                format!("{GOLD_ROOT}/transfer-source-passage-anchors.{generation}.jsonl")
            },
            private: if historical {
                LOCAL_CONTENT_ROOT.into()
            } else {
                format!("{GOLD_ROOT}/local-content/transfer-source-passages/{generation}")
            },
            event: event.into(),
            set_id: if historical {
                SET_ID.into()
            } else {
                format!(
                    "tos.transfer-candidate-set.golden-kernel-transfer-source-passages-{generation}"
                )
            },
            generation: generation.into(),
            historical,
        })
    }
    fn candidate_id(&self, target: &str) -> String {
        let id = target.replace(
            "tos-target-passage-candidate-",
            "tos-source-passage-candidate-",
        );
        if self.historical {
            id
        } else {
            format!("{id}-{}", self.generation)
        }
    }
    fn anchor_id(&self, passage: &str) -> String {
        format!(
            "{}.source-passage-candidate-{}",
            passage.replace("tos.passage.", "tos.anchor."),
            self.generation
        )
    }
}
fn witnesses(input: &Inputs, slug: &str, layer: &str) -> Result<(Value, Value, &'static str)> {
    let (dir, address_media) = match slug {
        "jenseits" => (JENSEITS_ITEM_DIR, "application/pdf"),
        "genealogie" => (GENE_ITEM_DIR, "application/pdf"),
        _ => (ANTI_ADDRESS_ITEM_DIR, "image/vnd.djvu"),
    };
    let address = input.witness(dir, address_media)?;
    let (content, relation) = match slug {
        "jenseits" if layer == "abbyy-xml-paragraph" => (
            input.witness(dir, "application/gzip")?,
            "same-fixity-bound-Item-navigation-layer",
        ),
        "jenseits" if layer == "djvu-xml-line" => (
            input.witness(dir, "application/vnd.djvu+xml")?,
            "same-fixity-bound-Item-navigation-layer",
        ),
        "antichrist" => (
            input.witness(ANTI_NAV_ITEM_DIR, "application/vnd.djvu+xml")?,
            "bounded-source-visible-two-page-offset-only-no-textual-identity",
        ),
        _ => (address.clone(), "same-fixity-bound-file-page-index"),
    };
    Ok((address, content, relation))
}
fn private_records(selected: &[Value]) -> Vec<Value> {
    selected.iter().map(|r|json!({"page":r["page"],"x_min":r["x_min"],"y_min":r["y_min"],"x_max":r["x_max"],"y_max":r["y_max"],"text":r["text"],"words":r["words"]})).collect()
}
fn output_entities(
    ctx: &ResearchExecution,
    paths: &Paths,
    payload: &Value,
    raw: &[u8],
    anchor_raw: &[u8],
) -> Result<Vec<Value>> {
    let mut result = vec![
        json!({"ref":paths.output,"role":"tracked-text-free-source-passage-candidate-set","sha256":sha(raw)}),
        json!({"ref":paths.anchors,"role":"tracked-proposed-source-passage-candidate-anchors","sha256":sha(anchor_raw)}),
    ];
    let mut private = BTreeMap::new();
    for c in array(&payload["passage_candidates"])? {
        ctx.tick(1)?;
        private.insert(
            s(&c["private_content_ref"])?,
            c["private_content_sha256"].clone(),
        );
    }
    for (r, h) in private {
        result.push(json!({"ref":r,"role":"gitignored-private-automatic-source-passage-candidate","sha256":h}));
    }
    Ok(result)
}
fn event(ctx: &ResearchExecution, paths: &Paths, journal: &[u8]) -> Result<Option<Value>> {
    let mut found = None;
    for (raw, row) in json_lines(ctx, journal)? {
        if row["event_id"] == paths.event {
            ensure(found.is_none(), "duplicate source event")?;
            if paths.historical {
                ensure(
                    sha(&raw) == LEGACY_EVENT_SHA,
                    "historical source event bytes changed",
                )?;
            }
            found = Some(row)
        }
    }
    if paths.historical {
        ensure(found.is_some(), "historical source event absent")?;
    }
    Ok(found)
}
fn validate_event(
    ctx: &ResearchExecution,
    input: &Inputs,
    paths: &Paths,
    event: &Value,
    entities: &[Value],
) -> Result<()> {
    schema(ctx, "ToS/contracts/provenance-event.schema.json", event)?;
    let mut expected = records::event(
        &paths.event,
        s(&event["started_at"])?,
        PDFTOTEXT_VERSION,
        &input.bindings,
        entities,
        s(&event["method"]["artifact_digest"])?,
        35,
        0,
        35,
        &paths.output,
    );
    if !paths.historical {
        expected["method"]["configuration"]["native_builder_ref"] = json!(BUILDER)
    }
    ensure(
        event["inputs"] == json!(input.bindings)
            && event["outputs"] == json!(entities)
            && event["event_type"] == "segmentation"
            && event["status"] == "completed_with_warnings"
            && event["rights_basis_ref"].is_null()
            && event["receipt_refs"] == json!([paths.output])
            && event["method"]["configuration"] == expected["method"]["configuration"],
        "source provenance closure drift",
    )?;
    if paths.historical {
        ensure(
            event["method"]["artifact_digest"] == LEGACY_BUILDER_SHA,
            "historical source builder digest drift",
        )?
    } else {
        ensure(
            event["agent_refs"]
                == json!([
                    "software:tos-native",
                    format!("software:poppler-{PDFTOTEXT_VERSION}")
                ])
                && event["method"]["runtime"] == "Rust plus Poppler 26.01.0"
                && event["method"]["configuration"]["native_builder_ref"] == BUILDER,
            "native source event producer drift",
        )?
    }
    Ok(())
}
fn expected_evidence(
    input: &Inputs,
    slug: &str,
    unit: &Value,
    start: usize,
    end: usize,
    content: &Value,
) -> Result<Value> {
    let is_pdf = slug != "antichrist";
    let returns: Value = serde_json::from_str(if is_pdf {
        PDF_VISIBLE_MARKER_RETURNS
    } else {
        ANTI_JP2_MARKER_RETURNS
    })
    .map_err(|e| e.to_string())?;
    let inventory = if is_pdf {
        None
    } else {
        Some(input.get(&format!("{ANTI_NAV_ITEM_DIR}/resource-inventory.json")))
    };
    let mut evidence = vec![];
    for (page, key, role) in [
        (start, key(&unit["unit_key"])?, "start"),
        (end, key(&unit["next"]["unit_key"])?, "end_exclusive"),
    ] {
        let r = array(&returns)?.iter().find(|v| {
            v["key"]
                == if is_pdf {
                    json!([slug, page, key])
                } else {
                    json!([page, key])
                }
        });
        let Some(r) = r else { continue };
        let (w, h) = if is_pdf {
            (1.0, 1.0)
        } else {
            let file = array(&inventory.unwrap()["files"])?
                .iter()
                .find(|v| v["profile"] == "scandata_pages_v1")
                .ok_or("scandata profile absent")?;
            let row = array(&file["resources"])?
                .iter()
                .find(|v| v["resource_id"] == format!("scandata-page-{page:04}"))
                .ok_or("scandata selected page absent")?;
            (
                f(&row["locator"]["width_pixels"])?,
                f(&row["locator"]["height_pixels"])?,
            )
        };
        let b = &r["line_bbox"];
        let page_value = json!({"width":w,"height":h,"records":[{"order":r["record_order"],"x_min":b[0],"y_min":b[1],"x_max":b[2],"y_max":b[3]}]});
        let p = BTreeMap::from([(page, page_value)]);
        evidence.push(visible_marker(&p, slug, page, &key, role, inventory)?.1);
    }
    if evidence.is_empty() {
        return Ok(Value::Null);
    }
    Ok(if is_pdf {
        json!({"pdf_witness":content,"relation":"same-PDF-page-image-mask-to-first-following-poppler-bbox-order","marker_returns":evidence})
    } else {
        json!({"image_witness":input.witness(ANTI_NAV_ITEM_DIR,"application/zip")?,"scandata_witness":input.witness(ANTI_NAV_ITEM_DIR,"application/xml")?,"relation":"same-Item-scandata-leaf-to-jp2-member-and-djvu-xml-object-order","marker_returns":evidence})
    })
}
fn validate(
    ctx: &ResearchExecution,
    input: &Inputs,
    paths: &Paths,
    payload: &Value,
    anchors: &[Value],
) -> Result<()> {
    schema(ctx, SCHEMA_PATH, payload)?;
    input.rights()?;
    ensure(
        payload["candidate_set_id"] == paths.set_id
            && payload["provenance_event_ref"] == paths.event
            && payload["target_passage_candidate_set_ref"] == TARGET_CANDIDATE_PATH
            && payload["target_passage_candidate_set_sha256"]
                == input.digest(TARGET_CANDIDATE_PATH)
            && payload["inputs"] == json!(input.bindings),
        "source input/identity closure drift",
    )?;
    let target = array(&input.get(TARGET_CANDIDATE_PATH)["passage_candidates"])?;
    ensure(target.len() == 35, "frozen target frame count")?;
    let target_by_id = target
        .iter()
        .map(|v| Ok((s(&v["passage_candidate_id"])?, v)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure(target_by_id.len() == 35, "duplicate frozen target")?;
    let maps = BTreeMap::from([
        ("jenseits".into(), flat_map(input.get(JENSEITS_MAP))?),
        ("genealogie".into(), flat_map(input.get(GENE_MAP))?),
        ("antichrist".into(), flat_map(input.get(ANTI_MAP))?),
    ]);
    let candidates = array(&payload["passage_candidates"])?;
    ensure(
        candidates.len() == 35 && anchors.len() == 35,
        "source candidate/anchor count drift",
    )?;
    let mut anchor_by_id = BTreeMap::new();
    for a in anchors {
        schema(ctx, SOURCE_ANCHOR_SCHEMA_PATH, a)?;
        ensure(
            anchor_by_id.insert(s(&a["anchor_id"])?, a).is_none(),
            "duplicate source anchor",
        )?;
    }
    let mut ids = BTreeSet::new();
    let mut layers = BTreeMap::<String, usize>::new();
    for c in candidates {
        ctx.tick(1)?;
        let id = s(&c["target_passage_candidate_id"])?;
        ensure(ids.insert(id), "duplicate source route")?;
        let target = target_by_id
            .get(id)
            .ok_or("source candidate leaves frozen frame")?;
        let (slug, unit, start, end) = selected_boundaries(&maps, c)?;
        let layer = layer_for(slug, s(&unit["qualified_unit_key"])?);
        *layers.entry(layer.into()).or_default() += 1;
        for k in [
            "frozen_page_candidate_id",
            "qualified_unit_key",
            "work_ref",
            "source_structural_anchor_ref",
        ] {
            ensure(c[k] == target[k], "source target-frame binding drift")?;
        }
        let expression = s(&input.get(map_ref(slug))["expression_ref"])?;
        let passage = passage(s(&unit["start"]["anchor_ref"])?);
        let expected_id = paths.candidate_id(id);
        let expected_anchor = paths.anchor_id(&passage);
        let private = format!("{}/{}.json", paths.private, expected_id);
        ensure(
            c["source_passage_candidate_id"] == expected_id
                && c["source_passage_ref"] == passage
                && c["source_expression_ref"] == expression
                && c["source_passage_anchor_ref"] == expected_anchor
                && c["private_content_ref"] == private
                && c["boundary_layer"] == layer
                && c["status"] == "materialized-layer-exact-candidate"
                && c["unresolved_boundaries"] == json!([]),
            "source route/layer/output identity drift",
        )?;
        private_boundary(ctx, &private)?;
        let (address, content, relation) = witnesses(input, slug, layer)?;
        ensure(
            c["address_witness"] == address
                && c["content_witness"] == content
                && c["navigation_relation"] == relation,
            "source witness/rights/navigation relation drift",
        )?;
        ensure(
            c["boundary_evidence"] == expected_evidence(input, slug, unit, start, end, &content)?,
            "source-visible marker evidence drift",
        )?;
        ensure(
            c["start"]["page"] == start && c["end_exclusive"]["page"] == end,
            "source boundary map drift",
        )?;
        let span = array(&c["navigation_page_span"])?
            .iter()
            .map(n)
            .collect::<Result<Vec<_>>>()?;
        ensure(
            !span.is_empty()
                && span[0] >= start
                && *span.last().unwrap() <= end
                && span == (*span.first().unwrap()..=*span.last().unwrap()).collect::<Vec<_>>(),
            "source navigation span drift",
        )?;
        let address_span = span
            .iter()
            .map(|v| {
                if slug == "antichrist" {
                    v.checked_sub(2).ok_or("address offset underflow".into())
                } else {
                    Ok(*v)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        ensure(
            c["address_page_span"] == json!(address_span),
            "source address offset drift",
        )?;
        for k in [
            "human_review_performed",
            "accepted_source_text",
            "source_to_target_alignment_created",
            "eligible_for_variant_execution",
        ] {
            ensure(c[k] == false, "source candidate authority escalation")?;
        }
        let anchor = anchor_by_id
            .get(expected_anchor.as_str())
            .ok_or("source anchor absent")?;
        ensure(
            anchor["item_id"] == content["item_ref"]
                && anchor["file_id"] == content["file_ref"]
                && anchor["file_sha256"] == content["file_sha256"]
                && anchor["passage_id"] == passage
                && anchor["status"] == "proposed"
                && anchor["provenance_event_ref"] == paths.event
                && anchor["selector_method"]["configuration_ref"] == paths.output,
            "source anchor witness/event closure drift",
        )?;
        let selectors = array(&anchor["selectors"])?;
        let structural = selectors
            .iter()
            .filter(|v| v["type"] == "structural")
            .collect::<Vec<_>>();
        let regions = selectors
            .iter()
            .filter(|v| v["type"] == "page_region")
            .collect::<Vec<_>>();
        let positions = selectors
            .iter()
            .filter(|v| v["type"] == "text_position")
            .collect::<Vec<_>>();
        ensure(
            structural.len() == 1
                && positions.len() == 1
                && selectors.len() == regions.len() + 2
                && regions
                    .iter()
                    .map(|v| v["page"].clone())
                    .collect::<Vec<_>>()
                    == json!(span).as_array().unwrap().clone()
                && positions[0]["start"] == 0
                && positions[0]["end"] == c["text_character_count"]
                && positions[0]["text_layer_ref"] == private,
            "source selector closure drift",
        )?;
        let mut structure = vec![format!(
            "expression:{}",
            expression.rsplit('.').next().unwrap()
        )];
        if !unit["series_key"].is_null() {
            structure.push(format!("series:{}", s(&unit["series_key"])?))
        }
        structure.push(format!("numbered-unit:{}", s(&unit["unit_key"])?));
        ensure(
            structural[0]["path"] == json!(structure)
                && structural[0]["scheme"] == "transfer-source-layer-exact-numbered-unit-v1",
            "source structural selector drift",
        )?;
    }
    ensure(
        layers
            == BTreeMap::from([
                ("abbyy-xml-paragraph".into(), 12),
                ("djvu-xml-line".into(), 9),
                ("jp2-visible-marker-plus-djvu-xml-line".into(), 4),
                ("pdf-visible-marker-plus-poppler-pdf-bbox-line".into(), 2),
                ("poppler-pdf-bbox-line".into(), 8),
            ]),
        "source layer counts drift",
    )?;
    ensure(
        payload["summary"]
            == json!({"conservative_source_route_count":35,"materialized_source_passage_candidate_count":35,"unresolved_source_boundary_count":0,"accepted_source_passage_count":0,"source_to_target_alignment_count":0,"eligible_target_unit_count":0,"target_gold_count":0,"human_review_count":0}),
        "source summary drift",
    )?;
    Ok(())
}
fn validate_private(ctx: &ResearchExecution, payload: &Value) -> Result<()> {
    for c in array(&payload["passage_candidates"])? {
        let mut f = ctx.source_file(s(&c["private_content_ref"])?, CAP as u64)?;
        let meta = f.metadata().map_err(|e| e.to_string())?;
        ensure(
            meta.len() == n(&c["private_content_bytes"])? as u64
                && meta.permissions().mode() & 0o777 == 0o600
                && ctx.hash_file(&mut f, CAP as u64)? == s(&c["private_content_sha256"])?,
            "source private size/mode/fixity drift",
        )?;
    }
    Ok(())
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let paths = Paths::selected(opts.generation, opts.event_id)?;
    let input = Inputs::read(ctx)?;
    input.rights()?;
    let journal = read_optional(ctx, PROVENANCE_PATH)?.unwrap_or_default();
    let found = event(ctx, &paths, &journal)?;
    let build = matches!(opts.action, Action::Build);
    if matches!(opts.action, Action::ValidateTracked) {
        ensure(
            opts.input_root.is_none(),
            "tracked source validation reads no private input",
        )?;
        let (raw, payload) = load(ctx, &paths.output)?;
        let anchor_raw = ctx.read(&paths.anchors)?;
        let anchors = json_lines(ctx, &anchor_raw)?
            .into_iter()
            .map(|(_, v)| v)
            .collect::<Vec<_>>();
        validate(ctx, &input, &paths, &payload, &anchors)?;
        let entities = output_entities(ctx, &paths, &payload, &raw, &anchor_raw)?;
        validate_event(
            ctx,
            &input,
            &paths,
            found.as_ref().ok_or("source event absent")?,
            &entities,
        )?;
        if let Some(root) = opts.output_root {
            validate_private(&ctx.select_directory(root)?, &payload)?;
        }
        return Ok(
            json!({"status":"passed","mode":"validate-tracked","summary":payload["summary"],"private_payloads_read":false,"private_content_checked":opts.output_root.is_some(),"publication_authorized":false}),
        );
    }
    let source = ctx.select_directory(
        opts.input_root
            .ok_or("explicit private source input root required")?,
    )?;
    let destination = ctx.select_directory(
        opts.output_root
            .ok_or("explicit private source output root required")?,
    )?;
    let target = array(&input.get(TARGET_CANDIDATE_PATH)["passage_candidates"])?;
    ensure(target.len() == 35, "frozen target route count")?;
    let maps = BTreeMap::from([
        ("jenseits".into(), flat_map(input.get(JENSEITS_MAP))?),
        ("genealogie".into(), flat_map(input.get(GENE_MAP))?),
        ("antichrist".into(), flat_map(input.get(ANTI_MAP))?),
    ]);
    let group = |slug: &str, layer: &str| -> &'static str {
        match (slug, layer) {
            ("jenseits", "abbyy-xml-paragraph") => "j-abbyy",
            ("jenseits", "djvu-xml-line") => "j-djvu",
            ("jenseits", _) => "j-pdf",
            ("genealogie", _) => "g-pdf",
            _ => "a-djvu",
        }
    };
    let mut wanted = BTreeMap::<&str, BTreeSet<usize>>::new();
    for c in target {
        let (slug, unit, start, end) = selected_boundaries(&maps, c)?;
        let layer = layer_for(slug, s(&unit["qualified_unit_key"])?);
        wanted
            .entry(group(slug, layer))
            .or_default()
            .extend(start..=end);
    }
    ensure(
        wanted.values().map(BTreeSet::len).sum::<usize>() <= 512,
        "source extraction page count bound",
    )?;
    let mut payloads = BTreeMap::new();
    for (name, dir, media) in [
        ("j-abbyy", JENSEITS_ITEM_DIR, "application/gzip"),
        ("j-djvu", JENSEITS_ITEM_DIR, "application/vnd.djvu+xml"),
        ("j-pdf", JENSEITS_ITEM_DIR, "application/pdf"),
        ("g-pdf", GENE_ITEM_DIR, "application/pdf"),
        ("a-djvu", ANTI_NAV_ITEM_DIR, "application/vnd.djvu+xml"),
        ("a-jp2", ANTI_NAV_ITEM_DIR, "application/zip"),
        ("a-scandata", ANTI_NAV_ITEM_DIR, "application/xml"),
    ] {
        payloads.insert(name, Payload::open(&input, &source, dir, media)?);
    }
    let mut poppler = Poppler::open(ctx)?;
    let mut pages = BTreeMap::new();
    for (name, wanted) in &wanted {
        let mut selected = BTreeMap::new();
        if name.ends_with("pdf") {
            let mut required = wanted.clone();
            while let Some(start) = required.first().copied() {
                let mut end = start;
                while end - start < 7 && required.contains(&(end + 1)) {
                    end += 1
                }
                selected.extend(pdf_records(poppler.pages(
                    ctx,
                    &payloads[name].file,
                    start,
                    end,
                )?)?);
                for p in start..=end {
                    required.remove(&p);
                }
            }
        } else {
            let raw = payloads.get_mut(name).unwrap().bytes(&source)?;
            selected = if *name == "j-abbyy" {
                xml_pages(
                    ctx,
                    flate2::read::MultiGzDecoder::new(Cursor::new(raw)),
                    true,
                    274,
                    wanted,
                    400.0,
                )?
            } else {
                xml_pages(
                    ctx,
                    Cursor::new(raw),
                    false,
                    if *name == "j-djvu" { 274 } else { 525 },
                    wanted,
                    if *name == "j-djvu" { 350.0 } else { 950.0 },
                )?
            };
        }
        pages.insert(*name, selected);
    }
    let mut candidates = vec![];
    let mut anchors = vec![];
    let mut private = BTreeMap::<String, Vec<u8>>::new();
    for c in target {
        ctx.tick(1)?;
        let (slug, unit, start_page, end_page) = selected_boundaries(&maps, c)?;
        let unit_key = s(&unit["unit_key"])?;
        let next_key = key(&unit["next"]["unit_key"])?;
        let layer = layer_for(slug, s(&unit["qualified_unit_key"])?);
        let group = group(slug, layer);
        let pages = &pages[group];
        let (address, content, relation) = witnesses(&input, slug, layer)?;
        let (start_marker, end_marker) = if group == "j-abbyy" {
            (
                marker(pages, start_page, unit_key, "abbyy")?,
                marker(pages, end_page, &next_key, "abbyy")?,
            )
        } else if group == "j-djvu" {
            (
                marker(pages, start_page, unit_key, "djvu")?,
                marker(pages, end_page, &next_key, "djvu")?,
            )
        } else {
            let inventory = if slug == "antichrist" {
                Some(input.get(&format!("{ANTI_NAV_ITEM_DIR}/resource-inventory.json")))
            } else {
                None
            };
            (
                visible_marker(pages, slug, start_page, unit_key, "start", inventory)?.0,
                visible_marker(pages, slug, end_page, &next_key, "end_exclusive", inventory)?.0,
            )
        };
        let boundary_evidence =
            expected_evidence(&input, slug, unit, start_page, end_page, &content)?;
        let (selected, text, regions) =
            extract(ctx, pages, start_page, &start_marker, end_page, &end_marker)?;
        let start = point(pages, &start_marker)?;
        let end = point(pages, &end_marker)?;
        let navigation = selected
            .iter()
            .map(|r| n(&r["page"]))
            .collect::<Result<BTreeSet<_>>>()?
            .into_iter()
            .collect::<Vec<_>>();
        let address_pages = navigation
            .iter()
            .map(|p| {
                if slug == "antichrist" {
                    p.checked_sub(2)
                        .ok_or("source address offset underflow".into())
                } else {
                    Ok(*p)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let candidate_id = paths.candidate_id(s(&c["passage_candidate_id"])?);
        let passage = passage(s(&unit["start"]["anchor_ref"])?);
        let anchor_id = paths.anchor_id(&passage);
        let private_ref = format!("{}/{}.json", paths.private, candidate_id);
        private_boundary(ctx, &private_ref)?;
        let expression = s(&input.get(map_ref(slug))["expression_ref"])?;
        let mut value = records::private(
            &candidate_id,
            c,
            expression,
            &content,
            &boundary_evidence,
            layer,
            &start,
            &end,
            &private_records(&selected),
            &text,
        );
        historical_private(&paths, &mut value);
        let bytes = encode(ctx, &value, true)?;
        let words = selected
            .iter()
            .map(|v| array(&v["words"]).map(Vec::len))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .sum();
        let mut row = records::candidate(
            &candidate_id,
            c,
            s(&c["work_ref"])?,
            expression,
            &passage,
            layer,
            &start,
            &end,
            &navigation,
            &address_pages,
            &content,
            &address,
            &boundary_evidence,
            relation,
            &anchor_id,
            &private_ref,
            &bytes,
            &text,
            selected.len(),
            words,
        );
        historical_candidate(&paths, &mut row);
        candidates.push(row);
        let mut structure = vec![format!(
            "expression:{}",
            expression.rsplit('.').next().unwrap()
        )];
        if !unit["series_key"].is_null() {
            structure.push(format!("series:{}", s(&unit["series_key"])?));
        }
        structure.push(format!("numbered-unit:{unit_key}"));
        let mut selectors = vec![
            json!({"type":"structural","path":structure,"scheme":"transfer-source-layer-exact-numbered-unit-v1"}),
        ];
        selectors.extend(regions);
        selectors.push(json!({"type":"text_position","start":0,"end":text.chars().count(),"text_layer_ref":private_ref}));
        anchors.push(records::anchor(
            &anchor_id,
            &content,
            &passage,
            &selectors,
            &paths.output,
            &paths.event,
        ));
        ensure(
            private.insert(private_ref, bytes).is_none(),
            "source private output collision",
        )?;
    }
    let mut payload = records::output(
        &paths.set_id,
        &input.digest(TARGET_CANDIDATE_PATH),
        &input.bindings,
        &candidates,
        35,
        0,
        &paths.event,
    );
    historical_output(&paths, &mut payload);
    validate(ctx, &input, &paths, &payload, &anchors)?;
    let rendered = encode(ctx, &payload, true)?;
    let rendered_anchors = jsonl(ctx, &anchors)?;
    let entities = output_entities(ctx, &paths, &payload, &rendered, &rendered_anchors)?;
    let mut fresh_event = if let Some(event) = found.clone() {
        event
    } else {
        ensure(build && !paths.historical, "selected source event absent")?;
        records::event(
            &paths.event,
            &utc_now()?,
            PDFTOTEXT_VERSION,
            &input.bindings,
            &entities,
            "pending",
            35,
            0,
            35,
            &paths.output,
        )
    };
    if found.is_none() {
        let mut h = tos_foundation::Digest256Hasher::new();
        for bytes in [
            include_bytes!("transfer_source_passages.rs").as_slice(),
            include_bytes!("transfer_source_passages/constants.rs").as_slice(),
            include_bytes!("transfer_source_passages/records.rs").as_slice(),
        ] {
            ctx.tick(bytes.len() as u64)?;
            h.update(bytes);
        }
        fresh_event["agent_refs"] = json!([
            "software:tos-native",
            format!("software:poppler-{PDFTOTEXT_VERSION}")
        ]);
        fresh_event["method"]["runtime"] = json!("Rust plus Poppler 26.01.0");
        fresh_event["method"]["artifact_digest"] = json!(h.finalize().to_hex());
        fresh_event["method"]["configuration"]["native_builder_ref"] = json!(BUILDER);
    }
    validate_event(ctx, &input, &paths, &fresh_event, &entities)?;
    let tracked = BTreeMap::from([
        (paths.output.clone(), rendered),
        (paths.anchors.clone(), rendered_anchors),
    ]);
    let mut missing_tracked = vec![];
    let mut missing_private = vec![];
    for (r, bytes) in &tracked {
        let missing = fresh_or_matching_limit(ctx, r, bytes, CAP)?;
        ensure(build || !missing, "source tracked output absent")?;
        if missing {
            missing_tracked.push(r)
        }
    }
    for (r, bytes) in &private {
        let missing = fresh_or_matching_limit(&destination, r, bytes, CAP)?;
        ensure(build || !missing, "source private output absent")?;
        if missing {
            missing_private.push(r)
        } else {
            ensure(
                destination
                    .source_file(r, CAP as u64)?
                    .metadata()
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o777
                    == 0o600,
                "source private output mode drift",
            )?;
        }
    }
    input.verify(ctx)?;
    for file in payloads.values_mut() {
        file.verify(&source)?;
    }
    poppler.verify()?;
    ensure(
        read_optional(ctx, PROVENANCE_PATH)?.unwrap_or_default() == journal,
        "source journal changed before output",
    )?;
    if build {
        for r in missing_private {
            destination.write(r, &private[r], 0o600, true)?;
        }
        for r in missing_tracked {
            ctx.write(r, &tracked[r], 0o644, true)?;
        }
        if found.is_none() {
            let mut updated = journal.clone();
            if !updated.is_empty() && !updated.ends_with(b"\n") {
                updated.push(b'\n')
            }
            updated.extend(jsonl(ctx, &[fresh_event])?);
            ensure(updated.len() <= CAP, "source journal byte bound")?;
            if journal.is_empty() {
                ctx.write(PROVENANCE_PATH, &updated, 0o644, true)?;
            } else {
                ctx.write_replacing_exact(PROVENANCE_PATH, &updated, 0o644, &journal)?;
            }
        }
    }

    validate_private(&destination, &payload)?;
    Ok(
        json!({"status":"passed","mode":if build{"build"}else{"check"},"summary":payload["summary"],"private_payloads_read":true,"private_content_checked":true,"publication_authorized":false}),
    )
}

// Historical replay reproduces the original receipt-bound descriptions.
fn historical_private(paths: &Paths, value: &mut Value) {
    if paths.historical {
        value["authority_boundary"] = json!(
            "private automatic source-layer slice only; not diplomatic or accepted German, source-to-target alignment, translation evidence, gold, or publication object"
        );
    }
}
fn historical_candidate(paths: &Paths, value: &mut Value) {
    if paths.historical {
        value["limitations"] = json!([
            "the boundary is exact only inside the named automatic or model-visible-marker-supported source layer",
            "the private slice is not diplomatic or accepted German text",
            "same numbering and paired structural starts do not establish passage or translation alignment",
            "the candidate remains ineligible and has no target gold or human review",
            "private source text is local-only and not authorized for publication"
        ]);
    }
}
fn historical_output(paths: &Paths, value: &mut Value) {
    if paths.historical {
        value["authority_boundary"] = json!(
            "all thirty-five private source-passage candidates are exact only inside their named automatic or model-visible-marker-supported layers; tracked data is source-text-free, and no accepted German, source-to-target alignment, translation, eligibility, gold, human, semantic, publication, or canon authority follows"
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_source_slice_matches_maintained_bytes_and_unicode_offsets() {
        let t = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        let f: Value = serde_json::from_str(include_str!(
            "transfer_source_passages/synthetic-parity.json"
        ))
        .unwrap();
        let pages = array(&f["pages"])
            .unwrap()
            .iter()
            .map(|p| (n(&p["page"]).unwrap(), p.clone()))
            .collect();
        let a = marker(&pages, 1, "7", "pdf").unwrap();
        let b = marker(&pages, 2, "8", "pdf").unwrap();
        let (selected, text, regions) = extract(&ctx, &pages, 1, &a, 2, &b).unwrap();
        assert_eq!(selected, *array(&f["selected"]).unwrap());
        assert_eq!(text, f["text"]);
        assert_eq!(regions, *array(&f["regions"]).unwrap());
        let value = records::private(
            "synthetic-source",
            &f["candidate"],
            "tos.expression.fixture",
            &f["witness"],
            &Value::Null,
            "poppler-pdf-bbox-line",
            &point(&pages, &a).unwrap(),
            &point(&pages, &b).unwrap(),
            &private_records(&selected),
            &text,
        );
        assert_eq!(value, f["private"]);
        assert_eq!(
            sha(&encode(&ctx, &value, true).unwrap()),
            f["private_sha256"]
        );
    }
    #[test]
    fn page_stream_preserves_abbyy_paragraphs_and_djvu_word_order() {
        let t = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        let xml = r#"<document xmlns="http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml"><page width="2500" height="3900"><block><text><par><line l="100" t="500" r="200" b="550"><formatting><charParams>7</charParams><charParams>.</charParams></formatting></line></par><par><line l="20" t="700" r="2100" b="750"><formatting><charParams>A</charParams><charParams> </charParams><charParams>😀</charParams></formatting></line></par></text></block></page><page width="2500" height="3900"/></document>"#;
        // Empty XML page is structurally counted; only the selected first page is retained.
        let xml = xml.replace(
            "<page width=\"2500\" height=\"3900\"/>",
            "<page width=\"2500\" height=\"3900\"></page>",
        );
        let pages = xml_pages(
            &ctx,
            Cursor::new(xml.as_bytes()),
            true,
            2,
            &BTreeSet::from([1]),
            400.0,
        )
        .unwrap();
        assert_eq!(pages[&1]["records"][0]["x_min"], 100);
        assert_eq!(pages[&1]["records"][1]["text"], "A 😀");
        assert_eq!(marker(&pages, 1, "7", "abbyy").unwrap()["order"], 0);
        assert!(
            xml_pages(
                &ctx,
                Cursor::new(xml.replace("FineReader6-schema-v1.xml", "wrong").as_bytes()),
                true,
                2,
                &BTreeSet::from([1]),
                400.0
            )
            .is_err()
        );
        let djvu = r#"<!DOCTYPE DjVuXML SYSTEM "never-resolved.dtd"><DjVuXML><BODY><OBJECT width="2500" height="3900"><PARAGRAPH><LINE><WORD coords="100,600,200,500">tail</WORD><WORD coords="1200,610,1300,505">8.</WORD></LINE></PARAGRAPH></OBJECT></BODY></DjVuXML>"#;
        let p = xml_pages(
            &ctx,
            Cursor::new(djvu.as_bytes()),
            false,
            1,
            &BTreeSet::from([1]),
            350.0,
        )
        .unwrap();
        assert_eq!(p[&1]["records"][0]["text"], "tail 8.");
        assert_eq!(marker(&p, 1, "8", "djvu").unwrap()["order"], 0);
        assert!(
            xml_pages(
                &ctx,
                Cursor::new(djvu.as_bytes()),
                false,
                2,
                &BTreeSet::from([1]),
                350.0
            )
            .is_err()
        );
    }
    #[test]
    fn reviewed_geometry_is_exact_and_does_not_claim_acceptance() {
        let t = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        let returns: Value = serde_json::from_str(PDF_VISIBLE_MARKER_RETURNS).unwrap();
        let r = &returns[0];
        let b = &r["line_bbox"];
        let mut pages = BTreeMap::from([(
            52,
            json!({"width":300.0,"height":600.0,"records":[{"order":r["record_order"],"x_min":b[0],"y_min":b[1],"x_max":b[2],"y_max":b[3]}]}),
        )]);
        let (_, e) = visible_marker(&pages, "jenseits", 52, "32", "start", None).unwrap();
        assert_eq!(e["human_review_performed"], false);
        assert_eq!(e["maker_type"], "model");
        assert_eq!(e["pixel_bbox"]["x"], 1168);
        pages.get_mut(&52).unwrap()["records"][0]["x_min"] = json!(55.0);
        assert!(visible_marker(&pages, "jenseits", 52, "32", "start", None).is_err());
        assert!(Paths::selected(Some("fresh"), None).is_err());
        assert!(Paths::selected(Some("fresh"), Some("tos.event.native.source-test")).is_ok());
        ctx.check().unwrap();
    }
}
