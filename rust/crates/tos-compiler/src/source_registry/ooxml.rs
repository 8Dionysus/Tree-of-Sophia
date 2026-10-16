//! Source-addressed OOXML observations, including raw cells and report blocks.
//! Formulae, links and hidden content are retained as reported evidence.
use crate::source_philosophy_dossier_docx::OfficeArchive;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, String>;
type Check<'a> = &'a mut dyn FnMut(u64) -> Result<()>;
const S: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
fn qualified(ns: &str, name: &str) -> String {
    format!("{{{ns}}}{name}")
}
#[derive(Default)]
struct Node {
    tag: String,
    attrs: Map<String, Value>,
    text: Option<String>,
    children: Vec<Node>,
}
impl Node {
    fn is(&self, ns: &str, name: &str) -> bool {
        self.tag == qualified(ns, name)
    }
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).and_then(Value::as_str)
    }
    fn direct(&self, ns: &str, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.is(ns, name))
    }
    fn children(&self, ns: &str, name: &str) -> Vec<&Node> {
        self.children.iter().filter(|n| n.is(ns, name)).collect()
    }
    fn descendants<'a>(&'a self, ns: &str, name: &str, out: &mut Vec<&'a Node>) {
        if self.is(ns, name) {
            out.push(self)
        }
        for child in &self.children {
            child.descendants(ns, name, out)
        }
    }
    fn all(&self, ns: &str, name: &str) -> Vec<&Node> {
        let mut out = Vec::new();
        self.descendants(ns, name, &mut out);
        out
    }
    fn texts(&self, ns: &str, name: &str) -> String {
        self.all(ns, name)
            .into_iter()
            .filter_map(|n| n.text.as_deref())
            .collect()
    }
    fn path(&self, ns: &str, parent: &str, child: &str) -> Vec<&Node> {
        self.direct(ns, parent)
            .map_or_else(Vec::new, |n| n.children(ns, child))
    }
}
fn expanded(name: &[u8], scope: &BTreeMap<String, String>, attribute: bool) -> Result<String> {
    let name = std::str::from_utf8(name).map_err(|e| e.to_string())?;
    let (prefix, local) = name.split_once(':').unwrap_or(("", name));
    if attribute && prefix.is_empty() {
        return Ok(name.into());
    }
    match scope.get(prefix) {
        Some(ns) if !ns.is_empty() => Ok(qualified(ns, local)),
        _ if prefix.is_empty() => Ok(local.into()),
        _ => Err("unbound XML namespace".into()),
    }
}
fn element(
    start: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    parent: &BTreeMap<String, String>,
) -> Result<(Node, BTreeMap<String, String>)> {
    let mut scope = parent.clone();
    let mut attributes = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let name = std::str::from_utf8(attr.key.as_ref())
            .map_err(|e| e.to_string())?
            .to_owned();
        let value = attr
            .decode_and_unescape_value(decoder)
            .map_err(|e| e.to_string())?
            .into_owned();
        if name == "xmlns" {
            scope.insert("".into(), value);
        } else if let Some(prefix) = name.strip_prefix("xmlns:") {
            scope.insert(prefix.into(), value);
        } else {
            attributes.push((name, value));
        }
    }
    let mut node = Node {
        tag: expanded(start.name().as_ref(), &scope, false)?,
        ..Node::default()
    };
    for (name, value) in attributes {
        if node
            .attrs
            .insert(expanded(name.as_bytes(), &scope, true)?, json!(value))
            .is_some()
        {
            return Err("duplicate expanded XML attribute".into());
        }
    }
    Ok((node, scope))
}
fn parse(text: &str, check: Check<'_>) -> Result<Node> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut scopes = vec![BTreeMap::from([(
        "xml".into(),
        "http://www.w3.org/XML/1998/namespace".into(),
    )])];
    let mut root = None;
    let mut events = 0u64;
    fn finish(node: Node, stack: &mut [Node], root: &mut Option<Node>) -> Result<()> {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node)
        } else if root.replace(node).is_some() {
            return Err("multiple XML roots".into());
        }
        Ok(())
    }
    fn append(stack: &mut [Node], text: &str) {
        if let Some(n) = stack.last_mut() {
            if n.children.is_empty() {
                n.text.get_or_insert_with(String::new).push_str(text);
            }
        }
    }
    loop {
        events += 1;
        check(1)?;
        if events > 4_000_000 || stack.len() > 256 {
            return Err("OOXML XML structural budget".into());
        }
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let (n, s) = element(&e, reader.decoder(), scopes.last().unwrap())?;
                stack.push(n);
                scopes.push(s);
            }
            Event::Empty(e) => {
                let (n, _) = element(&e, reader.decoder(), scopes.last().unwrap())?;
                finish(n, &mut stack, &mut root)?;
            }
            Event::End(_) => {
                let n = stack.pop().ok_or("unexpected XML end")?;
                scopes.pop();
                finish(n, &mut stack, &mut root)?;
            }
            Event::Text(e) => {
                let s = e.xml10_content().map_err(|e| e.to_string())?;
                check(s.len() as u64)?;
                append(&mut stack, &s)
            }
            Event::CData(e) => {
                let s = e.xml10_content().map_err(|e| e.to_string())?;
                check(s.len() as u64)?;
                append(&mut stack, &s)
            }
            Event::GeneralRef(e) => {
                let decoded = e.decode().map_err(|e| e.to_string())?;
                let s = if let Some(c) = e.resolve_char_ref().map_err(|e| e.to_string())? {
                    c.to_string()
                } else {
                    quick_xml::escape::resolve_predefined_entity(&decoded)
                        .ok_or("unknown XML entity")?
                        .to_owned()
                };
                append(&mut stack, &s)
            }
            Event::DocType(_) => return Err("OOXML DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("unclosed XML root".into());
    }
    root.ok_or_else(|| "XML root absent".into())
}
fn xml(archive: &OfficeArchive<'_>, part: &str, check: Check<'_>) -> Result<Node> {
    parse(&archive.xml_text(part, check)?, check)
}
fn relations(
    archive: &OfficeArchive<'_>,
    part: &str,
    check: Check<'_>,
) -> Result<Map<String, Value>> {
    let mut result = Map::new();
    if archive.contains(part) {
        for node in xml(archive, part, check)?.children {
            let id = node.attr("Id").ok_or("relationship Id absent")?.to_owned();
            result.insert(id, Value::Object(node.attrs));
        }
    }
    Ok(result)
}
fn relpart(part: &str) -> String {
    let (parent, leaf) = part.rsplit_once('/').unwrap_or(("", part));
    format!("{parent}/_rels/{leaf}.rels")
}
fn normalized_part(reference: &str) -> Result<String> {
    let path = if reference.starts_with('/') {
        reference.trim_start_matches('/').to_owned()
    } else {
        format!("xl/{reference}")
    };
    let mut parts = Vec::new();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or("OOXML relationship escapes container")?;
            }
            _ => parts.push(p),
        }
    }
    Ok(parts.join("/"))
}
pub fn column_index(address: &str) -> Result<usize> {
    let mut n = 0usize;
    let mut any = false;
    for c in address.bytes().take_while(u8::is_ascii_uppercase) {
        any = true;
        n = n
            .checked_mul(26)
            .and_then(|n| n.checked_add(usize::from(c - b'A' + 1)))
            .ok_or("column overflow")?;
    }
    if any {
        Ok(n)
    } else {
        Err("cell address has no column".into())
    }
}
pub fn column_name(mut n: usize) -> String {
    let mut name = Vec::new();
    while n > 0 {
        n -= 1;
        name.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    name.reverse();
    String::from_utf8(name).expect("ASCII column")
}
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y, m, d)
}
fn excel_date(days: f64, epoch1904: bool) -> Result<String> {
    if !days.is_finite() || days.abs() > 4_000_000.0 {
        return Err("Excel date outside calendar".into());
    }
    let day_us = 86_400_000_000i64;
    let micros = (days.trunc() as i64)
        .checked_mul(day_us)
        .and_then(|v| v.checked_add((days.fract() * day_us as f64).round_ties_even() as i64))
        .ok_or("Excel date overflow")?;
    let (y, m, d) = civil(micros.div_euclid(day_us) + if epoch1904 { -24107 } else { -25569 });
    if !(1..=9999).contains(&y) {
        return Err("Excel date outside calendar".into());
    }
    let rem = micros.rem_euclid(day_us);
    let seconds = rem / 1_000_000;
    let fraction = rem % 1_000_000;
    let mut result = format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    if fraction != 0 {
        result.push_str(&format!(".{fraction:06}"));
    }
    Ok(result)
}
pub fn workbook(raw: &[u8], check: Check<'_>) -> Result<Value> {
    let archive = OfficeArchive::open(raw, check)?;
    let strings = if archive.contains("xl/sharedStrings.xml") {
        xml(&archive, "xl/sharedStrings.xml", check)?
            .children
            .iter()
            .map(|n| n.texts(S, "t"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut date_styles = BTreeSet::new();
    if archive.contains("xl/styles.xml") {
        let styles = xml(&archive, "xl/styles.xml", check)?;
        let mut custom = BTreeMap::new();
        for n in styles.path(S, "numFmts", "numFmt") {
            custom.insert(
                n.attr("numFmtId")
                    .ok_or("numFmtId")?
                    .parse::<u64>()
                    .map_err(|e| e.to_string())?,
                n.attr("formatCode").unwrap_or(""),
            );
        }
        let strip = regex::Regex::new(r#""[^"]*"|\\."#).map_err(|e| e.to_string())?;
        let marker = regex::Regex::new("(?i)[ydhs]").map_err(|e| e.to_string())?;
        for (i, xf) in styles.path(S, "cellXfs", "xf").iter().enumerate() {
            let fmt = xf
                .attr("numFmtId")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|e| e.to_string())?;
            let code = strip.replace_all(custom.get(&fmt).copied().unwrap_or(""), "");
            if (14..=22).contains(&fmt) || (45..=47).contains(&fmt) || marker.is_match(&code) {
                date_styles.insert(i);
            }
        }
    }
    let wb = xml(&archive, "xl/workbook.xml", check)?;
    let epoch1904 = wb
        .direct(S, "workbookPr")
        .and_then(|n| n.attr("date1904"))
        .is_some_and(|v| matches!(v, "1" | "true"));
    let rels = relations(&archive, "xl/_rels/workbook.xml.rels", check)?;
    let mut result = Vec::new();
    for sheet in wb.path(S, "sheets", "sheet") {
        check(1)?;
        let id = sheet
            .attr(&qualified(R, "id"))
            .ok_or("sheet relationship absent")?;
        let part = normalized_part(
            rels.get(id)
                .and_then(|v| v["Target"].as_str())
                .ok_or("sheet relationship target absent")?,
        )?;
        let tree = xml(&archive, &part, check)?;
        let sheet_rels = relations(&archive, &relpart(&part), check)?;
        let hyperlinks = tree
            .path(S, "hyperlinks", "hyperlink")
            .into_iter()
            .map(|n| {
                let mut a = n.attrs.clone();
                let r = n
                    .attr(&qualified(R, "id"))
                    .and_then(|id| sheet_rels.get(id))
                    .cloned()
                    .unwrap_or(Value::Null);
                a.insert("relationship".into(), r);
                Value::Object(a)
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for row in tree.path(S, "sheetData", "row") {
            check(1)?;
            let mut cells = Vec::new();
            for cell in row.children(S, "c") {
                let raw = cell.direct(S, "v").and_then(|n| n.text.as_deref());
                let mut typ = cell.attr("t").unwrap_or("n");
                let mut value = json!(raw);
                match (typ, raw) {
                    ("s", Some(raw)) => {
                        value = json!(
                            strings
                                .get(raw.parse::<usize>().map_err(|e| e.to_string())?)
                                .ok_or("shared string index")?
                        )
                    }
                    ("inlineStr", _) => value = json!(cell.texts(S, "t")),
                    ("b", Some(raw)) => value = json!(raw == "1"),
                    ("n", Some(raw)) => {
                        let numeric = raw.parse::<f64>().map_err(|e| e.to_string())?;
                        if !numeric.is_finite() {
                            return Err("nonfinite numeric XML cell".into());
                        }
                        if date_styles.contains(
                            &cell
                                .attr("s")
                                .unwrap_or("0")
                                .parse::<usize>()
                                .map_err(|e| e.to_string())?,
                        ) {
                            value = json!(excel_date(numeric, epoch1904)?);
                            typ = "excel_datetime";
                        } else if numeric.fract() == 0.0 {
                            // Python int(float) retains arbitrarily large integral binary64 values.
                            value = serde_json::from_str(&format!("{numeric:.0}"))
                                .map_err(|e| e.to_string())?;
                        } else {
                            value = json!(numeric);
                        }
                    }
                    _ => {}
                }
                cells.push(json!({"cell":cell.attr("r"),"value":value,"type":typ,"xml_value":raw,"style":cell.attr("s"),"formula":cell.direct(S,"f").and_then(|n|n.text.as_deref())}));
            }
            rows.push(json!({"row":row.attr("r").ok_or("row number absent")?.parse::<u64>().map_err(|e|e.to_string())?,"attributes":row.attrs,"cells":cells}));
        }
        result.push(json!({"name":sheet.attr("name"),"part":part,"state":sheet.attr("state").unwrap_or("visible"),"rows":rows,"hyperlinks":hyperlinks,"columns":tree.path(S,"cols","col").iter().map(|n|&n.attrs).collect::<Vec<_>>(),"merges":tree.path(S,"mergeCells","mergeCell").iter().map(|n|n.attr("ref")).collect::<Vec<_>>() }));
    }
    Ok(json!(result))
}
fn report_text(node: &Node, out: &mut String) {
    if node.is(W, "t") {
        if let Some(t) = &node.text {
            out.push_str(t)
        }
    } else if node.is(W, "tab") {
        out.push('\t')
    } else if node.is(W, "br") || node.is(W, "cr") {
        out.push('\n')
    }
    for child in &node.children {
        report_text(child, out)
    }
}
fn blocks(
    node: &Node,
    address: &str,
    rels: &Map<String, Value>,
    out: &mut Vec<Value>,
    check: Check<'_>,
) -> Result<()> {
    check(1)?;
    if node.is(W, "p") || node.is(W, "tbl") {
        let links=node.all(W,"hyperlink").iter().map(|n|{let rid=n.attr(&qualified(R,"id"));let rel=rid.and_then(|r|rels.get(r));json!({"relationship_id":rid,"target":rel.and_then(|r|r.get("Target")),"target_mode":rel.and_then(|r|r.get("TargetMode")),"anchor":n.attr(&qualified(W,"anchor")),"text":n.texts(W,"t")})}).collect::<Vec<_>>();
        let mut text = String::new();
        report_text(node, &mut text);
        check(text.len() as u64)?;
        out.push(json!({"kind":if node.is(W,"p"){"paragraph"}else{"table"},"xml_path":address,"text":text,"hyperlinks":links,"field_instructions":node.all(W,"instrText").iter().map(|n|&n.text).collect::<Vec<_>>()}));
    }
    let mut counts = BTreeMap::<&str, usize>::new();
    for child in &node.children {
        let name = child.tag.rsplit('}').next().unwrap();
        let count = counts.entry(name).or_default();
        *count += 1;
        blocks(
            child,
            &format!("{address}/{name}[{count}]"),
            rels,
            out,
            check,
        )?;
    }
    Ok(())
}
pub fn docx(raw: &[u8], check: Check<'_>) -> Result<Value> {
    let archive = OfficeArchive::open(raw, check)?;
    let mut names = archive.names()?;
    names.sort();
    let mut parts = Vec::new();
    for part in names {
        if !part.starts_with("word/") || !part.ends_with(".xml") {
            continue;
        }
        let tree = xml(&archive, part, check)?;
        if tree.all(W, "p").is_empty() && tree.all(W, "tbl").is_empty() {
            continue;
        }
        let rels = relations(&archive, &relpart(part), check)?;
        let mut output = Vec::new();
        let local = tree.tag.rsplit('}').next().unwrap();
        blocks(&tree, &format!("/{local}[1]"), &rels, &mut output, check)?;
        parts.push(
            json!({"part":part,"blocks":output,"relationships":rels.values().collect::<Vec<_>>() }),
        );
    }
    Ok(json!(parts))
}
