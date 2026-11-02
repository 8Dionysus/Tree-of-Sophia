//! Generic bounded XML page navigation shared by the migrated source stages.
use quick_xml::{Reader, events::Event};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufReader, Read},
};
use tos_source_store::execution::Execution as ResearchExecution;

type Result<T> = std::result::Result<T, String>;
const CAP: usize = 4 * 1024 * 1024;
fn ensure(value: bool, message: &str) -> Result<()> {
    if value { Ok(()) } else { Err(message.into()) }
}

#[derive(Default)]
pub(crate) struct Node {
    pub(crate) name: String,
    pub(crate) attrs: BTreeMap<String, String>,
    pub(crate) content: Vec<Part>,
}
pub(crate) enum Part {
    Text(String),
    Child(Node),
}
impl Node {
    fn text(&mut self, text: &str) {
        if let Some(Part::Text(last)) = self.content.last_mut() {
            last.push_str(text)
        } else {
            self.content.push(Part::Text(text.into()))
        }
    }
    pub(crate) fn children(&self, name: &str) -> Vec<&Node> {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::Child(n) if n.name == name => Some(n),
                _ => None,
            })
            .collect()
    }
}
pub(crate) fn xml_with_doctype(
    ctx: &ResearchExecution,
    raw: &[u8],
    allow_doctype: bool,
) -> Result<Node> {
    fn attrs(
        e: &quick_xml::events::BytesStart<'_>,
        decoder: quick_xml::encoding::Decoder,
    ) -> Result<BTreeMap<String, String>> {
        let mut values = BTreeMap::new();
        for a in e.attributes() {
            let a = a.map_err(|e| e.to_string())?;
            let key = std::str::from_utf8(a.key.as_ref())
                .map_err(|e| e.to_string())?
                .to_owned();
            let value = a
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
                .map_err(|e| e.to_string())?
                .into_owned();
            ensure(
                values.len() < 128 && values.insert(key, value).is_none(),
                "XML attribute count/duplicate",
            )?;
        }
        Ok(values)
    }
    ensure(raw.len() <= CAP, "TEI source byte bound")?;
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut stack = vec![Node::default()];
    let mut count = 0;
    loop {
        ctx.tick(1)?;
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                count += 1;
                ensure(count <= 100000 && stack.len() < 128, "TEI node/depth bound")?;
                stack.push(Node {
                    name: std::str::from_utf8(e.local_name().as_ref())
                        .map_err(|e| e.to_string())?
                        .into(),
                    attrs: attrs(&e, reader.decoder())?,
                    content: vec![],
                });
            }
            Event::Empty(e) => {
                count += 1;
                ensure(count <= 100000, "TEI node bound")?;
                stack.last_mut().unwrap().content.push(Part::Child(Node {
                    name: std::str::from_utf8(e.local_name().as_ref())
                        .map_err(|e| e.to_string())?
                        .into(),
                    attrs: attrs(&e, reader.decoder())?,
                    content: vec![],
                }));
            }
            Event::End(_) => {
                ensure(stack.len() > 1, "unexpected XML end")?;
                let child = stack.pop().unwrap();
                stack.last_mut().unwrap().content.push(Part::Child(child));
            }
            Event::Text(e) => {
                let decoded = e
                    .xml_content(quick_xml::XmlVersion::Explicit1_0)
                    .map_err(|e| e.to_string())?;
                stack.last_mut().unwrap().text(&decoded);
            }
            Event::CData(e) => {
                let decoded = e
                    .xml_content(quick_xml::XmlVersion::Explicit1_0)
                    .map_err(|e| e.to_string())?;
                stack.last_mut().unwrap().text(&decoded);
            }
            Event::GeneralRef(e) => {
                let decoded = e.decode().map_err(|e| e.to_string())?;
                let entity = format!("&{decoded};");
                let value = quick_xml::escape::unescape(&entity).map_err(|e| e.to_string())?;
                stack.last_mut().unwrap().text(&value);
            }
            Event::Decl(e) => {
                ensure(
                    e.xml_version().map_err(|e| e.to_string())?
                        != quick_xml::XmlVersion::Explicit1_1,
                    "source requires XML 1.0",
                )?;
            }
            Event::Comment(_) | Event::PI(_) => {
                // Comments and processing instructions separate XPath character
                // data nodes, while contributing no text to flattened passages.
                // Empty separators are ignored by character-data selectors.
                stack
                    .last_mut()
                    .unwrap()
                    .content
                    .push(Part::Text(String::new()));
            }
            Event::DocType(_) if !allow_doctype => {
                return Err("bounded DTA source does not allow a DTD".into());
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure(stack.len() == 1, "unclosed XML element")?;
    let mut outer = stack.pop().unwrap();
    ensure(
        outer
            .content
            .iter()
            .filter(|x| matches!(x, Part::Child(_)))
            .count()
            == 1,
        "XML root count",
    )?;
    let at = outer
        .content
        .iter()
        .position(|x| matches!(x, Part::Child(_)))
        .unwrap();
    match outer.content.remove(at) {
        Part::Child(n) => Ok(n),
        _ => unreachable!(),
    }
}
pub(crate) fn descendants<'a>(node: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    if node.name == name {
        out.push(node)
    }
    for part in &node.content {
        if let Part::Child(n) = part {
            descendants(n, name, out)
        }
    }
}
pub(crate) fn node_text(node: &Node) -> String {
    let mut out = String::new();
    for part in &node.content {
        match part {
            Part::Text(t) => out.push_str(t),
            Part::Child(n) => out.push_str(&node_text(n)),
        }
    }
    out
}
pub(crate) fn visit_xml_pages(
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
        page == expected
            && visited == wanted.len()
            && if abbyy { namespace } else { djvu_root_seen },
        "source XML page closure drift",
    )?;
    Ok(())
}
