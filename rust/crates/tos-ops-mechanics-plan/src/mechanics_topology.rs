//! Read-only owner route, topology and documentation checks for mechanics/.
//! This is the existing mechanics_topology lane's native consumer; the Python
//! source remains an independent oracle until the whole route is accepted.

use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::python_casefold_unicode16_v1;
use unicode_general_category::{GeneralCategory, get_general_category};

pub type Issue = (String, String);

const TOPOLOGY: &str = "mechanics/topology.json";
const SCRIPT_INVENTORY: &str = "docs/validation/script_inventory.json";
const TEST_INVENTORY: &str = "tests/test_inventory.json";
const MAX_FILES: usize = 10_000;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ISSUES: usize = 4_096;
const MAX_ISSUE_BYTES: usize = 8 * 1_024;
const MAX_REFERENCES: usize = 100_000;
const MAX_REFERENCE_BYTES: usize = 64 * 1_024 * 1_024;
const MAX_ANCHOR_BYTES: usize = 64 * 1_024 * 1_024;
// Python 3.14 re `\s`, including U+001C..U+001F absent from Unicode
// White_Space. Keep this aligned with FND's pinned Python Unicode 16 law.
const PY_SPACE_REGEX: &str = r"\x09-\x0D\x1C-\x20\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";

// These are the fixed patterns of the maintained Python validator. Compile
// once for the complete operation, not once per document or heading.
struct Patterns {
    comments: Regex,
    inline: Regex,
    definition: Regex,
    heading_explicit: Regex,
    tag: Regex,
    markdown_link: Regex,
    attribute: Regex,
    anchor_explicit: Regex,
    heading: Regex,
    use_ref: Regex,
    script_ref: Regex,
}

impl Patterns {
    fn new() -> io::Result<Self> {
        fn pattern(text: &str) -> io::Result<Regex> {
            Regex::new(&text.replace("@W@", PY_SPACE_REGEX))
                .map_err(|error| invalid(error.to_string()))
        }
        Ok(Self {
            comments: pattern(r"(?s)<!--.*?-->")?,
            inline: pattern(r"\[([^\]]+)\]\(([^)@W@]+)")?,
            definition: pattern(r"(?m)^[ \t]{0,3}\[([^\]]+)\]:[@W@]*(?:<([^>\n]+)>|([^@W@]+))")?,
            heading_explicit: pattern(r"\{#([^}]+)\}")?,
            tag: pattern(r"<[^>]+>")?,
            markdown_link: pattern(r"\[([^\]]+)\]\([^)]+\)")?,
            attribute: pattern(r#"(?:id|name)[@W@]*=[@W@]*["']([^"']+)["']"#)?,
            anchor_explicit: pattern(r"\{#([A-Za-z0-9][A-Za-z0-9_-]*)\}")?,
            heading: pattern(r"(?m)^[ \t]{0,3}#{1,6}[@W@]+(.+?)[@W@]*#*[@W@]*$")?,
            use_ref: pattern(r"\[([^\]]+)\]\[([^\]]*)\]")?,
            script_ref: pattern(r"((?:\.\./)*(?:scripts|mechanics)/[A-Za-z0-9_./-]+\.(?:py|sh))")?,
        })
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

struct Source<'a> {
    root: &'a Path,
    deadline: Instant,
    cancel: &'a AtomicI32,
    files: usize,
    bytes: u64,
    cache: BTreeMap<String, Arc<str>>,
    references: usize,
    reference_bytes: usize,
    anchors: BTreeMap<PathBuf, BTreeSet<String>>,
    anchor_bytes: usize,
}

impl<'a> Source<'a> {
    fn new(root: &'a Path, deadline: Instant, cancel: &'a AtomicI32) -> io::Result<Self> {
        if !root.is_absolute() || fs::canonicalize(root)? != root {
            return Err(invalid("mechanics root must be an absolute canonical path"));
        }
        Ok(Self {
            root,
            deadline,
            cancel,
            files: 0,
            bytes: 0,
            cache: BTreeMap::new(),
            references: 0,
            reference_bytes: 0,
            anchors: BTreeMap::new(),
            anchor_bytes: 0,
        })
    }

    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::SeqCst) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "mechanics validation cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(invalid("mechanics operation deadline exceeded"));
        }
        Ok(())
    }

    fn path(&self, relative: &str) -> io::Result<PathBuf> {
        self.check()?;
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(invalid("mechanics input path escapes root"));
        }
        Ok(self.root.join(path))
    }

    fn read(&mut self, relative: &str) -> io::Result<Option<Arc<str>>> {
        self.check()?;
        if let Some(text) = self.cache.get(relative) {
            return Ok(Some(text.clone()));
        }
        let path = self.path(relative)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(invalid(format!("non-regular mechanics input: {relative}")));
        }
        self.files = self
            .files
            .checked_add(1)
            .ok_or_else(|| invalid("mechanics file count overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(metadata.len())
            .ok_or_else(|| invalid("mechanics byte count overflow"))?;
        if self.files > MAX_FILES || metadata.len() > MAX_FILE_BYTES || self.bytes > MAX_TOTAL_BYTES
        {
            return Err(invalid("mechanics topology input budget exceeded"));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        fs::File::open(path)?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(invalid(format!(
                "mechanics input grew beyond file budget: {relative}"
            )));
        }
        if bytes.len() as u64 != metadata.len() {
            return Err(invalid(format!(
                "mechanics input changed during read: {relative}"
            )));
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| invalid(format!("non-UTF-8 mechanics input: {relative}")))?;
        let text: Arc<str> = text.into();
        self.cache.insert(relative.to_owned(), Arc::clone(&text));
        Ok(Some(text))
    }

    fn charge_references(&mut self, count: usize, bytes: usize) -> io::Result<()> {
        self.references = self
            .references
            .checked_add(count)
            .ok_or_else(|| invalid("mechanics reference count overflow"))?;
        if self.references > MAX_REFERENCES {
            return Err(invalid("mechanics reference budget exceeded"));
        }
        self.reference_bytes = self
            .reference_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("mechanics reference byte overflow"))?;
        if self.reference_bytes > MAX_REFERENCE_BYTES {
            return Err(invalid("mechanics reference byte budget exceeded"));
        }
        Ok(())
    }

    fn remaining_references(&self) -> (usize, usize) {
        (
            MAX_REFERENCES - self.references,
            MAX_REFERENCE_BYTES - self.reference_bytes,
        )
    }
}

fn push(
    issues: &mut Vec<Issue>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> io::Result<()> {
    let path = path.into();
    let message = message.into();
    if issues.len() >= MAX_ISSUES {
        return Err(invalid("mechanics issue budget exceeded"));
    }
    if path.len() + message.len() > MAX_ISSUE_BYTES {
        return Err(invalid("mechanics issue byte budget exceeded"));
    }
    issues.push((path, message));
    Ok(())
}

fn required(source: &Source<'_>, issues: &mut Vec<Issue>, relative: &str) -> io::Result<()> {
    if !source.path(relative)?.is_file() {
        push(issues, relative, "missing required file")?;
    }
    Ok(())
}

fn absent(
    source: &Source<'_>,
    issues: &mut Vec<Issue>,
    relative: &str,
    message: &str,
) -> io::Result<()> {
    if source.path(relative)?.exists() {
        push(issues, relative, message)?;
    }
    Ok(())
}

fn directory_entries(path: &Path, remaining: usize) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        if entries.len() >= remaining {
            return Err(invalid("mechanics traversal entry budget exceeded"));
        }
        entries.push(entry?);
    }
    Ok(entries)
}

fn strings(value: Option<&Value>) -> Option<Vec<&str>> {
    let values = value?.as_array()?;
    values.iter().map(Value::as_str).collect()
}

fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn validate_package(
    source: &Source<'_>,
    issues: &mut Vec<Issue>,
    slug_name: &str,
    entry: &Value,
) -> io::Result<Vec<String>> {
    if !slug(slug_name) {
        push(
            issues,
            TOPOLOGY,
            format!("package slug is not normalized: {slug_name}"),
        )?;
    }
    for filename in [
        "AGENTS.md",
        "README.md",
        "PARTS.md",
        "PROVENANCE.md",
        "ROADMAP.md",
    ] {
        required(source, issues, &format!("mechanics/{slug_name}/{filename}"))?;
    }
    if !matches!(
        entry.get("class").and_then(Value::as_str),
        Some("head-fed/local" | "local")
    ) {
        push(
            issues,
            TOPOLOGY,
            format!("{slug_name}.class must be one of ['head-fed/local', 'local']"),
        )?;
    }
    if !matches!(
        entry.get("status").and_then(Value::as_str),
        Some("active" | "planted")
    ) {
        push(
            issues,
            TOPOLOGY,
            format!("{slug_name}.status must be one of ['active', 'planted']"),
        )?;
    }
    let parts = strings(entry.get("active_parts"));
    let parts = match parts {
        Some(parts) if !parts.is_empty() && parts.iter().all(|part| slug(part)) => parts,
        _ => {
            push(
                issues,
                TOPOLOGY,
                format!("{slug_name}.active_parts must be a non-empty normalized string list"),
            )?;
            Vec::new()
        }
    };
    let unique: BTreeSet<_> = parts.iter().copied().collect();
    if unique.len() != parts.len() {
        push(
            issues,
            TOPOLOGY,
            format!("{slug_name}.active_parts contains duplicates"),
        )?;
    }
    for part in &parts {
        required(
            source,
            issues,
            &format!("mechanics/{slug_name}/parts/{part}/README.md"),
        )?;
    }
    if entry.get("legacy_required") != Some(&Value::Bool(false)) {
        push(
            issues,
            TOPOLOGY,
            format!(
                "{slug_name}.legacy_required must remain false; historical archives live in pinned Git history"
            ),
        )?;
    }
    absent(
        source,
        issues,
        &format!("mechanics/{slug_name}/legacy"),
        "retired archives stay in pinned Git history, outside the active checkout",
    )?;
    Ok(parts.into_iter().map(str::to_owned).collect())
}

fn context_budget(
    source: &mut Source<'_>,
    issues: &mut Vec<Issue>,
    topology: &Value,
) -> io::Result<()> {
    let Some(budget) = topology
        .get("always_on_context_budget")
        .and_then(Value::as_object)
    else {
        return push(
            issues,
            TOPOLOGY,
            "always_on_context_budget must be an object",
        );
    };
    let surface = budget.get("surface").and_then(Value::as_str);
    if surface != Some("mechanics/README.md") {
        push(
            issues,
            TOPOLOGY,
            "always_on_context_budget.surface must be mechanics/README.md",
        )?;
    }
    if budget.get("metric").and_then(Value::as_str) != Some("whitespace_tokens_v1") {
        push(
            issues,
            TOPOLOGY,
            "always_on_context_budget.metric is unsupported",
        )?;
    }
    let Some(maximum) = budget
        .get("max_tokens")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
    else {
        return push(
            issues,
            TOPOLOGY,
            "always_on_context_budget.max_tokens must be a positive integer",
        );
    };
    let path = surface.unwrap_or("None");
    let Some(text) = (if let Some(surface) = surface {
        source.read(surface)?
    } else {
        None
    }) else {
        return push(issues, path, "always-on context surface is missing");
    };
    let measured = text
        .split(python_space)
        .filter(|part| !part.is_empty())
        .count() as u64;
    if measured > maximum {
        push(
            issues,
            path,
            format!("always-on context exceeds {maximum} whitespace tokens: {measured}"),
        )?;
    }
    Ok(())
}

fn discovered_packages(source: &Source<'_>) -> io::Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let root = source.path("mechanics")?;
    if !root.is_dir() {
        return Ok(names);
    }
    let entries = directory_entries(&root, MAX_FILES)?;
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 mechanics package"))?;
        if name == "__pycache__" || name == "legacy" {
            continue;
        }
        if entry.file_type()?.is_dir() && entry.path().join("AGENTS.md").is_file() {
            names.insert(name);
        }
    }
    Ok(names)
}

fn moved_targets(
    source: &Source<'_>,
    issues: &mut Vec<Issue>,
    topology: &Value,
    packages: &BTreeMap<String, Value>,
    parts: &BTreeMap<String, BTreeSet<String>>,
) -> io::Result<()> {
    let Some(accounting) = topology
        .get("moved_path_accounting")
        .and_then(Value::as_object)
    else {
        return push(issues, TOPOLOGY, "moved_path_accounting must be an object");
    };
    let Some(targets) = topology
        .get("moved_path_targets")
        .and_then(Value::as_object)
    else {
        return push(issues, TOPOLOGY, "moved_path_targets must be an object");
    };
    let mut accounted = BTreeSet::new();
    for (package, members) in accounting {
        let Some(members) = members.as_object() else {
            push(
                issues,
                TOPOLOGY,
                format!("moved_path_accounting.{package} must be an object"),
            )?;
            continue;
        };
        if !packages.contains_key(package) {
            push(
                issues,
                TOPOLOGY,
                format!("moved_path_accounting.{package} is not a known package"),
            )?;
        }
        let known = parts.get(package).cloned().unwrap_or_default();
        for (part, old_paths) in members {
            if !known.contains(part) {
                push(
                    issues,
                    TOPOLOGY,
                    format!("moved_path_accounting.{package}.{part} is not an active part"),
                )?;
            }
            let Some(old_paths) =
                strings(Some(old_paths)).filter(|paths| paths.iter().all(|path| !path.is_empty()))
            else {
                push(
                    issues,
                    TOPOLOGY,
                    format!("moved_path_accounting.{package}.{part} must be a string list"),
                )?;
                continue;
            };
            for old in old_paths {
                accounted.insert(old.to_owned());
                let Some(new) = targets
                    .get(old)
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    push(
                        issues,
                        TOPOLOGY,
                        format!("{old} is missing a moved_path_targets entry"),
                    )?;
                    continue;
                };
                let expected = format!("mechanics/{package}/parts/{part}/");
                if known.contains(part) && !new.starts_with(&expected) {
                    push(
                        issues,
                        TOPOLOGY,
                        format!("{old} target must stay under {expected}"),
                    )?;
                }
                absent(
                    source,
                    issues,
                    old,
                    "mechanic-owned payload must stay in mechanics/, not the old ToS/root path",
                )?;
                required(source, issues, new)?;
            }
        }
    }
    for old in targets.keys() {
        if !accounted.contains(old) {
            push(
                issues,
                TOPOLOGY,
                format!("{old} target is not listed in moved_path_accounting"),
            )?;
        }
    }
    Ok(())
}

fn casefold(value: &str) -> io::Result<String> {
    python_casefold_unicode16_v1(value, 1_000_000, 3_000_000, 12_000_000)
        .map_err(|error| invalid(format!("mechanics Unicode casefold: {error}")))
}

fn markdown_files(source: &Source<'_>) -> io::Result<Vec<PathBuf>> {
    let start = source.path("mechanics")?;
    if !start.is_dir() {
        return Ok(Vec::new());
    }
    let mut pending = vec![start];
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        let entries = directory_entries(&directory, MAX_FILES - visited)?;
        visited = visited
            .checked_add(entries.len())
            .ok_or_else(|| invalid("mechanics traversal overflow"))?;
        if visited > MAX_FILES {
            return Err(invalid("mechanics traversal entry budget exceeded"));
        }
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(source.root)
                .map_err(|_| invalid("mechanics traversal escaped root"))?;
            if relative
                .components()
                .any(|part| part.as_os_str() == "legacy")
            {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(invalid(format!(
                    "symlink in mechanics traversal: {}",
                    relative.display()
                )));
            }
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path.extension().is_some_and(|extension| extension == "md")
                && path.file_name().is_some_and(|name| name != "AGENTS.md")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn rendered_markdown(text: &str, patterns: &Patterns) -> io::Result<String> {
    let without_comments = patterns.comments.replace_all(text, "");
    let mut rendered = String::new();
    let mut fence: Option<String> = None;
    let mut suppressed = String::new();
    for line in without_comments.split_inclusive('\n') {
        let trimmed = line.trim_start_matches([' ', '\t']);
        let bytes = trimmed.as_bytes();
        let leader = bytes.first().copied().unwrap_or_default();
        let count = bytes.iter().take_while(|byte| **byte == leader).count();
        let marker = if matches!(leader, b'`' | b'~') && count >= 3 {
            Some(&trimmed[..count])
        } else {
            None
        };
        if let Some(current) = &fence {
            suppressed.push_str(line);
            if marker == Some(current.as_str()) && trimmed[count..].trim().is_empty() {
                fence = None;
                suppressed.clear();
            }
            continue;
        }
        if let Some(marker) = marker {
            fence = Some(marker.to_owned());
            suppressed.push_str(line);
            continue;
        }
        rendered.push_str(line);
    }
    // Python's fenced-block regex leaves an unclosed fence in the text.
    rendered.push_str(&suppressed);
    Ok(rendered)
}

fn push_reference(
    refs: &mut Vec<String>,
    bytes: &mut usize,
    value: &str,
    max_count: usize,
    max_bytes: usize,
) -> io::Result<()> {
    if refs.len() >= max_count {
        return Err(invalid("mechanics reference budget exceeded"));
    }
    *bytes = bytes
        .checked_add(value.len())
        .ok_or_else(|| invalid("mechanics reference byte overflow"))?;
    if *bytes > max_bytes {
        return Err(invalid("mechanics reference byte budget exceeded"));
    }
    refs.push(value.to_owned());
    Ok(())
}

fn references(
    text: &str,
    patterns: &Patterns,
    max_count: usize,
    max_bytes: usize,
) -> io::Result<(Vec<String>, BTreeMap<String, String>)> {
    let mut refs = Vec::new();
    let mut bytes = 0usize;
    for capture in patterns.inline.captures_iter(text) {
        let whole = capture
            .get(0)
            .ok_or_else(|| invalid("markdown inline capture"))?;
        if !text[..whole.start()].ends_with('!') {
            push_reference(&mut refs, &mut bytes, &capture[2], max_count, max_bytes)?;
        }
    }
    let mut definitions = BTreeMap::new();
    let mut definition_order = Vec::new();
    let mut visible_definitions = BTreeMap::new();
    for capture in patterns.definition.captures_iter(text) {
        let label = capture[1].trim_matches(python_space);
        let key = reference_label(label)?;
        let target = capture
            .get(2)
            .or_else(|| capture.get(3))
            .ok_or_else(|| invalid("markdown definition capture"))?
            .as_str();
        if !label.starts_with('^') {
            if !visible_definitions.contains_key(&key) {
                definition_order.push(key.clone());
            }
            visible_definitions.insert(key.clone(), target.to_owned());
        }
        definitions.insert(key, target.to_owned());
    }
    for key in definition_order {
        let target = visible_definitions
            .get(&key)
            .ok_or_else(|| invalid("markdown definition disappeared"))?;
        push_reference(&mut refs, &mut bytes, target, max_count, max_bytes)?;
    }
    Ok((refs, definitions))
}

fn reference_bytes(references: &[String]) -> io::Result<usize> {
    references.iter().try_fold(0usize, |total, reference| {
        total
            .checked_add(reference.len())
            .ok_or_else(|| invalid("mechanics reference byte overflow"))
    })
}

fn reference_parts(reference: &str) -> (&str, &str) {
    let stripped = reference.trim_matches(['<', '>']);
    let (target, fragment) = stripped.split_once('#').unwrap_or((stripped, ""));
    (target.split('?').next().unwrap_or(""), fragment)
}

fn decoded(value: &str) -> io::Result<String> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let mut output = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    // urllib.parse.unquote uses UTF-8 with replacement for malformed octets.
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn python_word(ch: char) -> bool {
    ch == '_'
        || matches!(
            get_general_category(ch),
            GeneralCategory::UppercaseLetter
                | GeneralCategory::LowercaseLetter
                | GeneralCategory::TitlecaseLetter
                | GeneralCategory::ModifierLetter
                | GeneralCategory::OtherLetter
                | GeneralCategory::DecimalNumber
                | GeneralCategory::LetterNumber
                | GeneralCategory::OtherNumber
        )
}

fn python_space(ch: char) -> bool {
    // Matches the owner-pinned Python Unicode 16 whitespace range in FND.
    matches!(ch,
        '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{0085}' |
        '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' |
        '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

fn reference_label(label: &str) -> io::Result<String> {
    let collapsed = label
        .trim_matches(python_space)
        .split(python_space)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    casefold(&collapsed)
}

fn heading_anchor(value: &str, patterns: &Patterns) -> io::Result<String> {
    let without_explicit = patterns.heading_explicit.replace_all(value, "");
    let without_tags = patterns.tag.replace_all(&without_explicit, "");
    let without_links = patterns.markdown_link.replace_all(&without_tags, "$1");
    let folded = casefold(&without_links)?;
    let mut slug = String::new();
    let mut dash = false;
    for ch in folded.chars() {
        if python_word(ch) {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.push(ch);
        } else if python_space(ch) || ch == '-' {
            dash = true;
        }
    }
    Ok(slug)
}

fn add_anchor(
    anchors: &mut BTreeSet<String>,
    local_bytes: &mut usize,
    retained_bytes: usize,
    anchor: String,
) -> io::Result<()> {
    if anchors.contains(&anchor) {
        return Ok(());
    }
    anchor_capacity(*local_bytes, retained_bytes, anchor.len())?;
    *local_bytes += anchor.len();
    anchors.insert(anchor);
    Ok(())
}

fn anchor_capacity(local_bytes: usize, retained_bytes: usize, additional: usize) -> io::Result<()> {
    let projected = retained_bytes
        .checked_add(local_bytes)
        .and_then(|bytes| bytes.checked_add(additional))
        .ok_or_else(|| invalid("mechanics anchor byte overflow"))?;
    if projected > MAX_ANCHOR_BYTES {
        return Err(invalid("mechanics anchor byte budget exceeded"));
    }
    Ok(())
}

/// Shared mechanics-owned Markdown grammar for cross-corpus route checks.
/// Source reads and cumulative budgets remain with the calling operation.
pub struct MarkdownRules {
    patterns: Patterns,
}
impl MarkdownRules {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            patterns: Patterns::new()?,
        })
    }
    pub fn rendered(&self, text: &str) -> io::Result<String> {
        rendered_markdown(text, &self.patterns)
    }
    pub fn references(
        &self,
        text: &str,
        count: usize,
        bytes: usize,
    ) -> io::Result<(Vec<String>, BTreeMap<String, String>)> {
        references(text, &self.patterns, count, bytes)
    }
    pub fn reference_uses(&self, text: &str) -> io::Result<Vec<(String, String)>> {
        let mut uses = Vec::new();
        let mut bytes = 0usize;
        for capture in self.patterns.use_ref.captures_iter(text) {
            let whole = capture
                .get(0)
                .ok_or_else(|| invalid("markdown use capture"))?;
            if text[..whole.start()].ends_with('!') {
                continue;
            }
            bytes = bytes
                .checked_add(capture[1].len() + capture[2].len())
                .ok_or_else(|| invalid("markdown reference byte overflow"))?;
            if uses.len() >= MAX_REFERENCES || bytes > MAX_REFERENCE_BYTES {
                return Err(invalid("markdown reference budget exceeded"));
            }
            uses.push((capture[1].to_owned(), capture[2].to_owned()));
        }
        Ok(uses)
    }
    pub fn anchors(
        &self,
        text: &str,
        retained_bytes: usize,
    ) -> io::Result<(BTreeSet<String>, usize)> {
        document_anchor_ids(text, &self.patterns, retained_bytes)
    }
    pub fn reference_label(&self, text: &str) -> io::Result<String> {
        reference_label(text)
    }
    pub fn normalized_fragment(&self, fragment: &str) -> io::Result<String> {
        casefold(&decoded(fragment)?)
    }
    pub fn reference_parts<'a>(&self, reference: &'a str) -> (&'a str, &'a str) {
        reference_parts(reference)
    }
    pub fn resolve(
        &self,
        root: &Path,
        document: &Path,
        reference: &str,
    ) -> io::Result<Option<PathBuf>> {
        resolve_reference_root(root, document, reference)
    }
}

fn document_anchor_ids(
    text: &str,
    patterns: &Patterns,
    retained_bytes: usize,
) -> io::Result<(BTreeSet<String>, usize)> {
    let mut anchors = BTreeSet::new();
    let mut local_bytes = 0usize;
    for capture in patterns.attribute.captures_iter(text) {
        add_anchor(
            &mut anchors,
            &mut local_bytes,
            retained_bytes,
            casefold(&decoded(&capture[1])?)?,
        )?;
    }
    for capture in patterns.anchor_explicit.captures_iter(text) {
        add_anchor(
            &mut anchors,
            &mut local_bytes,
            retained_bytes,
            casefold(&capture[1])?,
        )?;
    }
    let mut counts = BTreeMap::<String, usize>::new();
    for capture in patterns.heading.captures_iter(text) {
        let Some(title) = capture.get(1) else {
            continue;
        };
        let anchor = heading_anchor(title.as_str(), patterns)?;
        if anchor.is_empty() {
            continue;
        }
        let count = counts.get(&anchor).copied().unwrap_or_default();
        if count == 0 {
            if !anchors.contains(&anchor) {
                anchor_capacity(local_bytes, retained_bytes, anchor.len())?;
            }
            add_anchor(
                &mut anchors,
                &mut local_bytes,
                retained_bytes,
                anchor.clone(),
            )?;
        } else {
            add_anchor(
                &mut anchors,
                &mut local_bytes,
                retained_bytes,
                format!("{anchor}-{count}"),
            )?;
        }
        counts.insert(anchor, count + 1);
    }
    Ok((anchors, local_bytes))
}

fn document_has_fragment(
    source: &mut Source<'_>,
    path: &Path,
    fragment: &str,
    patterns: &Patterns,
) -> io::Result<bool> {
    let fragment = casefold(&decoded(fragment)?)?;
    if let Some(anchors) = source.anchors.get(path) {
        return Ok(anchors.contains(&fragment));
    }
    let relative = path
        .strip_prefix(source.root)
        .map_err(|_| invalid("anchor path escapes root"))?
        .to_str()
        .ok_or_else(|| invalid("non-UTF-8 anchor path"))?;
    let text = source
        .read(relative)?
        .ok_or_else(|| invalid("missing document anchor target"))?;
    let (anchors, local_bytes) = document_anchor_ids(&text, patterns, source.anchor_bytes)?;
    source.anchor_bytes = source
        .anchor_bytes
        .checked_add(local_bytes)
        .ok_or_else(|| invalid("mechanics anchor byte overflow"))?;
    let found = anchors.contains(&fragment);
    source.anchors.insert(path.to_owned(), anchors);
    Ok(found)
}

fn resolve_reference(
    source: &Source<'_>,
    document: &Path,
    reference: &str,
) -> io::Result<Option<PathBuf>> {
    resolve_reference_root(source.root, document, reference)
}
fn resolve_reference_root(
    root: &Path,
    document: &Path,
    reference: &str,
) -> io::Result<Option<PathBuf>> {
    let (target, fragment) = reference_parts(reference);
    if target.is_empty() && fragment.is_empty() {
        return Ok(None);
    }
    if target.starts_with('/') || target.contains("://") || target.starts_with("mailto:") {
        return Ok(None);
    }
    let candidates = if target.is_empty() {
        vec![document.to_owned()]
    } else {
        vec![
            document
                .parent()
                .ok_or_else(|| invalid("document parent"))?
                .join(target),
            root.join(target),
        ]
    };
    for candidate in candidates {
        let Ok(resolved) = fs::canonicalize(candidate) else {
            continue;
        };
        if resolved.starts_with(root) {
            return Ok(Some(resolved));
        }
    }
    Ok(None)
}

fn inventory(
    source: &mut Source<'_>,
    issues: &mut Vec<Issue>,
    relative: &str,
    key: &str,
    kind: &str,
) -> io::Result<BTreeSet<String>> {
    let Some(text) = source.read(relative)? else {
        push(
            issues,
            relative,
            format!("missing {kind} inventory for route references"),
        )?;
        return Ok(BTreeSet::new());
    };
    let Ok(value): Result<Value, _> = serde_json::from_str(&text) else {
        push(issues, relative, format!("invalid {kind} inventory"))?;
        return Ok(BTreeSet::new());
    };
    let Some(entries) = value.get(key).and_then(Value::as_array) else {
        push(
            issues,
            relative,
            format!("{kind} inventory must contain a {key} list"),
        )?;
        return Ok(BTreeSet::new());
    };
    Ok(entries
        .iter()
        .filter_map(|entry| entry.get("path").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

fn route_map(
    source: &mut Source<'_>,
    issues: &mut Vec<Issue>,
    patterns: &Patterns,
    order: &[String],
    packages: &BTreeMap<String, Value>,
    parts: &BTreeMap<String, BTreeSet<String>>,
) -> io::Result<()> {
    let Some(root_text) = source.read("mechanics/README.md")? else {
        return Ok(());
    };
    for marker in [
        "## Task-to-owner map",
        "## Progressive disclosure",
        "topology.json",
        "validation_lanes.json",
        "docs/decisions/README.md",
    ] {
        if !root_text.contains(marker) {
            push(
                issues,
                "mechanics/README.md",
                format!("missing executable-architecture route marker: {marker}"),
            )?;
        }
    }
    let mut visited = BTreeSet::new();
    for slug in order {
        if !visited.insert(slug) {
            continue;
        }
        if !packages.contains_key(slug) {
            continue;
        }
        if !root_text.contains(&format!("]({slug}/README.md)")) {
            push(
                issues,
                "mechanics/README.md",
                format!("package {slug} is missing from the human package map"),
            )?;
        }
        let path = format!("mechanics/{slug}/README.md");
        let Some(text) = source.read(&path)? else {
            continue;
        };
        let (count, bytes) = source.remaining_references();
        let (links, _) = references(&rendered_markdown(&text, patterns)?, patterns, count, bytes)?;
        source.charge_references(links.len(), reference_bytes(&links)?)?;
        let links: BTreeSet<_> = links
            .iter()
            .map(|link| reference_parts(link).0.to_owned())
            .collect();
        for companion in ["PARTS.md", "PROVENANCE.md", "ROADMAP.md"] {
            if !links.contains(companion) {
                push(
                    issues,
                    &path,
                    format!("package route does not link to {companion}"),
                )?;
            }
        }
        let parts_path = format!("mechanics/{slug}/PARTS.md");
        let Some(parts_text) = source.read(&parts_path)? else {
            continue;
        };
        let (count, bytes) = source.remaining_references();
        let (links, _) = references(
            &rendered_markdown(&parts_text, patterns)?,
            patterns,
            count,
            bytes,
        )?;
        source.charge_references(links.len(), reference_bytes(&links)?)?;
        let links: BTreeSet<_> = links
            .iter()
            .map(|link| reference_parts(link).0.to_owned())
            .collect();
        for part in parts.get(slug).into_iter().flat_map(|parts| parts.iter()) {
            let link = format!("parts/{part}/README.md");
            if !links.contains(&link) {
                push(
                    issues,
                    &parts_path,
                    format!("active part {part} is not present in the package selection map"),
                )?;
            }
        }
    }
    Ok(())
}

fn documentation_references(
    source: &mut Source<'_>,
    issues: &mut Vec<Issue>,
    patterns: &Patterns,
) -> io::Result<()> {
    let scripts = inventory(
        source,
        issues,
        SCRIPT_INVENTORY,
        "script_surfaces",
        "script",
    )?;
    let mut tests = None;
    for file in markdown_files(source)? {
        let relative = file
            .strip_prefix(source.root)
            .map_err(|_| invalid("mechanics document escaped root"))?
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 mechanics document"))?
            .to_owned();
        let text = source
            .read(&relative)?
            .ok_or_else(|| invalid("mechanics document disappeared"))?;
        let rendered = rendered_markdown(&text, patterns)?;
        let (count, bytes) = source.remaining_references();
        let (mut references, definitions) = references(&rendered, patterns, count, bytes)?;
        source.charge_references(references.len(), reference_bytes(&references)?)?;
        for capture in patterns.use_ref.captures_iter(&rendered) {
            let whole = capture
                .get(0)
                .ok_or_else(|| invalid("markdown use capture"))?;
            source.charge_references(1, whole.len())?;
            if rendered[..whole.start()].ends_with('!') {
                continue;
            }
            let label = if capture[2].is_empty() {
                &capture[1]
            } else {
                &capture[2]
            };
            let key = reference_label(label)?;
            if let Some(target) = definitions.get(&key) {
                source.charge_references(0, target.len())?;
                references.push(target.clone());
            } else {
                push(
                    issues,
                    &relative,
                    format!("unresolved reference-style documentation route: {label}"),
                )?;
            }
        }
        for reference in references {
            if reference.starts_with("http:")
                || reference.starts_with("https:")
                || reference.starts_with("mailto:")
            {
                continue;
            }
            let Some(resolved) = resolve_reference(source, &file, &reference)? else {
                push(
                    issues,
                    &relative,
                    format!("broken local documentation route: {reference}"),
                )?;
                continue;
            };
            let (_, fragment) = reference_parts(&reference);
            if !fragment.is_empty()
                && !document_has_fragment(source, &resolved, fragment, patterns)?
            {
                push(
                    issues,
                    &relative,
                    format!("broken local documentation fragment: {reference}"),
                )?;
            }
        }
        for capture in patterns.script_ref.captures_iter(&text) {
            let whole = capture
                .get(1)
                .ok_or_else(|| invalid("script reference capture"))?;
            source.charge_references(1, whole.len())?;
            if whole.start() > 0
                && matches!(text.as_bytes()[whole.start()-1], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-')
            {
                continue;
            }
            let reference = whole.as_str();
            let Some(resolved) = resolve_reference(source, &file, reference)? else {
                push(
                    issues,
                    &relative,
                    format!("stale executable reference: {reference}"),
                )?;
                continue;
            };
            let target = resolved
                .strip_prefix(source.root)
                .map_err(|_| invalid("script target escaped root"))?
                .to_str()
                .ok_or_else(|| invalid("non-UTF-8 script target"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let is_test = resolved
                .components()
                .any(|component| component.as_os_str() == "tests")
                && resolved.file_name().is_some_and(|name| {
                    name.to_string_lossy().starts_with("test")
                        && name.to_string_lossy().ends_with(".py")
                });
            let owning = if is_test {
                if tests.is_none() {
                    tests = Some(inventory(source, issues, TEST_INVENTORY, "tests", "test")?);
                }
                tests
                    .as_ref()
                    .ok_or_else(|| invalid("test inventory missing"))?
            } else {
                &scripts
            };
            if !owning.contains(&target) {
                push(
                    issues,
                    &relative,
                    format!(
                        "executable reference is absent from {} inventory: {target}",
                        if is_test { "test" } else { "script" }
                    ),
                )?;
            }
        }
    }
    Ok(())
}

/// Complete, read-only mechanics topology and route-document validation.
/// Python remains the independent lane oracle until source, cost and actual
/// native execution have been accepted by the owner.
pub fn validate(root: &Path) -> io::Result<Vec<Issue>> {
    static NEVER_CANCEL: AtomicI32 = AtomicI32::new(0);
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(30))
        .ok_or_else(|| invalid("mechanics deadline overflow"))?;
    validate_with_deadline(root, deadline, &NEVER_CANCEL)
}

/// Reuse the caller's original deadline rather than starting a new clock.
pub fn validate_with_deadline(
    root: &Path,
    deadline: Instant,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut source = Source::new(root, deadline, cancel)?;
    source.check()?;
    let patterns = Patterns::new()?;
    let mut issues = Vec::new();
    required(&source, &mut issues, "mechanics/AGENTS.md")?;
    required(&source, &mut issues, "mechanics/README.md")?;
    absent(
        &source,
        &mut issues,
        "mechanics/legacy",
        "root mechanics legacy is forbidden",
    )?;
    let Some(text) = source.read(TOPOLOGY)? else {
        push(&mut issues, TOPOLOGY, "missing mechanics topology")?;
        return Ok(issues);
    };
    let topology: Value = match serde_json::from_str(&text) {
        Ok(topology) => topology,
        Err(error) => {
            push(&mut issues, TOPOLOGY, format!("invalid JSON: {error}"))?;
            return Ok(issues);
        }
    };
    if !topology.is_object() {
        push(&mut issues, TOPOLOGY, "topology root must be a JSON object")?;
        return Ok(issues);
    }
    for (field, expected, message) in [
        (
            "schema_version",
            "tos_mechanics_topology_v2",
            "schema_version must be tos_mechanics_topology_v2",
        ),
        (
            "owner_repo",
            "Tree-of-Sophia",
            "owner_repo must be Tree-of-Sophia",
        ),
        ("root", "mechanics/", "root must be mechanics/"),
        (
            "legacy_policy",
            "pinned-git-history-no-active-legacy",
            "legacy_policy drifted",
        ),
    ] {
        if topology.get(field).and_then(Value::as_str) != Some(expected) {
            push(&mut issues, TOPOLOGY, message)?;
        }
    }
    let Some(entries) = topology.get("packages").and_then(Value::as_array) else {
        push(&mut issues, TOPOLOGY, "packages must be a list")?;
        return Ok(issues);
    };
    let mut packages = BTreeMap::new();
    let mut parts = BTreeMap::new();
    let mut order = Vec::new();
    for entry in entries {
        if !entry.is_object() {
            push(
                &mut issues,
                TOPOLOGY,
                "each package entry must be an object",
            )?;
            continue;
        }
        let Some(slug_name) = entry
            .get("slug")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
        else {
            push(
                &mut issues,
                TOPOLOGY,
                "package slug must be a non-empty string",
            )?;
            continue;
        };
        if packages.contains_key(slug_name) {
            push(
                &mut issues,
                TOPOLOGY,
                format!("duplicate package slug {slug_name}"),
            )?;
        }
        packages.insert(slug_name.to_owned(), entry.clone());
        order.push(slug_name.to_owned());
        parts.insert(
            slug_name.to_owned(),
            validate_package(&source, &mut issues, slug_name, entry)?
                .into_iter()
                .collect(),
        );
    }
    let package_dirs = discovered_packages(&source)?;
    for missing in package_dirs.difference(&packages.keys().cloned().collect()) {
        push(
            &mut issues,
            TOPOLOGY,
            format!("mechanics package directory missing from topology: {missing}"),
        )?;
    }
    for extra in packages
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        .difference(&package_dirs)
    {
        push(
            &mut issues,
            TOPOLOGY,
            format!("topology package missing mechanics directory: {extra}"),
        )?;
    }
    if order.len() != order.iter().collect::<BTreeSet<_>>().len() {
        push(&mut issues, TOPOLOGY, "packages must be unique")?;
    }
    source.check()?;
    context_budget(&mut source, &mut issues, &topology)?;
    route_map(
        &mut source,
        &mut issues,
        &patterns,
        &order,
        &packages,
        &parts,
    )?;
    source.check()?;
    documentation_references(&mut source, &mut issues, &patterns)?;
    source.check()?;
    moved_targets(&source, &mut issues, &topology, &packages, &parts)?;
    source.check()?;
    Ok(issues)
}
