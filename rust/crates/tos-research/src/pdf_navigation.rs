//! Descriptor-bound PDF text navigation for curated source stages.
use crate::navigation_xml::{Node, descendants, node_text, xml_with_doctype};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs::File, os::fd::AsRawFd};
use tos_compiler::owned_native_child::{CaptureLimits, capture_with_cancel};
use tos_source_store::execution::Execution;

const EXISTING_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;

fn ensure(value: bool, message: &str) -> Result<(), String> {
    if value { Ok(()) } else { Err(message.into()) }
}
fn coordinate(node: &Node, name: &str) -> Result<f64, String> {
    let value = node
        .attrs
        .get(name)
        .ok_or("bbox coordinate absent")?
        .parse::<f64>()
        .map_err(|_| "invalid bbox coordinate")?;
    ensure(value.is_finite(), "bbox finite coordinate")?;
    Ok(value)
}
fn line_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    a["y_min"]
        .as_f64()
        .unwrap()
        .total_cmp(&b["y_min"].as_f64().unwrap())
        .then(
            a["x_min"]
                .as_f64()
                .unwrap()
                .total_cmp(&b["x_min"].as_f64().unwrap()),
        )
        .then(
            a["source_order"]
                .as_u64()
                .unwrap()
                .cmp(&b["source_order"].as_u64().unwrap()),
        )
}
fn bbox(
    ctx: &Execution,
    raw: &[u8],
    start: usize,
    end: usize,
) -> Result<BTreeMap<usize, Value>, String> {
    let root = xml_with_doctype(ctx, raw, true)?;
    ensure(
        root.name == "html"
            && root
                .attrs
                .get("xmlns")
                .is_some_and(|v| v == "http://www.w3.org/1999/xhtml"),
        "bbox XHTML namespace",
    )?;
    let mut selected = Vec::new();
    descendants(&root, "page", &mut selected);
    ensure(selected.len() == end - start + 1, "bbox page count drift")?;
    let mut pages = BTreeMap::new();
    for (index, page) in selected.iter().enumerate() {
        ctx.tick(1)?;
        let width = coordinate(page, "width")?;
        let height = coordinate(page, "height")?;
        ensure(width > 0.0 && height > 0.0, "bbox page size")?;
        let mut selected = Vec::new();
        descendants(page, "line", &mut selected);
        let mut lines = Vec::new();
        for (order, line) in selected.into_iter().enumerate() {
            let words = line
                .children("word")
                .into_iter()
                .map(node_text)
                .collect::<Vec<_>>();
            if words.is_empty() {
                continue;
            }
            let x_min = coordinate(line, "xMin")?;
            let y_min = coordinate(line, "yMin")?;
            let x_max = coordinate(line, "xMax")?;
            let y_max = coordinate(line, "yMax")?;
            ensure(x_min <= x_max && y_min <= y_max, "bbox line ordering")?;
            lines.push(json!({"source_order":order,"x_min":x_min,"y_min":y_min,"x_max":x_max,"y_max":y_max,"words":words,"text":words.join(" ")}));
        }
        lines.sort_by(line_order);
        pages.insert(
            start + index,
            json!({"page":start + index,"width":width,"height":height,"lines":lines}),
        );
    }
    Ok(pages)
}

pub struct Poppler {
    root: Execution,
    held: File,
    digest: String,
    command: String,
}
impl Poppler {
    /// Hold the selected executable and verify the recipe's exact version.
    pub fn open(ctx: &Execution, version: &str) -> Result<Self, String> {
        let binary = std::fs::canonicalize("/usr/bin/pdftotext").map_err(|e| e.to_string())?;
        let root = ctx.select_directory(binary.parent().ok_or("Poppler parent")?)?;
        let mut held = root.source_file(
            binary
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("Poppler filename")?,
            32 * 1024 * 1024,
        )?;
        let digest = root.hash_file(&mut held, 32 * 1024 * 1024)?;
        let command = format!("/proc/{}/fd/{}", std::process::id(), held.as_raw_fd());
        let result = capture_with_cancel(
            std::process::Command::new(&command).arg("-v"),
            None,
            CaptureLimits {
                max_stdin_bytes: 0,
                max_stdout_bytes: 16384,
                max_stderr_bytes: 16384,
            },
            ctx.deadline(),
            ctx.cancellation_flag(),
        )?;
        let mut bytes = result.stdout;
        bytes.extend(result.stderr);
        let output = String::from_utf8(bytes).map_err(|e| e.to_string())?;
        ensure(
            result.status.success()
                && output.lines().next() == Some(format!("pdftotext version {version}").as_str()),
            "Poppler exact version drift",
        )?;
        Ok(Self {
            root,
            held,
            digest,
            command,
        })
    }
    /// Read one bounded page batch through the held PDF and executable.
    pub fn pages(
        &self,
        ctx: &Execution,
        pdf: &File,
        start: usize,
        end: usize,
    ) -> Result<BTreeMap<usize, Value>, String> {
        ensure(
            start > 0 && end >= start && end - start < 8 && end <= 100000,
            "bbox page chunk bound",
        )?;
        let pdf = format!("/proc/{}/fd/{}", std::process::id(), pdf.as_raw_fd());
        let result = capture_with_cancel(
            std::process::Command::new(&self.command).args([
                "-f",
                &start.to_string(),
                "-l",
                &end.to_string(),
                "-bbox-layout",
                &pdf,
                "-",
            ]),
            None,
            CaptureLimits {
                max_stdin_bytes: 0,
                max_stdout_bytes: EXISTING_OUTPUT_LIMIT,
                max_stderr_bytes: 16384,
            },
            ctx.deadline(),
            ctx.cancellation_flag(),
        )?;
        ensure(result.status.success(), "Poppler bbox extraction failed")?;
        ctx.tick(result.stdout.len() as u64)?;
        bbox(ctx, &result.stdout, start, end)
    }
    pub fn verify(&mut self) -> Result<(), String> {
        ensure(
            self.root.hash_file(&mut self.held, 32 * 1024 * 1024)? == self.digest,
            "Poppler executable changed",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bbox_keeps_word_and_coordinate_order_and_rejects_invalid_bounds() {
        let ctx = Execution::new(&std::env::current_dir().unwrap(), 30).unwrap();
        let input = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><page width="300" height="500"><line xMin="10" yMin="30" xMax="20" yMax="40"><word>2</word></line><line xMin="10" yMin="10" xMax="20" yMax="20"><word>1</word></line></page></body></html>"#;
        let pages = bbox(&ctx, input, 1, 1).unwrap();
        assert_eq!(pages[&1]["lines"][0]["words"][0], "1");
        assert_eq!(pages[&1]["lines"][1]["words"][0], "2");
        assert!(bbox(&ctx, input, 1, 2).is_err());
    }
}
