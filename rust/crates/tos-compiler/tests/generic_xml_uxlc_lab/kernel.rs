//! Maintained consumer for the completed, non-authoritative GenericXML lab.
//! This is deliberately a test consumer, not an inventory/publication API.
use quick_xml::{NsReader, events::Event, name::ResolveResult};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

pub type Result<T> = std::result::Result<T, String>;
pub fn sha(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}
pub fn canonical(value: &Value) -> Vec<u8> {
    // serde_json's preserve_order feature is unified by other workspace
    // consumers; Python sort_keys must not depend on that Cargo feature union.
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .map(|(key, value)| (key.clone(), sorted(value)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(values.iter().map(sorted).collect()),
            _ => value.clone(),
        }
    }
    let mut bytes = serde_json::to_vec_pretty(&sorted(value)).expect("JSON value serialization");
    bytes.push(b'\n');
    bytes
}
fn fail<T>(message: &str) -> Result<T> {
    Err(message.into())
}
fn object<'a>(value: &'a Value, label: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))
}
fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} must be a list"))
}
pub fn keys(value: &Value, expected: &[&str], label: &str) -> Result<()> {
    let actual: BTreeSet<&str> = object(value, label)?.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual != expected {
        return fail(&format!(
            "{label} shape mismatch: missing={:?} unexpected={:?}",
            expected.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&expected).collect::<Vec<_>>()
        ));
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Element {
    pub name: Value,
    pub attributes: BTreeMap<String, String>,
    pub attribute_names: Vec<Value>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}
#[derive(Debug)]
pub struct Xml {
    pub elements: Vec<Element>,
    pub content: Vec<String>,
}
fn expanded(namespace: ResolveResult<'_>, local: &[u8]) -> Result<Value> {
    let namespace = match namespace {
        ResolveResult::Unbound => None,
        ResolveResult::Bound(ns) => Some(
            std::str::from_utf8(ns.as_ref())
                .map_err(|_| "invalid namespace")?
                .to_owned(),
        ),
        ResolveResult::Unknown(_) => return fail("unbound XML namespace"),
    };
    Ok(
        json!({"namespace_uri":namespace,"local_name":std::str::from_utf8(local).map_err(|_| "invalid expanded name")?}),
    )
}
fn name_key(value: &Value) -> (String, String) {
    (
        value["namespace_uri"].as_str().unwrap_or("").into(),
        value["local_name"].as_str().unwrap_or("").into(),
    )
}
/// The invocation has explicit test-input ceilings; these are resource guards,
/// not a new source-format law or a claim about large private XML.
pub fn parse(raw: &[u8], max_bytes: usize, max_elements: usize, max_depth: usize) -> Result<Xml> {
    if raw.len() > max_bytes {
        return fail("XML input budget");
    }
    let mut reader = NsReader::from_reader(raw);
    reader.config_mut().expand_empty_elements = true;
    reader.config_mut().check_comments = true;
    let mut xml = Xml {
        elements: vec![],
        content: vec![],
    };
    let mut stack: Vec<usize> = vec![];
    let mut roots = 0;
    let mut events = 0;
    let mut text_run = String::new();
    loop {
        events += 1;
        let (ns, event) = reader
            .read_resolved_event()
            .map_err(|e| format!("invalid XML: {e}"))?;
        match event {
            Event::Start(element) => {
                if !text_run.is_empty() {
                    xml.content.push(std::mem::take(&mut text_run));
                }
                if xml.elements.len() >= max_elements || stack.len() >= max_depth {
                    return fail("XML element/depth budget");
                }
                if stack.is_empty() {
                    roots += 1;
                    if roots != 1 {
                        return fail("multiple XML roots");
                    }
                }
                let name = expanded(ns, element.local_name().as_ref())?;
                let mut attributes = BTreeMap::new();
                let mut attribute_names = vec![];
                let mut expanded_keys = BTreeSet::new();
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| "invalid XML attribute")?;
                    if attribute.key.as_ref() == b"xmlns"
                        || attribute.key.as_ref().starts_with(b"xmlns:")
                    {
                        continue;
                    }
                    let (ns, local) = reader.resolver().resolve_attribute(attribute.key);
                    let name = expanded(ns, local.as_ref())?;
                    if !expanded_keys.insert(name_key(&name)) {
                        return fail("duplicate expanded attribute");
                    }
                    let decoded = reader
                        .decoder()
                        .decode(&attribute.value)
                        .map_err(|_| "invalid XML attribute value")?;
                    let normalized = decoded
                        .replace("\r\n", "\n")
                        .replace(['\r', '\n', '\t'], " ");
                    let value = quick_xml::escape::unescape(&normalized)
                        .map_err(|_| "invalid XML attribute value")?
                        .into_owned();
                    // Provider coordinates use unqualified n; other attributes
                    // are still retained in structural metadata and content.
                    let key = if name["namespace_uri"].is_null() {
                        name["local_name"].as_str().unwrap().to_owned()
                    } else {
                        format!(
                            "{{{}}}{}",
                            name["namespace_uri"].as_str().unwrap(),
                            name["local_name"].as_str().unwrap()
                        )
                    };
                    xml.content.push(value.clone());
                    attributes.insert(key, value);
                    attribute_names.push(name);
                }
                attribute_names.sort_by_key(name_key);
                let index = xml.elements.len();
                let parent = stack.last().copied();
                xml.elements.push(Element {
                    name,
                    attributes,
                    attribute_names,
                    parent,
                    children: vec![],
                });
                if let Some(parent) = parent {
                    xml.elements[parent].children.push(index);
                }
                stack.push(index);
            }
            Event::End(_) => {
                if !text_run.is_empty() {
                    xml.content.push(std::mem::take(&mut text_run));
                }
                stack.pop().ok_or("unbalanced XML")?;
            }
            Event::Text(text) => {
                let value = text
                    .xml10_content()
                    .map_err(|_| "invalid XML text")?
                    .into_owned();
                if stack.is_empty()
                    && !value
                        .bytes()
                        .all(|c| matches!(c, b' ' | b'\r' | b'\n' | b'\t'))
                {
                    return fail("text outside XML root");
                }
                if !stack.is_empty() {
                    text_run.push_str(&value);
                }
            }
            Event::CData(text) => {
                if stack.is_empty() {
                    return fail("CDATA outside XML root");
                }
                text_run.push_str(&text.xml10_content().map_err(|_| "invalid CDATA")?);
            }
            Event::GeneralRef(reference) => {
                if stack.is_empty() {
                    return fail("reference outside XML root");
                }
                let name = reference.decode().map_err(|_| "invalid XML reference")?;
                text_run.push_str(
                    &quick_xml::escape::unescape(&format!("&{name};"))
                        .map_err(|_| "unsupported XML entity")?,
                );
            }
            Event::Comment(text) => {
                if !text_run.is_empty() {
                    xml.content.push(std::mem::take(&mut text_run));
                }
                if !stack.is_empty() {
                    xml.content.push(
                        text.decode()
                            .map_err(|_| "invalid XML comment")?
                            .into_owned(),
                    );
                }
            }
            Event::PI(text) => {
                if !text_run.is_empty() {
                    xml.content.push(std::mem::take(&mut text_run));
                }
                if !stack.is_empty() {
                    xml.content.push(
                        reader
                            .decoder()
                            .decode(text.content())
                            .map_err(|_| "invalid XML PI")?
                            .into_owned(),
                    );
                }
            }
            Event::DocType(_) => return fail("DOCTYPE forbidden"),
            Event::Decl(_) if events != 1 => return fail("misplaced XML declaration"),
            Event::Eof => break,
            _ => {}
        }
    }
    if roots != 1 || !stack.is_empty() {
        return fail("incomplete XML");
    }
    Ok(xml)
}
impl Xml {
    pub fn path(&self, index: usize) -> Value {
        let mut chain = vec![];
        let mut current = Some(index);
        while let Some(index) = current {
            let element = &self.elements[index];
            let ordinal = element.parent.map_or(1, |p| {
                self.elements[p]
                    .children
                    .iter()
                    .take_while(|&&i| i != index)
                    .filter(|&&i| self.elements[i].name == element.name)
                    .count()
                    + 1
            });
            chain.push(json!({"expanded_name":element.name,"same_name_sibling_position":ordinal}));
            current = element.parent;
        }
        chain.reverse();
        Value::Array(chain)
    }
    pub fn resolve(&self, path: &Value) -> Result<usize> {
        let path = array(path, "path")?;
        let first = path.first().ok_or("empty path")?;
        if first["expanded_name"] != self.elements[0].name {
            return fail("root expanded name mismatch");
        }
        if first["same_name_sibling_position"] != 1 {
            return fail("root sibling position mismatch");
        }
        let mut current = 0;
        for step in path.iter().skip(1) {
            let ordinal = step["same_name_sibling_position"]
                .as_u64()
                .ok_or("path ordinal out of range")?;
            let matching = self.elements[current]
                .children
                .iter()
                .copied()
                .filter(|&i| self.elements[i].name == step["expanded_name"])
                .collect::<Vec<_>>();
            if ordinal == 0 || ordinal > matching.len() as u64 {
                return fail("path ordinal out of range");
            }
            current = matching[ordinal as usize - 1];
        }
        Ok(current)
    }
    pub fn resources(&self) -> Vec<Value> {
        self.elements.iter().enumerate().map(|(index,e)|{
            let path=self.path(index); let steps=path.as_array().unwrap();
            json!({"resource_id":format!("xml-element-{:06}",index+1),"expanded_name":e.name,
                "locator":{"preorder":index+1,"depth":steps.len()-1,"parent_resource_id":e.parent.map(|p|format!("xml-element-{:06}",p+1)),"element_child_position":e.parent.map_or(1,|p|self.elements[p].children.iter().position(|&i|i==index).unwrap()+1),"same_name_sibling_position":steps.last().unwrap()["same_name_sibling_position"],"path":path},
                "element_child_count":e.children.len(),"attribute_count":e.attributes.len(),"attribute_expanded_names":e.attribute_names})
        }).collect()
    }
    pub fn summary(&self) -> Value {
        let resources = self.resources();
        let mut shapes=self.elements.iter().map(|e| {
            let mut children=e.children.iter().map(|&i|self.elements[i].name.clone()).collect::<Vec<_>>(); children.sort_by_key(name_key);
            json!({"expanded_name":e.name,"child_expanded_names_multiset":children,"attribute_expanded_names":e.attribute_names})
        }).collect::<Vec<_>>();
        shapes.sort_by_key(canonical);
        let namespaces = self
            .elements
            .iter()
            .filter_map(|e| e.name["namespace_uri"].as_str())
            .collect::<BTreeSet<_>>();
        json!({"resource_count":resources.len(),"attribute_count":self.elements.iter().map(|e|e.attributes.len()).sum::<usize>(),"max_depth":resources.iter().map(|r|r["locator"]["depth"].as_u64().unwrap()).max().unwrap_or(0),"namespace_uris":namespaces,"ordered_topology_sha256":sha(&canonical(&json!(resources))),"unordered_element_shape_sha256":sha(&canonical(&json!(shapes)))})
    }
    fn named(&self, parent: usize, name: &str) -> Vec<usize> {
        self.elements[parent]
            .children
            .iter()
            .copied()
            .filter(|&i| self.elements[i].name == json!({"namespace_uri":null,"local_name":name}))
            .collect()
    }
    fn one(&self, parent: usize, name: &str) -> Result<usize> {
        let children = self.named(parent, name);
        if children.len() != 1 {
            return fail(&format!("provider shape requires one {name} subtree"));
        }
        Ok(children[0])
    }
    fn provider(&self) -> Result<Vec<(String, usize)>> {
        if self.elements[0].name != json!({"namespace_uri":null,"local_name":"Tanach"}) {
            return fail("source is not the registered UXLC provider shape");
        }
        let tanach = self.one(0, "tanach")?;
        let book = self.one(tanach, "book")?;
        let chapter = self.one(book, "c")?;
        let verses = self.named(chapter, "v");
        if verses.is_empty() {
            return fail("provider shape requires verses");
        }
        let mut expected = vec![
            ("provider_book".into(), book),
            ("provider_chapter".into(), chapter),
        ];
        for verse in verses {
            expected.push(("provider_verse".into(), verse));
            expected.extend(
                self.named(verse, "w")
                    .into_iter()
                    .map(|i| ("provider_word".into(), i)),
            );
        }
        Ok(expected)
    }
}
const B_SCOPE: &str = "{\"node_kinds\":[\"element\"],\"excluded\":[\"text\",\"tail\",\"attribute_values\",\"comments\",\"processing_instructions\",\"dtd\",\"entity_expansions\"],\"path_identity\":\"expanded-name plus one-based same-name sibling position under exact file binding\"}";
const B_CLAIMS: &[&str] = &[
    "source_text_included",
    "element_content_fingerprints_included",
    "intrinsic_ids_claimed",
    "cross_file_identity_claimed",
    "tei_classification_claimed",
];
const B_KEYS: &[&str] = &[
    "schema_version",
    "lab_id",
    "candidate",
    "file_binding",
    "parser_posture",
    "scope",
    "summary",
    "resources",
    "source_text_included",
    "element_content_fingerprints_included",
    "intrinsic_ids_claimed",
    "cross_file_identity_claimed",
    "tei_classification_claimed",
    "authority_boundary",
];
const PARSER_KEYS: &[&str] = &[
    "parser",
    "mode",
    "recover",
    "load_dtd",
    "dtd_validation",
    "resolve_entities",
    "no_network",
    "huge_tree",
    "xinclude",
    "reject_doctype",
];
const PROVIDER_KEYS: &[&str] = &[
    "provider",
    "expression",
    "edition",
    "book_code",
    "selector",
    "projection_authority",
];
const C_SUMMARY_KEYS: &[&str] = &[
    "resource_count",
    "verse_count",
    "word_count",
    "word_counts_by_verse",
];
pub fn b(owner: &Value, xml: &Xml) -> Result<Value> {
    let resources = array(&owner["resources"], "B resources")?;
    keys(
        &owner["scope"],
        &["node_kinds", "excluded", "path_identity"],
        "B scope",
    )?;
    if owner["scope"] != serde_json::from_str::<Value>(B_SCOPE).unwrap() {
        return fail("B scope value mismatch");
    }
    if resources.len() != xml.elements.len() {
        return fail("resource/element count mismatch");
    }
    unique_ids(resources)?;
    let expected = xml.resources();
    for (index, resource) in resources.iter().enumerate() {
        keys(
            resource,
            &[
                "resource_id",
                "resource_kind",
                "expanded_name",
                "locator",
                "element_child_count",
                "attribute_count",
                "attribute_expanded_names",
            ],
            "B resource",
        )?;
        if resource["resource_kind"] != "xml_element" {
            return fail("resource kind mismatch");
        }
        let target = xml.resolve(&resource["locator"]["path"])?;
        if target != index {
            return fail("path does not return registered preorder element");
        }
        for (field, error) in [
            ("expanded_name", "expanded name mismatch"),
            ("element_child_count", "element child count mismatch"),
            ("attribute_count", "attribute count mismatch"),
            (
                "attribute_expanded_names",
                "attribute expanded names mismatch",
            ),
        ] {
            if resource[field] != expected[index][field] {
                return fail(error);
            }
        }
        for (field, error) in [
            ("preorder", "preorder mismatch"),
            ("depth", "depth mismatch"),
            ("element_child_position", "element child position mismatch"),
            (
                "same_name_sibling_position",
                "same-name sibling position mismatch",
            ),
        ] {
            if resource["locator"][field] != expected[index]["locator"][field] {
                return fail(error);
            }
        }
        let expected_parent = xml.elements[index]
            .parent
            .map(|p| resources[p]["resource_id"].clone())
            .unwrap_or(Value::Null);
        if resource["locator"]["parent_resource_id"] != expected_parent {
            return fail("parent ref mismatch");
        }
    }
    let summary = xml.summary();
    keys(
        &owner["summary"],
        &[
            "resource_count",
            "attribute_count",
            "max_depth",
            "namespace_uris",
            "ordered_topology_sha256",
            "unordered_element_shape_sha256",
        ],
        "B summary",
    )?;
    if owner["summary"] != summary {
        return fail("B summary mismatch");
    }
    Ok(
        json!({"resource_count":resources.len(),"resolved_exactly_once":resources.len(),"path_failures":0,"metadata_mismatches":0,"summary":summary}),
    )
}
fn unique_ids(resources: &[Value]) -> Result<()> {
    let mut ids = BTreeSet::new();
    for resource in resources {
        let id = resource["resource_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("resource IDs are missing")?;
        if !ids.insert(id) {
            return fail("resource IDs must be unique");
        }
    }
    Ok(())
}
pub fn provider_summary(projection: &Value) -> Result<Value> {
    let resources = array(&projection["resources"], "provider projection resources")?;
    let verses = resources
        .iter()
        .filter(|r| r["resource_kind"] == "provider_verse")
        .collect::<Vec<_>>();
    let words = resources
        .iter()
        .filter(|r| r["resource_kind"] == "provider_word")
        .collect::<Vec<_>>();
    for row in verses.iter().chain(words.iter()) {
        object(&row["provider_coordinate"], "provider coordinate")?;
    }
    let counts = verses
        .iter()
        .map(|v| {
            words
                .iter()
                .filter(|w| {
                    ["book", "chapter", "verse"]
                        .iter()
                        .all(|k| w["provider_coordinate"][k] == v["provider_coordinate"][k])
                })
                .count()
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"resource_count":resources.len(),"verse_count":verses.len(),"word_count":words.len(),"word_counts_by_verse":counts}),
    )
}
fn c_keys() -> Vec<&'static str> {
    vec![
        "schema_version",
        "lab_id",
        "candidate",
        "file_binding",
        "parser_posture",
        "provider_context",
        "summary",
        "resources",
        "source_text_included",
        "generic_xml_owner_claimed",
        "intrinsic_or_cross_corpus_word_ids_claimed",
        "accepted_structure_claimed",
        "authority_boundary",
    ]
}
pub fn projection(
    value: &Value,
    xml: &Xml,
    owner: Option<&Value>,
    registered: Option<&Value>,
) -> Result<Value> {
    let expected_keys = if owner.is_some() {
        vec![
            "schema_version",
            "provider_context",
            "generic_owner_candidate",
            "summary",
            "resources",
            "source_text_included",
            "accepted_structure_claimed",
            "authority_boundary",
        ]
    } else if value.get("candidate").is_some() {
        c_keys()
    } else {
        vec![
            "schema_version",
            "provider_context",
            "summary",
            "resources",
            "source_text_included",
            "generic_xml_owner_claimed",
            "intrinsic_or_cross_corpus_word_ids_claimed",
            "accepted_structure_claimed",
            "authority_boundary",
        ]
    };
    keys(
        value,
        &expected_keys,
        if owner.is_none() && value.get("candidate").is_some() {
            "C payload"
        } else {
            "provider projection"
        },
    )?;
    keys(
        &value["provider_context"],
        PROVIDER_KEYS,
        "provider context",
    )?;
    if owner.is_some() && value["generic_owner_candidate"] != "B" {
        return fail("generic projection owner must be B");
    }
    if registered.is_some_and(|registered| registered != &value["provider_context"]) {
        return fail("provider context differs from registered selection");
    }
    let resources = array(&value["resources"], "provider projection resources")?;
    keys(
        &value["summary"],
        C_SUMMARY_KEYS,
        "provider projection summary",
    )?;
    let expected = xml
        .provider()?
        .into_iter()
        .map(|(kind, index)| canonical(&json!([kind, xml.path(index)])))
        .collect::<BTreeSet<_>>();
    let actual = resources
        .iter()
        .map(|r| canonical(&json!([r["resource_kind"], r["source_element_path"]])))
        .collect::<Vec<_>>();
    let mut counts = BTreeMap::new();
    for key in &actual {
        *counts.entry(key.clone()).or_insert(0usize) += 1;
    }
    let actual_set = actual.iter().cloned().collect::<BTreeSet<_>>();
    let duplicate = counts.values().filter(|&&n| n > 1).count();
    let missing = expected.difference(&actual_set).count();
    let unexpected = actual_set.difference(&expected).count();
    if duplicate + missing + unexpected != 0 {
        return fail(&format!(
            "provider projection is incomplete or duplicated: duplicates={duplicate} missing={missing} unexpected={unexpected}"
        ));
    }
    unique_ids(resources)?;
    let mut cited = 0;
    for resource in resources {
        keys(
            resource,
            &[
                "resource_id",
                "resource_kind",
                "provider_coordinate",
                "source_element_path",
                "generic_resource_ref",
            ],
            "provider projection resource",
        )?;
        let index = xml.resolve(&resource["source_element_path"])?;
        let element = &xml.elements[index];
        let kind = resource["resource_kind"]
            .as_str()
            .ok_or("unknown provider resource kind")?;
        let name = match kind {
            "provider_book" => "book",
            "provider_chapter" => "c",
            "provider_verse" => "v",
            "provider_word" => "w",
            _ => return fail("unknown provider resource kind"),
        };
        if element.name != json!({"namespace_uri":null,"local_name":name}) {
            return fail("provider record resolves to wrong source element");
        }
        let coordinate = &resource["provider_coordinate"];
        object(coordinate, "provider coordinate/context")?;
        if coordinate["book"] != value["provider_context"]["book_code"] {
            return fail("provider book coordinate mismatch");
        }
        let coordinate_keys = match kind {
            "provider_book" => vec!["book"],
            "provider_chapter" => vec!["book", "chapter"],
            "provider_verse" => vec!["book", "chapter", "verse"],
            _ => vec!["book", "chapter", "verse", "provider_word_position"],
        };
        keys(
            coordinate,
            &coordinate_keys,
            &format!("provider {name} coordinate"),
        )?;
        if kind != "provider_book" {
            let parent = element.parent.ok_or("provider coordinate parent missing")?;
            let expected_parent = match kind {
                "provider_chapter" => "book",
                "provider_verse" => "c",
                _ => "v",
            };
            if xml.elements[parent].name
                != json!({"namespace_uri":null,"local_name":expected_parent})
            {
                return fail(&format!("provider {name} parent mismatch"));
            }
            let chapter = if kind == "provider_chapter" {
                index
            } else if kind == "provider_verse" {
                parent
            } else {
                xml.elements[parent]
                    .parent
                    .ok_or("provider word chapter parent mismatch")?
            };
            if xml.elements[chapter].name != json!({"namespace_uri":null,"local_name":"c"}) {
                return fail("provider word chapter parent mismatch");
            }
            if coordinate["chapter"] != json!(xml.elements[chapter].attributes.get("n")) {
                return fail("provider chapter coordinate does not match n");
            }
            if kind == "provider_verse" || kind == "provider_word" {
                let verse = if kind == "provider_verse" {
                    index
                } else {
                    parent
                };
                if coordinate["verse"] != json!(xml.elements[verse].attributes.get("n")) {
                    return fail("provider verse coordinate does not match n");
                }
                if kind == "provider_word"
                    && coordinate["provider_word_position"]
                        != json!(
                            xml.named(verse, "w")
                                .iter()
                                .position(|&i| i == index)
                                .map(|i| i + 1)
                        )
                {
                    return fail("provider word position does not match sibling position");
                }
            }
        }
        if let Some(owner) = owner {
            let referenced = array(&owner["resources"], "B resources")?
                .iter()
                .find(|r| r["resource_id"] == resource["generic_resource_ref"])
                .ok_or("projection generic ref missing from B")?;
            if referenced["locator"]["path"] != resource["source_element_path"] {
                return fail("projection path differs from cited B path");
            }
            cited += 1;
        } else if !resource["generic_resource_ref"].is_null() {
            return fail("primary C unexpectedly cites absent B");
        }
    }
    let summary = provider_summary(value)?;
    if value["summary"] != summary {
        return fail("provider projection summary mismatch");
    }
    Ok(
        json!({"resource_count":resources.len(),"resolved_exactly_once":actual_set.len(),"expected_resource_count":expected.len(),"unique_resource_count":actual_set.len(),"unique_resource_id_count":resources.len(),"generic_refs_verified":cited,"path_failures":0,"summary":summary}),
    )
}
#[derive(Default)]
pub struct Selection<'a> {
    pub candidate: Option<&'a str>,
    pub input: Option<&'a str>,
    pub lab: Option<&'a str>,
    pub schema: Option<&'a str>,
    pub provider: Option<&'a Value>,
}
fn file_binding(payload: &Value, raw: &[u8], label: &str, input: Option<&str>) -> Result<()> {
    if !payload["file_binding"].is_object() {
        return fail(&format!("{label} file binding missing"));
    }
    let binding = &payload["file_binding"];
    keys(
        binding,
        &["input_id", "file_id", "sha256", "byte_size", "media_type"],
        &format!("{label} file binding"),
    )?;
    for (passed, error) in [
        (binding["sha256"] == sha(raw), "file binding mismatch"),
        (
            binding["byte_size"] == raw.len(),
            "byte-size binding mismatch",
        ),
        (
            binding["file_id"] == format!("tos.file.sha256.{}", sha(raw)),
            "file ID mismatch",
        ),
        (
            input.is_none_or(|i| binding["input_id"] == i),
            "input ID mismatch",
        ),
        (
            binding["media_type"] == "application/xml",
            "media type mismatch",
        ),
    ] {
        if !passed {
            return fail(&format!("{label} {error}"));
        }
    }
    Ok(())
}
fn claims(payload: &Value, fields: &[&str], label: &str) -> Result<()> {
    if fields.iter().any(|f| payload[f] != false) {
        return fail(&format!("{label} claim posture mismatch"));
    }
    Ok(())
}
pub fn candidate(payload: &Value, raw: &[u8], selected: Selection<'_>) -> Result<Value> {
    let kind = payload["candidate"].as_str().unwrap_or("");
    if selected.candidate.is_some_and(|k| k != kind) {
        return fail("payload candidate differs from requested candidate");
    }
    if selected.lab.is_some_and(|s| payload["lab_id"] != s) {
        return fail("payload lab ID mismatch");
    }
    if selected
        .schema
        .is_some_and(|s| payload["schema_version"] != s)
    {
        return fail("payload schema version mismatch");
    }
    let xml = parse(raw, 8 * 1024 * 1024, 100_000, 256)?;
    match kind {
        "A" => {
            keys(
                payload,
                &[
                    "schema_version",
                    "lab_id",
                    "candidate",
                    "file_binding",
                    "parser_posture",
                    "scope",
                    "resources",
                    "source_text_included",
                    "element_return_supported",
                    "intrinsic_ids_claimed",
                    "authority_boundary",
                ],
                "A payload",
            )?;
            keys(&payload["parser_posture"], PARSER_KEYS, "A parser posture")?;
            file_binding(payload, raw, "A", selected.input)?;
            if payload["element_return_supported"] != false {
                return fail("A overclaims element return");
            }
            claims(
                payload,
                &["intrinsic_ids_claimed", "source_text_included"],
                "A payload",
            )?;
            if payload["scope"] != "one opaque XML document resource" {
                return fail("A scope mismatch");
            }
            if payload["resources"]
                != json!([{"resource_id":"xml-document-000001","resource_kind":"xml_document","locator":{"whole_file":true}}])
            {
                return fail("A resource shape mismatch");
            }
            Ok(
                json!({"document_fixity_match":true,"resource_count":1,"element_return_supported":false}),
            )
        }
        "B" => {
            keys(payload, B_KEYS, "B payload")?;
            keys(&payload["parser_posture"], PARSER_KEYS, "B parser posture")?;
            file_binding(payload, raw, "B", selected.input)?;
            claims(payload, B_CLAIMS, "B")?;
            b(payload, &xml)
        }
        "C" => {
            keys(payload, &c_keys(), "C payload")?;
            keys(&payload["parser_posture"], PARSER_KEYS, "C parser posture")?;
            file_binding(payload, raw, "C", selected.input)?;
            claims(
                payload,
                &[
                    "source_text_included",
                    "generic_xml_owner_claimed",
                    "intrinsic_or_cross_corpus_word_ids_claimed",
                    "accepted_structure_claimed",
                ],
                "C",
            )?;
            let mut result = projection(payload, &xml, None, selected.provider)?;
            result["generic_owner_supported"] = false.into();
            Ok(result)
        }
        "BC" => {
            keys(
                payload,
                &[
                    "schema_version",
                    "lab_id",
                    "candidate",
                    "owner",
                    "projection",
                    "source_text_included",
                    "authority_boundary",
                ],
                "BC payload",
            )?;
            claims(payload, &["source_text_included"], "BC")?;
            let owner = &payload["owner"];
            if !owner.is_object() || !payload["projection"].is_object() {
                return fail("BC owner/projection missing");
            }
            keys(owner, B_KEYS, "BC owner")?;
            keys(
                &owner["parser_posture"],
                PARSER_KEYS,
                "BC owner parser posture",
            )?;
            if owner["candidate"] != "B" {
                return fail("BC owner candidate mismatch");
            }
            if selected.lab.is_some_and(|s| owner["lab_id"] != s) {
                return fail("BC owner lab ID mismatch");
            }
            if owner["schema_version"] != "tos_lab_generic_xml_candidate_b_v1" {
                return fail("BC owner schema version mismatch");
            }
            file_binding(owner, raw, "BC owner", selected.input)?;
            claims(owner, B_CLAIMS, "BC owner")?;
            Ok(
                json!({"owner":b(owner,&xml)?,"projection":projection(&payload["projection"],&xml,Some(owner),selected.provider)?}),
            )
        }
        _ => fail("unknown candidate"),
    }
}

fn normalized(text: &str) -> String {
    tos_foundation::python_casefold_unicode16_v1(
        text,
        text.chars().count(),
        text.chars().count().saturating_mul(3),
        text.len().saturating_mul(3),
    )
    .expect("bounded Unicode casefold")
}
pub fn authority(value: &Value) -> bool {
    let Some(text) = value.as_str().filter(|text| !text.trim().is_empty()) else {
        return false;
    };
    let text = normalized(text);
    // Python's word boundary excludes combining marks, unlike Rust regex \w.
    // Its whitespace also includes the four information separators.
    let negative =
        regex::Regex::new(r"(?:^|[^\p{L}\p{N}_])(?:no|not|never|without)(?:$|[^\p{L}\p{N}_])")
            .unwrap();
    let positive=regex::Regex::new(r"(?:^|[^\p{L}\p{N}_])(?:accepted|canonical|promoted|admitted|authorized|approved|public[\s\x1c-\x1f]+contract|source[- ]text|semantic|translation|graph|canon|publication)(?:$|[^\p{L}\p{N}_])").unwrap();
    let clauses = text.split(['.', ';']).collect::<Vec<_>>();
    clauses.iter().any(|clause| negative.is_match(clause))
        && clauses
            .iter()
            .all(|clause| !positive.is_match(clause) || negative.is_match(clause))
}
pub fn authority_boundaries(payloads: &Value) -> Value {
    let mut checks = BTreeMap::new();
    for (label, payload) in payloads.as_object().unwrap() {
        checks.insert(label.clone(), authority(&payload["authority_boundary"]));
        for field in ["owner", "projection"] {
            if payload[field].is_object() {
                checks.insert(
                    format!("{label}:{field}"),
                    authority(&payload[field]["authority_boundary"]),
                );
            }
        }
    }
    let invalid = checks
        .iter()
        .filter(|(_, passed)| !**passed)
        .map(|(label, _)| label)
        .collect::<Vec<_>>();
    json!({"ok":!checks.is_empty()&&invalid.is_empty(),"checks":checks,"invalid_labels":invalid})
}
pub fn fingerprint_keys(value: &Value, path: &str) -> Vec<String> {
    let mut hits = vec![];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                let folded = normalized(key);
                let normalized = folded
                    .chars()
                    .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                    .collect::<String>();
                let nested_path = format!("{path}.{key}");
                if ["fingerprint", "digest", "hash", "sha", "sha256"].contains(&normalized.as_str())
                    || (["fingerprint", "digest", "hash", "sha"]
                        .iter()
                        .any(|word| normalized.contains(word))
                        && [
                            "content", "text", "value", "label", "source", "element", "word",
                        ]
                        .iter()
                        .any(|word| normalized.contains(word)))
                {
                    hits.push(nested_path.clone());
                }
                hits.extend(fingerprint_keys(nested, &nested_path));
            }
        }
        Value::Array(values) => {
            for (i, nested) in values.iter().enumerate() {
                hits.extend(fingerprint_keys(nested, &format!("{path}[{i}]")));
            }
        }
        _ => {}
    }
    hits.sort();
    hits.dedup();
    hits
}
fn field_values<'a>(value: &'a Value, field: &str, path: &str, out: &mut Vec<(String, &'a Value)>) {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                let path = format!("{path}.{key}");
                if key == field {
                    out.push((path.clone(), nested));
                }
                field_values(nested, field, &path, out);
            }
        }
        Value::Array(values) => {
            for (i, nested) in values.iter().enumerate() {
                field_values(nested, field, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}
pub fn content_equality(payloads: &Value) -> Value {
    let mut checks = BTreeMap::new();
    let mut labels = BTreeSet::new();
    let mut fields = BTreeSet::new();
    for (label, payload) in payloads.as_object().unwrap() {
        let mut values = vec![];
        field_values(payload, "content_equality_claimed", "payload", &mut values);
        let passed = values.iter().all(|(_, v)| **v == false);
        checks.insert(label, passed);
        if !passed {
            labels.insert(label);
            fields.extend(
                values
                    .into_iter()
                    .filter(|(_, v)| **v != false)
                    .map(|(path, _)| format!("{label}:{path}")),
            );
        }
    }
    json!({"ok":checks.values().all(|&v|v),"checks":checks,"invalid_labels":labels,"invalid_fields":fields})
}
pub fn word_claims(payloads: &Value) -> Value {
    let mut checks = BTreeMap::new();
    let mut labels = BTreeSet::new();
    let mut fields = BTreeSet::new();
    for (label, payload) in payloads.as_object().unwrap() {
        if !payload.is_object() {
            continue;
        }
        let primary = payload["candidate"] == "C";
        if !primary && !label.ends_with(":projection") {
            continue;
        }
        let required = if primary {
            vec![
                "accepted_structure_claimed",
                "intrinsic_or_cross_corpus_word_ids_claimed",
            ]
        } else {
            vec!["accepted_structure_claimed"]
        };
        for field in required {
            let key = format!("{label}:{field}");
            let ok = payload[field] == false;
            checks.insert(key.clone(), ok);
            if !ok {
                labels.insert(label);
                fields.insert(key);
            }
        }
    }
    json!({"ok":!checks.is_empty()&&labels.is_empty(),"checks":checks,"invalid_labels":labels,"invalid_fields":fields})
}
pub fn parser_postures(payloads: &Value, expected: &Value) -> Value {
    let mut checks = BTreeMap::new();
    let mut labels = BTreeSet::new();
    for (label, payload) in payloads.as_object().unwrap() {
        if !["A", "B", "C"]
            .iter()
            .any(|kind| payload["candidate"] == *kind)
        {
            continue;
        }
        let passed = payload["parser_posture"].is_object()
            && expected
                .as_object()
                .unwrap()
                .iter()
                .all(|(key, value)| payload["parser_posture"][key] == *value);
        checks.insert(label, passed);
        if !passed {
            labels.insert(label);
        }
    }
    json!({"ok":!checks.is_empty()&&labels.is_empty(),"checks":checks,"invalid_labels":labels,"expected":expected})
}
pub fn source_values(xml: &Xml) -> Vec<String> {
    let values = xml
        .content
        .iter()
        .map(|s| {
            tos_foundation::python_strip_unicode16_v1(s, s.chars().count())
                .expect("bounded strip")
                .to_owned()
        })
        .filter(|value| {
            !value.is_empty()
                && (value.chars().count() >= 3
                    || value
                        .chars()
                        .any(|c| ('\u{0590}'..='\u{05ff}').contains(&c)))
        })
        .collect::<BTreeSet<_>>();
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    values
}
pub fn json_strings(value: &Value) -> Vec<String> {
    let mut strings = vec![];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                strings.push(key.clone());
                if !["local_name", "namespace_uri"].contains(&key.as_str()) {
                    strings.extend(json_strings(nested));
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                strings.extend(json_strings(value));
            }
        }
        Value::String(s) => strings.push(s.clone()),
        _ => {}
    }
    strings
}
pub fn value_hits(
    values: &[String],
    strings: &[String],
    short: bool,
    delimited: bool,
) -> Vec<String> {
    values
        .iter()
        .filter(|value| {
            strings.iter().any(|text| {
                if text == *value {
                    return true;
                }
                if short || value.chars().count() >= 16 {
                    return text.contains(value.as_str());
                }
                if !delimited {
                    return false;
                }
                text.match_indices(value.as_str()).any(|(start, _)| {
                    let end = start + value.len();
                    let left = text[..start].chars().next_back();
                    let right = text[end..].chars().next();
                    left.is_none_or(|c| "-/:.".contains(c))
                        && right.is_none_or(|c| "-/:.".contains(c))
                        && (left.is_some() || right.is_some())
                })
            })
        })
        .cloned()
        .collect()
}
pub fn malformed_content(xml: &str) -> (Vec<String>, bool) {
    let patterns = [
        r"(?s)<!--(.*?)-->",
        r"(?s)<\?[^\s?]+\s+(.+?)\?>",
        r#"(?s)\s+[A-Za-z_][\w:.-]*\s*=\s*'([^']*)'"#,
        r#"(?s)\s+[A-Za-z_][\w:.-]*\s*=\s*"([^"]*)""#,
        r">([^<]*)<",
    ];
    let mut strings = vec![];
    for pattern in patterns {
        for capture in regex::Regex::new(pattern).unwrap().captures_iter(xml) {
            let value = capture[1].trim();
            if !value.is_empty() {
                strings.push(value.to_owned());
            }
        }
    }
    if let Some((_, trailing)) = xml.rsplit_once('>') {
        if !trailing.trim().is_empty() {
            strings.push(trailing.trim().into());
        }
    }
    (
        strings,
        xml.matches('<').count() != xml.matches('>').count()
            || ['\'', '"'].iter().any(|&q| xml.matches(q).count() % 2 != 0),
    )
}
pub fn fixture_strings(manifest: &Value) -> (Vec<String>, Vec<String>) {
    let mut strings = vec![];
    let mut incomplete = vec![];
    for key in ["fixtures", "security_fixtures"] {
        for fixture in manifest[key].as_array().unwrap() {
            let raw = fixture["xml"].as_str().unwrap();
            match parse(raw.as_bytes(), 8 * 1024 * 1024, 100_000, 256) {
                Ok(xml) => strings.extend(
                    xml.content
                        .into_iter()
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty()),
                ),
                Err(_) => {
                    let (fallback, partial) = malformed_content(raw);
                    strings.extend(fallback);
                    if partial {
                        incomplete.push(fixture["id"].as_str().unwrap_or("<unknown>").into());
                    }
                }
            }
        }
    }
    (strings, incomplete)
}
fn selection_key(value: &Value) -> String {
    ["candidate", "selection_kind", "selection_id"]
        .map(|key| value[key].as_str().unwrap_or(""))
        .join(":")
}
pub fn consumer_bindings(observations: &Value, consumer: &Value) -> Value {
    let mut observed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for process in observations["processes"].as_array().unwrap() {
        if process["exit_code"] == 0
            && process["output_sha256"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        {
            observed
                .entry(selection_key(process))
                .or_default()
                .insert(process["output_sha256"].as_str().unwrap().into());
        }
    }
    let checks = consumer["checks"].as_array().unwrap();
    let mut counts = BTreeMap::new();
    for check in checks {
        *counts.entry(selection_key(check)).or_insert(0usize) += 1;
    }
    let missing = observed
        .keys()
        .filter(|key| !counts.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = counts
        .keys()
        .filter(|key| !observed.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    let duplicates = counts
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let mut failures = BTreeSet::new();
    for (values, error) in [
        (&missing, "missing-consumer-check"),
        (&unexpected, "unexpected-consumer-check"),
        (&duplicates, "duplicate-consumer-check"),
    ] {
        failures.extend(values.iter().map(|key| format!("{key}:{error}")));
    }
    let mut matched = 0;
    for check in checks {
        let key = selection_key(check);
        match observed.get(&key) {
            None => {
                failures.insert(format!("{key}:missing-observation"));
            }
            Some(hashes)
                if !check["output_sha256"]
                    .as_str()
                    .is_some_and(|hash| hashes.contains(hash)) =>
            {
                failures.insert(format!("{key}:hash-mismatch"));
            }
            _ => matched += 1,
        }
    }
    json!({"ok":failures.is_empty(),"observed_unique_selection_count":observed.len(),"consumer_check_count":checks.len(),"consumer_unique_selection_count":counts.len(),"missing_selection_keys":missing,"unexpected_selection_keys":unexpected,"duplicate_consumer_selection_keys":duplicates,"matched_check_count":matched,"failures":failures})
}
/// Live facts must be obtained by the selected test consumer; this checks the
/// exact bytes, rather than trusting a historical receipt's digest/size alone.
pub fn source_binding(raw: Option<&[u8]>, manifest: &Value, receipt: &Value, id: &str) -> Value {
    let record = &manifest["exact_sources"][id];
    let receipt = &receipt["sources"][id];
    let digest = raw.map(sha);
    let size = raw.map(|r| r.len());
    let mut failures = BTreeSet::new();
    if raw.is_none() {
        failures.insert("source-file-missing");
    }
    match receipt["source_sha256"].as_str() {
        None => {
            failures.insert("source-receipt-digest-missing");
        }
        Some(expected) if digest.as_deref() != Some(expected) => {
            failures.insert("source-receipt-digest-mismatch");
        }
        _ => {}
    }
    match receipt["source_bytes"].as_u64() {
        None => {
            failures.insert("source-receipt-size-missing");
        }
        Some(expected) if size.map(|n| n as u64) != Some(expected) => {
            failures.insert("source-receipt-size-mismatch");
        }
        _ => {}
    }
    if record["sha256"]
        .as_str()
        .is_some_and(|expected| digest.as_deref() != Some(expected))
    {
        failures.insert("manifest-digest-mismatch");
    }
    if record["byte_size"]
        .as_u64()
        .is_some_and(|expected| size.map(|n| n as u64) != Some(expected))
    {
        failures.insert("manifest-size-mismatch");
    }
    json!({"ok":failures.is_empty(),"source_id":id,"path":record["path"],"actual_sha256":digest,"receipt_sha256":receipt["source_sha256"],"manifest_sha256":record["sha256"],"actual_bytes":size,"receipt_bytes":receipt["source_bytes"],"manifest_bytes":record["byte_size"],"failures":failures})
}
pub fn admission_boundary(manifest: &Value, tree_ids: &Value, changed: &[String]) -> Value {
    let control = &manifest["public_contract_control"]["admission_boundary"];
    let mut failures = BTreeSet::new();
    let mut checks = BTreeMap::new();
    let baseline = control["baseline_ref"].as_str().filter(|s| !s.is_empty());
    if baseline.is_none() {
        failures.insert("admission-baseline-ref-missing".into());
    }
    let roots = control["roots"].as_array().filter(|roots| {
        roots
            .iter()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
    });
    if roots.is_none() {
        failures.insert("admission-roots-incomplete".into());
    }
    if !control["baseline_tree_ids"].is_object() {
        failures.insert("admission-baseline-tree-ids-missing".into());
    }
    let allowed = control["allowed_changed_paths"]
        .as_array()
        .filter(|rows| {
            rows.iter()
                .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        })
        .cloned()
        .unwrap_or_default();
    for root in roots.into_iter().flatten() {
        let root = root.as_str().unwrap();
        let ok = control["baseline_tree_ids"][root].is_string()
            && tree_ids[root] == control["baseline_tree_ids"][root];
        checks.insert(root, ok);
        if !ok {
            failures.insert(format!("admission-baseline-tree-mismatch:{root}"));
        }
    }
    let unexpected = changed
        .iter()
        .filter(|path| !allowed.contains(&json!(path)))
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        failures.insert("admission-surface-changed".into());
    }
    json!({"ok":failures.is_empty(),"baseline_ref":baseline,"roots":control["roots"],"baseline_tree_checks":checks,"changed_paths":changed,"allowed_changed_paths":allowed,"unexpected_changed_paths":unexpected,"failures":failures})
}
/// Completed-method findings are selected by exact bytes. A new/mutated
/// program needs a new owner assessment; it inherits no Python AST verdict.
pub fn frozen_methods(
    freeze: &Value,
    actual: &BTreeMap<String, Vec<u8>>,
    required: &[String],
) -> Value {
    let frozen = freeze["frozen_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| Some((r["ref"].as_str()?, r["sha256"].as_str()?)))
        .collect::<BTreeMap<_, _>>();
    let mut missing = vec![];
    let mut mismatch = vec![];
    for path in required {
        match frozen.get(path.as_str()) {
            None => missing.push(path),
            Some(expected)
                if actual
                    .get(path)
                    .is_none_or(|raw| sha(raw).as_str() != *expected) =>
            {
                mismatch.push(path)
            }
            _ => {}
        }
    }
    json!({"ok":missing.is_empty()&&mismatch.is_empty(),"method_file_count":required.len(),"missing_frozen_method_files":missing,"active_method_digest_mismatches":mismatch})
}

pub struct TrackedFile<'a> {
    pub relative: &'a str,
    pub bytes: Option<&'a [u8]>,
    pub excluded: bool,
    pub manifest: bool,
}
pub fn disclosure_scan(values: &[String], manifest: &Value, files: &[TrackedFile<'_>]) -> Value {
    let mut leaks = BTreeSet::new();
    let mut errors = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut undecodable = BTreeSet::new();
    let mut collisions = BTreeSet::new();
    let mut scanned = 0;
    for file in files {
        if file.excluded {
            continue;
        }
        let Some(raw) = file.bytes else {
            missing.insert(file.relative);
            errors.insert(format!("missing-tracked-file:{}", file.relative));
            continue;
        };
        scanned += 1;
        let hits = if file.relative.to_ascii_lowercase().ends_with(".json") || file.manifest {
            let Ok(payload) = serde_json::from_slice::<Value>(raw) else {
                errors.insert(format!("invalid-json:{}", file.relative));
                continue;
            };
            if file.manifest {
                let mut control = manifest.clone();
                control.as_object_mut().unwrap().remove("fixtures");
                control.as_object_mut().unwrap().remove("security_fixtures");
                for value in value_hits(values, &json_strings(&control), true, false) {
                    collisions.insert(values.iter().position(|v| v == &value).unwrap() + 1);
                }
                let (strings, incomplete) = fixture_strings(manifest);
                for id in incomplete {
                    errors.insert(format!("incomplete-fixture-content-extraction:{id}"));
                }
                value_hits(values, &strings, true, false)
            } else {
                value_hits(values, &json_strings(&payload), true, false)
            }
        } else {
            let Ok(text) = std::str::from_utf8(raw) else {
                undecodable.insert(file.relative);
                errors.insert(format!("undecodable-tracked-file:{}", file.relative));
                continue;
            };
            value_hits(values, &[text.into()], false, true)
        };
        for value in hits {
            leaks.insert(format!(
                "{}:source-value-{}",
                file.relative,
                values.iter().position(|v| v == &value).unwrap() + 1
            ));
        }
    }
    json!({"ok":leaks.is_empty()&&errors.is_empty(),"leaks":leaks,"scan_errors":errors,"tracked_file_count":files.len(),"scanned_file_count":scanned,"excluded_file_count":files.iter().filter(|f|f.excluded).count(),"missing_tracked_files":missing,"undecodable_tracked_files":undecodable,"manifest_control_collision_count":collisions.len(),"manifest_control_collision_indexes":collisions,"scan_scope":"all Git-tracked files under the laboratory directory","scanned_manifest":true})
}
/// Facts are obtained from the test's actual synthetic filesystem/selected Git
/// observation. The kernel neither discovers nor touches the private UXLC root.
pub struct PrivateFile<'a> {
    pub path: &'a str,
    pub regular: bool,
    pub mode: u32,
    pub ignored: bool,
}
pub fn private_posture(
    root: &str,
    root_exists: bool,
    root_directory: bool,
    root_mode: u32,
    root_ignored: bool,
    observations: &Value,
    files: &[PrivateFile<'_>],
) -> Value {
    let mut failures = BTreeSet::new();
    let mut evidence = vec![];
    if !root_directory {
        failures.insert("private-root-missing".into());
    } else if !root_ignored {
        failures.insert("private-root-not-ignored".into());
    }
    let sources = observations["processes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["selection_kind"] == "source")
        .collect::<Vec<_>>();
    let within = |path: &str| std::path::Path::new(path).starts_with(root);
    for (i, process) in sources.iter().enumerate() {
        if process["output_scope"] != "absolute" {
            failures.insert(format!("source-process-{}:not-absolute", i + 1));
            continue;
        }
        let Some(path) = process["output_ref"].as_str() else {
            failures.insert(format!("source-process-{}:missing-output-reference", i + 1));
            continue;
        };
        if !within(path) {
            failures.insert(format!("source-process-{}:outside-private-root", i + 1));
            continue;
        }
        if !files.iter().any(|f| f.path == path && f.regular) {
            failures.insert(format!("source-process-{}:missing-output", i + 1));
        }
    }
    for file in files {
        if !within(file.path) {
            failures.insert(format!("file-outside-private-root:{}", file.path));
            continue;
        }
        if !file.regular {
            failures.insert(format!("file-missing:{}", file.path));
            continue;
        }
        evidence.push(json!({"path":file.path,"mode":format!("{:04o}",file.mode&0o777),"gitignored":file.ignored}));
        if file.mode & 0o777 != 0o600 {
            failures.insert(format!("file-mode-{:04o}:{}", file.mode & 0o777, file.path));
        }
        if !file.ignored {
            failures.insert(format!("file-not-ignored:{}", file.path));
        }
    }
    evidence.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let applicable = !sources.is_empty() || !files.is_empty() || root_exists;
    json!({"ok":applicable&&failures.is_empty(),"applicable":applicable,"private_root":root,"private_root_mode":if root_directory{Some(format!("{:04o}",root_mode&0o777))}else{None},"private_root_gitignored":root_ignored,"checked_source_process_count":sources.len(),"checked_private_file_count":evidence.len(),"files":evidence,"failures":failures})
}
pub fn changed_paths(outputs: &[&str]) -> Vec<String> {
    outputs
        .iter()
        .flat_map(|s| s.lines())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
