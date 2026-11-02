//! Read-only census of tracked JSON and JSONL field values; unknowns fail closed.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::{Path, PathBuf}, process::Command};

#[derive(Deserialize)]
struct Contract { rules: Vec<Rule> }
#[derive(Deserialize)]
struct Rule { field: String, class: String, old_profile: Option<String>, file_suffix: Option<String>, file_contains: Option<String>, pointer_contains: Option<String>, producer_context: String }
impl Rule {
    fn matches(&self, field: &str, file: &str, pointer: &str) -> bool {
        self.field == field && self.file_suffix.as_ref().is_none_or(|v| file.ends_with(v)) && self.file_contains.as_ref().is_none_or(|v| file.contains(v)) && self.pointer_contains.as_ref().is_none_or(|v| pointer.contains(v))
    }
}
fn is_digest(s: &str) -> bool {
    let s = s.strip_prefix("sha256:").unwrap_or(s);
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
type Counts = BTreeMap<(String,String,String,String), u64>;
fn walk(value: &Value, file: &str, pointer: &str, rules: &[Rule], counts: &mut Counts) {
    match value {
        Value::Object(map) => for (field, value) in map {
            let escaped = field.replace('~', "~0").replace('/', "~1");
            let child = format!("{pointer}/{escaped}");
            if let Some(s) = value.as_str().filter(|s| is_digest(s)) {
                let _ = s;
                let matching = rules.iter().filter(|r| r.matches(field, file, &child)).collect::<Vec<_>>();
                let (class, profile) = if matching.len() == 1 { (&*matching[0].class, matching[0].old_profile.as_deref().unwrap_or("")) } else { ("unclassified", "") };
                let group = file.split('/').take(2).collect::<Vec<_>>().join("/");
                *counts.entry((field.clone(), class.into(), profile.into(), group)).or_default() += 1;
            }
            walk(value, file, &child, rules, counts);
        },
        Value::Array(a) => for v in a { walk(v, file, &format!("{pointer}/*"), rules, counts); },
        _ => ()
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
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("JSON without duplicate members") }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_string<E: serde::de::Error>(self, _: String) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_unit<E: serde::de::Error>(self) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_none<E: serde::de::Error>(self) -> Result<NoDuplicates,E> { Ok(NoDuplicates) }
            fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<NoDuplicates,D::Error> { NoDuplicates::deserialize(d) }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<NoDuplicates,A::Error> { while a.next_element::<NoDuplicates>()?.is_some() {} Ok(NoDuplicates) }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<NoDuplicates,A::Error> {
                let mut seen = std::collections::HashSet::new();
                while let Some(key) = a.next_key::<String>()? {
                    if !seen.insert(key) { return Err(serde::de::Error::custom("duplicate JSON member")); }
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
    let git = Command::new("git").arg("-C").arg(&root).args(["rev-parse", "--show-toplevel"]).output().map_err(|e| e.to_string())?;
    if !git.status.success() { return Err("root must be inside a Git worktree".into()); }
    let repo = PathBuf::from(String::from_utf8(git.stdout).map_err(|e| e.to_string())?.trim_end());
    let contract: Contract = serde_json::from_slice(&std::fs::read(repo.join("ToS/contracts/digest-field-classes.v1.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    for r in &contract.rules {
        if !["canonical-json", "raw-bytes", "other"].contains(&r.class.as_str()) || r.producer_context.is_empty() || (r.class == "canonical-json" && r.old_profile.as_deref().unwrap_or("").is_empty()) { return Err("invalid classification rule".into()); }
    }
    let listed = Command::new("git").arg("-C").arg(&repo).args(["ls-files", "-z", "--cached"]).output().map_err(|e| e.to_string())?;
    if !listed.status.success() { return Err("git ls-files failed".into()); }
    let mut counts = Counts::new(); let mut errors = Vec::new(); let mut files = 0usize;
    for p in listed.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let file = std::str::from_utf8(p).map_err(|e| e.to_string())?;
        if !(file.ends_with(".json") || file.ends_with(".jsonl")) { continue; }
        let path = repo.join(file); if !path.starts_with(&root) { continue; }
        files += 1;
        let data = match std::fs::read_to_string(&path) { Ok(v) => v, Err(e) => { errors.push(format!("{file}: {e}")); continue; } };
        if file.ends_with(".jsonl") {
            for (line, text) in data.lines().enumerate() { if text.trim().is_empty() { continue; } match parse_value(text) { Ok(v) => walk(&v,file,"",&contract.rules,&mut counts), Err(e) => errors.push(format!("{file}:{}: {e}",line+1)) } }
        } else { match parse_value(&data) { Ok(v) => walk(&v,file,"",&contract.rules,&mut counts), Err(e) => errors.push(format!("{file}: {e}")) } }
    }
    let unknown: u64 = counts.iter().filter(|((_,class,_,_),_)| class == "unclassified").map(|(_,n)| n).sum();
    let rows: Vec<_> = counts.iter().map(|((field,class,profile,group),count)| json!({"field":field,"class":class,"old_profile":if profile.is_empty() { None } else { Some(profile) },"file_group":group,"count":count})).collect();
    println!("{}", serde_json::to_string_pretty(&json!({"schema_version":"tos_digest_inventory_v1","root":root,"scope":"tracked .json and .jsonl files, object string values only","files":files,"unclassified":unknown,"parse_errors":errors,"counts":rows})).map_err(|e| e.to_string())?);
    Ok(unknown == 0 && errors.is_empty())
}
fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(root) = args.next() else { eprintln!("usage: tos-digest-inventory ROOT"); std::process::exit(2) };
    if args.next().is_some() { eprintln!("usage: tos-digest-inventory ROOT"); std::process::exit(2); }
    match run(Path::new(&root)) { Ok(true) => (), Ok(false) => std::process::exit(1), Err(e) => { eprintln!("inventory refused: {e}"); std::process::exit(2); } }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn matches_context_and_fails_closed() {
        let c: Contract = serde_json::from_str(include_str!("../../../../../ToS/contracts/digest-field-classes.v1.json")).unwrap();
        let mut counts=Counts::new(); let h="a".repeat(64);
        walk(&json!({"exact_sha256":h,"unknown_sha256":h}),"ToS/a/source-text-unit.json","",&c.rules,&mut counts);
        assert_eq!(counts.get(&("exact_sha256".into(),"raw-bytes".into(),"".into(),"ToS/a".into())),Some(&1));
        assert_eq!(counts.get(&("unknown_sha256".into(),"unclassified".into(),"".into(),"ToS/a".into())),Some(&1));
        assert!(parse_value("{\"a\":1,\"a\":2}").is_err()); assert!(!is_digest(&"g".repeat(64))); assert!(is_digest(&format!("sha256:{h}")));
    }
}
