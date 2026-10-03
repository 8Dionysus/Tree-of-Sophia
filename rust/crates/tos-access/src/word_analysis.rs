//! Private task preparation through the selected source reader. No inference or writes.
use crate::{AccessError, AccessErrorCode, AccessExecutor, AccessProfile};
use std::io::{Read, Write};
use tos_query::reading_search::WordAnalysisRequest;

fn invalid(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::InvalidRequest, message)
}

pub(crate) fn run_cli(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let result = (|| {
        let mut request = WordAnalysisRequest {
            query: String::new(),
            language: String::new(),
            rank: 1,
            include_semantic_neighbors: false,
            request_ref: None,
        };
        let mut candidate_path = None;
        let mut at = 1;
        while at < args.len() {
            let option = args[at].as_str();
            if option == "--include-semantic-neighbors" {
                request.include_semantic_neighbors = true;
                at += 1;
                continue;
            }
            let value = args
                .get(at + 1)
                .ok_or_else(|| invalid("word-analysis option requires a value"))?;
            match option {
                "--query" => request.query = value.clone(),
                "--language" => request.language = value.clone(),
                "--rank" => {
                    if value.len() > profile.max_request_bytes {
                        return Err(invalid("word-analysis rank exceeds request budget"));
                    }
                    request.rank = tos_query::reading_search::parse_word_analysis_rank(
                        value,
                        tos_foundation::JsonLimits::default().max_integer_digits,
                    )
                    .map_err(|_| invalid("word-analysis rank must be a bounded positive integer"))?
                }
                "--request" => request.request_ref = Some(value.clone()),
                "--validate-candidate" => candidate_path = Some(value.clone()),
                _ => return Err(invalid("unsupported word-analysis option")),
            }
            at += 2;
        }
        if request.query.trim().is_empty()
            || !["de", "ru", "en"].contains(&request.language.as_str())
        {
            return Err(invalid(
                "word-analysis requires a query and de, ru or en language",
            ));
        }
        crate::checked_execute(profile.deadline_probe(), |probe| {
            let candidate = candidate_path
                .map(|path| {
                    crate::knowledge::check_abort(&probe)?;
                    // Caller-selected input, never executable software. Bound actual reads
                    // as well as metadata, and exclude devices/FIFOs before opening.
                    let metadata = std::fs::symlink_metadata(&path)
                        .map_err(|_| invalid("word-analysis candidate unavailable"))?;
                    if !metadata.file_type().is_file() {
                        return Err(invalid("word-analysis candidate must be a regular file"));
                    }
                    use std::os::unix::fs::OpenOptionsExt;
                    let file = std::fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                        .open(path)
                        .map_err(|_| invalid("word-analysis candidate unavailable"))?;
                    if !file
                        .metadata()
                        .map_err(|_| invalid("word-analysis candidate unavailable"))?
                        .is_file()
                    {
                        return Err(invalid("word-analysis candidate must be a regular file"));
                    }
                    let cap = profile.max_request_bytes;
                    let mut bytes = Vec::new();
                    file.take(cap as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| invalid("word-analysis candidate read failed"))?;
                    if bytes.len() > cap {
                        return Err(AccessError::new(
                            AccessErrorCode::BudgetExceeded,
                            "word-analysis candidate byte limit exceeded",
                        ));
                    }
                    crate::knowledge::check_abort(&probe)?;
                    Ok(bytes)
                })
                .transpose()?;
            executor.word_analysis(request, candidate.as_deref(), probe)
        })
    })();
    match result {
        Ok(packet) => crate::cli::write_packet(packet, profile, stdout, stderr),
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            match error.code {
                AccessErrorCode::InvalidRequest => 2,
                AccessErrorCode::Unavailable => 3,
                _ => 1,
            }
        }
    }
}

pub(crate) const MCP_TOOL: &str = "tos_zarathustra_prepare_word_analysis";
pub(crate) const OPERATION_ID: &str = "tos.zarathustra.word_analysis_task";

struct WireArguments<'a> {
    query: &'a [u16],
    language: String,
    rank: usize,
    semantic: bool,
}

/// Shared wire validation also serves the software-only unavailable response.
/// JSON permits lone surrogates; only the positive UTF-8 reader rejects them.
pub(crate) fn validate_arguments(
    args: &tos_foundation::JsonValue,
    profile: AccessProfile,
) -> Result<(), AccessError> {
    parse_arguments(args, profile).map(|_| ())
}

pub(crate) fn from_arguments(
    args: &tos_foundation::JsonValue,
    profile: AccessProfile,
) -> Result<WordAnalysisRequest, AccessError> {
    let wire = parse_arguments(args, profile)?;
    Ok(WordAnalysisRequest {
        query: String::from_utf16(wire.query)
            .map_err(|_| invalid("word-analysis query must be valid Unicode for source reading"))?,
        language: wire.language,
        rank: wire.rank,
        include_semantic_neighbors: wire.semantic,
        request_ref: None,
    })
}

/// MCP's maintained typed adapter coercion, followed by Core's 1..100 bound.
/// The standalone command keeps its separate positive-rank contract.
fn parse_arguments(
    args: &tos_foundation::JsonValue,
    profile: AccessProfile,
) -> Result<WireArguments<'_>, AccessError> {
    use tos_foundation::{JsonNumberKind, JsonValue};
    let Some(JsonValue::String(query)) = args.object_get("query") else {
        return Err(invalid("word-analysis query is required"));
    };
    let whitespace = |unit: u16| {
        char::from_u32(unit as u32).is_some_and(|ch| {
            let mut bytes = [0; 4];
            tos_foundation::python_strip_unicode16_v1(ch.encode_utf8(&mut bytes), 1)
                .is_ok_and(str::is_empty)
        })
    };
    let units = query.units();
    let begin = units
        .iter()
        .position(|u| !whitespace(*u))
        .unwrap_or(units.len());
    let end = units
        .iter()
        .rposition(|u| !whitespace(*u))
        .map_or(begin, |i| i + 1);
    let query = &units[begin..end];
    if query.is_empty() || char::decode_utf16(query.iter().copied()).count() > 256 {
        return Err(invalid(
            "word-analysis query must contain 1 to 256 characters",
        ));
    }
    let language = match args.object_get("language") {
        None => "ru",
        Some(v) => v
            .as_str()
            .ok_or_else(|| invalid("word-analysis language must be a string"))?,
    };
    let language = tos_foundation::python_strip_unicode16_v1(language, profile.max_request_bytes)
        .map_err(|_| invalid("word-analysis language exceeds request budget"))?
        .to_lowercase();
    if !matches!(language.as_str(), "de" | "ru" | "en") {
        return Err(invalid("unsupported word-analysis language"));
    }
    let rank = match args.object_get("rank") {
        None | Some(JsonValue::Bool(_)) => 1,
        Some(JsonValue::Number(n)) if n.kind == JsonNumberKind::Int => bounded_rank(&n.lexeme)?,
        Some(JsonValue::Number(n)) => {
            let value = n
                .lexeme
                .parse::<f64>()
                .map_err(|_| invalid("word-analysis rank must be an integer"))?;
            if !value.is_finite() || value.fract() != 0.0 {
                return Err(invalid("word-analysis rank must be an integer"));
            }
            value.clamp(1.0, 100.0) as usize
        }
        Some(JsonValue::String(s)) => bounded_rank(
            s.as_str()
                .ok_or_else(|| invalid("word-analysis rank must be an integer"))?,
        )?,
        _ => return Err(invalid("word-analysis rank must be an integer")),
    };
    let semantic = match args.object_get("include_semantic_neighbors") {
        None => false,
        Some(JsonValue::Bool(v)) => *v,
        Some(JsonValue::Number(n)) => match n.lexeme.parse::<f64>() {
            Ok(0.0) => false,
            Ok(1.0) => true,
            _ => {
                return Err(invalid(
                    "word-analysis semantic-neighbor flag must be boolean",
                ));
            }
        },
        Some(JsonValue::String(s)) => {
            match s.as_str().unwrap_or("").to_ascii_lowercase().as_str() {
                "0" | "off" | "f" | "false" | "n" | "no" => false,
                "1" | "on" | "t" | "true" | "y" | "yes" => true,
                _ => {
                    return Err(invalid(
                        "word-analysis semantic-neighbor flag must be boolean",
                    ));
                }
            }
        }
        _ => {
            return Err(invalid(
                "word-analysis semantic-neighbor flag must be boolean",
            ));
        }
    };
    Ok(WireArguments {
        query,
        language,
        rank,
        semantic,
    })
}

fn bounded_rank(raw: &str) -> Result<usize, AccessError> {
    let invalid = || invalid("word-analysis rank must be an integer");
    let raw = raw.trim();
    let (negative, raw) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    let mut parts = raw.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    if parts.next().is_some()
        || fraction.is_some_and(|s| s.is_empty() || !s.bytes().all(|b| b == b'0'))
    {
        return Err(invalid());
    }
    let mut previous = false;
    let mut digits = 0;
    let mut value = 0usize;
    for b in whole.bytes() {
        if b == b'_' && previous {
            previous = false;
            continue;
        }
        if !b.is_ascii_digit() {
            return Err(invalid());
        }
        previous = true;
        digits += 1;
        if digits > 4300 {
            return Err(invalid());
        }
        value = (value * 10 + (b - b'0') as usize).min(100);
    }
    if !previous {
        return Err(invalid());
    }
    Ok(if negative { 1 } else { value.max(1) })
}

pub(crate) fn prepare_capability<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    arguments: &tos_foundation::JsonValue,
    profile: AccessProfile,
    probe: std::sync::Arc<dyn tos_query::AbortProbe>,
) -> Result<crate::PreparedPacket<'hold>, AccessError> {
    crate::knowledge::check_abort(&probe)?;
    let request = from_arguments(arguments, profile)?;
    let mut packet = executor.word_analysis(request, None, probe.clone())?;
    // Preserve the native task bytes and its held source fence; this is only
    // the existing adapter envelope, not a second query or software lookup.
    let prefix = br#"{"schema":"tos_zarathustra_word_analysis_capability_v1","available":true,"reason":null,"provider_ref":"rust/crates/tos-query/src/reading_search/word_analysis.rs","publication_posture":"local_full_tree_only","task":"#;
    let suffix = br#", "authority":{"source_owner":"Tree-of-Sophia","access_plane_is_source":false,"is_semantic_truth":false,"writes_to_tree":false,"reviewed":false,"canon":false}}"#;
    let length = prefix
        .len()
        .checked_add(packet.body.len())
        .and_then(|n| n.checked_add(suffix.len()))
        .filter(|n| *n <= profile.max_response_bytes)
        .ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "word-analysis capability response byte budget",
            )
        })?;
    let mut body = Vec::with_capacity(length);
    body.extend_from_slice(prefix);
    body.extend_from_slice(&packet.body);
    body.extend_from_slice(suffix);
    packet.body = body;
    packet.fence.recheck()?;
    crate::knowledge::check_abort(&probe)?;
    Ok(packet)
}

// The maintained HTTP adapter uses int(string), default1 on ValueError,
// then clamps1..100. This differs from MCP's Pydantic integral-string grammar.
pub(crate) fn http_rank(raw: &str) -> usize {
    let raw = raw.trim_matches(char::is_whitespace);
    let (negative, raw) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    let mut previous = false;
    let mut digits = 0usize;
    let mut value = 0usize;
    for ch in raw.chars() {
        if ch == '_' && previous {
            previous = false;
            continue;
        }
        if !tos_foundation::python_decimal_unicode16_v1(ch) {
            return 1;
        }
        digits += 1;
        if digits > 4300 {
            return 1;
        }
        let mut code = ch as u32;
        let mut offset = 0usize;
        while code > 0
            && char::from_u32(code - 1).is_some_and(tos_foundation::python_decimal_unicode16_v1)
        {
            code -= 1;
            offset += 1;
            if offset > 64 {
                return 1;
            }
        }
        value = (value * 10 + offset % 10).min(100);
        previous = true;
    }
    if negative || !previous {
        1
    } else {
        value.max(1)
    }
}
