//! Check or write the nine current canon-to-public compatibility mirrors.
//! The canon files own the content; mirror bytes remain derived.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

const PAIRS: &[(&str, &str)] = &[
    (
        "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json",
        "ToS/public-compatibility/source_node.example.json",
    ),
    (
        "ToS/canon/concept/becoming/node.json",
        "ToS/public-compatibility/concept_node.example.json",
    ),
    (
        "ToS/canon/principle/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/solitude-as-ripening/node.json",
        "ToS/public-compatibility/principle_node.example.json",
    ),
    (
        "ToS/canon/lineage/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/becoming-to-overcoming/node.json",
        "ToS/public-compatibility/lineage_node.example.json",
    ),
    (
        "ToS/canon/event/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-origin/node.json",
        "ToS/public-compatibility/event_node.example.json",
    ),
    (
        "ToS/canon/state/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/solitary-ripening-10y/node.json",
        "ToS/public-compatibility/state_node.example.json",
    ),
    (
        "ToS/canon/support/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/zarathustra/node.json",
        "ToS/public-compatibility/support_node.example.json",
    ),
    (
        "ToS/canon/analogy/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/bee-honey-analogy/node.json",
        "ToS/public-compatibility/analogy_node.example.json",
    ),
    (
        "ToS/canon/synthesis/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-reflective-origin/node.json",
        "ToS/public-compatibility/synthesis_node.example.json",
    ),
];

pub type Issue = (&'static str, &'static str);

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read(root: &Path, relative: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(root.join(relative))?;
    let limit = JsonLimits::default().max_bytes;
    if file.metadata()?.len() > limit as u64 {
        return Err(invalid(format!(
            "public mirror input exceeds JSON bound: {relative}"
        )));
    }
    let mut raw = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > limit {
        return Err(invalid(format!(
            "public mirror input exceeds JSON bound: {relative}"
        )));
    }
    Ok(raw)
}

fn append(output: &mut Vec<u8>, bytes: &[u8], limit: usize) -> io::Result<()> {
    if output
        .len()
        .checked_add(bytes.len())
        .is_none_or(|next| next > limit)
    {
        return Err(invalid(
            "public mirror expected JSON exceeds foundation bound",
        ));
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn indentation(output: &mut Vec<u8>, depth: usize, limit: usize) -> io::Result<()> {
    const SPACES: [u8; 128] = [b' '; 128];
    let width = depth
        .checked_mul(2)
        .filter(|width| *width <= SPACES.len())
        .ok_or_else(|| invalid("public mirror expected JSON depth exceeded"))?;
    append(output, &SPACES[..width], limit)
}

fn scalar(output: &mut Vec<u8>, value: &JsonValue, limit: usize) -> io::Result<()> {
    // Foundation owns Python-compatible scalar escaping and float spelling.
    let bytes = canonical_bytes_v1(
        value,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .map_err(|error| invalid(format!("public mirror JSON scalar: {error}")))?;
    append(output, &bytes, limit)
}

fn render(output: &mut Vec<u8>, value: &JsonValue, depth: usize, limit: usize) -> io::Result<()> {
    if depth > JsonLimits::default().max_depth {
        return Err(invalid("public mirror expected JSON depth exceeded"));
    }
    match value {
        JsonValue::Array(items) => {
            append(output, b"[", limit)?;
            for (index, item) in items.iter().enumerate() {
                let separator: &[u8] = if index == 0 { b"\n" } else { b",\n" };
                append(output, separator, limit)?;
                indentation(output, depth + 1, limit)?;
                render(output, item, depth + 1, limit)?;
            }
            if !items.is_empty() {
                append(output, b"\n", limit)?;
                indentation(output, depth, limit)?;
            }
            append(output, b"]", limit)
        }
        JsonValue::Object(entries) => {
            append(output, b"{", limit)?;
            for (index, (key, item)) in entries.iter().enumerate() {
                let separator: &[u8] = if index == 0 { b"\n" } else { b",\n" };
                append(output, separator, limit)?;
                indentation(output, depth + 1, limit)?;
                scalar(output, &JsonValue::String(key.clone()), limit)?;
                append(output, b": ", limit)?;
                render(output, item, depth + 1, limit)?;
            }
            if !entries.is_empty() {
                append(output, b"\n", limit)?;
                indentation(output, depth, limit)?;
            }
            append(output, b"}", limit)
        }
        _ => scalar(output, value, limit),
    }
}

fn encode_json(value: &JsonValue) -> io::Result<Vec<u8>> {
    let limit = JsonLimits::default().max_bytes;
    let mut output = Vec::new();
    render(&mut output, value, 0, limit)?;
    append(&mut output, b"\n", limit)?;
    Ok(output)
}

fn prepare_sources(root: &Path) -> io::Result<Vec<(&'static str, JsonValue)>> {
    if !root.is_absolute() || std::fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    let mut sources = Vec::with_capacity(PAIRS.len());
    for (canonical, public) in PAIRS {
        let raw = read(root, canonical)?;
        let document = parse_json(&raw, JsonMode::RequestLastWins, JsonLimits::default())
            .map_err(|error| invalid(format!("invalid JSON in {canonical}: {error}")))?;
        sources.push((*public, document.into_root()));
    }
    Ok(sources)
}

/// Prepare all canonical sources before reporting ordered missing/drift issues.
/// A clean result proves mirror byte compatibility only.
pub fn validate(root: &Path) -> io::Result<Vec<Issue>> {
    let sources = prepare_sources(root)?;
    let mut issues = Vec::new();
    for (public, value) in &sources {
        let raw = match read(root, public) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                issues.push((*public, "missing compatibility mirror"));
                continue;
            }
            result => result?,
        };
        let actual = String::from_utf8(raw)
            .map_err(|error| invalid(format!("invalid UTF-8 in {public}: {error}")))?;
        // Path.read_text() uses universal newline translation by default.
        let actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        let expected = encode_json(value)?;
        if actual.as_bytes() != expected.as_slice() {
            issues.push((*public, "out of sync with canonical tree; run tos-ops-mechanics-plan --repo-root PATH --public-mirror-sync"));
        }
    }
    Ok(issues)
}

/// Write the nine derived mirrors in the maintained order. Like the source
/// writer, an error after a completed write leaves earlier mirrors in place.
pub fn write_examples(root: &Path) -> io::Result<Vec<&'static str>> {
    let sources = prepare_sources(root)?;
    let mut written = Vec::with_capacity(sources.len());
    for (public, value) in &sources {
        // Encode before opening the destination: an encoding refusal must not
        // truncate that mirror, while earlier completed writes stay visible.
        let expected = encode_json(value)?;
        let mut file = File::create(root.join(public))?;
        file.write_all(&expected)?;
        written.push(*public);
    }
    Ok(written)
}
