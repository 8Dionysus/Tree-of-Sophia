//! Native producer for the private 210-witness constructor library.
//!
//! This is presentation data over existing source and candidate records. It
//! retains their unreviewed posture and never makes source text public.

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use tos_foundation::Digest256;

pub type Result<T> = std::result::Result<T, String>;

pub const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
pub const CANDIDATE: &str = "ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1";
pub const ALIGNMENT_DIR: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1";
pub const ANALYSIS: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json";
pub const DE_CITATIONS: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/dta-first-editions-parts-1-4-v1/citation-spine.v1.jsonl";
pub const RU_PARAGRAPHS: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/antonovsky-1911-structural-paragraph-v2/paragraph-spine.v2.jsonl";
pub const RU_STRUCTURE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/antonovsky-1911-structural-paragraph-v2/structure-spine.v2.jsonl";
pub const ANNOTATION: &str =
    "tos.annotation.eternal-return-candidate.sid-5cbb125d0d411355a3b40aeefa71de2f";
pub const WORK_ID: &str = "tos.work.friedrich-nietzsche.also-sprach-zarathustra";

const ROMAN: [&str; 4] = ["I", "II", "III", "IV"];
const DE_YEARS: [u16; 4] = [1883, 1883, 1884, 1891];

// These are navigation labels authored by the former producer. They are not
// claims about the historical Russian translation or accepted interpretation.
const CHAPTER_LABELS: &[(&str, &str, &str)] = &[
    ("p1.r1", "Предисловие Заратустры", "Zarathustra’s Prologue"),
    ("p1.r4", "О потусторонниках", "On the Otherworldly"),
    (
        "p1.r10",
        "О проповедниках смерти",
        "On the Preachers of Death",
    ),
    (
        "p1.r13",
        "О базарных мухах",
        "On the Flies of the Marketplace",
    ),
    ("p1.r15", "О друге", "On the Friend"),
    (
        "p1.r16",
        "О тысяче и одной цели",
        "On a Thousand and One Goals",
    ),
    ("p1.r23", "О дарящей добродетели", "On the Giving Virtue"),
    ("p2.r5", "О добродетельных", "On the Virtuous"),
    ("p2.r10", "Танцевальная песнь", "The Dance Song"),
    ("p2.r11", "Надгробная песнь", "The Tomb Song"),
    ("p2.r15", "О непорочном познании", "On Immaculate Knowledge"),
    ("p2.r17", "О поэтах", "On Poets"),
    ("p2.r19", "Прорицатель", "The Soothsayer"),
    ("p2.r20", "Об избавлении", "On Redemption"),
    ("p2.r22", "Самый тихий час", "The Stillest Hour"),
    ("p3.r1", "Странник", "The Wanderer"),
    (
        "p3.r2",
        "О видении и загадке",
        "On the Vision and the Riddle",
    ),
    (
        "p3.r3",
        "О блаженстве против воли",
        "On Bliss Against One’s Will",
    ),
    ("p3.r4", "Перед восходом солнца", "Before Sunrise"),
    (
        "p3.r5",
        "Об умаляющей добродетели",
        "On the Diminishing Virtue",
    ),
    ("p3.r8", "Об отступниках", "On Apostates"),
    ("p3.r9", "Возвращение домой", "The Homecoming"),
    ("p3.r10", "О трёх злых", "On the Three Evils"),
    (
        "p3.r12",
        "О старых и новых скрижалях",
        "On Old and New Tablets",
    ),
    ("p3.r13", "Выздоравливающий", "The Convalescent"),
    (
        "p3.r15",
        "Другая танцевальная песнь",
        "The Other Dance Song",
    ),
    ("p3.r16", "Семь печатей", "The Seven Seals"),
    ("p4.r1", "Медовое приношение", "The Honey Offering"),
    ("p4.r3", "Беседа с королями", "Conversation with the Kings"),
    ("p4.r6", "В отставке", "Retired"),
    ("p4.r9", "Тень", "The Shadow"),
    ("p4.r10", "В полдень", "At Noon"),
    ("p4.r15", "О науке", "On Science"),
    ("p4.r17", "Пробуждение", "The Awakening"),
    ("p4.r18", "Праздник осла", "The Ass Festival"),
    (
        "p4.r19",
        "Песнь ночного странника",
        "The Night Wanderer’s Song",
    ),
];

const CLASS_NOTES: &[(&str, &str, &str)] = &[
    (
        "core",
        "Основной материал исследовательского досье: здесь отмечена явная формула возвращения. Это отбор для рассмотрения, а не принятое толкование.",
        "Core material in the research dossier: an explicit recurrence formulation was identified here. This is a selection for consideration, not an accepted interpretation.",
    ),
    (
        "supporting",
        "Сопутствующий материал досье: фрагмент включён для чтения образов времени, круга, жизни или утверждения. Связь с понятием ещё требует рассмотрения.",
        "Supporting material in the dossier: the passage was included to examine images of time, the circle, life, or affirmation. Its relation to the concept remains open to review.",
    ),
    (
        "ambiguous",
        "Неоднозначный материал: словесное соседство делает фрагмент интересным для сравнения, но само по себе не подтверждает вечное возвращение.",
        "Ambiguous material: verbal proximity makes the passage useful for comparison, but does not by itself establish eternal recurrence.",
    ),
    (
        "excluded",
        "Контрольный фрагмент: в досье это местное возвращение исключено из положительных свидетельств вечного возвращения. Его сохранение помогает различать значения.",
        "Control passage: the dossier excludes this local return from positive evidence for eternal recurrence. Keeping it visible helps distinguish meanings.",
    ),
];

const SPEAKER_LABELS: &[(&str, &str, &str)] = &[
    (
        "animals_eagle_and_serpent",
        "Звери — орёл и змея",
        "The animals — eagle and serpent",
    ),
    ("dwarf", "Карлик", "The dwarf"),
    (
        "external_narrator",
        "Внешний повествователь",
        "External narrator",
    ),
    (
        "mixed_external_narrator_and_zarathustra",
        "Повествователь и Заратустра",
        "Narrator and Zarathustra",
    ),
    ("paratext_heading", "Заголовок", "Heading"),
    (
        "spirit_of_gravity_as_dwarf_voice",
        "Дух тяжести в голосе карлика",
        "The spirit of gravity in the dwarf’s voice",
    ),
    (
        "ugliest_man",
        "Самый безобразный человек",
        "The ugliest man",
    ),
    ("zarathustra", "Заратустра", "Zarathustra"),
    (
        "zarathustra_as_storyteller",
        "Заратустра как рассказчик",
        "Zarathustra as storyteller",
    ),
    (
        "zarathustra_midnight_song_voice",
        "Полуночная песнь Заратустры",
        "Zarathustra’s midnight song",
    ),
    (
        "zarathustra_song_voice",
        "Песенный голос Заратустры",
        "Zarathustra’s singing voice",
    ),
];

const DEMO_IDS: &[(&str, &str)] = &[
    (
        "tos.annotation.semantic-evidence-candidate.sid-7ebaa9e0fcebf5be121f19eda32fa6ee",
        "moment",
    ),
    (
        "tos.annotation.semantic-evidence-candidate.sid-22f2b9f4f255c2745006082db8c28909",
        "all-things",
    ),
    (
        "tos.annotation.semantic-evidence-candidate.sid-ccdeb398711d81a0b4999a00748e39d2",
        "same-life",
    ),
];

/// Insertion-ordered output value so its pretty form matches the former
/// `json.dumps(..., ensure_ascii=False, indent=2)` producer byte for byte.
#[derive(Clone, Debug)]
pub enum Out {
    Null,
    Bool(bool),
    Integer(u64),
    Signed(i64),
    String(String),
    Array(Vec<Out>),
    Object(Vec<(String, Out)>),
}

impl Serialize for Out {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Integer(value) => serializer.serialize_u64(*value),
            Self::Signed(value) => serializer.serialize_i64(*value),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => {
                let mut seq = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    seq.serialize_element(value)?;
                }
                seq.end()
            }
            Self::Object(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

fn object(fields: Vec<(&str, Out)>) -> Out {
    Out::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn string(value: impl Into<String>) -> Out {
    Out::String(value.into())
}
fn number(value: usize) -> Out {
    Out::Integer(value as u64)
}
fn bi(ru: impl Into<String>, en: impl Into<String>) -> Out {
    object(vec![("ru", string(ru)), ("en", string(en))])
}
fn node(
    identity: impl Into<String>,
    kind: &str,
    parent: Option<String>,
    title: Out,
    body: Out,
    refs: Vec<Out>,
    more: Vec<(&str, Out)>,
) -> Out {
    let mut fields = vec![
        ("id", string(identity)),
        ("kind", string(kind)),
        ("parentId", parent.map_or(Out::Null, string)),
        ("title", title),
        ("body", body),
        ("sourceRefs", Out::Array(refs)),
    ];
    fields.extend(more);
    object(fields)
}
fn source_ref(label: impl Into<String>, path: &Path, pointer: &str) -> Out {
    let mut value = path.to_string_lossy().into_owned();
    if !pointer.is_empty() {
        value.push('#');
        value.push_str(pointer);
    }
    object(vec![("label", string(label)), ("ref", string(value))])
}

#[derive(Clone, Debug)]
pub struct Build {
    pub data: Out,
    pub report: Out,
}

impl Build {
    pub fn output_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(&self.data).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn summary_bytes(&self, status: &str, output: &Path, bytes: &[u8]) -> Result<Vec<u8>> {
        let report = match &self.report {
            Out::Object(fields) => fields.clone(),
            _ => return Err("library report is not an object".into()),
        };
        let mut fields = vec![
            ("status".to_owned(), string(status)),
            ("output".to_owned(), string(output.to_string_lossy())),
            ("bytes".to_owned(), Out::Integer(bytes.len() as u64)),
            (
                "sha256".to_owned(),
                string(Digest256::of_bytes(bytes).to_hex()),
            ),
        ];
        fields.extend(report);
        serde_json::to_vec(&Out::Object(fields)).map_err(|error| error.to_string())
    }
}

fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(message.into())
}
fn require(condition: bool, message: &'static str) -> Result<()> {
    if condition { Ok(()) } else { fail(message) }
}
fn get<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| format!("missing field {name}"))
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    get(value, name)?
        .as_str()
        .ok_or_else(|| format!("field {name} is not a string"))
}
fn string_value(value: &Value) -> Result<&str> {
    value.as_str().ok_or_else(|| "expected a string".into())
}
fn array<'a>(value: &'a Value, name: &str) -> Result<&'a [Value]> {
    get(value, name)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("field {name} is not an array"))
}
fn integer(value: &Value, name: &str) -> Result<usize> {
    usize::try_from(
        get(value, name)?
            .as_u64()
            .ok_or_else(|| format!("field {name} is not a nonnegative integer"))?,
    )
    .map_err(|_| format!("field {name} is outside this platform's range"))
}
fn boolean(value: &Value, name: &str) -> Result<bool> {
    get(value, name)?
        .as_bool()
        .ok_or_else(|| format!("field {name} is not a boolean"))
}
fn sha(text: &str) -> String {
    Digest256::of_bytes(text.as_bytes()).to_hex()
}

fn read_json(path: &Path) -> Result<Value> {
    let raw = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&raw).map_err(|error| format!("{}: {error}", path.display()))
}
pub fn normalize_newlines(value: String) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}
fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map(normalize_newlines)
        .map_err(|error| format!("{}: {error}", path.display()))
}
fn read_rows(path: &Path) -> Result<Vec<Value>> {
    read_text(path)?
        .lines()
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(line, raw)| {
            serde_json::from_str(raw)
                .map_err(|error| format!("{}:{}: {error}", path.display(), line + 1))
        })
        .collect()
}

fn source_path(repo: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = Path::new(relative);
    let candidate = if candidate.is_absolute() {
        candidate.to_owned()
    } else {
        repo.join(candidate)
    };
    let canonical = fs::canonicalize(&candidate)
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    require(
        canonical.starts_with(repo),
        "Source reference escapes the source repository",
    )?;
    Ok(canonical)
}
fn path_string(repo: &Path, relative: &str) -> Result<PathBuf> {
    source_path(repo, relative)
}
fn read_repo_json(repo: &Path, relative: &str) -> Result<Value> {
    read_json(&path_string(repo, relative)?)
}
fn read_repo_rows(repo: &Path, relative: &str) -> Result<Vec<Value>> {
    read_rows(&path_string(repo, relative)?)
}
fn source_ref_from(
    repo: &Path,
    label: impl Into<String>,
    relative: &str,
    pointer: &str,
) -> Result<Out> {
    Ok(source_ref(label, &source_path(repo, relative)?, pointer))
}

fn count_inc(counts: &mut Vec<(String, usize)>, key: &str) {
    if let Some((_, count)) = counts.iter_mut().find(|(name, _)| name == key) {
        *count += 1;
    } else {
        counts.push((key.to_owned(), 1));
    }
}
fn count_get(counts: &[(String, usize)], key: &str) -> usize {
    counts
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, count)| *count)
        .unwrap_or(0)
}
fn count_object(counts: &[(String, usize)]) -> Out {
    Out::Object(
        counts
            .iter()
            .map(|(key, count)| (key.clone(), number(*count)))
            .collect(),
    )
}
fn chapter_label(reading: &str) -> Option<(&'static str, &'static str)> {
    CHAPTER_LABELS
        .iter()
        .find(|(key, _, _)| *key == reading)
        .map(|(_, ru, en)| (*ru, *en))
}
fn class_note(class: &str) -> Result<(&'static str, &'static str)> {
    CLASS_NOTES
        .iter()
        .find(|(key, _, _)| *key == class)
        .map(|(_, ru, en)| (*ru, *en))
        .ok_or_else(|| format!("unknown evidence class {class}"))
}
fn speaker_label(role: &str) -> Result<(&'static str, &'static str)> {
    SPEAKER_LABELS
        .iter()
        .find(|(key, _, _)| *key == role)
        .map(|(_, ru, en)| (*ru, *en))
        .ok_or_else(|| format!("unknown speaker role {role}"))
}
fn demo_id(identity: &str) -> Option<&'static str> {
    DEMO_IDS
        .iter()
        .find(|(key, _)| *key == identity)
        .map(|(_, id)| *id)
}
fn demo_identity(id: &str) -> bool {
    DEMO_IDS.iter().any(|(_, demo)| *demo == id)
}
fn reading_order(reading: &str) -> Result<(usize, usize)> {
    let (part, number) = reading
        .strip_prefix('p')
        .and_then(|rest| rest.split_once(".r"))
        .ok_or_else(|| format!("invalid reading reference {reading}"))?;
    let part = part
        .parse()
        .map_err(|_| format!("invalid reading reference {reading}"))?;
    let number = number
        .parse()
        .map_err(|_| format!("invalid reading reference {reading}"))?;
    Ok((part, number))
}
fn codepoint_slice(value: &str, start: usize, end: usize) -> Result<String> {
    let length = value.chars().count();
    require(
        start <= end && end <= length,
        "Source selector is outside its layer",
    )?;
    let byte_at = |position: usize| -> usize {
        if position == length {
            value.len()
        } else {
            value
                .char_indices()
                .nth(position)
                .map(|(index, _)| index)
                .unwrap_or(value.len())
        }
    };
    Ok(value[byte_at(start)..byte_at(end)].to_owned())
}
fn python_whitespace(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '\u{001c}'..='\u{001f}')
}
fn python_trim_start(value: &str) -> &str {
    let start = value
        .char_indices()
        .find(|(_, ch)| !python_whitespace(*ch))
        .map(|(i, _)| i)
        .unwrap_or(value.len());
    &value[start..]
}
fn python_collapse(value: &str) -> String {
    value
        .split(python_whitespace)
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn source_text(
    repo: &Path,
    part: usize,
    side: &str,
    anchor_id: &str,
    anchor_maps: &HashMap<(usize, String), HashMap<String, (usize, Value)>>,
    layer_cache: &mut HashMap<PathBuf, String>,
) -> Result<(String, Out, String)> {
    let map = anchor_maps
        .get(&(part, side.to_owned()))
        .ok_or_else(|| "missing alignment anchor map".to_owned())?;
    let (index, anchor) = map
        .get(anchor_id)
        .ok_or_else(|| format!("missing source anchor {anchor_id}"))?;
    let layer = source_path(repo, text(anchor, "text_layer_ref")?)?;
    if !layer_cache.contains_key(&layer) {
        let contents = read_text(&layer)?;
        require(
            sha(&contents) == text(anchor, "text_layer_sha256")?,
            "Source text-layer digest mismatch",
        )?;
        layer_cache.insert(layer.clone(), contents);
    } else {
        require(
            sha(layer_cache.get(&layer).expect("cached layer"))
                == text(anchor, "text_layer_sha256")?,
            "Conflicting source layer binding",
        )?;
    }
    let contents = layer_cache.get(&layer).expect("cached layer");
    let selector = get(anchor, "selector")?;
    require(
        text(selector, "type")? == "text_position"
            && text(selector, "position_unit")? == "unicode_code_point"
            && text(selector, "interval")? == "half_open",
        "Unsupported source selector",
    )?;
    let start = integer(selector, "start")?;
    let end = integer(selector, "end")?;
    let exact = codepoint_slice(contents, start, end)?;
    require(
        sha(&exact) == text(anchor, "exact_sha256")?,
        "Exact anchor digest mismatch",
    )?;
    let packet_ref = format!("{ALIGNMENT_DIR}/part-{part}.translation-alignment-packet.v1.json");
    let lang = if side == "source_side" { "DE" } else { "RU" };
    let label = format!("{lang} · {anchor_id}");
    let pointer = format!("/{side}/anchors/{index}");
    let reference = source_ref_from(repo, label, &packet_ref, &pointer)?;
    let locator = text(get(anchor, "source_return")?, "locator_ref")?.to_owned();
    Ok((exact, reference, locator))
}

#[derive(Default)]
struct XmlFrame {
    name: String,
    path: String,
    child_counts: HashMap<String, usize>,
    heading_parent: Option<String>,
}
fn xml_segment_path(parent: &str, name: &str, index: usize) -> String {
    if parent.is_empty() {
        format!("/{name}#{index}")
    } else {
        format!("{parent}/{name}#{index}")
    }
}
fn locator_key(locator: &str) -> Result<String> {
    let mut key = String::new();
    for segment in locator.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        let (name, index) = if let Some(open) = segment.rfind('[') {
            if !segment.ends_with(']') {
                return fail("Malformed chapter locator");
            }
            let index: usize = segment[open + 1..segment.len() - 1]
                .parse()
                .map_err(|_| "Malformed chapter locator")?;
            if index == 0 {
                return fail("Malformed chapter locator");
            }
            (&segment[..open], index)
        } else {
            (segment, 1)
        };
        if name.is_empty() {
            return fail("Malformed chapter locator");
        }
        key.push('/');
        key.push_str(name);
        key.push('#');
        key.push_str(&index.to_string());
    }
    Ok(key)
}
fn xml_name(raw: &[u8]) -> Result<String> {
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|error| error.to_string())
}
fn xml_heading_index(raw: &[u8]) -> Result<HashMap<String, String>> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    let mut stack: Vec<XmlFrame> = Vec::new();
    let mut headings = HashMap::new();
    let mut active_heading: Option<(String, String)> = None;
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(event) => {
                let name = xml_name(event.local_name().as_ref())?;
                let parent_path = stack
                    .last()
                    .map(|frame| frame.path.clone())
                    .unwrap_or_default();
                let index = if let Some(parent) = stack.last_mut() {
                    let count = parent.child_counts.entry(name.clone()).or_default();
                    *count += 1;
                    *count
                } else {
                    1
                };
                let path = xml_segment_path(&parent_path, &name, index);
                let heading_parent = if name == "head"
                    && active_heading.is_none()
                    && !headings.contains_key(&parent_path)
                {
                    active_heading = Some((parent_path.clone(), String::new()));
                    Some(parent_path)
                } else {
                    None
                };
                stack.push(XmlFrame {
                    name,
                    path,
                    child_counts: HashMap::new(),
                    heading_parent,
                });
            }
            Event::Empty(event) => {
                let name = xml_name(event.local_name().as_ref())?;
                let parent_path = stack
                    .last()
                    .map(|frame| frame.path.clone())
                    .unwrap_or_default();
                if let Some(parent) = stack.last_mut() {
                    *parent.child_counts.entry(name.clone()).or_default() += 1;
                }
                if name == "head" && !headings.contains_key(&parent_path) {
                    headings.insert(parent_path, String::new());
                }
            }
            Event::Text(event) => {
                if let Some((_, content)) = active_heading.as_mut() {
                    content.push_str(&event.xml10_content().map_err(|error| error.to_string())?);
                }
            }
            Event::CData(event) => {
                if let Some((_, content)) = active_heading.as_mut() {
                    content.push_str(&event.xml10_content().map_err(|error| error.to_string())?);
                }
            }
            Event::GeneralRef(event) => {
                if let Some((_, content)) = active_heading.as_mut() {
                    let name = event.decode().map_err(|error| error.to_string())?;
                    if let Some(character) = event
                        .resolve_char_ref()
                        .map_err(|error| error.to_string())?
                    {
                        content.push(character);
                    } else if let Some(replacement) =
                        quick_xml::escape::resolve_predefined_entity(&name)
                    {
                        content.push_str(replacement);
                    } else {
                        return fail("Unknown entity in source heading");
                    }
                }
            }
            Event::End(event) => {
                let name = xml_name(event.local_name().as_ref())?;
                let frame = stack
                    .pop()
                    .ok_or_else(|| "Unbalanced source XML".to_owned())?;
                require(frame.name == name, "Mismatched source XML element")?;
                if let Some(parent) = frame.heading_parent {
                    if let Some((captured_parent, content)) = active_heading.take() {
                        require(
                            captured_parent == parent,
                            "Mismatched source heading capture",
                        )?;
                        headings.insert(parent, python_collapse(&content));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    require(stack.is_empty(), "Unclosed source XML element")?;
    Ok(headings)
}
fn source_heading(
    repo: &Path,
    raw_return: &str,
    locator: &str,
    xml_cache: &mut HashMap<PathBuf, HashMap<String, String>>,
) -> Result<(String, PathBuf)> {
    let file_ref = raw_return.split('#').next().unwrap_or(raw_return);
    let path = source_path(repo, file_ref)?;
    if !xml_cache.contains_key(&path) {
        let raw = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        xml_cache.insert(path.clone(), xml_heading_index(&raw)?);
    }
    let key = locator_key(locator)?;
    let heading = xml_cache
        .get(&path)
        .and_then(|index| index.get(&key))
        .cloned()
        .ok_or_else(|| {
            "Chapter locator or source heading does not resolve in the German source".to_owned()
        })?;
    Ok((heading, path))
}

#[derive(Default)]
struct ChapterInfo {
    id: String,
    unit: String,
}

pub fn build(repo: &Path, demo_path: &Path) -> Result<Build> {
    let repo = fs::canonicalize(repo).map_err(|error| format!("{}: {error}", repo.display()))?;
    let work_path = |suffix: &str| format!("{WORK}/{suffix}");
    let candidate_path = |suffix: &str| format!("{CANDIDATE}/{suffix}");

    let work = read_repo_json(&repo, &work_path("work.json"))?;
    require(
        text(&work, "record_id")? == WORK_ID,
        "Unexpected work identity",
    )?;
    let dossier = read_repo_json(&repo, &candidate_path("concept-candidate.v1.json"))?;
    require(
        text(&dossier, "annotation_id")? == ANNOTATION
            && text(&dossier, "review_status")? == "unreviewed",
        "Dossier identity or review posture changed",
    )?;
    let body = get(&dossier, "body")?;
    require(
        get(body, "concept_id")?.is_null()
            && !boolean(body, "graph_effect")?
            && !boolean(body, "canon_effect")?,
        "Library requires the existing research-candidate posture",
    )?;

    let private = read_repo_json(&repo, ANALYSIS)?;
    let evidence_rows = read_repo_rows(&repo, &candidate_path("evidence-spine.v1.jsonl"))?;
    let mut evidence = HashMap::<String, Value>::new();
    for entry in &evidence_rows {
        evidence.insert(text(entry, "evidence_id")?.to_owned(), entry.clone());
    }
    let private_evidence_rows = array(&private, "evidence")?;
    let mut private_evidence = HashMap::<String, Value>::new();
    for entry in private_evidence_rows {
        private_evidence.insert(text(entry, "evidence_id")?.to_owned(), entry.clone());
    }
    require(
        evidence.len() == evidence_rows.len()
            && evidence_rows.len() == 210
            && evidence.keys().collect::<HashSet<_>>()
                == private_evidence.keys().collect::<HashSet<_>>(),
        "Expected all 210 unique dossier evidence units",
    )?;
    for (identity, entry) in &evidence {
        let private_entry = private_evidence
            .get(identity)
            .ok_or_else(|| "Private analysis lacks tracked evidence".to_owned())?;
        let fields = entry
            .as_object()
            .ok_or_else(|| "Evidence entry is not an object".to_owned())?;
        require(
            fields
                .iter()
                .all(|(key, value)| private_entry.get(key) == Some(value)),
            "Tracked evidence and private analysis disagree",
        )?;
        require(
            !boolean(entry, "accepted")?
                && !boolean(entry, "graph_effect")?
                && !boolean(entry, "canon_effect")?,
            "Evidence posture changed",
        )?;
    }
    let de_citation_rows = read_repo_rows(&repo, DE_CITATIONS)?;
    let ru_paragraph_rows = read_repo_rows(&repo, RU_PARAGRAPHS)?;
    let ru_structure_rows = read_repo_rows(&repo, RU_STRUCTURE)?;
    let speaker_rows = read_repo_rows(
        &repo,
        &candidate_path("review-preparation-v1/speaker-attribution-candidates.v1.jsonl"),
    )?;
    let mut de_citations = HashMap::<String, Value>::new();
    let mut source_order = HashMap::<String, usize>::new();
    for entry in &de_citation_rows {
        let id = text(entry, "unit_id")?.to_owned();
        if !source_order.contains_key(&id) {
            source_order.insert(id.clone(), source_order.len());
        }
        de_citations.insert(id, entry.clone());
    }
    let ru_paragraphs: HashMap<String, Value> = ru_paragraph_rows
        .iter()
        .map(|entry| Ok((text(entry, "paragraph_unit_id")?.to_owned(), entry.clone())))
        .collect::<Result<_>>()?;
    let ru_structures: HashMap<String, Value> = ru_structure_rows
        .iter()
        .map(|entry| Ok((text(entry, "structure_unit_id")?.to_owned(), entry.clone())))
        .collect::<Result<_>>()?;
    let speaker_entries: HashMap<String, Value> = speaker_rows
        .iter()
        .map(|entry| Ok((text(entry, "evidence_ref")?.to_owned(), entry.clone())))
        .collect::<Result<_>>()?;

    let demo = read_json(demo_path)?;
    require(
        text(&demo, "schema")? == "tos_local_story_demo_v1"
            && text(&demo, "id")? == "eternal-return",
        "Unexpected source demo packet",
    )?;
    let mut demo_nodes = HashMap::<String, Value>::new();
    for entry in array(&demo, "steps")? {
        demo_nodes.insert(text(entry, "id")?.to_owned(), entry.clone());
    }

    let mut packets = HashMap::<usize, Value>::new();
    let mut anchor_maps = HashMap::<(usize, String), HashMap<String, (usize, Value)>>::new();
    for part in 1..=4 {
        let packet_relative =
            format!("{ALIGNMENT_DIR}/part-{part}.translation-alignment-packet.v1.json");
        let packet = read_repo_json(&repo, &packet_relative)?;
        let rights = get(&packet, "rights_and_visibility")?;
        require(
            text(rights, "effective_visibility")? == "local_only"
                && !boolean(rights, "publication_authorized")?,
            "Source visibility changed; review library privacy",
        )?;
        for side in ["source_side", "target_side"] {
            let mut anchors = HashMap::new();
            for (index, entry) in array(get(&packet, side)?, "anchors")?.iter().enumerate() {
                anchors.insert(
                    text(entry, "anchor_ref")?.to_owned(),
                    (index, entry.clone()),
                );
            }
            anchor_maps.insert((part, side.to_owned()), anchors);
        }
        packets.insert(part, packet);
    }

    let mut readings = Vec::<(String, usize)>::new();
    let mut chapter_keys = BTreeSet::new();
    let mut part_counts = Vec::<(String, usize)>::new();
    let mut class_counts = Vec::<(String, usize)>::new();
    for entry in &evidence_rows {
        let reading = text(entry, "reading_ref")?;
        count_inc(&mut readings, reading);
        if reading != "p2.rNone" {
            chapter_keys.insert(reading.to_owned());
        }
        count_inc(&mut part_counts, &integer(entry, "part")?.to_string());
        count_inc(&mut class_counts, text(entry, "evidence_class")?);
    }
    let expected_chapters: BTreeSet<String> = CHAPTER_LABELS
        .iter()
        .map(|(key, _, _)| (*key).to_owned())
        .collect();
    require(
        chapter_keys == expected_chapters,
        "Chapter coverage changed; review presentation labels",
    )?;

    let mut layer_cache = HashMap::<PathBuf, String>::new();
    let mut xml_cache = HashMap::<PathBuf, HashMap<String, String>>::new();
    let mut checked_anchor_count = 0usize;
    let mut nodes = vec![node(
        "work",
        "work",
        None,
        bi("Так говорил Заратустра", "Thus Spoke Zarathustra"),
        bi(
            "Фридрих Ницше. В библиотеке — 210 выбранных фрагментов из четырёх частей книги. Открывайте главы, сравнивайте тексты и добавляйте собственные мысли к прочитанному.",
            "Friedrich Nietzsche. The library offers 210 selected passages from the book’s four parts. Open chapters, compare texts, and add your own thoughts to what you read.",
        ),
        vec![
            source_ref_from(&repo, WORK_ID, &work_path("work.json"), "")?,
            source_ref_from(
                &repo,
                ANNOTATION,
                &candidate_path("concept-candidate.v1.json"),
                "",
            )?,
            source_ref_from(
                &repo,
                "210 · coverage",
                &candidate_path("coverage-receipt.v1.json"),
                "",
            )?,
        ],
        vec![(
            "sourceNote",
            bi(
                "Фридрих Ницше. Локальная библиотека объединяет 210 выбранных свидетельств из исследовательского досье вечного возвращения: 36 глав четырёх частей и один внеглавный эпиграф. Это весь отбор досье, а не полный текст книги. Названия глав переведены для навигации; точные тексты сохраняют собственные языки и источники.",
                "Friedrich Nietzsche. This local library contains all 210 selected evidence units from the eternal-return research dossier: 36 chapters across four parts and one epigraph outside the chapter structure. It is the complete dossier selection, not the complete book. Chapter titles are translated for navigation; exact texts retain their own languages and sources.",
            ),
        )],
    )];

    for part in 1..=4 {
        let packet = packets
            .get(&part)
            .ok_or_else(|| "Missing alignment packet".to_owned())?;
        let packet_ref =
            format!("{ALIGNMENT_DIR}/part-{part}.translation-alignment-packet.v1.json");
        let chapter_count = chapter_keys
            .iter()
            .filter(|reading| reading.starts_with(&format!("p{part}.")))
            .count();
        let fragment_count = count_get(&part_counts, &part.to_string());
        let part_body_ru = format!(
            "В этой части для чтения открыты {chapter_count} глав. В библиотеке: {fragment_count} фрагментов. Выберите главу, чтобы приблизиться к тексту."
        );
        let part_body_en = format!(
            "Explore {chapter_count} chapters from this part, with {fragment_count} passages in the library. Choose a chapter to approach the text."
        );
        let part_source_ru = format!(
            "{fragment_count} выбранных фрагментов. Немецкое свидетельство — издание {} года, TEI DTA; русский слой — техническое извлечение перевода Антоновского 1911 года. Сопоставления предложены для рассмотрения и не устанавливают принятую эквивалентность переводов.",
            DE_YEARS[part - 1]
        );
        let part_source_en = format!(
            "{fragment_count} selected passages. The German witness is the {} edition in DTA TEI; the Russian layer is a technical extraction of Antonovsky’s 1911 translation. Alignments are proposals for consideration, not accepted translation equivalences.",
            DE_YEARS[part - 1]
        );
        nodes.push(node(
            format!("part-{part}"),
            "part",
            Some("work".into()),
            bi(
                format!("Часть {}", ROMAN[part - 1]),
                format!("Part {}", ROMAN[part - 1]),
            ),
            bi(part_body_ru, part_body_en),
            vec![
                source_ref_from(
                    &repo,
                    text(get(packet, "source_side")?, "expression_ref")?,
                    &packet_ref,
                    "/source_side",
                )?,
                source_ref_from(
                    &repo,
                    text(get(packet, "target_side")?, "expression_ref")?,
                    &packet_ref,
                    "/target_side",
                )?,
            ],
            vec![("sourceNote", bi(part_source_ru, part_source_en))],
        ));
    }

    let mut chapter_info = HashMap::<String, ChapterInfo>::new();
    let mut ordered_chapters: Vec<String> = chapter_keys.iter().cloned().collect();
    ordered_chapters
        .sort_by_key(|reading| reading_order(reading).unwrap_or((usize::MAX, usize::MAX)));
    for reading in ordered_chapters {
        let selected: Vec<&Value> = evidence_rows
            .iter()
            .filter(|entry| text(entry, "reading_ref").ok() == Some(reading.as_str()))
            .collect();
        let entry = selected
            .first()
            .ok_or_else(|| "Chapter has no evidence".to_owned())?;
        let part = integer(entry, "part")?;
        let mut source_units = Vec::<Value>::new();
        for row in &selected {
            for unit in array(row, "source_paragraph_unit_refs")? {
                let identity = string_value(unit)?;
                source_units.push(
                    de_citations
                        .get(identity)
                        .ok_or_else(|| format!("Missing German citation {identity}"))?
                        .clone(),
                );
            }
        }
        let majors: HashSet<Option<String>> = source_units
            .iter()
            .map(|unit| {
                get(unit, "nearest_major_unit_id")
                    .ok()
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        require(
            majors.len() == 1 && !majors.contains(&None),
            "A presentation chapter crosses source chapter boundaries",
        )?;
        let major = majors
            .iter()
            .next()
            .and_then(Option::as_ref)
            .ok_or_else(|| "Missing major source unit".to_owned())?;
        let chapter = de_citations
            .get(major)
            .ok_or_else(|| "Missing major source citation".to_owned())?;
        let (part_from_reading, reading_number) = reading_order(&reading)?;
        require(
            integer(chapter, "part_order")? == part
                && integer(chapter, "major_correspondence_sequence")?
                    == reading_number + usize::from(part == 1 && reading_number > 1),
            "Dossier reading does not match source-owned chapter",
        )?;
        let source_anchors = array(entry, "source_anchor_refs")?;
        let first_anchor_id = string_value(
            source_anchors
                .first()
                .ok_or_else(|| "Chapter evidence lacks source anchor".to_owned())?,
        )?;
        let anchor_map = anchor_maps
            .get(&(part, "source_side".to_owned()))
            .ok_or_else(|| "Missing source-side anchor map".to_owned())?;
        let (_, anchor) = anchor_map
            .get(first_anchor_id)
            .ok_or_else(|| format!("Missing chapter source anchor {first_anchor_id}"))?;
        let raw_return = text(get(anchor, "source_return")?, "locator_ref")?;
        let (heading, xml_ref) = source_heading(
            &repo,
            raw_return,
            text(chapter, "source_locator")?,
            &mut xml_cache,
        )?;
        let mut ru_unit_refs = BTreeSet::new();
        for row in &selected {
            for unit in array(row, "target_paragraph_unit_refs")? {
                let unit = string_value(unit)?;
                let paragraph = ru_paragraphs
                    .get(unit)
                    .ok_or_else(|| format!("Missing Russian paragraph {unit}"))?;
                ru_unit_refs.insert(text(paragraph, "reading_unit_ref")?.to_owned());
            }
        }
        let mut refs = vec![
            source_ref_from(
                &repo,
                text(chapter, "unit_id")?,
                DE_CITATIONS,
                &format!("unit={}", text(chapter, "unit_id")?),
            )?,
            source_ref(heading.clone(), &xml_ref, text(chapter, "source_locator")?),
        ];
        for ru_unit in &ru_unit_refs {
            let structure = ru_structures
                .get(ru_unit)
                .ok_or_else(|| format!("Missing Russian structure unit {ru_unit}"))?;
            require(
                text(structure, "part_id")? == format!("part_{part}")
                    && integer(structure, "reading_unit_ordinal_within_part")? == reading_number,
                "Russian chapter correspondence differs from this presentation grouping",
            )?;
            refs.push(source_ref_from(
                &repo,
                ru_unit.clone(),
                RU_STRUCTURE,
                &format!("unit={ru_unit}"),
            )?);
        }
        let identity = format!("chapter-{reading}");
        chapter_info.insert(
            reading.clone(),
            ChapterInfo {
                id: identity.clone(),
                unit: text(chapter, "unit_id")?.to_owned(),
            },
        );
        let count = count_get(&readings, &reading);
        let chapter_title_ru = chapter_label(&reading)
            .ok_or_else(|| "Missing chapter navigation label".to_owned())?
            .0;
        let chapter_title_en = chapter_label(&reading)
            .ok_or_else(|| "Missing chapter navigation label".to_owned())?
            .1;
        let chapter_source_ru = format!(
            "{heading} Здесь собрано {count} фрагментов, отобранных досье. Откройте их для чтения и сравнения. Русское и английское названия служат навигации; немецкий заголовок возвращает к конкретной главе исходного издания."
        );
        let chapter_source_en = format!(
            "{heading} This chapter contains {count} passages selected by the dossier. Open them for reading and comparison. Russian and English titles provide navigation; the German heading returns to the specific chapter in the source edition."
        );
        nodes.push(node(
            identity, "chapter", Some(format!("part-{part}")),
            bi(chapter_title_ru, chapter_title_en),
            bi(format!("Часть {} · фрагментов для чтения: {count}. Откройте любой, чтобы читать и сравнивать.", ROMAN[part - 1]), format!("Part {} · {count} passages to explore. Open any passage to read and compare.", ROMAN[part - 1])),
            refs,
            vec![("sourceNote", bi(chapter_source_ru, chapter_source_en))],
        ));
        let _ = part_from_reading;
    }

    let mut sorted_evidence = evidence_rows.iter().collect::<Vec<_>>();
    sorted_evidence.sort_by_key(|entry| {
        let part = integer(entry, "part").unwrap_or(usize::MAX);
        let first_unit = array(entry, "source_paragraph_unit_refs")
            .ok()
            .and_then(|refs| refs.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let order = de_citations
            .get(first_unit)
            .and_then(|citation| text(citation, "unit_id").ok())
            .and_then(|id| source_order.get(id))
            .copied()
            .unwrap_or(usize::MAX);
        (part, order)
    });
    let mut reading_positions = HashMap::<String, usize>::new();
    let mut translated_count = 0usize;
    let mut bilingual_count = 0usize;
    for entry in sorted_evidence {
        let identity = text(entry, "evidence_id")?;
        let part = integer(entry, "part")?;
        let reading = text(entry, "reading_ref")?;
        let local_number = {
            let count = reading_positions.entry(reading.to_owned()).or_default();
            *count += 1;
            *count
        };
        let raw = private_evidence
            .get(identity)
            .ok_or_else(|| "Private analysis lacks evidence".to_owned())?;
        let packet = packets
            .get(&part)
            .ok_or_else(|| "Missing alignment packet".to_owned())?;
        let packet_relative =
            format!("{ALIGNMENT_DIR}/part-{part}.translation-alignment-packet.v1.json");
        let mapping = array(packet, "alignments")?
            .iter()
            .find(|item| text(item, "alignment_id").ok() == text(entry, "alignment_ref").ok())
            .ok_or_else(|| {
                format!(
                    "Missing alignment {}",
                    text(entry, "alignment_ref").unwrap_or("")
                )
            })?;
        require(
            text(mapping, "status")? == text(entry, "alignment_status")?
                && text(mapping, "status")? != "accepted",
            "Alignment review status changed",
        )?;
        let mut exact = HashMap::<String, String>::new();
        let mut refs = Vec::<Out>::new();
        for (lang, side, anchor_field, mapping_field) in [
            (
                "de",
                "source_side",
                "source_anchor_refs",
                "ordered_source_anchor_refs",
            ),
            (
                "ru",
                "target_side",
                "target_anchor_refs",
                "ordered_target_anchor_refs",
            ),
        ] {
            let anchors = array(entry, anchor_field)?;
            require(
                get(mapping, mapping_field)? == get(entry, anchor_field)?,
                "Evidence and mapping anchors disagree",
            )?;
            let mut pieces = Vec::new();
            for anchor in anchors {
                let anchor_id = string_value(anchor)?;
                let (piece, reference, _locator) =
                    source_text(&repo, part, side, anchor_id, &anchor_maps, &mut layer_cache)?;
                checked_anchor_count += 1;
                pieces.push(piece);
                refs.push(reference);
            }
            let joined = pieces.join("\n");
            require(
                sha(&joined) == text(entry, &format!("{lang}_exact_sha256"))?
                    && joined == text(raw, &format!("{lang}_text"))?,
                "Evidence excerpt does not match its exact anchored bytes",
            )?;
            if !pieces.is_empty() {
                refs.push(source_ref_from(
                    &repo,
                    format!(
                        "{} · SHA-256 {}",
                        lang.to_uppercase(),
                        text(entry, &format!("{lang}_exact_sha256"))?
                    ),
                    &candidate_path("evidence-spine.v1.jsonl"),
                    &format!("evidence={identity}"),
                )?);
            }
            exact.insert(lang.to_owned(), joined);
        }
        if !exact["de"].is_empty() && !exact["ru"].is_empty() {
            bilingual_count += 1;
        }
        let mut de_units = Vec::new();
        for unit in array(entry, "source_paragraph_unit_refs")? {
            let unit = string_value(unit)?;
            de_units.push(
                de_citations
                    .get(unit)
                    .ok_or_else(|| format!("Missing German citation {unit}"))?
                    .clone(),
            );
        }
        for unit in &de_units {
            refs.push(source_ref_from(
                &repo,
                text(unit, "display_citation")?,
                DE_CITATIONS,
                &format!("unit={}", text(unit, "unit_id")?),
            )?);
        }
        for unit in array(entry, "target_paragraph_unit_refs")? {
            let unit = string_value(unit)?;
            let paragraph = ru_paragraphs
                .get(unit)
                .ok_or_else(|| format!("Missing Russian paragraph {unit}"))?;
            refs.push(source_ref_from(
                &repo,
                text(paragraph, "display_citation")?,
                RU_PARAGRAPHS,
                &format!("unit={unit}"),
            )?);
        }
        refs.push(source_ref_from(
            &repo,
            identity,
            &candidate_path("evidence-spine.v1.jsonl"),
            &format!("evidence={identity}"),
        )?);
        refs.push(source_ref_from(
            &repo,
            text(entry, "alignment_ref")?,
            &packet_relative,
            &format!("alignment={}", text(entry, "alignment_ref")?),
        )?);
        let parent = if let Some(chapter) = chapter_info.get(reading) {
            require(
                de_units.iter().all(|unit| {
                    get(unit, "nearest_major_unit_id")
                        .ok()
                        .and_then(Value::as_str)
                        == Some(chapter.unit.as_str())
                }),
                "Fragment parent is not its source chapter",
            )?;
            chapter.id.clone()
        } else {
            require(
                reading == "p2.rNone"
                    && de_units.iter().all(|unit| {
                        get(unit, "nearest_major_unit_id")
                            .ok()
                            .is_some_and(Value::is_null)
                    }),
                "Unexpected material outside chapters",
            )?;
            format!("part-{part}")
        };
        let short_id = demo_id(identity).unwrap_or(identity);
        let (chapter_label_ru, chapter_label_en) =
            chapter_label(reading).unwrap_or(("Эпиграф", "Epigraph"));
        let ru_exact = exact.get("ru").map(String::as_str).unwrap_or("");
        let de_exact = exact.get("de").map(String::as_str).unwrap_or("");
        let incipit = python_trim_start(ru_exact);
        let ru_title = if incipit.chars().count() > 65 {
            let prefix = incipit.chars().take(64).collect::<String>();
            format!("{}…", python_trim_end(&prefix))
        } else {
            python_trim_end(incipit).to_owned()
        };
        let ru_title = if !ru_title.is_empty() {
            ru_title
        } else {
            format!("{} · {local_number}", chapter_label_ru)
        };
        let mut title = bi(ru_title, format!("{chapter_label_en} · {local_number}"));
        let mut quote = bi(
            if !ru_exact.is_empty() {
                ru_exact
            } else {
                de_exact
            },
            de_exact,
        );
        let mut quote_note = bi(
            "Русский: точное техническое извлечение перевода Антоновского 1911 года; сохранены историческая орфография, переносы и возможные ошибки извлечения. Английский перевод отсутствует. При выборе EN показывается точный немецкий текст.",
            "English translation unavailable — showing the exact German witness. Russian text is the exact technical extraction of Antonovsky’s 1911 translation, retaining historical spelling, line breaks, and possible extraction errors.",
        );
        if ru_exact.is_empty() {
            quote_note = bi(
                "В текущем сопоставлении нет русского фрагмента: показан точный немецкий текст. Этот пробел не доказывает отсутствия перевода в книге. Английский перевод также отсутствует.",
                "No Russian passage is present in this alignment: the exact German text is shown. This gap does not establish an omission in the translated book. English translation is also unavailable.",
            );
        }
        let mut body;
        if demo_identity(short_id) {
            let previous = demo_nodes
                .get(short_id)
                .ok_or_else(|| format!("Missing demo translation {short_id}"))?;
            require(
                get(get(previous, "source")?, "exact")? == &value_exact(&exact),
                "Existing demo translation is bound to a different source excerpt",
            )?;
            let previous_quote = get(previous, "quote")?;
            require(
                previous_quote.is_object()
                    && previous_quote
                        .as_object()
                        .is_some_and(|fields| fields.len() == 2)
                    && previous_quote
                        .get("ru")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty())
                    && previous_quote
                        .get("en")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty()),
                "Existing bilingual demo quote is incomplete",
            )?;
            quote = localized_out(previous_quote)?;
            quote_note = localized_out(get(previous, "quoteNote")?)?;
            title = localized_out(get(previous, "title")?)?;
            translated_count += 1;
            body = localized_out(get(previous, "body")?)?;
        } else if chapter_label(reading).is_some() {
            body = bi(
                format!("Из главы «{chapter_label_ru}» · часть {}.", ROMAN[part - 1]),
                format!("From {chapter_label_en} · Part {}.", ROMAN[part - 1]),
            );
        } else {
            body = bi("Эпиграф ко второй части.", "Epigraph to Part II.");
        }
        let (mut note_ru, mut note_en) =
            class_note(text(entry, "evidence_class")?)?.to_owned_pair();
        if !boolean(entry, "positive_evidence_eligible")? {
            note_ru.push_str(" Сопоставление одностороннее; как двуязычное подтверждение этот фрагмент не используется.");
            note_en.push_str(
                " The alignment is one-sided; this entry is not used as bilingual support.",
            );
        }
        let source_note = bi(note_ru, note_en);
        let voice = speaker_entries.get(identity);
        let speaker = if let Some(voice) = voice {
            let role = text(voice, "primary_role")?;
            let (base_ru, base_en) = speaker_label(role)?;
            let mut speaker_ru = format!("{base_ru} · предварительная атрибуция");
            let mut speaker_en = format!("{base_en} · provisional attribution");
            if get(voice, "alternative_roles")
                .ok()
                .and_then(Value::as_array)
                .is_some_and(|roles| {
                    roles.iter().any(|role| {
                        role.as_str() == Some("animals_voicing_a_hypothetical_zarathustra")
                    })
                })
            {
                speaker_ru = "Звери, в том числе передающие предполагаемые слова Заратустры · атрибуция открыта".into();
                speaker_en = "The animals, including their imagined words of Zarathustra · attribution remains open".into();
            }
            refs.push(source_ref_from(
                &repo,
                text(voice, "speaker_attribution_candidate_id")?,
                &candidate_path("review-preparation-v1/speaker-attribution-candidates.v1.jsonl"),
                &format!("evidence={identity}"),
            )?);
            bi(speaker_ru, speaker_en)
        } else {
            bi(
                "Голос не установлен в этом досье",
                "The speaker has not been identified in this dossier",
            )
        };
        nodes.push(node(
            short_id,
            "fragment",
            Some(parent),
            title,
            body,
            refs,
            vec![
                ("exact", exact_out(&exact)),
                ("quote", quote),
                ("quoteNote", quote_note),
                ("speaker", speaker),
                ("sourceNote", source_note),
            ],
        ));
    }

    nodes.push(node(
        "dossier", "dossier", Some("work".into()),
        bi("Вечное возвращение · досье", "Eternal recurrence · dossier"),
        bi("Исследовательский кандидат: 210 фрагментов и три открытых направления чтения — космологическое, экзистенциальное и поэтическое. Сравнивайте их опоры и развивайте собственное понимание.", "A research candidate: 210 passages and three open directions of reading — cosmological, existential, and poetic. Compare their evidence and develop your own understanding."),
        vec![
            source_ref_from(&repo, ANNOTATION, &candidate_path("concept-candidate.v1.json"), "")?,
            source_ref_from(&repo, "210 · coverage", &candidate_path("coverage-receipt.v1.json"), "")?,
            source_ref_from(&repo, "3 · readings", &candidate_path("review-preparation-v1/interpretation-review-matrix.v1.json"), "")?,
        ],
        vec![("sourceNote", bi("Исследовательский кандидат объединяет явные формулы, сопутствующие образы, неоднозначные места и исключённые контрольные примеры. Здесь открыты космологическое, экзистенциальное и поэтическое чтения. Ни одно из них не принято; concept_id не выдан, graph_effect=false и canon_effect=false. Свободные связи в вашем Древе выражают ваши собственные заметки и не меняют этот статус.", "This research candidate brings together explicit formulations, supporting images, ambiguous passages, and excluded controls. Cosmological, existential, and poetic readings remain open. None has been accepted; no concept_id has been issued, graph_effect=false and canon_effect=false. Free connections in your Tree express your own notes and do not change that status."))],
    ));

    require(
        bilingual_count == 206 && translated_count == 3,
        "Unexpected language coverage",
    )?;
    let identities: HashSet<String> = nodes
        .iter()
        .map(|item| match item {
            Out::Object(fields) => fields
                .iter()
                .find(|(key, _)| key == "id")
                .and_then(|(_, value)| match value {
                    Out::String(id) => Some(id.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
            _ => String::new(),
        })
        .collect();
    require(
        identities.len() == nodes.len() && nodes.len() == 252,
        "Library must contain 252 unique nodes",
    )?;
    for item in &nodes {
        let fields = match item {
            Out::Object(fields) => fields,
            _ => return fail("Library node is not an object"),
        };
        let get_out = |name: &str| {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value)
        };
        let id = match get_out("id") {
            Some(Out::String(id)) => id.as_str(),
            _ => return fail("Library node lacks an ID"),
        };
        let parent = get_out("parentId").ok_or_else(|| "Library node lacks a parent".to_owned())?;
        if id == "work" {
            require(matches!(parent, Out::Null), "Invalid structural parent")?;
        } else if let Out::String(parent) = parent {
            require(identities.contains(parent), "Invalid structural parent")?;
        } else {
            return fail("Invalid structural parent");
        }
        let kind = match get_out("kind") {
            Some(Out::String(kind)) => kind.as_str(),
            _ => "",
        };
        let mut required_fields = vec!["title", "body", "sourceNote"];
        if kind == "fragment" {
            required_fields.extend(["quote", "quoteNote", "speaker"]);
        }
        for field in required_fields {
            let localized = get_out(field)
                .and_then(|value| match value {
                    Out::Object(fields) => Some(fields),
                    _ => None,
                })
                .ok_or_else(|| "Incomplete bilingual UI field".to_owned())?;
            for lang in ["ru", "en"] {
                require(
                    localized.iter().any(|(key, value)| {
                        key == lang && matches!(value, Out::String(text) if !text.is_empty())
                    }),
                    "Incomplete bilingual UI fields",
                )?;
            }
        }
        if let Some(Out::Array(refs)) = get_out("sourceRefs") {
            for reference in refs {
                let path =
                    match reference {
                        Out::Object(fields) => fields
                            .iter()
                            .find(|(key, _)| key == "ref")
                            .and_then(|(_, value)| match value {
                                Out::String(reference) => Some(
                                    reference.split('#').next().unwrap_or(reference).to_owned(),
                                ),
                                _ => None,
                            }),
                        _ => None,
                    }
                    .ok_or_else(|| "Malformed source reference".to_owned())?;
                require(Path::new(&path).is_file(), "Missing source reference")?;
            }
        } else {
            return fail("Library node lacks source references");
        }
    }

    let mut kinds = Vec::<(String, usize)>::new();
    for item in &nodes {
        if let Out::Object(fields) = item {
            if let Some((_, Out::String(kind))) = fields.iter().find(|(key, _)| key == "kind") {
                count_inc(&mut kinds, kind);
            }
        }
    }
    let mut data = object(vec![
        ("schema", string("tos_constructor_library_v1")),
        ("rootId", string("work")),
        ("nodes", Out::Array(nodes)),
    ]);
    let fingerprint = fingerprint(&data)?;
    if let Out::Object(fields) = &mut data {
        fields.push(("fingerprint".into(), string(fingerprint)));
    }
    let report = object(vec![
        ("nodes", number(identities.len())),
        ("kinds", count_object(&kinds)),
        ("evidence_units", number(evidence.len())),
        ("evidence_classes", count_object(&class_counts)),
        ("de_ru_pairs", number(bilingual_count)),
        ("de_only", number(evidence.len() - bilingual_count)),
        ("en_demo_translations", number(translated_count)),
        ("en_unavailable", number(evidence.len() - translated_count)),
        ("anchor_excerpts_checked", number(checked_anchor_count)),
        ("graph_effect", Out::Bool(false)),
        ("canon_effect", Out::Bool(false)),
    ]);
    Ok(Build { data, report })
}

fn python_trim_end(value: &str) -> &str {
    let end = value
        .char_indices()
        .rev()
        .find(|(_, ch)| !python_whitespace(*ch))
        .map(|(i, ch)| i + ch.len_utf8())
        .unwrap_or(0);
    &value[..end]
}
trait PairOwned {
    fn to_owned_pair(self) -> (String, String);
}
impl PairOwned for (&str, &str) {
    fn to_owned_pair(self) -> (String, String) {
        (self.0.to_owned(), self.1.to_owned())
    }
}
fn exact_out(exact: &HashMap<String, String>) -> Out {
    object(vec![
        ("de", string(exact.get("de").cloned().unwrap_or_default())),
        ("ru", string(exact.get("ru").cloned().unwrap_or_default())),
    ])
}
fn localized_out(value: &Value) -> Result<Out> {
    let fields = value
        .as_object()
        .ok_or_else(|| "Incomplete bilingual UI fields".to_owned())?;
    require(
        fields.len() == 2 && value.get("ru").is_some() && value.get("en").is_some(),
        "Incomplete bilingual UI fields",
    )?;
    let ru = text(value, "ru")?;
    let en = text(value, "en")?;
    require(
        !ru.is_empty() && !en.is_empty(),
        "Incomplete bilingual UI fields",
    )?;
    Ok(bi(ru, en))
}
fn value_exact(exact: &HashMap<String, String>) -> Value {
    serde_json::json!({"ru": exact.get("ru").cloned().unwrap_or_default(), "de": exact.get("de").cloned().unwrap_or_default()})
}
fn fingerprint(value: &Out) -> Result<String> {
    fn sort_fields(value: &mut Out) {
        match value {
            Out::Array(values) => values.iter_mut().for_each(sort_fields),
            Out::Object(fields) => {
                fields.sort_by(|left, right| left.0.cmp(&right.0));
                fields.iter_mut().for_each(|(_, value)| sort_fields(value));
            }
            _ => {}
        }
    }
    let mut sorted = value.clone();
    sort_fields(&mut sorted);
    let bytes = serde_json::to_vec(&sorted).map_err(|error| error.to_string())?;
    Ok(Digest256::of_bytes(&bytes).to_hex())
}
