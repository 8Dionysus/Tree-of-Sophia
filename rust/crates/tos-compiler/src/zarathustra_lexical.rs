//! Maintained source-gated lexical observation producer. No source, rights,
//! linguistic, semantic, publication or canon admission is performed here.
use quick_xml::{NsReader, events::Event, name::ResolveResult};
use regex::Regex;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath, parse_json,
    python_casefold_unicode16_v1,
};
use unicode_normalization::UnicodeNormalization;

pub type Result<T> = std::result::Result<T, String>;
pub const PLAN_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/index-plan.v1.json";
pub const GENERATOR_REF: &str = "rust/crates/tos-compiler/src/zarathustra_lexical.rs";
pub const AUTHORITY: &str = "mechanical source-observation and rebuildable local search only; no accepted German, rights clearance, lexeme, lemma, translation, sign, concept, claim, relation, graph, canon, or publication authority";
const CURRENT_PURPOSE: &str = "This artifact records mechanical source observations and supports rebuildable local search. Source assessment and permitted uses remain explicit in their own records.";
const TEI: &str = "http://www.tei-c.org/ns/1.0";
const SOURCE: &str = "ToS/source-witnesses/";
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexicalLimits {
    pub max_file_bytes: usize,
    pub max_total_input_bytes: usize,
    pub max_elements: usize,
    pub max_depth: usize,
    pub max_tokens: usize,
    pub max_token_bytes: usize,
    pub max_projection_bytes: usize,
    pub max_database_bytes: u64,
}
impl LexicalLimits {
    pub fn maintained() -> Self {
        Self {
            max_file_bytes: 32 * 1024 * 1024,
            max_total_input_bytes: 128 * 1024 * 1024,
            max_elements: 500_000,
            max_depth: 128,
            max_tokens: 2_000_000,
            max_token_bytes: 16384,
            max_projection_bytes: 64 * 1024 * 1024,
            max_database_bytes: 512 * 1024 * 1024,
        }
    }
    fn check(self) -> Result<()> {
        if self.max_file_bytes == 0
            || self.max_file_bytes > 64 * 1024 * 1024
            || self.max_total_input_bytes == 0
            || self.max_total_input_bytes > 256 * 1024 * 1024
            || self.max_elements == 0
            || self.max_elements > 1_000_000
            || self.max_depth == 0
            || self.max_depth > 128
            || self.max_tokens == 0
            || self.max_tokens > 2_000_000
            || self.max_token_bytes == 0
            || self.max_token_bytes > 16384
            || self.max_projection_bytes == 0
            || self.max_projection_bytes > 128 * 1024 * 1024
            || self.max_database_bytes == 0
            || self.max_database_bytes > 1024 * 1024 * 1024
        {
            return Err("lexical resource declaration outside supported bounds".into());
        }
        Ok(())
    }
}
pub(crate) fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err("lexical cancellation/deadline".into());
    }
    Ok(())
}
pub(crate) fn sha(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}
pub(crate) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("lexical required string: {key}"))
}
pub(crate) fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("lexical required array: {key}"))
}
pub(crate) fn number(v: &Value, key: &str) -> Result<u64> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("lexical required unsigned number: {key}"))
}
pub(crate) fn parse(raw: &[u8], max: usize) -> Result<Value> {
    let limits = JsonLimits::new(max, 96, 2_000_000, 4300).map_err(|e| e.to_string())?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| e.to_string())?;
    serde_json::from_slice(raw).map_err(|e| format!("unsupported lexical JSON representation: {e}"))
}
pub(crate) fn canonical(v: &Value, max: usize) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    if raw.len() > max {
        return Err("lexical projection bytes".into());
    }
    let limits = JsonLimits::new(max, 96, 2_000_000, 4300).map_err(|e| e.to_string())?;
    let d = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(|e| e.to_string())?;
    let mut b = tos_foundation::canonical_bytes_v1(
        d.root(),
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(b)
}
pub(crate) fn hash_local_database(
    path: &Path,
    max_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(u64, String)> {
    let mut file = tos_fd_open::open_absolute_regular(path, max_bytes)
        .map_err(|error| format!("cannot open local lexical database: {error}"))?;
    let initial_size = file
        .metadata()
        .map_err(|error| format!("cannot stat local lexical database: {error}"))?
        .len();
    if initial_size > max_bytes {
        return Err("local lexical database exceeds byte limit".into());
    }
    let mut digest = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        active(deadline, cancelled)?;
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read local lexical database: {error}"))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| "local lexical database size overflow".to_owned())?;
        if total > max_bytes {
            return Err("local lexical database exceeds byte limit".into());
        }
        digest.update(&buffer[..read]);
    }
    let final_size = file
        .metadata()
        .map_err(|error| format!("cannot restat local lexical database: {error}"))?
        .len();
    if total != initial_size || final_size != initial_size {
        return Err("local database byte-size drift during verification".into());
    }
    Ok((total, digest.finalize().to_hex()))
}

/// Exact immutable input bytes captured via the shared no-symlink FD opener.
/// Revalidation detects source movement before any candidate is returned.
pub struct LexicalCapture<'a> {
    derived_root: Option<PathBuf>,
    cut: Option<(
        &'a tos_source_store::CorpusCutReader,
        Instant,
        &'a AtomicBool,
    )>,
    root: PathBuf,
    files: BTreeMap<String, Vec<u8>>,
    bytes: usize,
    limits: LexicalLimits,
}
impl<'a> LexicalCapture<'a> {
    pub fn new(root: &Path, limits: LexicalLimits) -> Result<Self> {
        limits.check()?;
        tos_fd_open::open_absolute_directory(root).map_err(|e| e.to_string())?;
        Ok(Self {
            cut: None,
            derived_root: None,
            root: root.into(),
            files: BTreeMap::new(),
            bytes: 0,
            limits,
        })
    }
    pub fn from_cut(
        root: &Path,
        cut: &'a tos_source_store::CorpusCutReader,
        limits: LexicalLimits,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        let mut c = Self::new(root, limits)?;
        c.cut = Some((cut, deadline, cancelled));
        Ok(c)
    }
    pub fn set_derived_root(&mut self, root: &Path) -> Result<()> {
        if self
            .files
            .keys()
            .any(|p| p.starts_with("ToS/derived-exports/"))
        {
            return Err("derived capture root must be selected before reads".into());
        }
        tos_fd_open::open_absolute_directory(root).map_err(|e| e.to_string())?;
        self.derived_root = Some(root.into());
        Ok(())
    }
    pub fn limits(&self) -> LexicalLimits {
        self.limits
    }
    pub fn read(&mut self, relative: &str) -> Result<Vec<u8>> {
        RelativePath::parse(relative).map_err(|e| e.to_string())?;
        if let Some(b) = self.files.get(relative) {
            return Ok(b.clone());
        }
        // The existing retained-builder route is exact software byte custody,
        // independent of the older authored corpus cut's membership.
        let b = if relative.starts_with("ToS/research-packets/retained-builder-inputs/")
            && relative.ends_with(".py")
        {
            read_file(
                &self.root.join(relative),
                self.limits.max_file_bytes.min(1024 * 1024),
            )?
        } else if relative.starts_with("ToS/derived-exports/") {
            if let Some(root) = &self.derived_root {
                read_file(&root.join(relative), self.limits.max_file_bytes)?
            } else if self.cut.is_some() {
                return Err("explicit derived-input root required".into());
            } else {
                read_file(&self.root.join(relative), self.limits.max_file_bytes)?
            }
        } else if let Some((cut, deadline, cancelled)) =
            self.cut.filter(|_| relative.starts_with("ToS/"))
        {
            cut.read_member(
                cut.current().revision(),
                &RelativePath::parse(relative).map_err(|e| e.to_string())?,
                self.limits.max_file_bytes as u64,
                deadline,
                cancelled,
            )
            .map_err(|e| e.to_string())?
            .raw
        } else {
            read_file(&self.root.join(relative), self.limits.max_file_bytes)?
        };
        self.bytes = self
            .bytes
            .checked_add(b.len())
            .ok_or("lexical input bytes overflow")?;
        if self.bytes > self.limits.max_total_input_bytes {
            return Err("lexical input bytes".into());
        }
        self.files.insert(relative.into(), b.clone());
        Ok(b)
    }
    pub fn json(&mut self, relative: &str) -> Result<Value> {
        let b = self.read(relative)?;
        let v = parse(&b, self.limits.max_file_bytes)?;
        if !v.is_object() {
            return Err("lexical input must be object".into());
        }
        Ok(v)
    }
    pub fn revalidate(&self) -> Result<()> {
        for (p, b) in &self.files {
            if self.cut.is_some()
                && p.starts_with("ToS/")
                && !p.starts_with("ToS/derived-exports/")
                && !p.starts_with("ToS/research-packets/retained-builder-inputs/")
            {
                continue;
            }
            let root = if p.starts_with("ToS/derived-exports/") {
                self.derived_root.as_ref().unwrap_or(&self.root)
            } else {
                &self.root
            };
            if read_file(&root.join(p), self.limits.max_file_bytes)? != *b {
                return Err(format!("lexical source changed: {p}"));
            }
        }
        Ok(())
    }
    pub fn member_digests(&self) -> BTreeMap<String, String> {
        self.files
            .iter()
            .map(|(p, b)| (p.clone(), sha(b)))
            .collect()
    }
}
pub(crate) fn read_file(path: &Path, max: usize) -> Result<Vec<u8>> {
    let mut f = tos_fd_open::open_absolute_regular(path, max as u64).map_err(|e| e.to_string())?;
    let mut b = Vec::new();
    f.by_ref()
        .take(max as u64 + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() > max {
        return Err("lexical read bytes".into());
    }
    Ok(b)
}
/// Schema authority is supplied by the existing native validation executor.
/// Both full plan and full projection must be checked against captured contracts.
pub trait LexicalSchema {
    fn check(&mut self, contract: &str, raw: &[u8]) -> Result<()>;
}
#[derive(Debug)]
struct Node {
    name: String,
    namespace: Option<String>,
    path: String,
    children: Vec<usize>,
    text: String,
    tail: String,
}
fn namespace(ns: ResolveResult<'_>) -> Result<Option<String>> {
    match ns {
        ResolveResult::Unbound => Ok(None),
        ResolveResult::Bound(s) => Ok(Some(
            std::str::from_utf8(s.as_ref())
                .map_err(|_| "XML namespace UTF-8")?
                .into(),
        )),
        ResolveResult::Unknown(_) => Err("unbound lexical XML namespace".into()),
    }
}
fn append_character_data(nodes: &mut [Node], stack: &[usize], s: &str) -> Result<()> {
    if let Some(&i) = stack.last() {
        if let Some(&last) = nodes[i].children.last() {
            nodes[last].tail.push_str(s);
        } else {
            nodes[i].text.push_str(s);
        }
    } else if !s.chars().all(|c| matches!(c, ' ' | '\n' | '\r' | '\t')) {
        return Err("XML text outside root".into());
    }
    Ok(())
}
fn parse_tei(
    raw: &[u8],
    l: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<Node>> {
    let mut reader = NsReader::from_reader(raw);
    reader.config_mut().expand_empty_elements = true;
    reader.config_mut().check_comments = true;
    let mut nodes: Vec<Node> = vec![];
    let mut stack: Vec<usize> = vec![];
    let mut counts: Vec<BTreeMap<String, usize>> = vec![];
    let mut roots = 0;
    loop {
        active(deadline, cancelled)?;
        let (ns, event) = reader
            .read_resolved_event()
            .map_err(|e| format!("lexical XML: {e}"))?;
        match event {
            Event::Start(e) => {
                if nodes.len() >= l.max_elements || stack.len() >= l.max_depth {
                    return Err("lexical XML element/depth budget".into());
                }
                let name = std::str::from_utf8(e.local_name().as_ref())
                    .map_err(|_| "XML local UTF-8")?
                    .to_owned();
                let ns = namespace(ns)?;
                let path = if let Some(&p) = stack.last() {
                    let n = counts.last_mut().unwrap().entry(name.clone()).or_default();
                    *n += 1;
                    format!("{}/{}[{}]", nodes[p].path, name, n)
                } else {
                    roots += 1;
                    if roots != 1 {
                        return Err("multiple lexical XML roots".into());
                    }
                    name.clone()
                };
                let mut attribute_names = BTreeSet::new();
                for a in e.attributes() {
                    let a = a.map_err(|_| "invalid lexical XML attributes")?;
                    if a.key.as_ref() == b"xmlns" || a.key.as_ref().starts_with(b"xmlns:") {
                        continue;
                    }
                    let (ns, local) = reader.resolver().resolve_attribute(a.key);
                    let ns = namespace(ns)?;
                    let local = std::str::from_utf8(local.as_ref())
                        .map_err(|_| "XML attribute name UTF-8")?;
                    if !attribute_names.insert((ns, local.to_owned())) {
                        return Err("duplicate expanded lexical XML attribute".into());
                    }
                    let raw = reader
                        .decoder()
                        .decode(&a.value)
                        .map_err(|_| "XML attribute UTF-8")?;
                    quick_xml::escape::unescape(&raw).map_err(|_| "XML attribute reference")?;
                }
                let i = nodes.len();
                nodes.push(Node {
                    name,
                    namespace: ns,
                    path,
                    children: vec![],
                    text: String::new(),
                    tail: String::new(),
                });
                if let Some(&p) = stack.last() {
                    nodes[p].children.push(i);
                }
                stack.push(i);
                counts.push(BTreeMap::new());
            }
            Event::End(_) => {
                stack.pop().ok_or("unbalanced lexical XML")?;
                counts.pop();
            }
            Event::Text(t) => {
                let s = t.xml10_content().map_err(|_| "XML character data")?;
                append_character_data(&mut nodes, &stack, &s)?;
            }
            Event::CData(t) => {
                let s = t.xml10_content().map_err(|_| "XML CDATA")?;
                if stack.is_empty() {
                    return Err("CDATA outside root".into());
                }
                append_character_data(&mut nodes, &stack, &s)?;
            }
            Event::GeneralRef(r) => {
                if stack.is_empty() {
                    return Err("XML reference outside root".into());
                }
                let n = r.decode().map_err(|_| "XML reference UTF-8")?;
                let s = quick_xml::escape::unescape(&format!("&{n};"))
                    .map_err(|_| "unsupported XML entity")?
                    .into_owned();
                append_character_data(&mut nodes, &stack, &s)?;
            }
            Event::DocType(_) => return Err("unsupported lexical XML DTD entity profile".into()),
            Event::Decl(d) => {
                if d.version().map_err(|_| "XML version")?.as_ref() != b"1.0" {
                    return Err("unsupported lexical XML version".into());
                }
                if d.encoding()
                    .transpose()
                    .map_err(|_| "XML encoding")?
                    .is_some_and(|s| !s.eq_ignore_ascii_case(b"UTF-8"))
                {
                    return Err("unsupported lexical XML encoding".into());
                }
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => break,
            _ => return Err("unsupported lexical XML event".into()),
        }
    }
    if roots != 1
        || !stack.is_empty()
        || nodes[0].name != "TEI"
        || nodes[0].namespace.as_deref() != Some(TEI)
    {
        return Err("unsupported TEI root namespace".into());
    }
    Ok(nodes)
}
pub fn normalize_form(exact: &str, l: LexicalLimits) -> Result<String> {
    if exact.len() > l.max_token_bytes {
        return Err("lexical token bytes".into());
    }
    let n: String = exact.nfc().collect();
    python_casefold_unicode16_v1(
        &n,
        l.max_token_bytes,
        l.max_token_bytes * 3,
        l.max_token_bytes * 3,
    )
    .map_err(|e| e.to_string())
}
pub fn word_spans(
    text: &str,
    joiners: &BTreeSet<char>,
    max_token_bytes: usize,
) -> Result<Vec<(usize, usize, String)>> {
    word_spans_checked(
        text,
        joiners,
        max_token_bytes,
        LexicalLimits::maintained().max_tokens,
        || Ok(()),
    )
}
fn word_spans_checked(
    text: &str,
    joiners: &BTreeSet<char>,
    max_token_bytes: usize,
    max_spans: usize,
    mut check: impl FnMut() -> Result<()>,
) -> Result<Vec<(usize, usize, String)>> {
    static LETTERS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = LETTERS
        .get_or_init(|| Regex::new(r"^[\p{L}\p{M}]$").expect("constant Unicode letter/mark regex"));
    let mut c = Vec::new();
    let mut letters = Vec::new();
    for (i, scalar) in text.chars().enumerate() {
        if i % 4096 == 0 {
            check()?;
        }
        let mut utf8 = [0u8; 4];
        letters.push(re.is_match(scalar.encode_utf8(&mut utf8)));
        c.push(scalar);
    }
    let mut result = vec![];
    let mut start = None;
    for i in 0..c.len() {
        if i % 4096 == 0 {
            check()?;
        }
        if letters[i] {
            start.get_or_insert(i);
            continue;
        }
        if joiners.contains(&c[i]) && start.is_some() && i + 1 < c.len() && letters[i + 1] {
            continue;
        }
        if let Some(s) = start.take() {
            let value: String = c[s..i].iter().collect();
            if value.len() > max_token_bytes {
                return Err("lexical token bytes".into());
            }
            if result.len() >= max_spans {
                return Err("lexical token budget".into());
            }
            result.push((s, i, value));
        }
    }
    if let Some(s) = start {
        let value: String = c[s..].iter().collect();
        if value.len() > max_token_bytes {
            return Err("lexical token bytes".into());
        }
        if result.len() >= max_spans {
            return Err("lexical token budget".into());
        }
        result.push((s, c.len(), value));
    }
    Ok(result)
}
#[derive(Default)]
struct SourceResult {
    receipt: Value,
    occurrences: Vec<Value>,
    pages: Vec<Value>,
    sections: Vec<Value>,
}
struct Walker<'a> {
    nodes: &'a [Node],
    item: &'a Value,
    pages: BTreeMap<String, Value>,
    sections: BTreeMap<String, Value>,
    body_pages: BTreeMap<String, Value>,
    body_sections: BTreeMap<String, Value>,
    page_tokens: BTreeMap<String, Vec<(String, String)>>,
    page_sections: BTreeMap<String, BTreeSet<String>>,
    occurrences: Vec<Value>,
    ids: BTreeSet<String>,
    excluded: BTreeSet<String>,
    joiners: BTreeSet<char>,
    limits: LexicalLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Walker<'_> {
    fn add(
        &mut self,
        s: &str,
        path: &str,
        page: Option<&str>,
        sections: &[String],
        status: &str,
    ) -> Result<()> {
        for (start, end, exact) in word_spans_checked(
            s,
            &self.joiners,
            self.limits.max_token_bytes,
            self.limits
                .max_tokens
                .saturating_sub(self.occurrences.len()),
            || active(self.deadline, self.cancelled),
        )? {
            active(self.deadline, self.cancelled)?;
            let page =
                page.ok_or_else(|| format!("indexed body token lacks page anchor: {path}"))?;
            if self.occurrences.len() >= self.limits.max_tokens {
                return Err("lexical token budget".into());
            }
            let normal = normalize_form(&exact, self.limits)?;
            let locator = format!(
                "{}\x1f{path}\x1f{start}\x1f{end}\x1f{exact}",
                text(self.item, "file_sha256")?
            );
            let id = format!(
                "tos.occurrence.lexical-zarathustra-dta-v1.{}",
                sha(locator.as_bytes())
            );
            if !self.ids.insert(id.clone()) {
                return Err("occurrence locator collision".into());
            }
            self.occurrences.push(json!({"occurrence_id":id,"item_ref":self.item["item_ref"],"file_id":self.item["file_id"],"file_sha256":self.item["file_sha256"],"token_ordinal":self.occurrences.len()+1,"exact_form":exact,"normalized_form":normal,"exact_form_sha256":sha(exact.as_bytes()),"normalized_form_sha256":sha(normal.as_bytes()),"page_resource_id":page,"section_resource_id":sections.last(),"text_node_path":path,"start_offset":start,"end_offset":end,"editorial_status":status}));
            self.page_tokens
                .entry(page.into())
                .or_default()
                .push((exact, normal));
            self.page_sections
                .entry(page.into())
                .or_default()
                .extend(sections.iter().cloned());
        }
        Ok(())
    }
    fn visit(
        &mut self,
        i: usize,
        mut page: Option<String>,
        mut sections: Vec<String>,
        status: &str,
    ) -> Result<Option<String>> {
        active(self.deadline, self.cancelled)?;
        let n = &self.nodes[i];
        let name = n.name.clone();
        let path = n.path.clone();
        if name == "pb" {
            let r = self
                .pages
                .get(&path)
                .ok_or_else(|| format!("page absent from inventory: {path}"))?
                .clone();
            let id = text(&r, "resource_id")?.to_owned();
            self.body_pages.insert(id.clone(), r);
            page = Some(id);
        }
        if name == "div" {
            let r = self
                .sections
                .get(&path)
                .ok_or_else(|| format!("division absent from inventory: {path}"))?
                .clone();
            let id = text(&r, "resource_id")?.to_owned();
            self.body_sections.insert(id.clone(), r);
            sections.push(id);
        }
        if self.excluded.contains(&name) {
            return Ok(page);
        }
        if name == "choice" {
            let children = &n.children;
            let selected = ["sic", "orig", "abbr"]
                .iter()
                .find_map(|p| {
                    children
                        .iter()
                        .find(|&&c| self.nodes[c].name == *p)
                        .copied()
                })
                .or_else(|| children.first().copied());
            if let Some(c) = selected {
                let status = match self.nodes[c].name.as_str() {
                    "sic" => "source-marked-sic",
                    "orig" => "source-marked-orig",
                    "abbr" => "source-marked-abbr",
                    _ => status,
                };
                page = self.visit(c, page, sections, status)?;
            }
            return Ok(page);
        }
        let status = match name.as_str() {
            "supplied" => "source-editorial-supplied",
            "corr" => "source-editorial-corr",
            "sic" => "source-marked-sic",
            _ => status,
        };
        let s = n.text.clone();
        let children = n.children.clone();
        self.add(
            &s,
            &format!("{path}/text()[1]"),
            page.as_deref(),
            &sections,
            status,
        )?;
        for c in children {
            page = self.visit(c, page, sections.clone(), status)?;
            let tail = self.nodes[c].tail.clone();
            let p = self.nodes[c].path.clone();
            self.add(
                &tail,
                &format!("{p}/tail()[1]"),
                page.as_deref(),
                &sections,
                status,
            )?;
        }
        Ok(page)
    }
}
fn source_item(
    item: &Value,
    plan: &Value,
    cap: &mut LexicalCapture,
    payload: &mut LexicalCapture,
    l: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SourceResult> {
    let manifest_ref = text(item, "manifest_ref")?;
    let manifest = cap.json(manifest_ref)?;
    let inventory = cap.json(text(item, "resource_inventory_ref")?)?;
    let rights = cap.json(text(item, "rights_ref")?)?;
    if manifest["item_id"] != item["item_ref"]
        || inventory["item_id"] != item["item_ref"]
        || !array(&rights, "scope_refs")?.contains(&item["item_ref"])
    {
        return Err("lexical item/rights scope drift".into());
    }
    let files: Vec<_> = array(&manifest, "payload_files")?
        .iter()
        .filter(|f| f["file_id"] == item["file_id"] && f["sha256"] == item["file_sha256"])
        .collect();
    if files.len() != 1 || files[0]["media_type"] != "application/xml" {
        return Err("exact XML manifest file absent".into());
    }
    let f = files[0];
    let mr = manifest_ref
        .strip_prefix(SOURCE)
        .ok_or("manifest outside source-witnesses")?;
    let relative = Path::new(mr)
        .parent()
        .ok_or("manifest parent")?
        .join(text(f, "relative_path")?);
    let raw = payload.read(relative.to_str().ok_or("payload path UTF-8")?)?;
    if raw.len() as u64 != number(f, "byte_size")? || sha(&raw) != text(item, "file_sha256")? {
        return Err("lexical payload fixity drift".into());
    }
    let inv: Vec<_> = array(&inventory, "files")?
        .iter()
        .filter(|f| f["file_id"] == item["file_id"] && f["file_sha256"] == item["file_sha256"])
        .collect();
    if inv.len() != 1 || inv[0]["profile"] != "tei_structure_v1" {
        return Err("exact TEI inventory absent".into());
    }
    let mut pages = BTreeMap::new();
    let mut sections = BTreeMap::new();
    for r in array(inv[0], "resources")? {
        if let Some(p) = r["locator"]["tei_path"].as_str() {
            match r["resource_kind"].as_str() {
                Some("tei_page_break") => {
                    pages.insert(p.into(), r.clone());
                }
                Some("tei_division") => {
                    sections.insert(p.into(), r.clone());
                }
                _ => {}
            }
        }
    }
    if pages.is_empty() || sections.is_empty() {
        return Err("TEI inventory lacks pages/divisions".into());
    }
    let nodes = parse_tei(&raw, l, deadline, cancelled)?;
    let body = nodes
        .iter()
        .position(|n| n.name == "body" && n.namespace.as_deref() == Some(TEI))
        .ok_or("TEI body absent")?;
    let excluded = array(&plan["scope"], "excluded_elements")?
        .iter()
        .map(|s| {
            s.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "excluded element string".into())
        })
        .collect::<Result<_>>()?;
    let joiners = array(&plan["tokenization"], "internal_joiners")?
        .iter()
        .map(|s| {
            let s = s.as_str().ok_or("joiner string")?;
            let mut c = s.chars();
            let ch = c.next().ok_or("empty joiner")?;
            if c.next().is_some() {
                return Err("unsupported non-scalar lexical joiner".into());
            }
            Ok(ch)
        })
        .collect::<Result<_>>()?;
    let mut w = Walker {
        nodes: &nodes,
        item,
        pages,
        sections,
        body_pages: BTreeMap::new(),
        body_sections: BTreeMap::new(),
        page_tokens: BTreeMap::new(),
        page_sections: BTreeMap::new(),
        occurrences: vec![],
        ids: BTreeSet::new(),
        excluded,
        joiners,
        limits: l,
        deadline,
        cancelled,
    };
    w.visit(body, None, vec![], "witness-text")?;
    if w.occurrences.is_empty() || w.body_pages.is_empty() || w.body_sections.is_empty() {
        return Err("TEI body lacks tokens/pages/divisions".into());
    }
    let pages:Vec<_>=w.body_pages.iter().map(|(id,r)|{let tokens=w.page_tokens.get(id).cloned().unwrap_or_default();json!({"item_ref":item["item_ref"],"resource_id":id,"tei_path":r["locator"]["tei_path"],"facs_ref":r["locator"]["tei_facs_ref"],"page_label":r["locator"]["tei_page_label"],"exact_text":tokens.iter().map(|t|t.0.as_str()).collect::<Vec<_>>().join(" "),"normalized_text":tokens.iter().map(|t|t.1.as_str()).collect::<Vec<_>>().join(" "),"section_refs":w.page_sections.get(id).cloned().unwrap_or_default()})}).collect();
    let sections:Vec<_>=w.body_sections.iter().map(|(id,r)|json!({"item_ref":item["item_ref"],"resource_id":id,"tei_path":r["locator"]["tei_path"],"tei_depth":r["locator"]["tei_depth"],"page_label":r["locator"]["tei_page_label"],"tei_n":r["locator"]["tei_n"],"tei_type":r["locator"]["tei_type"],"parent_resource_id":r["locator"]["parent_resource_id"]})).collect();
    text(&manifest, "embodiment_ref")?;
    text(&rights, "assessment_status")?;
    text(&rights, "review_status")?;
    let receipt = json!({"part_order":item["part_order"],"item_ref":item["item_ref"],"file_id":item["file_id"],"file_sha256":item["file_sha256"],"manifest_ref":item["manifest_ref"],"resource_inventory_ref":item["resource_inventory_ref"],"rights_ref":item["rights_ref"],"rights_assessment_status":rights["assessment_status"],"rights_review_status":rights["review_status"],"language":item["language"],"edition_ref":manifest["embodiment_ref"],"body_page_count":pages.len(),"section_count":sections.len(),"token_occurrence_count":w.occurrences.len()});
    Ok(SourceResult {
        receipt,
        occurrences: w.occurrences,
        pages,
        sections,
    })
}
fn db<T>(r: rusqlite::Result<T>) -> Result<T> {
    r.map_err(|e| format!("lexical SQLite: {e}"))
}
fn nullable<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
fn scalar_count(c: &Connection, sql: &str, p: &[&dyn rusqlite::ToSql]) -> Result<u64> {
    db(c.query_row(sql, p, |r| r.get(0)))
}
fn database(
    path: &Path,
    results: &[SourceResult],
    plan: &Value,
    plan_sha: &str,
    l: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value> {
    let c = db(Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ))?;
    db(c.execute_batch(include_str!("zarathustra_lexical_schema.sql")))?;
    db(c.pragma_update(None, "max_page_count", l.max_database_bytes / 4096))?;
    c.progress_handler(10000, Some(move || Instant::now() >= deadline));
    for (k, v) in [
        ("plan_id", text(plan, "plan_id")?),
        ("plan_sha256", plan_sha),
        ("authority_boundary", text(plan, "authority_boundary")?),
    ] {
        db(c.execute(
            "INSERT INTO metadata(key,value) VALUES (?,?)",
            params![k, v],
        ))?;
    }
    db(c.execute_batch("BEGIN"))?;
    let mut forms: BTreeMap<String, u64> = BTreeMap::new();
    let mut all = vec![];
    for r in results {
        active(deadline, cancelled)?;
        let s = &r.receipt;
        db(c.execute("INSERT INTO source_items(item_ref,part_order,file_id,file_sha256,language,edition_ref,manifest_ref,resource_inventory_ref,rights_ref) VALUES (?,?,?,?,?,?,?,?,?)",params![text(s,"item_ref")?,number(s,"part_order")?,text(s,"file_id")?,text(s,"file_sha256")?,text(s,"language")?,text(s,"edition_ref")?,text(s,"manifest_ref")?,text(s,"resource_inventory_ref")?,text(s,"rights_ref")?]))?;
        for p in &r.pages {
            active(deadline, cancelled)?;
            db(c.execute("INSERT INTO pages(item_ref,resource_id,tei_path,facs_ref,page_label) VALUES (?,?,?,?,?)",params![text(p,"item_ref")?,text(p,"resource_id")?,text(p,"tei_path")?,nullable(p,"facs_ref"),nullable(p,"page_label")]))?;
            let sec = array(p, "section_refs")?
                .iter()
                .map(|v| v.as_str().ok_or("section ref"))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .join(" ");
            let normal = text(p, "normalized_text")?;
            let page = nullable(p, "page_label")
                .filter(|s| !s.is_empty())
                .unwrap_or(text(p, "resource_id")?);
            db(c.execute("INSERT INTO page_fts(item_ref,page_resource_id,section_refs,exact_text,normalized_text,lemma,phrase,prefix,section,page,language,edition,translation,sign_candidate) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",params![text(p,"item_ref")?,text(p,"resource_id")?,sec,text(p,"exact_text")?,normal,"",normal,normal,sec,page,text(s,"language")?,text(s,"edition_ref")?,"",""]))?;
        }
        for p in &r.sections {
            active(deadline, cancelled)?;
            db(c.execute("INSERT INTO sections(item_ref,resource_id,tei_path,tei_depth,page_label,tei_n,tei_type,parent_resource_id) VALUES (?,?,?,?,?,?,?,?)",params![text(p,"item_ref")?,text(p,"resource_id")?,text(p,"tei_path")?,p["tei_depth"].as_i64(),nullable(p,"page_label"),nullable(p,"tei_n"),nullable(p,"tei_type"),nullable(p,"parent_resource_id")]))?;
        }
        for o in &r.occurrences {
            active(deadline, cancelled)?;
            *forms.entry(text(o, "exact_form")?.into()).or_default() += 1;
            all.push(o);
        }
    }
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    for (exact, count) in &forms {
        active(deadline, cancelled)?;
        let hash = sha(exact.as_bytes());
        if hashes.insert(hash.clone(), exact.clone()).is_some() {
            return Err("exact form SHA collision".into());
        }
        let normal = normalize_form(exact, l)?;
        db(c.execute("INSERT INTO forms(form_key,exact_form,normalized_form,exact_form_sha256,normalized_form_sha256,occurrence_count) VALUES (?,?,?,?,?,?)",params![format!("lexical-form:sha256:{hash}"),exact,normal,hash,sha(normal.as_bytes()),count]))?;
    }
    all.sort_by(|a, b| {
        text(a, "item_ref")
            .unwrap()
            .cmp(text(b, "item_ref").unwrap())
            .then_with(|| {
                number(a, "token_ordinal")
                    .unwrap()
                    .cmp(&number(b, "token_ordinal").unwrap())
            })
    });
    for o in &all {
        active(deadline, cancelled)?;
        db(c.execute("INSERT INTO occurrences(occurrence_id,item_ref,token_ordinal,form_key,exact_form,normalized_form,exact_form_sha256,normalized_form_sha256,page_resource_id,section_resource_id,text_node_path,start_offset,end_offset,editorial_status) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",params![text(o,"occurrence_id")?,text(o,"item_ref")?,number(o,"token_ordinal")?,format!("lexical-form:sha256:{}",text(o,"exact_form_sha256")?),text(o,"exact_form")?,text(o,"normalized_form")?,text(o,"exact_form_sha256")?,text(o,"normalized_form_sha256")?,text(o,"page_resource_id")?,nullable(o,"section_resource_id"),text(o,"text_node_path")?,number(o,"start_offset")?,number(o,"end_offset")?,text(o,"editorial_status")?]))?;
    }
    db(c.execute_batch("COMMIT"))?;
    let first=db(c.query_row("SELECT exact_form,normalized_form,item_ref,page_resource_id,section_resource_id FROM occurrences ORDER BY item_ref,token_ordinal LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?))))?;
    let section: String = db(c.query_row(
        "SELECT section_resource_id FROM occurrences WHERE section_resource_id IS NOT NULL LIMIT 1",
        [],
        |r| r.get(0),
    ))?;
    let phrase:String=db(c.query_row("SELECT normalized_text FROM page_fts WHERE length(normalized_text)>0 ORDER BY item_ref,page_resource_id",[],|r|r.get(0)))?;
    let tokens: Vec<_> = phrase.split_whitespace().take(2).collect();
    if tokens.len() != 2 {
        return Err("local index lacks two-token phrase".into());
    }
    let phrase = format!(
        "normalized_text : \"{}\"",
        tokens
            .iter()
            .map(|t| t.replace('"', "\"\""))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let prefix = format!("{}%", first.1.chars().take(3).collect::<String>());
    let queries: [(&str, &str, &dyn rusqlite::ToSql); 6] = [
        (
            "exact_form",
            "SELECT count(*) FROM occurrences WHERE exact_form = ?",
            &first.0 as &dyn rusqlite::ToSql,
        ),
        (
            "normalized_form",
            "SELECT count(*) FROM occurrences WHERE normalized_form = ?",
            &first.1,
        ),
        (
            "prefix",
            "SELECT count(*) FROM occurrences WHERE normalized_form LIKE ?",
            &prefix,
        ),
        (
            "phrase",
            "SELECT count(*) FROM page_fts WHERE page_fts MATCH ?",
            &phrase,
        ),
        (
            "section",
            "SELECT count(*) FROM occurrences WHERE section_resource_id = ?",
            &section,
        ),
        (
            "edition",
            "SELECT count(*) FROM occurrences WHERE item_ref = ?",
            &first.2,
        ),
    ];
    let mut probes = serde_json::Map::new();
    for (field, sql, param) in queries {
        let count = scalar_count(&c, sql, &[param])?;
        if count == 0 {
            return Err(format!("lexical probe returned zero: {field}"));
        }
        probes.insert(
            field.into(),
            json!({"status":"passed","result_count":count}),
        );
    }
    for (field, sql, p) in [
        (
            "page",
            "SELECT count(*) FROM occurrences WHERE item_ref = ? AND page_resource_id = ?",
            vec![&first.2 as &dyn rusqlite::ToSql, &first.3],
        ),
        (
            "language",
            "SELECT count(*) FROM occurrences o JOIN source_items s USING(item_ref) WHERE s.language = 'de'",
            vec![],
        ),
    ] {
        let count = scalar_count(&c, sql, &p)?;
        if count == 0 {
            return Err(format!("lexical probe returned zero: {field}"));
        }
        probes.insert(
            field.into(),
            json!({"status":"passed","result_count":count}),
        );
    }
    probes.insert("lemma".into(),json!({"status":"blocked-not-materialized","result_count":0,"blocker_refs":["german-language-competence-not-attested","morphology-method-not-compared"]}));
    probes.insert(
        "translation".into(),
        json!({"status":"not-applicable","result_count":0}),
    );
    probes.insert("sign_candidate".into(),json!({"status":"blocked-not-materialized","result_count":0,"blocker_refs":["task-specific-source-gate-not-satisfied","human-sign-review-not-triggered"]}));
    let mut counts = serde_json::Map::new();
    for table in ["source_items", "pages", "sections", "occurrences", "forms"] {
        counts.insert(
            table.into(),
            json!(scalar_count(
                &c,
                &format!("SELECT count(*) FROM {table}"),
                &[]
            )?),
        );
    }
    let enabled: u64 = scalar_count(
        &c,
        "SELECT count(*) FROM pragma_compile_options WHERE compile_options LIKE '%ENABLE_FTS5%'",
        &[],
    )?;
    if enabled == 0 {
        return Err("SQLite lacks FTS5".into());
    }
    db(c.execute_batch("VACUUM"))?;
    active(deadline, cancelled)?;
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > l.max_database_bytes {
        return Err("lexical database bytes".into());
    }
    Ok(
        json!({"sqlite_version":rusqlite::version(),"fts5_enabled":true,"table_counts":counts,"query_probes":probes}),
    )
}
#[derive(Default)]
struct Hits {
    count: u64,
    pages: BTreeMap<String, u64>,
    sections: BTreeMap<String, u64>,
}
#[derive(Default)]
struct Form {
    normalized: String,
    count: u64,
    editorial: u64,
    unsectioned: u64,
    items: BTreeMap<u64, (String, Hits)>,
}
fn form_rows(results: &[SourceResult]) -> Result<Vec<Value>> {
    let mut forms: BTreeMap<String, Form> = BTreeMap::new();
    let mut exacts = BTreeMap::new();
    for r in results {
        for o in &r.occurrences {
            let exact = text(o, "exact_form")?;
            let h = text(o, "exact_form_sha256")?;
            if exacts
                .insert(h.to_owned(), exact.to_owned())
                .is_some_and(|s| s != exact)
            {
                return Err("tracked exact form collision".into());
            }
            let f = forms.entry(h.into()).or_default();
            let normal = text(o, "normalized_form_sha256")?;
            if !f.normalized.is_empty() && f.normalized != normal {
                return Err("normalized form hash drift".into());
            }
            f.normalized = normal.into();
            f.count += 1;
            if o["editorial_status"] != "witness-text" {
                f.editorial += 1;
            }
            let sec = nullable(o, "section_resource_id");
            if sec.is_none() {
                f.unsectioned += 1;
            }
            let entry = f
                .items
                .entry(number(&r.receipt, "part_order")?)
                .or_insert_with(|| (text(o, "item_ref").unwrap().into(), Hits::default()));
            entry.1.count += 1;
            *entry
                .1
                .pages
                .entry(text(o, "page_resource_id")?.into())
                .or_default() += 1;
            if let Some(s) = sec {
                *entry.1.sections.entry(s.into()).or_default() += 1;
            }
        }
    }
    Ok(forms.into_iter().map(|(h,f)|json!({"form_key":format!("lexical-form:sha256:{h}"),"exact_form_sha256":h,"normalized_form_sha256":f.normalized,"occurrence_count":f.count,"source_editorial_occurrence_count":f.editorial,"unsectioned_occurrence_count":f.unsectioned,"source_items":f.items.into_values().map(|(item,h)|json!({"item_ref":item,"occurrence_count":h.count,"page_hits":h.pages.into_iter().map(|(id,count)|json!({"resource_id":id,"occurrence_count":count})).collect::<Vec<_>>(),"section_hits":h.sections.into_iter().map(|(id,count)|json!({"resource_id":id,"occurrence_count":count})).collect::<Vec<_>>()})).collect::<Vec<_>>()})).collect())
}
/// Result remains a disposable local candidate. Private source capture roots
/// and native generator identity are receipted separately from any admission.
pub struct LexicalBuild {
    pub projection: Value,
    pub projection_bytes: Vec<u8>,
    pub source_digests: BTreeMap<String, String>,
    pub payload_digests: BTreeMap<String, String>,
}
/// Final mutable software/payload binding check after worker and cut finalization.
/// Authored inputs remain anchored by the caller's complete authentic cut EOF.
pub fn revalidate_build_inputs(
    build: &LexicalBuild,
    software: &Path,
    payload: &Path,
    limits: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    for (relative, expected) in build
        .source_digests
        .iter()
        .filter(|(p, _)| !p.starts_with("ToS/"))
    {
        active(deadline, cancelled)?;
        RelativePath::parse(relative).map_err(|e| e.to_string())?;
        let (_, actual) = hash_local_database(
            &software.join(relative),
            limits.max_file_bytes as u64,
            deadline,
            cancelled,
        )?;
        if &actual != expected {
            return Err(format!("lexical software input changed: {relative}"));
        }
    }
    for (relative, expected) in &build.payload_digests {
        active(deadline, cancelled)?;
        RelativePath::parse(relative).map_err(|e| e.to_string())?;
        let (_, actual) = hash_local_database(
            &payload.join(relative),
            limits.max_file_bytes as u64,
            deadline,
            cancelled,
        )?;
        if &actual != expected {
            return Err(format!("lexical payload input changed: {relative}"));
        }
    }
    Ok(())
}
/// The caller must create a fresh disposable database file. This API never
/// overwrites the canonical projection, changes provenance or selects data.
fn build_inner(
    cut: Option<&tos_source_store::CorpusCutReader>,
    repo: &Path,
    payload_root: &Path,
    plan_ref: &str,
    database_path: &Path,
    schema: &mut dyn LexicalSchema,
    l: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<LexicalBuild> {
    l.check()?;
    active(deadline, cancelled)?;
    if !database_path.is_absolute()
        || !database_path
            .components()
            .any(|p| p.as_os_str() == "local-content")
    {
        return Err("source-bearing SQLite must remain below explicit local-content".into());
    }
    tos_fd_open::open_absolute_directory(database_path.parent().ok_or("database parent")?)
        .map_err(|e| e.to_string())?;
    let mut cap = if let Some(cut) = cut {
        LexicalCapture::from_cut(repo, cut, l, deadline, cancelled)?
    } else {
        LexicalCapture::new(repo, l)?
    };
    let mut payload = LexicalCapture::new(payload_root, l)?;
    let raw = cap.read(plan_ref)?;
    schema.check("ToS/contracts/lexical-index-plan.schema.json", &raw)?;
    let plan = parse(&raw, l.max_file_bytes)?;
    if plan["authority_boundary"] != AUTHORITY && plan["authority_boundary"] != CURRENT_PURPOSE {
        return Err("plan authority boundary drift".into());
    }
    let mut items = array(&plan, "source_items")?.to_vec();
    items.sort_by_key(|i| i["part_order"].as_u64());
    if items
        .iter()
        .enumerate()
        .any(|(i, v)| v["part_order"] != json!(i + 1))
    {
        return Err("source part order not contiguous".into());
    }
    let mut results = vec![];
    let mut tokens = 0usize;
    for i in &items {
        let r = source_item(i, &plan, &mut cap, &mut payload, l, deadline, cancelled)?;
        tokens = tokens
            .checked_add(r.occurrences.len())
            .ok_or("lexical token overflow")?;
        if tokens > l.max_tokens {
            return Err("lexical aggregate token budget".into());
        }
        results.push(r);
    }
    let generator_raw = cap.read(GENERATOR_REF)?;
    if generator_raw != include_bytes!("zarathustra_lexical.rs") {
        return Err("native lexical generator source differs from compiled program".into());
    }
    let generator_sha = sha(&generator_raw);
    let sql_raw = cap.read("rust/crates/tos-compiler/src/zarathustra_lexical_schema.sql")?;
    if sql_raw != include_bytes!("zarathustra_lexical_schema.sql") {
        return Err("native lexical SQL source differs from compiled program".into());
    }
    let sql_sha = sha(&sql_raw);
    let plan_sha = sha(&raw);
    // Never manufacture a native producer receipt with the legacy Python digest.
    // The SQL companion remains an explicit source member in the build receipt.
    let _sql_sha = sql_sha;
    std::fs::OpenOptions::new()
        .mode(0o600)
        .write(true)
        .create_new(true)
        .open(database_path)
        .map_err(|e| format!("fresh lexical output required: {e}"))?;
    let result = (|| {
        let mut local = database(
            database_path,
            &results,
            &plan,
            &plan_sha,
            l,
            deadline,
            cancelled,
        )?;
        local["relative_path"] = plan["local_projection"]["relative_path"].clone();
        let (database_bytes, database_sha256) =
            hash_local_database(database_path, l.max_database_bytes, deadline, cancelled)?;
        local["database_sha256"] = json!(database_sha256);
        local["database_bytes"] = json!(database_bytes);
        let forms = form_rows(&results)?;
        let normalized: BTreeSet<_> = results
            .iter()
            .flat_map(|r| r.occurrences.iter())
            .map(|o| text(o, "normalized_form_sha256").unwrap())
            .collect();
        let projection = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/lexical-index-projection.schema.json","schema_version":"tos_lexical_index_projection_v1","generated_or_authored":"generated_from_source","projection_id":"lexical-index-projection:zarathustra-dta-first-editions-parts-1-4-v1","plan_id":plan["plan_id"],"plan_ref":plan_ref,"plan_sha256":plan_sha,"generator_ref":GENERATOR_REF,"generator_sha256":generator_sha,"builder":{"surface":GENERATOR_REF},"work_ref":plan["work_ref"],"source_items":results.iter().map(|r|r.receipt.clone()).collect::<Vec<_>>(),"summary":{"source_item_count":results.len(),"body_page_count":results.iter().map(|r|r.pages.len()).sum::<usize>(),"section_count":results.iter().map(|r|r.sections.len()).sum::<usize>(),"token_occurrence_count":tokens,"exact_form_row_count":forms.len(),"normalized_form_hash_count":normalized.len(),"source_editorial_occurrence_count":results.iter().flat_map(|r|r.occurrences.iter()).filter(|o|o["editorial_status"]!="witness-text").count(),"unsectioned_occurrence_count":results.iter().flat_map(|r|r.occurrences.iter()).filter(|o|o["section_resource_id"].is_null()).count(),"semantic_fields_populated":0},"hash_profiles":{"exact_form":{"algorithm":"sha256","input":"UTF-8 exact XML character sequence","normalization":"none","confidentiality":"none-low-entropy-dictionary-recovery-possible"},"normalized_form":{"algorithm":"sha256","input":"UTF-8 derived search key","normalization":"unicode-nfc-casefold","confidentiality":"none-low-entropy-dictionary-recovery-possible"}},"field_posture":plan["field_posture"],"form_rows":forms,"local_projection_receipt":local,"content_exposure":{"tracked_exact_strings":false,"tracked_sequence":false,"tracked_context":false,"tracked_occurrence_positions":false,"tracked_form_hashes":true,"dictionary_recovery_possible":true,"confidentiality_claimed":false},"rights_and_visibility":plan["rights_and_visibility"],"semantic_boundary":plan["semantic_boundary"],"authority_boundary":plan["authority_boundary"]});
        let bytes = canonical(&projection, l.max_projection_bytes)?;
        schema.check("ToS/contracts/lexical-index-projection.schema.json", &bytes)?;
        cap.revalidate()?;
        payload.revalidate()?;
        active(deadline, cancelled)?;
        Ok(LexicalBuild {
            projection,
            projection_bytes: bytes,
            source_digests: cap.member_digests(),
            payload_digests: payload.member_digests(),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(database_path);
    }
    result
}

/// Read-only filesystem capture route for parity controls. Production command
/// uses build_from_cut with an authentic retained corpus revision.
pub fn build(
    repo: &Path,
    payload: &Path,
    plan: &str,
    database: &Path,
    schema: &mut dyn LexicalSchema,
    l: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<LexicalBuild> {
    build_inner(
        None, repo, payload, plan, database, schema, l, deadline, cancelled,
    )
}
pub fn build_from_cut(
    cut: &tos_source_store::CorpusCutReader,
    software: &Path,
    payload: &Path,
    plan: &str,
    database: &Path,
    schema: &mut dyn LexicalSchema,
    l: LexicalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<LexicalBuild> {
    build_inner(
        Some(cut),
        software,
        payload,
        plan,
        database,
        schema,
        l,
        deadline,
        cancelled,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn unicode_spans_preserve_scalar_offsets_exact_forms_and_search_only_normalization() {
        let j = ['-', '\'', '’', '‐', '‑'].into_iter().collect();
        assert_eq!(
            word_spans("12 A\u{0308} ß-Ö 'a' x--y 𐐀", &j, 100).unwrap(),
            vec![
                (3, 5, "A\u{0308}".into()),
                (6, 9, "ß-Ö".into()),
                (11, 12, "a".into()),
                (14, 15, "x".into()),
                (17, 18, "y".into()),
                (19, 20, "𐐀".into())
            ]
        );
        assert_eq!(
            normalize_form("A\u{0308}ẞ", LexicalLimits::maintained()).unwrap(),
            "äss"
        );
        assert!(word_spans("abcdef", &j, 3).is_err());
    }
    #[test]
    fn tei_choice_tail_editorial_and_page_sections_keep_python_paths() {
        let raw=br#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body><div><pb/>Alpha<!--skip-->Beta<choice><corr>wrong</corr><sic>source</sic></choice>tail<note><pb/>excluded</note><supplied>editorial</supplied></div></body></text></TEI>"#;
        let l = LexicalLimits::maintained();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(10);
        let nodes = parse_tei(raw, l, deadline, &cancelled).unwrap();
        let item = json!({"item_ref":"item","file_id":"file","file_sha256":"0123"});
        let page = json!({"resource_id":"page"});
        let section = json!({"resource_id":"section"});
        let mut w = Walker {
            nodes: &nodes,
            item: &item,
            pages: BTreeMap::from([("TEI/text[1]/body[1]/div[1]/pb[1]".into(), page)]),
            sections: BTreeMap::from([("TEI/text[1]/body[1]/div[1]".into(), section)]),
            body_pages: BTreeMap::new(),
            body_sections: BTreeMap::new(),
            page_tokens: BTreeMap::new(),
            page_sections: BTreeMap::new(),
            occurrences: vec![],
            ids: BTreeSet::new(),
            excluded: BTreeSet::from(["note".into()]),
            joiners: BTreeSet::new(),
            limits: l,
            deadline,
            cancelled: &cancelled,
        };
        w.visit(2, None, vec![], "witness-text").unwrap();
        assert_eq!(
            w.occurrences
                .iter()
                .map(|v| v["exact_form"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["AlphaBeta", "source", "tail", "editorial"]
        );
        assert_eq!(
            w.occurrences[0]["text_node_path"],
            "TEI/text[1]/body[1]/div[1]/pb[1]/tail()[1]"
        );
        assert_eq!(w.occurrences[1]["editorial_status"], "source-marked-sic");
        assert_eq!(w.occurrences[2]["editorial_status"], "witness-text");
        assert_eq!(
            w.occurrences[3]["editorial_status"],
            "source-editorial-supplied"
        );
        assert_eq!(w.occurrences[1]["section_resource_id"], "section");
    }
    #[test]
    fn unanchored_body_unknown_namespace_and_budgets_refuse() {
        let l = LexicalLimits::maintained();
        let c = AtomicBool::new(false);
        let d = Instant::now() + Duration::from_secs(10);
        assert!(parse_tei(b"<TEI><text/></TEI>", l, d, &c).is_err());
        assert!(
            parse_tei(
                b"<TEI xmlns='http://www.tei-c.org/ns/1.0'><text x:a='bad'/></TEI>",
                l,
                d,
                &c
            )
            .is_err()
        );
        let mut short = l;
        short.max_depth = 1;
        assert!(
            parse_tei(
                b"<TEI xmlns='http://www.tei-c.org/ns/1.0'><text/></TEI>",
                short,
                d,
                &c
            )
            .is_err()
        );
        c.store(true, Ordering::Relaxed);
        assert!(parse_tei(b"<TEI/>", l, d, &c).is_err());
    }
}
/// Preserve the exact authored JSONL prefix and append one explicitly private
/// native export observation. The caller stores this only with the candidate.
pub fn candidate_provenance(
    capture: &mut LexicalCapture<'_>,
    build: &LexicalBuild,
    event_time: &str,
    schema: &mut dyn LexicalSchema,
    l: LexicalLimits,
) -> Result<Vec<u8>> {
    let plan = capture.json(PLAN_REF)?;
    let mut raw = capture.read(text(&plan, "provenance_ref")?)?;
    let source = std::str::from_utf8(&raw).map_err(|_| "lexical provenance UTF-8")?;
    let events = source
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse(s.as_bytes(), l.max_file_bytes))
        .collect::<Result<Vec<_>>>()?;
    if events.is_empty() {
        return Err("empty authored lexical provenance".into());
    }
    let mut by_id = BTreeMap::new();
    for e in &events {
        let id = text(e, "event_id")?;
        if by_id.insert(id, e).is_some() {
            return Err("duplicate lexical provenance event".into());
        }
        schema.check(
            "ToS/contracts/provenance-event.schema.json",
            &canonical(e, l.max_file_bytes)?,
        )?;
    }
    let mut current = *by_id
        .get("tos.event.export.zarathustra-lexical-index-v1.2026-07-29")
        .ok_or("lexical base event absent")?;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(text(current, "event_id")?.to_owned()) {
            return Err("lexical provenance cycle".into());
        }
        let next: Vec<_> = events
            .iter()
            .filter(|e| e["supersedes_event_ref"] == current["event_id"])
            .collect();
        match next.len() {
            0 => break,
            1 => current = next[0],
            _ => return Err("ambiguous lexical provenance successor".into()),
        }
    }
    if seen.len() != events.len() {
        return Err("orphan authored lexical provenance".into());
    }
    let projection_sha = sha(&build.projection_bytes);
    let p = &build.projection;
    let inputs=build.source_digests.iter().map(|(r,h)|json!({"ref":r,"role":"captured-source-or-software-input","sha256":h})).chain(build.payload_digests.iter().map(|(r,h)|json!({"ref":format!("{SOURCE}{r}"),"role":"exact-private-payload-input","sha256":h}))).collect::<Vec<_>>();
    let successor = json!({"schema_version":"tos_provenance_event_v1","event_id":format!("tos.event.export.zarathustra-lexical-index-v1.native-candidate.{projection_sha}"),"event_type":"export","started_at":event_time,"ended_at":event_time,"agent_refs":["software:tos-native-lexical-index-v1"],"inputs":inputs,"outputs":[{"ref":plan["tracked_projection"]["relative_path"],"role":"private-candidate-text-free-projection","sha256":projection_sha},{"ref":plan["local_projection"]["relative_path"],"role":"private-candidate-source-bearing-sqlite","sha256":p["local_projection_receipt"]["database_sha256"]}],"method":{"maker_type":"software","name":"native-zarathustra-lexical-index","version":"1","artifact_digest":p["generator_sha256"],"runtime":format!("SQLite {}",rusqlite::version()),"configuration":{"candidate_only":true,"source_digests":build.source_digests,"payload_digests":build.payload_digests,"authority_boundary":plan["authority_boundary"]}},"status":"completed_with_warnings","warnings":["Private candidate export only; source, rights, linguistic, semantic, canon and publication authority remain unresolved."],"event_version":number(current,"event_version")?.checked_add(1).ok_or("lexical event version overflow")?,"supersedes_event_ref":current["event_id"]});
    let next = canonical(&successor, l.max_file_bytes)?;
    schema.check("ToS/contracts/provenance-event.schema.json", &next)?;
    if !raw.ends_with(b"\n") {
        raw.push(b'\n');
    }
    raw.extend_from_slice(&next);
    if raw.len() > l.max_file_bytes {
        return Err("lexical candidate provenance bytes".into());
    }
    capture.revalidate()?;
    Ok(raw)
}
