//! Read-only candidate for the maintained active-naming default validator.
//! Authored route law and exact content exceptions remain source-owned.
use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, Instant};
use tos_foundation::python_lower_unicode16_v1;
use unicode_general_category::{GeneralCategory, get_general_category};

const MAX_ENTRIES: usize = 10_000;
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_ISSUES: usize = 4096;
const MAX_ISSUE_BYTES: usize = 8192;
const SPACE: &str = r"\x09-\x0D\x1C-\x20\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";
const ALLOWED: &[&str] = &[
    "first-wave",
    "first-wave-resident",
    "one-seeder",
    "may_seed_drafts",
    "may_seed_gold",
    "seed_claim_ref",
    "seed.focus_node_id",
    "seed.node_ids",
    "seed.text_query",
    "seed_field",
];
const LABELS: &[&str] = &[
    "deployment-watchtower",
    "federation-harvest",
    "adoption-forge",
    "constitution-runtime",
];
const QUOTES: [(&str, &str); 2] = [
    (
        "ToS Deep Research_ A48 — Океания _ khipu _ rongorongo as frontier seed.docx",
        "[quoted-external-artifact-identity]",
    ),
    (
        "Bentham включён как заданный master-seed и как пороговая фигура: его ранние тексты до 1820 года учитываются только как генеалогический вход, тогда как ядро документа остаётся в пределах 1820–1900.",
        "[quoted-capture-provenance-fragment]",
    ),
];
const TOKENS: &[&str] = &[
    "wave",
    "waves",
    "seed",
    "seeds",
    "seeded",
    "seed-pack",
    "seed_pack",
];
const ASCII_WORD: &str = "A-Za-z0-9İıſK";

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn decimal(ch: char) -> bool {
    get_general_category(ch) == GeneralCategory::DecimalNumber
}
fn ascii_fold(ch: char) -> char {
    match ch {
        'İ' | 'ı' => 'i',
        'ſ' => 's',
        'K' => 'k',
        _ => ch.to_ascii_lowercase(),
    }
}
fn run_char(ch: char) -> bool {
    ascii_fold(ch).is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '/' | '-')
}
fn ascii_word(ch: char) -> bool {
    ascii_fold(ch).is_ascii_alphanumeric()
}
fn literal(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'i' => "[iIİı]".into(),
            's' => "[sSſ]".into(),
            'k' => "[kKK]".into(),
            c if c.is_ascii_lowercase() => format!("[{c}{}]", c.to_ascii_uppercase()),
            c => regex::escape(&c.to_string()),
        })
        .collect()
}

struct Patterns {
    path: Regex,
    route: Regex,
    experience: Regex,
    version: Regex,
    token: Regex,
    candidate: Regex,
    separators: Regex,
}
impl Patterns {
    fn new() -> io::Result<Self> {
        // Pin Python \d to the already-selected Unicode16 category primitive,
        // rather than silently adopting the regex engine's Unicode version.
        // This one bounded 1,114,112-point pass is done once per operation.
        let mut digits = String::from("[");
        let mut start = None;
        let mut previous = 0;
        for point in 0..=0x10ffff {
            if char::from_u32(point).is_some_and(decimal) {
                if start.is_none() {
                    start = Some(point);
                }
                previous = point;
            } else if let Some(first) = start.take() {
                digits.push_str(&format!(r"\x{{{first:x}}}-\x{{{previous:x}}}"));
            }
        }
        if let Some(first) = start {
            digits.push_str(&format!(r"\x{{{first:x}}}-\x{{{previous:x}}}"));
        }
        digits.push(']');
        let pattern = |s: &str| Regex::new(s).map_err(|e| invalid(e.to_string()));
        let tokens = TOKENS
            .iter()
            .map(|t| literal(t))
            .collect::<Vec<_>>()
            .join("|");
        let before = format!("(?:^|[^{ASCII_WORD}])");
        let after = format!("(?:$|[^{ASCII_WORD}])");
        let candidates = TOKENS
            .iter()
            .copied()
            .chain([
                "zv",
                "experience",
                "deployment",
                "federation",
                "adoption",
                "constitution",
            ])
            .map(literal)
            .collect::<Vec<_>>()
            .join("|");
        Ok(Self {
            path: pattern(&format!("{before}({tokens}){after}"))?,
            route: pattern(&format!(
                "{before}({}{digits}+(?:[-_][{ASCII_WORD}]+)+)",
                literal("zv")
            ))?,
            experience: pattern(&format!("{before}({})", literal("experience.v0.")))?,
            version: pattern(&format!(
                "{before}({}[6-9](?:\\.{digits}+)?){after}",
                literal("v0.")
            ))?,
            token: pattern(&tokens)?,
            candidate: pattern(&candidates)?,
            separators: pattern(&format!("[_{SPACE}]+"))?,
        })
    }
    fn normalized_label(&self, text: &str) -> io::Result<Option<String>> {
        let normalized = self.separators.replace_all(text, "-");
        let lower = python_lower_unicode16_v1(
            &normalized,
            normalized.len(),
            normalized.len() * 3,
            normalized.len() * 3,
        )
        .map_err(|e| invalid(e.to_string()))?;
        Ok(LABELS
            .iter()
            .find(|label| lower.contains(**label))
            .map(|s| (*s).to_owned()))
    }
    fn path_issue(&self, text: &str) -> io::Result<Option<String>> {
        for pattern in [&self.path, &self.route] {
            if let Some(c) = pattern.captures(text) {
                return Ok(Some(c[1].to_owned()));
            }
        }
        self.normalized_label(text)
    }
    fn active_reference(&self, text: &str) -> io::Result<Option<String>> {
        let mut cursor = 0;
        // The maintained optimized route searches tokens first, then inspects
        // only their maximal path-like run once. Do not regex every word.
        while let Some(token) = self.token.find_at(text, cursor) {
            let mut begin = token.start();
            while let Some(ch) = text[..begin].chars().next_back().filter(|c| run_char(*c)) {
                begin -= ch.len_utf8();
            }
            let mut end = token.end();
            for ch in text[end..].chars() {
                if !run_char(ch) {
                    break;
                }
                end += ch.len_utf8();
            }
            cursor = end;
            let run = &text[begin..end];
            let marker = run.char_indices().any(|(at, c)| {
                matches!(c, '-' | '_' | '/')
                    || decimal(c)
                    || (c == '.' && run[at + 1..].chars().next().is_some_and(ascii_word))
            }) || text[end..].chars().next().is_some_and(decimal);
            if !marker {
                continue;
            }
            // Python lower() is distinct from regex IGNORECASE: notably İ/ſ.
            let lower = python_lower_unicode16_v1(run, run.len(), run.len() * 3, run.len() * 3)
                .map_err(|e| invalid(e.to_string()))?;
            if !ALLOWED.contains(&lower.as_str()) {
                return Ok(Some(run.to_owned()));
            }
        }
        Ok(None)
    }
    fn content_issue(&self, text: &str) -> io::Result<Option<String>> {
        if !self.candidate.is_match(text) {
            return Ok(None);
        }
        let mut visible = Cow::Borrowed(text);
        for (quote, placeholder) in QUOTES {
            if visible.contains(quote) {
                visible = Cow::Owned(visible.replace(quote, placeholder));
            }
        }
        if let Some(reference) = self.active_reference(&visible)? {
            return Ok(Some(reference));
        }
        for pattern in [&self.route, &self.experience] {
            if let Some(c) = pattern.captures(&visible) {
                return Ok(Some(c[1].to_owned()));
            }
        }
        self.normalized_label(&visible)
    }
    fn experience_pass(&self, text: &str) -> Option<String> {
        self.version.captures(text).map(|c| c[1].to_owned())
    }
}

fn excluded(rel: &str) -> bool {
    if rel.split('/').any(|part| {
        matches!(
            part,
            ".git" | ".agents" | ".pytest_cache" | "__pycache__" | "legacy" | "node_modules"
        )
    }) {
        return true;
    }
    if matches!(
        rel,
        "CHANGELOG.md"
            | "docs/validation/documentation-family.current.json"
            | "kag/indexes/index_family.manifest.json"
            | "access/web/package-lock.json"
            | "scripts/validate_active_naming.py"
            | "kag/indexes/source_surface_index.json"
            | "kag/indexes/repo_artifact_index.json"
            | "kag/indexes/repo_anchor_index.json"
            | "kag/indexes/repo_entity_index.json"
            | "kag/indexes/repo_event_index.json"
            | "kag/indexes/repo_assertion_index.json"
            | "kag/indexes/repo_relation_index.json"
    ) {
        return true;
    }
    [
        "kag/indexes/segments",
        "kag/indexes/shards",
        "kag/receipts/index_family_budget",
    ]
    .iter()
    .any(|prefix| {
        rel == *prefix
            || rel
                .strip_prefix(prefix)
                .is_some_and(|tail| tail.starts_with('/'))
    })
}
fn text_suffix(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.rsplit_once('.')
        .filter(|(stem, tail)| stem.chars().any(|c| c != '.') && !tail.is_empty())
        .is_some_and(|(_, suffix)| {
            matches!(
                suffix,
                "csv" | "json" | "md" | "py" | "txt" | "yaml" | "yml"
            )
        })
}

fn scalar(value: &Value, out: &mut Vec<String>, bytes: &mut usize) -> io::Result<()> {
    let fragment = match value {
        Value::String(s) => Some(s.clone()),
        Value::Bool(v) => Some(v.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(items) => {
            for item in items {
                scalar(item, out, bytes)?;
            }
            None
        }
        _ => None,
    };
    if let Some(fragment) = fragment {
        *bytes = bytes
            .checked_add(fragment.len() + 1)
            .ok_or_else(|| invalid("active topology text overflow"))?;
        if *bytes > MAX_FILE_BYTES {
            return Err(invalid("active topology text budget exceeded"));
        }
        out.push(fragment);
    }
    Ok(())
}
fn topology_text(text: &str) -> io::Result<Cow<'_, str>> {
    let payload: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(e) if e.to_string().contains("recursion limit") => {
            return Err(invalid("active topology JSON depth limit"));
        }
        Err(_) => return Ok(Cow::Borrowed(text)),
    };
    let Some(object) = payload.as_object() else {
        return Ok(Cow::Borrowed(text));
    };
    let mut fragments = Vec::new();
    let mut bytes = 0;
    for key in ["schema_version", "owner_repo", "root", "legacy_policy"] {
        if let Some(v) = object.get(key) {
            scalar(v, &mut fragments, &mut bytes)?;
        }
    }
    if let Some(packages) = object.get("packages").and_then(Value::as_array) {
        for package in packages {
            if let Some(package) = package.as_object() {
                for key in ["slug", "class", "status", "active_parts", "legacy_required"] {
                    if let Some(v) = package.get(key) {
                        scalar(v, &mut fragments, &mut bytes)?;
                    }
                }
            }
        }
    }
    if let Some(targets) = object.get("moved_path_targets").and_then(Value::as_object) {
        for v in targets.values() {
            scalar(v, &mut fragments, &mut bytes)?;
        }
    }
    Ok(Cow::Owned(fragments.join("\n")))
}

struct Scan {
    deadline: Instant,
    entries: usize,
    bytes: usize,
    issues: Vec<String>,
}
impl Scan {
    fn current(&self) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            Err(invalid("active naming wall deadline"))
        } else {
            Ok(())
        }
    }
    fn issue(&mut self, rel: &str, kind: &str, value: &str) -> io::Result<()> {
        self.current()?;
        if self.issues.len() >= MAX_ISSUES
            || rel.len() + kind.len() + value.len() + 4 > MAX_ISSUE_BYTES
        {
            return Err(invalid("active naming diagnostic budget exceeded"));
        }
        self.issues.push(format!("{rel}: {kind}: {value}"));
        Ok(())
    }
    fn path(&mut self, rel: &str, patterns: &Patterns) -> io::Result<()> {
        if let Some(value) = patterns.path_issue(rel)? {
            self.issue(rel, "retired active name in path", &value)?;
        }
        if rel.starts_with("mechanics/experience/") {
            if let Some(value) = patterns.experience_pass(rel) {
                self.issue(rel, "retired experience pass marker in path", &value)?;
            }
        }
        Ok(())
    }
    fn read(&mut self, path: &Path) -> io::Result<Option<String>> {
        let remaining = MAX_INPUT_BYTES - self.bytes;
        let cap = MAX_FILE_BYTES.min(remaining);
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(path)?;
        let meta = file.metadata()?;
        if !meta.is_file() || meta.len() > cap as u64 {
            return Err(invalid("active naming text file budget exceeded"));
        }
        let mut bytes = Vec::with_capacity(meta.len() as usize);
        file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > cap || bytes.len() as u64 != meta.len() {
            return Err(invalid("active naming text changed or exceeded its budget"));
        }
        self.bytes += bytes.len();
        // Python's Path.read_text universal-newline transform precedes checks.
        match String::from_utf8(bytes) {
            Ok(s) if s.contains('\r') => Ok(Some(s.replace("\r\n", "\n").replace('\r', "\n"))),
            Ok(s) => Ok(Some(s)),
            Err(_) => Ok(None),
        }
    }
    fn directory(
        &mut self,
        root: &Path,
        home: &Path,
        patterns: &Patterns,
        depth: usize,
    ) -> io::Result<()> {
        self.current()?;
        if depth > 128 {
            return Err(invalid("active naming directory depth limit"));
        }
        let mut directories = Vec::new();
        let mut files = Vec::new();
        for entry in fs::read_dir(home)? {
            self.current()?;
            self.entries += 1;
            if self.entries > MAX_ENTRIES {
                return Err(invalid("active naming entry budget exceeded"));
            }
            let entry = entry?;
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .map_err(|e| invalid(e.to_string()))?
                .to_str()
                .ok_or_else(|| invalid("non-UTF8 active naming path"))?
                .to_owned();
            if excluded(&rel) {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(invalid("active naming does not follow symlinks"));
            }
            if kind.is_dir() {
                directories.push((rel, path));
            } else {
                files.push((rel, path, kind.is_file()));
            }
        }
        directories.sort_by(|a, b| a.0.cmp(&b.0));
        files.sort_by(|a, b| a.0.cmp(&b.0));
        // Same os.walk ordering: sorted immediate directories, then files,
        // then recurse each selected directory in sorted order.
        for (rel, _) in &directories {
            self.path(rel, patterns)?;
        }
        for (rel, path, regular) in files {
            self.path(&rel, patterns)?;
            if !regular || !text_suffix(&path) {
                continue;
            }
            let Some(text) = self.read(&path)? else {
                continue;
            };
            self.current()?;
            let active = if rel == "mechanics/topology.json" {
                topology_text(&text)?
            } else {
                Cow::Borrowed(text.as_str())
            };
            if let Some(value) = patterns.content_issue(&active)? {
                self.issue(&rel, "retired active path/id reference in content", &value)?;
            }
            if rel.starts_with("mechanics/experience/") {
                if let Some(value) = patterns.experience_pass(&active) {
                    self.issue(&rel, "retired experience pass marker in content", &value)?;
                }
            }
        }
        for (_, path) in directories {
            self.directory(root, &path, patterns, depth + 1)?;
        }
        Ok(())
    }
}
/// Native default read-only consumer only. Optional Python external-cache
/// feedback is deliberately retained under its existing owner; no cache write.
pub fn validate(root: &Path) -> io::Result<Vec<String>> {
    if !root.is_absolute() || fs::canonicalize(root)? != root {
        return Err(invalid(
            "active naming root must be absolute without symlinks",
        ));
    }
    let mut scan = Scan {
        deadline: Instant::now() + Duration::from_secs(300),
        entries: 0,
        bytes: 0,
        issues: Vec::new(),
    };
    let patterns = Patterns::new()?;
    scan.directory(root, root, &patterns, 0)?;
    scan.current()?;
    Ok(scan.issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_naming_keeps_linear_runs_exact_content_exceptions_and_path_law() {
        let p = Patterns::new().unwrap();
        for reference in ALLOWED {
            assert!(p.content_issue(reference).unwrap().is_none(), "{reference}");
            assert_eq!(
                p.path_issue(&format!("ToS/{reference}/item.json"))
                    .unwrap()
                    .is_some(),
                *reference != "one-seeder",
            );
            assert!(
                p.content_issue(&format!("{reference}_retired"))
                    .unwrap()
                    .is_some()
            );
        }
        for (quote, _) in QUOTES {
            assert!(p.content_issue(quote).unwrap().is_none());
            assert!(p.path_issue(quote).unwrap().is_some());
            assert!(
                p.content_issue(&quote[..quote.len() - 1])
                    .unwrap()
                    .is_some()
            );
        }
        for (text, expected) in [
            ("seed.", None),
            ("seed.v1", Some("seed.v1")),
            ("seed.Ω", None),
            ("seed١", Some("seed")),
            ("seed١-wave", Some("seed")),
            ("ſeed-pack", Some("ſeed-pack")),
            ("wıve-pack", None),
            ("seed_claim_ref", None),
            ("first-wave-resident", None),
            ("İıK/ſEED-pack", Some("İıK/ſEED-pack")),
            ("İ Ω seed-pack", Some("seed-pack")),
            ("ſEED_CLAIM_REF wave-pack", Some("ſEED_CLAIM_REF")),
            ("seed. wave-pack", Some("wave-pack")),
            ("seed_claim_ref/wave-pack", Some("seed_claim_ref/wave-pack")),
            ("seed_claim_ref wave١", Some("wave")),
        ] {
            assert_eq!(
                p.active_reference(text).unwrap().as_deref(),
                expected,
                "{text}"
            );
        }
        for text in [
            "zv1-old-route",
            "experience.v0.7.adoption_forge",
            "deployment-watchtower",
            "federation harvest",
            "Adoption Forge",
            "CONSTITUTION_RUNTIME",
        ] {
            assert!(p.content_issue(text).unwrap().is_some(), "{text}");
        }
        assert!(p.content_issue("Tree-of-Sophia v0.7").unwrap().is_none());
        assert_eq!(p.experience_pass("v0.8.0").as_deref(), Some("v0.8.0"));
        assert!(
            p.path_issue("mechanics/experience/parts/adoption-boundary/README.md")
                .unwrap()
                .is_none()
        );
        let long = format!(
            "{}seed_claim_ref {}wave-pack",
            "x-".repeat(4096),
            "x_".repeat(4096)
        );
        assert!(
            p.active_reference(&long)
                .unwrap()
                .unwrap()
                .ends_with("seed_claim_ref")
        );
        for rel in [
            "legacy/wave/entry.md",
            "kag/indexes/segments/source/00.jsonl",
            "kag/indexes/repo_relation_index.json",
            "access/web/package-lock.json",
        ] {
            assert!(excluded(rel));
        }
        assert!(!excluded("access/web/package.json"));
        assert!(!text_suffix(Path::new("...md")));
        assert!(text_suffix(Path::new(".name.md")));
    }
}
