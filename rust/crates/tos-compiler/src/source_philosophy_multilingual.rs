//! Exact source-owned multilingual display mechanics, never translation admission.
use crate::source_philosophy_support::{bytes, required, truth};
use crate::{Error, Result};
use regex::Regex;
use serde_json::{Value, json};
const DRAFT: &[(&str, &str)] = &[
    ("до\\s*н\\.?\\s*э\\.?", "BCE"),
    ("н\\.?\\s*э\\.?", "CE"),
    ("тысячелетия|тыс\\.", "millennium"),
    ("вв\\.", "centuries"),
    ("в\\.", "century"),
    ("Западная Азия", "West Asia"),
    ("Северная Африка", "North Africa"),
    ("Южная Азия", "South Asia"),
    ("Центральная Азия", "Central Asia"),
    ("Восточная Азия", "East Asia"),
    ("Юго-Восточная Азия", "Southeast Asia"),
    ("Египетский|Египетская", "Egyptian"),
    ("Египет", "Egypt"),
    ("позднеегипетская|поздний|поздняя", "late"),
    ("ранний|ранняя", "early"),
    ("многоязычный", "multilingual"),
    ("многоязычие", "multilingualism"),
    ("письменная фиксация", "written fixation"),
    ("письменный|письменное", "written"),
    ("писцовая этика", "scribal ethics"),
    ("школьная словесность", "school literature"),
    ("мудрость", "wisdom"),
    ("право|закон", "law"),
    ("нормативно", "normative"),
    ("государственно", "state"),
    ("ритуальн(?:ый|ая|ое|ые|ого|ой|ых|ым|ыми|ом|ую)", "ritual"),
    ("ритуал", "ritual"),
    ("текстов(?:ый|ая|ое|ые|ого|ой|ых|ым|ыми|ом|ую)", "textual"),
    ("текст", "text"),
    ("письмо", "writing"),
    ("храмовая ученость", "temple scholarship"),
    ("ученость", "scholarship"),
    ("комментарий", "commentary"),
    ("корпусы", "corpora"),
    ("корпус", "corpus"),
    ("знаки", "signs"),
    ("печати", "seals"),
    ("предел реконструкции", "limits of reconstruction"),
    ("клинописный слой", "cuneiform layer"),
    ("клинопись", "cuneiform"),
    ("трехъязычие", "trilingualism"),
    ("царские надписи", "royal inscriptions"),
    ("надписи", "inscriptions"),
    ("эпиграфика", "epigraphy"),
    ("санскритские", "Sanskrit"),
    ("палийские", "Pali"),
    ("буддизм", "Buddhism"),
    ("джайнизм", "Jainism"),
    ("дисциплина", "discipline"),
    ("шастра", "shastra"),
    ("даршаны", "darshanas"),
    ("канон", "canon"),
    ("Авеста", "Avesta"),
    ("ахеменидские", "Achaemenid"),
    ("Маат", "Ma'at"),
    ("Мани", "Mani"),
    ("манихейство", "Manichaeism"),
    ("Коптский", "Coptic"),
    ("Эламский", "Elamite"),
    ("Элам", "Elam"),
    ("Индская цивилизация", "Indus Civilization"),
    ("Хеттское|Хеттский", "Hittite"),
    ("Левантская", "Levantine"),
    ("иврито-арамейский", "Hebrew-Aramaic"),
    ("документальный мир", "documentary world"),
    ("Вторая храмовая Иудея", "Second Temple Judea"),
    ("Шан", "Shang"),
    ("ранний Чжоу", "Early Zhou"),
    ("Ста школ", "Hundred Schools"),
    ("Имперское конфуцианство", "Imperial Confucianism"),
    (
        "Ведийско-брахманическая традиция",
        "Vedic-Brahmanical tradition",
    ),
    ("ранние Упанишады", "Early Upanishads"),
    ("Шраманские традиции", "Shramana traditions"),
    ("Тхеравада", "Theravada"),
    ("палийского канона", "Pali Canon"),
    ("Древний Иран", "Ancient Iran"),
    ("Герметические", "Hermetic"),
    ("гностические", "Gnostic"),
    ("мандеистские", "Mandaean"),
    ("поздней античности", "Late Antiquity"),
    ("до ислама", "before Islam"),
];
pub const LABEL_LEDGER: &str = "ToS/philosophy/atlas/multilingual/content-labels.json";
#[derive(Clone, Copy, Debug)]
pub struct MultilingualLimits {
    pub max_ledger_bytes: usize,
    pub max_label_bytes: usize,
    pub max_work_bytes: u64,
}
impl Default for MultilingualLimits {
    fn default() -> Self {
        Self {
            max_ledger_bytes: 2 * 1024 * 1024,
            max_label_bytes: 32768,
            max_work_bytes: 32 * 1024 * 1024,
        }
    }
}
pub struct Multilingual {
    ledger: Value,
    patterns: Vec<(Regex, &'static str)>,
    limits: MultilingualLimits,
}
impl Multilingual {
    pub fn from_ledger(ledger: &Value) -> Result<Self> {
        Self::from_ledger_with_limits(ledger, MultilingualLimits::default())
    }
    pub fn from_ledger_with_limits(ledger: &Value, limits: MultilingualLimits) -> Result<Self> {
        if limits.max_ledger_bytes == 0
            || limits.max_ledger_bytes > 8 * 1024 * 1024
            || limits.max_label_bytes == 0
            || limits.max_label_bytes > 65536
            || limits.max_work_bytes == 0
        {
            return Err(Error::Budget("philosophy multilingual limits"));
        }
        bytes(ledger, limits.max_ledger_bytes)?;
        if required(ledger, "schema_version")? != "tos_philosophy_multilingual_labels_v1" {
            return Err(Error::Invalid("philosophy multilingual source version"));
        }
        let patterns = DRAFT
            .iter()
            .map(|(p, r)| {
                regex::RegexBuilder::new(&format!(
                    "({})(?:$|[^0-9A-Za-zА-Яа-яЁё_İıſK])",
                    p.replace(r"\s", r"[\s\x{1c}-\x{1f}]")
                ))
                .case_insensitive(true)
                .size_limit(1 << 20)
                .build()
                .map(|p| (p, *r))
                .map_err(|e| Error::Source(e.to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            ledger: ledger.clone(),
            patterns,
            limits,
        })
    }
    fn exact(&self, text: &str, language: &str) -> Option<String> {
        self.ledger["label_sets"]["exact_labels"][text][language]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    }
    fn dossier(&self, text: &str, language: &str, context: Option<&str>) -> Option<String> {
        let (clean, docx, prefix) = clean_prefix(text);
        let explicit = dossier_id(&clean);
        let identity = if let Some(id) = context {
            if !valid_dossier_id(id) || explicit.is_some_and(|s| s != id) {
                return None;
            }
            id
        } else {
            explicit?
        };
        let titles = &self.ledger["label_sets"]["dossier_titles"][identity];
        let title = titles[language].as_str().filter(|s| !s.is_empty())?;
        if context.is_none() {
            let suffix = clean[identity.len()..]
                .trim_matches(crate::source_philosophy_support::source_space);
            let reviewed = titles
                .as_object()
                .into_iter()
                .flat_map(|o| o.values())
                .filter_map(Value::as_str)
                .any(|v| {
                    suffix
                        == format!(
                            "— {}",
                            v.trim_matches(crate::source_philosophy_support::source_space)
                        )
                });
            let prefixed = prefix.is_some()
                && suffix
                    .chars()
                    .next()
                    .is_some_and(|c| matches!(c, '—' | ':' | '-'))
                && !suffix
                    .chars()
                    .skip(1)
                    .collect::<String>()
                    .trim_matches(crate::source_philosophy_support::source_space)
                    .is_empty();
            if !(suffix.is_empty() || prefixed || reviewed) {
                return None;
            }
        }
        let prefix = match prefix {
            Some("Corpus Or Prepared Source Document:") => "Corpus Or Prepared Source Document: ",
            Some(_) => "ToS Deep Research: ",
            None => "",
        };
        Some(format!(
            "{prefix}{identity} — {title}{}",
            if docx { ".docx" } else { "" }
        ))
    }
    fn draft(&self, text: &str) -> Result<String> {
        let mut translated = text.to_owned();
        let mut work = 0u64;
        for (pattern, replacement) in &self.patterns {
            work = work
                .checked_add(translated.len() as u64)
                .ok_or(Error::Budget("philosophy label work"))?;
            if work > self.limits.max_work_bytes {
                return Err(Error::Budget("philosophy label work"));
            }
            let mut out = String::new();
            let mut cursor = 0;
            let mut search = 0;
            while let Some(captures) = pattern.captures_at(&translated, search) {
                let m = captures.get(1).expect("draft source pattern capture");
                if translated[..m.start()]
                    .chars()
                    .next_back()
                    .is_some_and(word_char)
                {
                    search = m.start()
                        + translated[m.start()..]
                            .chars()
                            .next()
                            .expect("nonempty draft")
                            .len_utf8();
                    continue;
                }
                out.push_str(&translated[cursor..m.start()]);
                out.push_str(replacement);
                cursor = m.end();
                search = m.end();
                if out.len() > self.limits.max_label_bytes {
                    return Err(Error::Budget("philosophy translated label"));
                }
            }
            out.push_str(&translated[cursor..]);
            translated = out;
            if translated.len() > self.limits.max_label_bytes {
                return Err(Error::Budget("philosophy translated label"));
            }
        }
        let separator = Regex::new(r"[\s\x{1c}-\x{1f}]+—[\s\x{1c}-\x{1f}]+")
            .map_err(|e| Error::Source(e.to_string()))?;
        translated = separator.replace_all(&translated, ": ").into_owned();
        for (word, replacement) in [("и", "and"), ("как", "as")] {
            let chars = translated.char_indices().collect::<Vec<_>>();
            let mut out = String::new();
            let mut cursor = 0;
            for (i, (offset, _)) in chars.iter().enumerate() {
                if *offset < cursor {
                    continue;
                }
                let tail = &translated[*offset..];
                if tail.get(..word.len()).map(str::to_lowercase).as_deref() != Some(word) {
                    continue;
                }
                let end = offset + word.len();
                if !translated.is_char_boundary(end) {
                    continue;
                }
                let before = i == 0
                    || matches!(chars[i - 1].1, '/' | '—' | '-')
                    || crate::source_philosophy_support::source_space(chars[i - 1].1);
                let after = end == translated.len()
                    || translated[end..].chars().next().is_some_and(|c| {
                        matches!(c, '/' | '—' | '-')
                            || crate::source_philosophy_support::source_space(c)
                    });
                if before && after {
                    out.push_str(&translated[cursor..*offset]);
                    out.push_str(replacement);
                    cursor = end;
                }
            }
            out.push_str(&translated[cursor..]);
            translated = out;
        }
        let out = translated
            .split(crate::source_philosophy_support::source_space)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if out.len() > self.limits.max_label_bytes {
            return Err(Error::Budget("philosophy translated label"));
        }
        Ok(out)
    }
    fn component(&self, text: &str, language: &str) -> Result<(String, &'static str)> {
        let text = text.trim_matches(crate::source_philosophy_support::source_space);
        if let Some(v) = self.exact(text, language) {
            return Ok((v, "reviewed"));
        }
        if let Some(v) = self.dossier(text, language, None) {
            return Ok((v, "reviewed"));
        }
        if language == "en" && text.chars().any(russian_char) {
            let v = self.draft(text)?;
            return Ok((if v.is_empty() { text.into() } else { v }, "draft"));
        }
        Ok((text.into(), "source"))
    }
    fn translated(
        &self,
        text: &str,
        language: &str,
        context: Option<&str>,
    ) -> Result<(String, &'static str)> {
        if let Some(v) = context.and_then(|id| self.dossier(text, language, Some(id))) {
            return Ok((v, "reviewed"));
        }
        let text = text.trim_matches(crate::source_philosophy_support::source_space);
        if let Some(v) = self.exact(text, language) {
            return Ok((v, "reviewed"));
        }
        if let Some((prefix, tail)) = text.split_once(':') {
            let (p, ps) = self.component(prefix, language)?;
            let (t, ts) = self.component(tail, language)?;
            if p != prefix.trim_matches(crate::source_philosophy_support::source_space)
                || t != tail.trim_matches(crate::source_philosophy_support::source_space)
            {
                return Ok((
                    format!("{p}: {t}"),
                    if ps == "draft" || ts == "draft" {
                        "draft"
                    } else {
                        "reviewed"
                    },
                ));
            }
        }
        if let Some(v) = self.dossier(text, language, None) {
            return Ok((v, "reviewed"));
        }
        if language == "en" && text.chars().any(russian_char) {
            let v = self.draft(text)?;
            return Ok((if v.is_empty() { text.into() } else { v }, "draft"));
        }
        Ok((text.into(), "source"))
    }
    pub fn label(&self, label: &str, source_ref: &str, properties: &Value) -> Result<Value> {
        if label.len() > self.limits.max_label_bytes || source_ref.len() > 4096 {
            return Err(Error::Budget("philosophy label bytes"));
        }
        let field = |names: &[&str]| -> Value {
            names
                .iter()
                .map(|k| &properties[*k])
                .find(|v| truth(v))
                .and_then(Value::as_str)
                .filter(|s| {
                    !s.trim_matches(crate::source_philosophy_support::source_space)
                        .is_empty()
                })
                .map(|s| json!(s))
                .unwrap_or(Value::Null)
        };
        let original = field(&["original_label", "original_title", "attested_original"]);
        let context = if properties["node_type"] == "prepared-dossier" {
            properties["dossier_id"].as_str()
        } else {
            None
        };
        let (ru, rs) = self.translated(label, "ru", context)?;
        let (en, es) = self.translated(label, "en", context)?;
        let os = if matches!(
            properties["node_type"].as_str(),
            Some("domain-root" | "atlas" | "atlas-section" | "view-section")
        ) {
            "not_applicable"
        } else if original.is_null() {
            "pending"
        } else {
            "source"
        };
        Ok(
            json!({"schema_version":"tos_multilingual_label_v1","label":{"original":original,"ru":ru,"en":en},"language":{"original_language":field(&["original_language","language"]),"original_script":field(&["original_script","script"]),"transliteration":field(&["transliteration","title_transliteration"])},"translation_status":{"original":os,"ru":rs,"en":es},"source_ref":source_ref}),
        )
    }
    pub fn content_language_contract(&self) -> Result<Value> {
        let mut out = json!({"schema_version":"tos_multilingual_content_contract_v1","source_ref":LABEL_LEDGER});
        for k in [
            "display_languages",
            "required_translation_languages",
            "original_language_rule",
            "downstream_consumer_rule",
            "text_bearing_node_rule",
        ] {
            out[k] = self
                .ledger
                .get(k)
                .ok_or(Error::Invalid("philosophy language contract"))?
                .clone();
        }
        let p = &self.ledger["planting_contracts"];
        out["language_registry_ref"] = p.get("language_registry_ref").cloned().unwrap_or(json!(
            "ToS/philosophy/atlas/multilingual/language-registry.json"
        ));
        out["text_bearing_nodes_contract_ref"] =
            p.get("text_bearing_nodes_ref").cloned().unwrap_or(json!(
                "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json"
            ));
        Ok(out)
    }
}
fn russian_char(c: char) -> bool {
    matches!(c, 'А'..='я' | 'Ё' | 'ё')
}
fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || russian_char(c) || matches!(c, 'İ' | 'ı' | 'ſ' | 'K')
}
fn valid_dossier_id(s: &str) -> bool {
    dossier_id(s) == Some(s)
}
fn dossier_id(s: &str) -> Option<&str> {
    static GRAMMAR: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let grammar = GRAMMAR.get_or_init(|| {
        Regex::new(r"^(A\d{2}|T[23]-\d{2})(?:[\s\x{1c}-\x{1f}]|$)")
            .expect("maintained dossier grammar")
    });
    let captures = grammar.captures(s)?;
    let identity = captures.get(1)?;
    Some(&s[identity.start()..identity.end()])
}

fn clean_prefix(text: &str) -> (String, bool, Option<&'static str>) {
    let mut text = text
        .trim_matches(crate::source_philosophy_support::source_space)
        .to_owned();
    let docx = text.to_lowercase().ends_with(".docx");
    if docx {
        text.truncate(text.len() - 5);
    }
    let mut prefix = None;
    for p in [
        "ToS Deep Research:",
        "ToS Deep Research_",
        "ToS Deep Research —",
        "Corpus Or Prepared Source Document:",
    ] {
        if text.starts_with(p) {
            prefix = Some(p);
            text = text[p.len()..]
                .trim_matches(crate::source_philosophy_support::source_space)
                .into();
            break;
        }
    }
    (text, docx, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_retains_word_boundaries_and_long_alternation() {
        let multi = Multilingual::from_ledger(
            &json!({"schema_version":"tos_philosophy_multilingual_labels_v1","label_sets":{}}),
        )
        .unwrap();
        assert_eq!(
            multi.draft("ритуальными текстовыми").unwrap(),
            "ritual textual"
        );
        assert_eq!(
            multi.draft("Западная Азия И Южная Азия").unwrap(),
            "West Asia and South Asia"
        );
        assert_eq!(
            multi.draft("xШан Шан/Шан Шанx").unwrap(),
            "xШан Shang/Shang Шанx"
        );
        let material = multi
            .label(
                "Шан",
                "ToS/philosophy/source.json",
                &json!({"original_label":"商","original_language":"zh"}),
            )
            .unwrap();
        assert_eq!(material["label"]["original"], "商");
        assert_eq!(material["translation_status"]["en"], "draft");
        assert_eq!(material["translation_status"]["original"], "source");
    }
}
