//! Read-only census of tracked JSON and JSONL field values; unknowns fail closed.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, jcs_raw_bytes_v1, parse_json};

#[derive(Deserialize)]
struct Contract {
    rules: Vec<Rule>,
}
#[derive(Deserialize)]
struct Rule {
    field: String,
    class: String,
    old_profile: Option<String>,
    file_suffix: Option<String>,
    file_contains: Option<String>,
    pointer_contains: Option<String>,
    producer_context: String,
}
impl Rule {
    fn matches(&self, field: &str, file: &str, pointer: &str) -> bool {
        (self.field == field || self.field == "*")
            && self.file_suffix.as_ref().is_none_or(|v| file.ends_with(v))
            && self.file_contains.as_ref().is_none_or(|v| file.contains(v))
            && self
                .pointer_contains
                .as_ref()
                .is_none_or(|v| pointer.contains(v))
    }
}
fn is_digest(s: &str) -> bool {
    let s = s.strip_prefix("sha256:").unwrap_or(s);
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
type Counts = BTreeMap<(String, String, String, String, String), u64>;
fn walk(value: &Value, file: &str, pointer: &str, rules: &[Rule], counts: &mut Counts) -> bool {
    let mut canonical = false;
    match value {
        Value::Object(map) => {
            for (field, value) in map {
                let escaped = field.replace('~', "~0").replace('/', "~1");
                let child = format!("{pointer}/{escaped}");
                if let Some(s) = value.as_str().filter(|s| is_digest(s)) {
                    let _ = s;
                    let matching = rules
                        .iter()
                        .filter(|r| r.matches(field, file, &child))
                        .collect::<Vec<_>>();
                    let (class, profile) = if matching.len() == 1 {
                        (
                            &*matching[0].class,
                            matching[0].old_profile.as_deref().unwrap_or(""),
                        )
                    } else {
                        ("unclassified", "")
                    };
                    canonical |= class == "canonical-json";
                    let group = file.to_owned();
                    *counts
                        .entry((
                            field.clone(),
                            class.into(),
                            profile.into(),
                            group,
                            child.clone(),
                        ))
                        .or_default() += 1;
                }
                canonical |= walk(value, file, &child, rules, counts);
            }
        }
        Value::Array(a) => {
            for v in a {
                canonical |= walk(v, file, &format!("{pointer}/*"), rules, counts);
            }
        }
        _ => (),
    }
    canonical
}

type NumberCounts = BTreeMap<(String, String, String, String), u64>;
fn numbers(
    value: &JsonValue,
    file: &str,
    pointer: &str,
    counts: &mut NumberCounts,
    cache: &mut BTreeMap<String, (String, String)>,
) {
    match value {
        JsonValue::Number(number) => {
            let lexeme = number.lexeme.clone();
            let (status, result) = cache.entry(lexeme.clone()).or_insert_with(|| {
                let limits = JsonLimits {
                    max_bytes: lexeme.len().saturating_add(512),
                    max_integer_digits: lexeme.len().max(1),
                    ..JsonLimits::default()
                };
                match jcs_raw_bytes_v1(lexeme.as_bytes(), limits) {
                    Ok(bytes) => {
                        let encoded = String::from_utf8(bytes).expect("JCS number is UTF-8");
                        (
                            if encoded == lexeme {
                                "unchanged"
                            } else {
                                "changed"
                            }
                            .into(),
                            encoded,
                        )
                    }
                    Err(e) => ("refused".into(), e.to_string()),
                }
            });
            if status != "unchanged" {
                *counts
                    .entry((
                        file.into(),
                        pointer.into(),
                        lexeme,
                        format!("{status}: {result}"),
                    ))
                    .or_default() += 1;
            }
        }
        JsonValue::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                numbers(child, file, &format!("{pointer}/{i}"), counts, cache);
            }
        }
        JsonValue::Object(items) => {
            for (key, child) in items {
                let key = key.as_str().unwrap_or("<invalid-unicode>");
                numbers(
                    child,
                    file,
                    &format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
                    counts,
                    cache,
                );
            }
        }
        _ => (),
    }
}
fn scan_numbers(
    raw: &str,
    file: &str,
    counts: &mut NumberCounts,
    cache: &mut BTreeMap<String, (String, String)>,
    errors: &mut Vec<String>,
) {
    let size = raw.len().max(1);
    let limits = JsonLimits {
        max_bytes: size,
        max_depth: 128,
        max_visits: size,
        max_integer_digits: size,
    };
    match parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits) {
        Ok(doc) => numbers(doc.root(), file, "", counts, cache),
        Err(e) => errors.push(format!("{file}: {e}")),
    }
}
// serde_json::Value keeps only the last duplicate member. Detect duplicates
// first so the census never silently drops a digest occurrence.
struct NoDuplicates;
impl<'de> Deserialize<'de> for NoDuplicates {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Check;
        impl<'de> serde::de::Visitor<'de> for Check {
            type Value = NoDuplicates;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON without duplicate members")
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_string<E: serde::de::Error>(self, _: String) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<NoDuplicates, E> {
                Ok(NoDuplicates)
            }
            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                d: D,
            ) -> Result<NoDuplicates, D::Error> {
                NoDuplicates::deserialize(d)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<NoDuplicates, A::Error> {
                while a.next_element::<NoDuplicates>()?.is_some() {}
                Ok(NoDuplicates)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<NoDuplicates, A::Error> {
                let mut seen = std::collections::HashSet::new();
                while let Some(key) = a.next_key::<String>()? {
                    if !seen.insert(key) {
                        return Err(serde::de::Error::custom("duplicate JSON member"));
                    }
                    a.next_value::<NoDuplicates>()?;
                }
                Ok(NoDuplicates)
            }
        }
        d.deserialize_any(Check)
    }
}
fn parse_value(s: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str::<NoDuplicates>(s)?;
    serde_json::from_str(s)
}
fn run(root: &Path) -> Result<bool, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let git = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| e.to_string())?;
    if !git.status.success() {
        return Err("root must be inside a Git worktree".into());
    }
    let repo = PathBuf::from(
        String::from_utf8(git.stdout)
            .map_err(|e| e.to_string())?
            .trim_end(),
    );
    let contract: Contract = serde_json::from_slice(
        &std::fs::read(repo.join("ToS/contracts/digest-field-classes.v1.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for r in &contract.rules {
        if !["canonical-json", "raw-bytes", "other", "not-a-digest"].contains(&r.class.as_str())
            || r.producer_context.is_empty()
            || (r.class == "canonical-json" && r.old_profile.as_deref().unwrap_or("").is_empty())
        {
            return Err("invalid classification rule".into());
        }
    }
    let listed = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["ls-files", "-z", "--cached"])
        .output()
        .map_err(|e| e.to_string())?;
    if !listed.status.success() {
        return Err("git ls-files failed".into());
    }
    let mut counts = Counts::new();
    let mut number_counts = NumberCounts::new();
    let mut number_cache = BTreeMap::new();
    let mut number_errors = Vec::new();
    let mut errors = Vec::new();
    let mut files = 0usize;
    for p in listed.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let file = std::str::from_utf8(p).map_err(|e| e.to_string())?;
        if !(file.ends_with(".json") || file.ends_with(".jsonl")) {
            continue;
        }
        let path = repo.join(file);
        if !path.starts_with(&root) {
            continue;
        }
        files += 1;
        let data = match std::fs::read_to_string(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{file}: {e}"));
                continue;
            }
        };
        if file.ends_with(".jsonl") {
            for (line, text) in data.lines().enumerate() {
                if text.trim().is_empty() {
                    continue;
                }
                match parse_value(text) {
                    Ok(v) => {
                        if walk(&v, file, "", &contract.rules, &mut counts) {
                            scan_numbers(
                                text,
                                &format!("{file}:{}", line + 1),
                                &mut number_counts,
                                &mut number_cache,
                                &mut number_errors,
                            );
                        }
                    }
                    Err(e) => errors.push(format!("{file}:{}: {e}", line + 1)),
                }
            }
        } else {
            match parse_value(&data) {
                Ok(v) => {
                    if walk(&v, file, "", &contract.rules, &mut counts) {
                        scan_numbers(
                            &data,
                            file,
                            &mut number_counts,
                            &mut number_cache,
                            &mut number_errors,
                        );
                    }
                }
                Err(e) => errors.push(format!("{file}: {e}")),
            }
        }
    }
    let unknown: u64 = counts
        .iter()
        .filter(|((_, class, _, _, _), _)| class == "unclassified")
        .map(|(_, n)| n)
        .sum();
    let rows: Vec<_> = counts.iter().map(|((field,class,profile,group,pointer),count)| json!({"field":field,"class":class,"old_profile":if profile.is_empty() { None } else { Some(profile) },"file":group,"pointer":pointer,"count":count})).collect();
    let changed_numbers: u64 = number_counts
        .iter()
        .filter(|((_, _, _, outcome), _)| outcome.starts_with("changed:"))
        .map(|(_, n)| n)
        .sum();
    let refused_numbers: u64 = number_counts
        .iter()
        .filter(|((_, _, _, outcome), _)| outcome.starts_with("refused:"))
        .map(|(_, n)| n)
        .sum();
    let numeric_rows: Vec<_> = number_counts.iter().map(|((file,pointer,lexeme,outcome),count)| json!({"file":file,"pointer":pointer,"lexeme":lexeme,"outcome":outcome,"count":count})).collect();
    println!("{}", serde_json::to_string_pretty(&json!({"numbers_scope":"Candidate JSON documents or JSONL lines containing a classified canonical-json digest reference; verify the actual hashed input before migration", "number_scan_errors":number_errors, "changed_numbers":changed_numbers, "refused_numbers":refused_numbers, "numbers":numeric_rows, "schema_version":"tos_digest_inventory_v1","root":root,"scope":"tracked .json and .jsonl files, object string values only","files":files,"unclassified":unknown,"parse_errors":errors,"counts":rows})).map_err(|e| e.to_string())?);
    Ok(unknown == 0 && errors.is_empty() && number_errors.is_empty())
}
fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: tos-digest-inventory ROOT");
        std::process::exit(2)
    };
    if args.next().is_some() {
        eprintln!("usage: tos-digest-inventory ROOT");
        std::process::exit(2);
    }
    match run(Path::new(&root)) {
        Ok(true) => (),
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("inventory refused: {e}");
            std::process::exit(2);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbers_report_spelling_and_refusal() {
        let raw = r#"{"a":1.0,"b":1e0,"c":9007199254740993,"d":-0.0,"e":2,"f":1e20}"#;
        let mut counts = NumberCounts::new();
        let mut errors = Vec::new();
        scan_numbers(
            raw,
            "sample.json",
            &mut counts,
            &mut BTreeMap::new(),
            &mut errors,
        );
        assert!(errors.is_empty());
        let rows: Vec<_> = counts
            .keys()
            .map(|(_, pointer, lexeme, outcome)| {
                (pointer.as_str(), lexeme.as_str(), outcome.as_str())
            })
            .collect();
        assert!(
            rows.iter()
                .any(|(p, l, o)| *p == "/a" && *l == "1.0" && *o == "changed: 1")
        );
        assert!(
            rows.iter()
                .any(|(p, l, o)| *p == "/b" && *l == "1e0" && *o == "changed: 1")
        );
        assert!(
            rows.iter()
                .any(|(p, _, o)| *p == "/c" && o.starts_with("refused:"))
        );
        assert!(
            rows.iter()
                .any(|(p, _, o)| *p == "/d" && o.starts_with("refused:"))
        );
        assert!(!rows.iter().any(|(p, _, _)| *p == "/e"));
        assert!(
            rows.iter()
                .any(|(p, _, o)| *p == "/f" && *o == "changed: 100000000000000000000")
        );
    }
    #[test]
    fn matches_context_and_fails_closed() {
        let c: Contract = serde_json::from_str(include_str!(
            "../../../../../ToS/contracts/digest-field-classes.v1.json"
        ))
        .unwrap();
        let mut counts = Counts::new();
        let h = "a".repeat(64);
        walk(
            &json!({"exact_sha256":h,"unknown_sha256":h}),
            "ToS/a/source-text-unit.json",
            "",
            &c.rules,
            &mut counts,
        );
        assert_eq!(
            counts.get(&(
                "exact_sha256".into(),
                "raw-bytes".into(),
                "".into(),
                "ToS/a/source-text-unit.json".into(),
                "/exact_sha256".into()
            )),
            Some(&1)
        );
        assert_eq!(
            counts.get(&(
                "unknown_sha256".into(),
                "unclassified".into(),
                "".into(),
                "ToS/a/source-text-unit.json".into(),
                "/unknown_sha256".into()
            )),
            Some(&1)
        );
        walk(
            &json!({"growth_history":[{"source_contracts":{"ToS/x":h}}]}),
            "ToS/a/source.human-forms.json",
            "",
            &c.rules,
            &mut counts,
        );
        assert_eq!(
            counts.get(&(
                "ToS/x".into(),
                "raw-bytes".into(),
                "".into(),
                "ToS/a/source.human-forms.json".into(),
                "/growth_history/*/source_contracts/ToS~1x".into()
            )),
            Some(&1)
        );
        assert!(parse_value("{\"a\":1,\"a\":2}").is_err());
        assert!(!is_digest(&"g".repeat(64)));
        assert!(is_digest(&format!("sha256:{h}")));
    }
}
