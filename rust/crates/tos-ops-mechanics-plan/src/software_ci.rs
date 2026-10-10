//! Candidate for the maintained software CI selector and required gate.
//! It selects checks; it cannot run them or accept a release.

use crate::executor::{self, Limits};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

const MAX_PATHS: usize = 4096;
const MAX_PATH_BYTES: usize = 4 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const MODES: [&str; 4] = ["none", "browser", "reader", "full"];

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Selection {
    pub schema_version: u8,
    pub software_mode: &'static str,
    pub worker: bool,
    pub rust: bool,
    pub changed_paths: Vec<String>,
    pub forced_full: bool,
}

/// The ordered path rules are the maintained software selector, including
/// unknown/shared sources and empty selection requiring all checks.
pub fn select(paths: Vec<String>, force_full: bool) -> io::Result<Selection> {
    if paths.len() > MAX_PATHS
        || paths
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.len()))
            .filter(|n| *n <= MAX_PATH_BYTES)
            .is_none()
    {
        return Err(invalid("software CI changed-path budget exceeded"));
    }
    let (mut mode, mut worker, mut rust) = (0usize, false, false);
    for path in &paths {
        // PurePosixPath drops empty and '.' components; backslash is literal.
        let parts: Vec<_> = path
            .split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .collect();
        if path.starts_with('/') || parts.contains(&"..") || path.contains('\n') {
            return Err(invalid(format!("invalid Git path: {path:?}")));
        }
        let name = parts.last().copied().unwrap_or("");
        let suffix = name
            .rsplit_once('.')
            .filter(|(stem, tail)| stem.chars().any(|c| c != '.') && !tail.is_empty())
            .map(|(_, tail)| tail);
        if suffix == Some("md")
            && name != "AGENTS.md"
            && (parts.len() == 1 || path.starts_with("docs/") || path.starts_with("access/"))
        {
            continue;
        }
        if matches!(
            path.as_str(),
            "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml"
        ) || path.starts_with("rust/")
            || path.starts_with("tests/conformance/rust/")
        {
            rust = true;
            mode = mode.max(1);
        } else if path.starts_with("access/deploy/cloudflare-worker/") {
            worker = true;
        } else if path.starts_with("access/web/") || path.starts_with("access/e2e/") {
            mode = mode.max(1);
        } else if path.starts_with("access/src/") || path.starts_with("access/tests/") {
            mode = mode.max(2);
            worker = true;
        } else {
            (mode, worker, rust) = (3, true, true);
        }
    }
    let forced_full = force_full || paths.is_empty();
    if forced_full {
        (mode, worker, rust) = (3, true, true);
    }
    Ok(Selection {
        schema_version: 2,
        software_mode: MODES[mode],
        worker,
        rust,
        changed_paths: paths,
        forced_full,
    })
}

/// Failure/cancellation/missing and unexpected skipping are never accepted.
pub fn gate(needs: &Value) -> io::Result<()> {
    let plan = needs.get("plan");
    if plan.and_then(|p| p.get("result")).and_then(Value::as_str) != Some("success") {
        return Err(invalid(
            "check selection or documentation validation did not succeed",
        ));
    }
    let outputs = plan.and_then(|p| p.get("outputs"));
    let mode = outputs
        .and_then(|o| o.get("software_mode"))
        .and_then(Value::as_str);
    let worker = outputs
        .and_then(|o| o.get("worker"))
        .and_then(Value::as_str);
    let rust = outputs.and_then(|o| o.get("rust")).and_then(Value::as_str);
    if !mode.is_some_and(|m| MODES.contains(&m))
        || !matches!(worker, Some("true" | "false"))
        || !matches!(rust, Some("true" | "false"))
    {
        return Err(invalid("missing or invalid check selection"));
    }
    for (job, required) in [
        ("software", mode != Some("none")),
        ("worker", worker == Some("true")),
        ("rust", rust == Some("true")),
    ] {
        let expected = if required { "success" } else { "skipped" };
        if needs
            .get(job)
            .and_then(|j| j.get("result"))
            .and_then(Value::as_str)
            != Some(expected)
        {
            return Err(invalid(format!("{job}: expected {expected}")));
        }
    }
    Ok(())
}

struct Docs {
    fence: Regex,
    link: Regex,
    reference: Regex,
    marker: Regex,
    scheme: Regex,
}
impl Docs {
    fn new() -> io::Result<Self> {
        // Same pinned Python-16 whitespace law as mechanics topology.
        const SPACE: &str = r"\x09-\x0D\x1C-\x20\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";
        let re = |s: &str| Regex::new(&s.replace("@W@", SPACE)).map_err(|e| invalid(e.to_string()));
        Ok(Self {
            fence: re(r"^[@W@]{0,3}(`{3,}|~{3,})")?,
            link: re(
                r#"!?\[[^\]\n]*\]\([@W@]*(?:<([^>]+)>|([^@W@)]+))(?:[@W@]+['\"][^\n]*?['\"])?[@W@]*\)"#,
            )?,
            reference: re(r"(?m)^[@W@]*\[[^\]\n]+\]:[@W@]*<?([^@W@>]+)>?")?,
            marker: re(r"(?m)^(<<<<<<< |>>>>>>> )")?,
            scheme: re(r"^[A-Za-z][A-Za-z0-9+.-]*:")?,
        })
    }
    fn links(&self, text: &str) -> BTreeSet<String> {
        let mut visible = String::with_capacity(text.len());
        let mut fence: Option<(u8, usize)> = None;
        for line in text.lines().flat_map(|line| {
            line.split([
                '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
                '\u{2029}',
            ])
        }) {
            if let Some(capture) = self.fence.captures(line) {
                let marker = capture.get(1).unwrap().as_str();
                match fence {
                    None => fence = Some((marker.as_bytes()[0], marker.len())),
                    Some((kind, length))
                        if marker.as_bytes()[0] == kind && marker.len() >= length =>
                    {
                        fence = None
                    }
                    _ => (),
                }
                continue;
            }
            if fence.is_none() {
                visible.push_str(line);
                visible.push('\n');
            }
        }
        self.link
            .captures_iter(&visible)
            .map(|c| c.get(1).or_else(|| c.get(2)).unwrap().as_str().to_owned())
            .chain(
                self.reference
                    .captures_iter(&visible)
                    .map(|c| c[1].to_owned()),
            )
            .collect()
    }
    fn local_path(&self, href: &str) -> io::Result<Option<String>> {
        // urllib.urlsplit removes CR/LF/TAB and leading C0/space. Only its
        // local pathname is consumed, never network content. Keep its unique
        // malformed-netloc checks before deciding an external URL is skipped.
        let url: String = href
            .trim_start_matches(|c: char| c <= '\u{20}')
            .chars()
            .filter(|c| !matches!(c, '\r' | '\n' | '\t'))
            .collect();
        let scheme = self.scheme.find(&url);
        let remainder = scheme.as_ref().map(|m| &url[m.end()..]).unwrap_or(&url);
        if let Some(authority) = remainder.strip_prefix("//") {
            let netloc = authority.split(['/', '?', '#']).next().unwrap_or("");
            check_netloc(netloc)?;
            return Ok(None);
        }
        if scheme.is_some() {
            return Ok(None);
        }
        let path = url.split(['?', '#']).next().unwrap_or("");
        if path.is_empty() || path.starts_with('/') {
            return Ok(None);
        }
        let decoded = unquote(path);
        if decoded.contains('\0') {
            return Err(invalid("embedded null byte in link"));
        }
        Ok(Some(decoded))
    }
}

fn check_netloc(netloc: &str) -> io::Result<()> {
    // Complete Unicode-16 scalar set whose NFKC decomposition includes the
    // five reserved netloc separators. Derived from the pinned Python oracle;
    // this narrow check avoids introducing another normalization dependency.
    if netloc.chars().any(|c| {
        matches!(
            c,
            '\u{2047}'
                ..='\u{2049}'
                    | '\u{2100}'
                    | '\u{2101}'
                    | '\u{2105}'
                    | '\u{2106}'
                    | '\u{2a74}'
                    | '\u{fe13}'
                    | '\u{fe16}'
                    | '\u{fe55}'
                    | '\u{fe56}'
                    | '\u{fe5f}'
                    | '\u{fe6b}'
                    | '\u{ff03}'
                    | '\u{ff0f}'
                    | '\u{ff1a}'
                    | '\u{ff1f}'
                    | '\u{ff20}'
        )
    }) {
        return Err(invalid(
            "netloc contains invalid characters under NFKC normalization",
        ));
    }
    if !netloc.contains(['[', ']']) {
        return Ok(());
    }
    if !(netloc.contains('[') && netloc.contains(']')) {
        return Err(invalid("Invalid IPv6 URL"));
    }
    let host = netloc.rsplit('@').next().unwrap();
    let Some(bracketed) = host.strip_prefix('[') else {
        return Err(invalid("Invalid IPv6 URL"));
    };
    let (address, port) = bracketed
        .split_once(']')
        .ok_or_else(|| invalid("Invalid IPv6 URL"))?;
    if !port.is_empty() && !port.starts_with(':') {
        return Err(invalid("Invalid IPv6 URL"));
    }
    if address.starts_with(['v', 'V']) {
        let (version, future) = address[1..]
            .split_once('.')
            .ok_or_else(|| invalid("IPvFuture address is invalid"))?;
        if version.is_empty()
            || !version.bytes().all(|c| c.is_ascii_hexdigit())
            || future.is_empty()
        {
            return Err(invalid("IPvFuture address is invalid"));
        }
    } else {
        let bare = if let Some((ip, scope)) = address.split_once('%') {
            if scope.is_empty() || scope.contains('%') {
                return Err(invalid("Invalid IPv6 scope"));
            }
            ip
        } else {
            address
        };
        bare.parse::<std::net::Ipv6Addr>()
            .map_err(|_| invalid("Invalid IPv6 URL"))?;
    }
    Ok(())
}

fn unquote(value: &str) -> String {
    fn hex(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }
    let bytes = value.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                result.push(a * 16 + b);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

struct Budget<'a> {
    deadline: Instant,
    bytes: usize,
    cancel: &'a AtomicI32,
}
impl Budget<'_> {
    fn charge(&mut self, bytes: usize) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 {
            return Err(invalid("software CI cancelled"));
        }
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("CI byte overflow"))?;
        if self.bytes > MAX_TOTAL_BYTES {
            return Err(invalid("software CI aggregate input budget exceeded"));
        }
        if Instant::now() >= self.deadline {
            return Err(invalid("software CI whole wall deadline"));
        }
        Ok(())
    }
    fn git(&mut self, root: &Path, parts: &[&str], cap: usize) -> io::Result<(i32, Vec<u8>)> {
        self.charge(0)?;
        let wall = self.deadline.saturating_duration_since(Instant::now());
        let argv = std::iter::once("git")
            .chain(parts.iter().copied())
            .map(str::to_owned)
            .collect();
        let (code, stdout, stderr) = executor::capture_ci_git(
            root,
            argv,
            Limits {
                command_wall: wall.min(Duration::from_secs(30)),
                lane_wall: wall,
                cleanup_grace: Duration::from_secs(1),
                output_bytes: cap.min(MAX_TOTAL_BYTES - self.bytes),
            },
            self.cancel,
        )?;
        self.charge(stdout.len() + stderr.len())?;
        Ok((code, stdout))
    }
}

fn read_doc(path: &Path, remaining: usize) -> io::Result<String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > MAX_DOCUMENT_BYTES.min(remaining) as u64 {
        return Err(invalid("CI Markdown must be a bounded regular file"));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    let cap = MAX_DOCUMENT_BYTES.min(remaining);
    file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > cap || bytes.len() as u64 != meta.len() {
        return Err(invalid("CI Markdown changed or exceeded its byte budget"));
    }
    String::from_utf8(bytes).map_err(|e| invalid(e.to_string()))
}

/// Actual Git path selection plus current/new Markdown link validation.
/// No rename detection: deletion and addition both influence the final mode.
pub fn plan(
    root: &Path,
    base: &str,
    force_full: bool,
    cancel: &AtomicI32,
) -> io::Result<Selection> {
    if base.is_empty() || base.starts_with('-') || base.len() > 4096 || base.contains('\0') {
        return Err(invalid("invalid base ref"));
    }
    let mut budget = Budget {
        deadline: Instant::now() + Duration::from_secs(120),
        bytes: 0,
        cancel,
    };
    let (code, bytes) = budget.git(
        root,
        &["diff", "--name-only", "--no-renames", "-z", base, "HEAD"],
        MAX_PATH_BYTES,
    )?;
    if code != 0 {
        return Err(invalid(format!(
            "Git changed-path selection failed with exit code {code}"
        )));
    }
    let mut paths = BTreeSet::new();
    for path in bytes.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        if paths.len() >= MAX_PATHS {
            return Err(invalid("CI changed-path count exceeded"));
        }
        paths.insert(
            std::str::from_utf8(path)
                .map_err(|e| invalid(e.to_string()))?
                .to_owned(),
        );
    }
    let selection = select(paths.into_iter().collect(), force_full)?;
    let docs = Docs::new()?;
    let mut errors = Vec::new();
    for name in &selection.changed_paths {
        let target = root.join(name);
        if !name.ends_with(".md") || !target.is_file() {
            continue;
        }
        budget.charge(0)?;
        let content = read_doc(&target, MAX_TOTAL_BYTES - budget.bytes)?;
        budget.charge(content.len())?;
        if docs.marker.is_match(&content) {
            errors.push(format!("{name}: unresolved merge marker"));
        }
        let object = format!("{base}:{name}");
        let (code, previous) = budget.git(root, &["show", &object], MAX_DOCUMENT_BYTES)?;
        let old = if code == 0 {
            docs.links(std::str::from_utf8(&previous).map_err(|e| invalid(e.to_string()))?)
        } else {
            BTreeSet::new()
        };
        for href in docs.links(&content).difference(&old) {
            let Some(path) = docs.local_path(href)? else {
                continue;
            };
            let destination = target.parent().unwrap().join(path);
            if !fs::canonicalize(&destination).is_ok_and(|p| p.starts_with(root) && p.exists()) {
                errors.push(format!("{name}: missing repository link target {href}"));
            }
        }
    }
    budget.charge(0)?;
    if !errors.is_empty() {
        return Err(invalid(errors.join("\n")));
    }
    Ok(selection)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn omission_requires_a_known_surface_and_gate_requires_every_job() {
        // Reject option-shaped refs before any Git reader or filesystem call.
        assert!(
            plan(
                Path::new("/"),
                "--output=foreign",
                false,
                &AtomicI32::new(0)
            )
            .is_err()
        );
        for (paths, mode, worker, rust) in [
            (vec!["README.md", "docs/RELEASING.md"], "none", false, false),
            (vec!["access/web/src/Graph.tsx"], "browser", false, false),
            (vec!["access/e2e/test_webmcp.mjs"], "browser", false, false),
            (
                vec!["access/src/tos_access/native_access_core.py"],
                "reader",
                true,
                false,
            ),
            (
                vec!["access/tests/test_software_boundary.py"],
                "reader",
                true,
                false,
            ),
            (
                vec!["rust/crates/tos-access/src/software_archive.rs"],
                "browser",
                false,
                true,
            ),
            (
                vec!["access/deploy/cloudflare-worker/src/index.ts"],
                "none",
                true,
                false,
            ),
            (
                vec!["rust/crates/tos-foundation/src/lib.rs"],
                "browser",
                false,
                true,
            ),
            (
                vec!["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"],
                "browser",
                false,
                true,
            ),
            (
                vec![
                    "access/web/package-lock.json",
                    "access/deploy/cloudflare-worker/package.json",
                ],
                "browser",
                true,
                false,
            ),
        ] {
            let result = select(paths.into_iter().map(str::to_owned).collect(), false).unwrap();
            assert_eq!(
                (result.software_mode, result.worker, result.rust),
                (mode, worker, rust)
            );
        }
        for path in [
            "access/contracts/query-store.v1.json",
            "access/profiles/reader.json",
            "requirements-dev.txt",
            "pytest.ini",
            ".github/workflows/repo-validation.yml",
            "scripts/software_ci.py",
            "tests/test_software_ci.py",
            "AGENTS.md",
            "access/AGENTS.md",
            "ToS/doctrine/README.md",
            "new-unclassified/input.xyz",
            "...md",
        ] {
            let result = select(vec![path.into()], false).unwrap();
            assert_eq!(
                (result.software_mode, result.worker, result.rust),
                ("full", true, true),
                "{path}"
            );
        }
        assert!(select(Vec::new(), false).unwrap().forced_full);
        assert_eq!(
            select(vec!["README.md".into()], true)
                .unwrap()
                .software_mode,
            "full"
        );
        for path in ["/absolute", "a/../b", "a\nb"] {
            assert!(select(vec![path.into()], false).is_err());
        }
        for (mode, worker, rust) in [
            ("none", false, false),
            ("none", false, true),
            ("browser", false, false),
            ("reader", true, false),
            ("full", true, true),
            ("none", true, false),
        ] {
            let needs = serde_json::json!({"plan":{"result":"success","outputs":{"software_mode":mode,"worker":worker.to_string(),"rust":rust.to_string()}},
                "software":{"result":if mode=="none" {"skipped"} else {"success"}},
                "worker":{"result":if worker {"success"} else {"skipped"}},
                "rust":{"result":if rust {"success"} else {"skipped"}}});
            gate(&needs).unwrap();
            for job in ["plan", "software", "worker", "rust"] {
                for failure in [Some("failure"), Some("cancelled"), None] {
                    let mut bad = needs.clone();
                    if let Some(failure) = failure {
                        bad[job]["result"] = failure.into();
                    } else {
                        bad.as_object_mut().unwrap().remove(job);
                    }
                    assert!(gate(&bad).is_err());
                }
            }
        }
    }

    #[test]
    fn new_local_link_checks_keep_fences_unicode_and_url_refusals() {
        let docs = Docs::new().unwrap();
        assert_eq!(
            docs.links("```md\n[x](fake.md)\n```\n[x](real.md#part)\n[r]: other.md\n"),
            BTreeSet::from(["real.md#part".into(), "other.md".into()])
        );
        assert_eq!(
            docs.links("```md\u{85}[x](fake.md)\u{85}```\u{85}[x](real.md)"),
            BTreeSet::from(["real.md".into()])
        );
        assert_eq!(
            docs.links("[x](\u{1c}target.md\u{1c})"),
            BTreeSet::from(["target.md".into()])
        );
        assert_eq!(
            docs.local_path("doc%20name.md?query#part").unwrap(),
            Some("doc name.md".into())
        );
        for href in [
            "https://example.invalid/no-fetch",
            "//[::1]/x",
            "//[v1.name]/x",
            "/hosted",
            "#part",
        ] {
            assert!(docs.local_path(href).unwrap().is_none());
        }
        for href in [
            "https://[",
            "//host]/x",
            "//[127.0.0.1]/x",
            "//[name]/x",
            "https://℀/x",
            "a%00b",
        ] {
            assert!(docs.local_path(href).is_err(), "{href}");
        }
    }
}
