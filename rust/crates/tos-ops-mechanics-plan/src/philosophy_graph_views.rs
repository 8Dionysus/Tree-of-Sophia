//! Maintained graph-view catalog validator over the existing native builder.
//! Derived equality and lens mechanics are not source, canon or runtime admission.
use crate::route_cards::{OutputBudget, RouteSources};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use tos_compiler::source_philosophy_views::{self, ViewLimits};
use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode, canonical_bytes_v1, parse_json};
const CATALOG: &str = "ToS/derived-exports/philosophy_graph_views.min.json";
struct Sources<'a> {
    reader: RouteSources,
    cancel: &'a AtomicI32,
}
impl Sources<'_> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 {
            return Err(io::Error::other("philosophy graph views cancelled"));
        }
        self.reader.check()
    }
    fn raw(&mut self, path: &str) -> io::Result<Vec<u8>> {
        self.check()?;
        self.reader.bytes(path)
    }
    fn object(&mut self, path: &str) -> io::Result<Value> {
        let raw = self.raw(path)?;
        decode_object(&raw, path, 8 * 1024 * 1024)
    }
    fn derived_object(&mut self, path: &str, max: usize) -> io::Result<Value> {
        self.check()?;
        // Each derived operand has its own explicit read profile. It never
        // enters the authored text cache or supplies source/canon authority.
        let mut read_bytes = 0;
        let raw = self.reader.bounded_bytes(path, max, &mut read_bytes, max)?;
        decode_object(&raw, path, max)
    }
    fn builder_read(&mut self, path: &str) -> tos_compiler::Result<Vec<u8>> {
        let raw = self
            .raw(path)
            .map_err(|e| tos_compiler::Error::Source(e.to_string()))?;
        // The old json.loads reader keeps decoded duplicates last. Normalize
        // only JSON at this representation seam before the existing builder's
        // strict source decoder; Markdown bytes go unchanged to its parser.
        if path.ends_with(".json") {
            canonical(&raw, ViewLimits::default().max_source_bytes)
                .map_err(|e| tos_compiler::Error::Source(e.to_string()))
        } else {
            Ok(raw)
        }
    }
}
fn decode_object(raw: &[u8], path: &str, max: usize) -> io::Result<Value> {
    let normalized = canonical(raw, max)?;
    let value: Value = serde_json::from_slice(&normalized).map_err(io::Error::other)?;
    if !value.is_object() {
        return Err(io::Error::other(format!(
            "{path} must contain a JSON object"
        )));
    }
    Ok(value)
}
fn limits(max: usize) -> io::Result<JsonLimits> {
    JsonLimits::new(max, 96, 2_000_000, 4096).map_err(io::Error::other)
}
fn canonical(raw: &[u8], max: usize) -> io::Result<Vec<u8>> {
    let limits = limits(max)?;
    let document = parse_json(raw, JsonMode::RequestLastWins, limits).map_err(io::Error::other)?;
    canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(io::Error::other)
}
struct BoundedWriter<'a> {
    raw: Vec<u8>,
    max: usize,
    check: &'a mut dyn FnMut() -> io::Result<()>,
}
impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (self.check)()?;
        if self
            .raw
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.max)
        {
            return Err(io::Error::other("graph view canonical rendering bound"));
        }
        self.raw.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn render(value: &Value) -> io::Result<Vec<u8>> {
    render_payload(
        value,
        ViewLimits::default().max_output_bytes,
        &mut || Ok(()),
    )
}
/// Existing sorted compact spelling with a caller's original operation guard.
/// The returned bytes exclude the maintained final LF.
pub(crate) fn render_payload(
    value: &Value,
    max: usize,
    check: &mut impl FnMut() -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    check()?;
    let mut output = BoundedWriter {
        raw: Vec::new(),
        max,
        check,
    };
    serde_json::to_writer(&mut output, value).map_err(io::Error::other)?;
    // Both sides use Python sorted compact spelling. The common final LF in
    // render_payload cancels in equality; no new digest/identity is issued.
    (output.check)()?;
    let rendered = canonical(&output.raw, max)?;
    (output.check)()?;
    Ok(rendered)
}
fn schema_check(
    validator: &jsonschema::Validator,
    value: &Value,
    source: &Sources<'_>,
) -> io::Result<()> {
    // The maintained validator sorts all errors and reports only the first.
    // Select the same minimum without retaining the complete error collection.
    let mut first = None;
    for error in validator.iter_errors(value) {
        source.check()?;
        let mut current = value;
        let mut order = Vec::new();
        let mut location = String::new();
        for segment in error.instance_path().segments() {
            let part = segment.to_string();
            if current.is_array() {
                let index = part.parse::<usize>().map_err(io::Error::other)?;
                order.push((0, index, String::new()));
                location.push_str(&format!("[{index}]"));
                current = current.get(index).unwrap_or(&Value::Null);
            } else {
                order.push((1, 0, part.clone()));
                if !location.is_empty() {
                    location.push('.');
                }
                location.push_str(&part);
                current = current.get(&part).unwrap_or(&Value::Null);
            }
        }
        if first
            .as_ref()
            .is_none_or(|(previous, _, _)| &order < previous)
        {
            first = Some((order, location, error));
        }
    }
    if let Some((_, location, error)) = first {
        return Err(io::Error::other(format!(
            "schema violation at {}: {error}",
            if location.is_empty() {
                "<root>"
            } else {
                &location
            }
        )));
    }
    Ok(())
}
fn require(ok: bool, message: &'static str) -> io::Result<()> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
fn list_has(value: Option<&Value>, needle: &str) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(needle)))
}
fn view<'a>(views: &BTreeMap<&str, &'a Value>, id: &str) -> io::Result<&'a Value> {
    views
        .get(id)
        .copied()
        .ok_or_else(|| io::Error::other(format!("philosophy graph catalog missing view {id}")))
}
pub fn run_validation(root: &Path, cancel: &AtomicI32) -> io::Result<()> {
    let mut source = Sources {
        reader: RouteSources::new(root)?,
        cancel,
    };
    // This function consumes the existing atlas projection, exactly like the
    // old build_payload. It does not rebuild atlas, graph or a fullphi stage.
    let atlas = source.derived_object(source_philosophy_views::ATLAS_REF, 128 * 1024 * 1024)?;
    let empty = Vec::new();
    let nodes=match atlas.get("nodes"){None=>&empty,Some(v)=>v.as_array().ok_or_else(||io::Error::other("ToS/derived-exports/philosophy_atlas_projection.min.json must expose nodes and edges"))?};
    let edges=match atlas.get("edges"){None=>&empty,Some(v)=>v.as_array().ok_or_else(||io::Error::other("ToS/derived-exports/philosophy_atlas_projection.min.json must expose nodes and edges"))?};
    source.check()?;
    let expected = source_philosophy_views::build_views(
        &mut |path| source.builder_read(path),
        nodes,
        edges,
        ViewLimits::default(),
    )
    .map_err(io::Error::other)?;
    drop(atlas);
    let schema = source.object(source_philosophy_views::VIEWS_SCHEMA)?;
    // Python's Draft202012Validator here has no FormatChecker; do not silently
    // reuse the asserted-source or topology FormatChecker comparison profile.
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(false)
        .offline()
        .build(&schema)
        .map_err(io::Error::other)?;
    schema_check(&validator, &expected, &source)?;
    let current = source.derived_object(CATALOG, ViewLimits::default().max_output_bytes)?;
    schema_check(&validator, &current, &source)?;
    source.check()?;
    let expected_bytes = render(&expected)?;
    drop(expected);
    let current_bytes = render(&current)?;
    require(
        current_bytes == expected_bytes,
        "ToS/derived-exports/philosophy_graph_views.min.json does not match the canonical rebuild",
    )?;
    drop(current_bytes);
    drop(expected_bytes);
    validate_assertions(&current, &mut || source.check())
}
/// Shared maintained assertions after schema and canonical rebuild equality.
pub(crate) fn validate_assertions(
    current: &Value,
    check: &mut impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let counts = current
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| io::Error::other("graph view catalog counts must be an object"))?;
    for (key, expected, message) in [
        (
            "views",
            11,
            "philosophy graph view catalog must expose 11 graph views",
        ),
        (
            "graph_layers",
            7,
            "philosophy graph view catalog must expose 7 graph layers",
        ),
        (
            "lens_review_contracts",
            11,
            "philosophy graph view catalog must expose 11 lens review contracts",
        ),
        (
            "diagnostics",
            0,
            "philosophy graph view catalog must not contain diagnostics",
        ),
    ] {
        // Schema has already asserted nonnegative integer values. Integral
        // float spelling follows Python equality here rather than raw lexemes.
        require(
            counts.get(key).and_then(Value::as_f64) == Some(expected as f64),
            message,
        )?;
    }
    require(
        current
            .get("runtime_projection_boundary")
            .and_then(|v| v.get("runtime_owner"))
            .and_then(Value::as_str)
            == Some("abyss-stack"),
        "philosophy graph view catalog must keep runtime projection ownership in abyss-stack",
    )?;
    require(
        current
            .get("default_lens_review_requirements")
            .and_then(|v| v.get("ui_mcp_payload_mode"))
            .and_then(Value::as_str)
            == Some("cluster-first"),
        "philosophy graph views must default to cluster-first UI/MCP packets",
    )?;
    // Python dict comprehension preserves the last duplicate view_id. The
    // current schema and canonical equality are checked before this consumer.
    let mut views = BTreeMap::new();
    for v in current
        .get("views")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        check()?;
        if v.is_object() {
            if let Some(id) = v.get("view_id").and_then(Value::as_str) {
                views.insert(id, v);
            }
        }
    }
    let chronology = view(&views, "chronology")?;
    require(
        list_has(
            chronology
                .get("current_projection_filters")
                .and_then(|v| v.get("row_fields")),
            "formation",
        ),
        "chronology view must expose formation row fields",
    )?;
    let evidence = view(&views, "source-evidence")?;
    require(
        list_has(evidence.get("graph_layers"), "evidence-relation"),
        "source-evidence view must include the evidence-relation layer",
    )?;
    require(
        evidence
            .get("source_posture")
            .and_then(Value::as_str)
            .is_some_and(|s| s.contains("research packets remain preparation")),
        "source-evidence view must preserve research-packet/source-witness boundary",
    )?;
    let canon = view(&views, "canon-promotion")?;
    require(
        list_has(canon.get("graph_layers"), "candidate-relation")
            && list_has(canon.get("graph_layers"), "canonical-relation"),
        "canon-promotion view must bridge candidate and canonical graph layers",
    )?;
    require(
        list_has(
            canon
                .get("collapse_rule")
                .and_then(|v| v.get("default_cluster_kinds")),
            "canon-candidate-status",
        ),
        "canon-promotion view must collapse by canon/candidate status",
    )?;
    check()
}
pub fn run(root: &Path, cancel: &AtomicI32) -> io::Result<i32> {
    match run_validation(root, cancel) {
        Ok(()) => {
            writeln!(
                io::stdout().lock(),
                "[ok] validated ToS/derived-exports/philosophy_graph_views.min.json"
            )?;
            Ok(0)
        }
        Err(error) => {
            let signal = cancel.load(Ordering::Relaxed);
            if signal != 0 {
                return Ok(128 + signal);
            }
            let message = error.to_string();
            OutputBudget::new().reserve(
                message
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("graph views diagnostic size overflow"))?,
            )?;
            writeln!(io::stderr().lock(), "{message}")?;
            Ok(1)
        }
    }
}
