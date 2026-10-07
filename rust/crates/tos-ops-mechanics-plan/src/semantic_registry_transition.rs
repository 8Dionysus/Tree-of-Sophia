//! Exact, read-only semantic-registry transition gate. Current authored bytes
//! and immutable baseline objects remain separate. This gate grants no meaning,
//! assessment, reader execution, publication, or semantic admission.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use jsonschema::{Draft, Registry};
use serde_json::{Value, json};
use tos_foundation::Digest256;
use tos_validation::item_rules::ItemLimits;
use tos_validation::semantic_registry_rules::{
    decode_snapshot_member, validate_semantic_registries,
};

use crate::executor::{Limits, capture_ci_git};

const BASELINE_ENV: &str = "TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT";
const INTRO_ENV: &str = "TOS_SEMANTIC_REGISTRY_ALLOW_INITIAL_INTRODUCTION";
const REGISTRIES: [&str; 2] = [
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];
const SCHEMAS: [&str; 2] = [
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
];
const READER: &str = "scripts/source_record_profiles.py";
const MEMBER_BYTES: usize = 1_048_576;
const STATE_BYTES: usize = 64 * 1_048_576;
const TOTAL_GIT_BYTES: usize = 16 * 1_048_576;
const HELP: &str = "choose the exact pre-change commit, ensure its object is available locally, then pass --baseline-commit FULL_COMMIT_OID or set TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT=FULL_COMMIT_OID before tos-validation-lanes --repo-root ABSOLUTE_REPO --run semantic_registry_transition";

fn invalid(text: impl Into<String>) -> io::Error {
    io::Error::other(text.into())
}
struct Gate<'a> {
    root: &'a Path,
    root_dir: File,
    limits: Limits,
    deadline: Instant,
    cancel: &'a AtomicI32,
    git_bytes: usize,
    source_bytes: usize,
    retained: usize,
}
impl Gate<'_> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 {
            return Err(invalid("semantic registry transition cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(invalid("semantic registry transition deadline exceeded"));
        }
        Ok(())
    }
    fn item_limits(&self) -> ItemLimits {
        ItemLimits {
            max_member_bytes: MEMBER_BYTES,
            max_total_bytes: 8 * MEMBER_BYTES as u64,
            max_state_bytes: STATE_BYTES - self.retained.min(STATE_BYTES),
            max_issues: 4096,
            deadline: self.deadline,
        }
    }
    fn git(&mut self, args: &[&str]) -> io::Result<Vec<u8>> {
        self.check()?;
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let wall = remaining.min(Duration::from_secs(30));
        if wall.is_zero() {
            return Err(invalid("semantic registry Git deadline exceeded"));
        }
        let mut argv = vec!["/usr/bin/env".into()];
        let mut trace_name_bytes = 0usize;
        // Trace destinations are writes, including ordinary environment-driven
        // Git trace2. Disable these only in this child, never globally.
        for (key, _) in std::env::vars_os() {
            if key.as_encoded_bytes().starts_with(b"GIT_TRACE") {
                trace_name_bytes = trace_name_bytes
                    .checked_add(key.as_encoded_bytes().len())
                    .ok_or_else(|| invalid("Git environment accounting overflow"))?;
                if trace_name_bytes > 4096 || argv.len() > 256 {
                    return Err(invalid("Git trace environment exceeds gate argument limit"));
                }
                argv.push("-u".into());
                argv.push(
                    key.into_string()
                        .map_err(|_| invalid("non-UTF8 Git trace environment name"))?,
                );
            }
        }
        argv.extend(
            [
                "--",
                "GIT_NO_LAZY_FETCH=1",
                "GIT_OPTIONAL_LOCKS=0",
                "git",
                "--no-replace-objects",
                "-C",
            ]
            .map(str::to_owned),
        );
        argv.push(
            self.root
                .to_str()
                .ok_or_else(|| invalid("Git root must be UTF-8"))?
                .into(),
        );
        argv.extend(args.iter().map(|arg| (*arg).into()));
        let cap = self
            .limits
            .output_bytes
            .min(2 * MEMBER_BYTES)
            .min(TOTAL_GIT_BYTES.saturating_sub(self.git_bytes));
        if cap == 0 {
            return Err(invalid(
                "semantic registry cumulative Git output limit exceeded",
            ));
        }
        let limits = Limits {
            command_wall: wall,
            lane_wall: wall,
            cleanup_grace: self.limits.cleanup_grace.min(Duration::from_secs(1)),
            output_bytes: cap,
        };
        let (code, stdout, stderr) = capture_ci_git(self.root, argv, limits, self.cancel)?;
        self.git_bytes = self
            .git_bytes
            .checked_add(stdout.len())
            .and_then(|n| n.checked_add(stderr.len()))
            .ok_or_else(|| invalid("Git output accounting overflow"))?;
        self.check()?;
        if stdout.len() > MEMBER_BYTES + 1 {
            return Err(invalid(
                "semantic registry Git query stdout exceeds 1 MiB limit",
            ));
        }
        if code != 0 {
            return Err(invalid(format!(
                "cannot read exact Git baseline: {}",
                String::from_utf8_lossy(&stderr).trim()
            )));
        }
        Ok(stdout)
    }
    fn text_git(&mut self, args: &[&str]) -> io::Result<String> {
        String::from_utf8(self.git(args)?).map_err(|_| invalid("Git metadata output is not UTF-8"))
    }
    fn read(&mut self, reference: &str, commit: Option<&str>) -> io::Result<Vec<u8>> {
        self.check()?;
        let raw = if let Some(commit) = commit {
            let object = format!("{commit}:{reference}");
            let size = self
                .text_git(&["cat-file", "-s", &object])?
                .trim()
                .parse::<u64>()
                .map_err(|_| invalid("invalid Git blob size"))?;
            if size > MEMBER_BYTES as u64 {
                return Err(invalid(format!(
                    "baseline {reference} exceeds the 1 MiB metadata limit"
                )));
            }
            self.git(&["cat-file", "blob", &object])?
        } else {
            self.read_current(reference)?
        };
        if raw.len() > MEMBER_BYTES {
            return Err(invalid(format!(
                "{reference} exceeds the 1 MiB metadata limit"
            )));
        }
        self.source_bytes = self
            .source_bytes
            .checked_add(raw.len())
            .ok_or_else(|| invalid("snapshot byte accounting overflow"))?;
        if self.source_bytes > 8 * MEMBER_BYTES {
            return Err(invalid("semantic registry snapshot bytes exceed 8 MiB"));
        }
        self.check()?;
        Ok(raw)
    }
    // The four fixed member names are walked from one retained root. Every
    // parent and final inode is selected without following symlinks; nonblock
    // prevents a final FIFO swap from hanging before the regular-file check.
    #[cfg(target_os = "linux")]
    fn read_current(&self, reference: &str) -> io::Result<Vec<u8>> {
        use std::os::fd::{AsRawFd, FromRawFd};
        if !REGISTRIES.contains(&reference) && !SCHEMAS.contains(&reference) {
            return Err(invalid("unknown registry gate member"));
        }
        let mut directory = self.root_dir.try_clone()?;
        let mut parts = reference.split('/').peekable();
        while let Some(part) = parts.next() {
            self.check()?;
            let name = std::ffi::CString::new(part).map_err(|_| invalid("NUL registry member"))?;
            let final_part = parts.peek().is_none();
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if final_part { 0 } else { libc::O_DIRECTORY };
            let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(invalid(format!(
                    "current {reference}: {}",
                    io::Error::last_os_error()
                )));
            }
            let file = unsafe { File::from_raw_fd(fd) };
            if !final_part {
                directory = file;
                continue;
            }
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                return Err(invalid(format!(
                    "current {reference} must be a regular metadata file"
                )));
            }
            if metadata.len() > MEMBER_BYTES as u64 {
                return Err(invalid(format!(
                    "current {reference} exceeds the 1 MiB metadata limit"
                )));
            }
            self.check()?;
            // Size/regular admission precedes allocation. One fixed payload
            // workspace also detects growth past the cap without Vec regrowth.
            let mut raw = Vec::new();
            raw.try_reserve_exact(MEMBER_BYTES + 1)
                .map_err(|_| invalid("registry member buffer allocation refused"))?;
            file.take(MEMBER_BYTES as u64 + 1).read_to_end(&mut raw)?;
            if raw.len() > MEMBER_BYTES {
                return Err(invalid(format!(
                    "{reference} exceeds the 1 MiB metadata limit"
                )));
            }
            self.check()?;
            return Ok(raw);
        }
        Err(invalid("empty registry member"))
    }
    #[cfg(not(target_os = "linux"))]
    fn read_current(&self, _reference: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "registry gate requires Linux descriptor custody",
        ))
    }
    fn decode(
        &self,
        raw: &[u8],
        label: &str,
        live_pair_state: usize,
    ) -> io::Result<(Value, usize)> {
        self.check()?;
        // Decoder/rule boundaries carry the actual signal. Foundation parsing
        // and schema compilation/evaluation are finite in-process calls; the
        // outer strict runner owns the hard wall during those calls.
        let mut limits = self.item_limits();
        limits.max_state_bytes = limits
            .max_state_bytes
            .checked_sub(live_pair_state)
            .ok_or_else(|| invalid("semantic registry live pair state exceeded"))?;
        decode_snapshot_member(raw, limits, self.cancel)
            .map_err(|e| invalid(format!("{label}: {e:?}")))
    }
    fn validate_rules(
        &self,
        current: &[Value; 2],
        previous: Option<&[Value; 2]>,
    ) -> io::Result<Value> {
        self.check()?;
        let limits = ItemLimits {
            max_state_bytes: STATE_BYTES,
            ..self.item_limits()
        };
        let result = validate_semantic_registries(
            &current[0],
            &current[1],
            previous.map(|p| (&p[0], &p[1])),
            limits,
            self.cancel,
        )
        .map_err(|e| invalid(format!("semantic registry invariants: {e:?}")))?;
        self.check()?;
        Ok(result)
    }
    fn snapshot(
        &mut self,
        commit: Option<&str>,
    ) -> io::Result<([Value; 2], BTreeMap<String, String>)> {
        let label = commit
            .map(|c| format!("baseline {c}"))
            .unwrap_or_else(|| "current working tree".into());
        let mut records = Vec::with_capacity(2);
        let mut digests = BTreeMap::new();
        for (reference, schema_ref) in REGISTRIES.iter().zip(SCHEMAS) {
            let raw = self.read(reference, commit)?;
            let schema_raw = self.read(schema_ref, commit)?;
            let (value, state) = self.decode(&raw, &format!("{label}:{reference}"), 0)?;
            let (schema, schema_state) =
                self.decode(&schema_raw, &format!("{label}:{schema_ref}"), state)?;
            if self
                .retained
                .checked_add(state)
                .and_then(|n| n.checked_add(schema_state))
                .is_none_or(|n| n > STATE_BYTES)
            {
                return Err(invalid(
                    "semantic registry retained snapshot state exceeded",
                ));
            }
            self.check()?;
            // Exactly the maintained Draft202012/no FormatChecker/empty
            // registry route; external schema retrieval remains unavailable.
            let registry = Registry::new()
                .prepare()
                .map_err(|e| invalid(format!("schema registry: {e}")))?;
            let validator = jsonschema::options()
                .with_draft(Draft::Draft202012)
                .with_registry(&registry)
                .should_validate_formats(false)
                .offline()
                .build(&schema)
                .map_err(|e| invalid(format!("{label}:{schema_ref}: invalid JSON Schema: {e}")))?;
            self.check()?;
            if let Some(error) = validator.iter_errors(&value).next() {
                let location = error
                    .instance_path()
                    .segments()
                    .map(|part| part.to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                return Err(invalid(format!(
                    "{label}:{reference}:{}: {error}",
                    if location.is_empty() {
                        "<root>"
                    } else {
                        &location
                    }
                )));
            }
            self.check()?;
            drop(validator);
            drop(registry);
            drop(schema);
            self.retained += state;
            records.push(value);
            digests.insert((*reference).into(), Digest256::of_bytes(&raw).to_hex());
            digests.insert(schema_ref.into(), Digest256::of_bytes(&schema_raw).to_hex());
        }
        let records = records
            .try_into()
            .map_err(|_| invalid("registry pair arity"))?;
        Ok((records, digests))
    }
}

// Same retained-parent descriptor pattern as the existing RouteSources
// owner, specialized here to one gate root and its four fixed input members.
#[cfg(target_os = "linux")]
fn open_root(root: &Path) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    if !root.is_absolute() || root.as_os_str().len() > 4096 || root.components().count() > 128 {
        return Err(invalid("invalid registry gate root"));
    }
    let mut directory = File::open("/")?;
    for part in root.components() {
        if let Component::Normal(part) = part {
            let name = std::ffi::CString::new(part.as_bytes())
                .map_err(|_| invalid("NUL registry root"))?;
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            directory = unsafe { File::from_raw_fd(fd) };
        }
    }
    Ok(directory)
}
#[cfg(not(target_os = "linux"))]
fn open_root(_root: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "registry gate requires Linux descriptor custody",
    ))
}

fn transition(gate: &mut Gate<'_>, commit: &str, allow: bool) -> io::Result<Value> {
    if !matches!(commit.len(), 40 | 64)
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || commit.bytes().all(|b| b == b'0')
    {
        return Err(invalid(format!(
            "missing or invalid baseline: a nonzero full commit OID is required; {HELP}"
        )));
    }
    let top = gate.text_git(&["rev-parse", "--show-toplevel"])?;
    if Path::new(top.trim()).canonicalize()? != gate.root {
        return Err(invalid("repo root must be the exact Git worktree root"));
    }
    let kind = gate
        .text_git(&["cat-file", "-t", commit])
        .map_err(|e| invalid(format!("{e}; {HELP}")))?;
    if kind.trim() != "commit" {
        return Err(invalid(format!(
            "baseline {commit} is not a commit object; {HELP}"
        )));
    }
    let required = [REGISTRIES[0], REGISTRIES[1], SCHEMAS[0], SCHEMAS[1]];
    let mut args = vec!["ls-tree", "--name-only", commit, "--"];
    args.extend(required);
    let present = gate.text_git(&args)?;
    let present: BTreeSet<_> = present.lines().collect();
    let introduction = present.is_empty();
    if !introduction && present != required.into_iter().collect() {
        return Err(invalid(
            "partial baseline registry/contract snapshot; absent objects cannot be treated as an initial introduction",
        ));
    }
    let previous = if introduction {
        if !allow {
            return Err(invalid(format!(
                "baseline has no registries/contracts; initial introduction requires explicit --allow-initial-introduction or {INTRO_ENV}=1"
            )));
        }
        if gate
            .text_git(&["rev-parse", "--is-shallow-repository"])?
            .trim()
            != "false"
        {
            return Err(invalid(
                "initial introduction requires complete local baseline ancestry, not a shallow history",
            ));
        }
        let graft = gate.text_git(&["rev-parse", "--git-path", "info/grafts"])?;
        let graft = gate.root.join(graft.trim());
        if std::env::var_os("GIT_GRAFT_FILE").is_some() || fs::symlink_metadata(graft).is_ok() {
            return Err(invalid(
                "initial introduction requires unmodified baseline ancestry; Git grafts are not allowed",
            ));
        }
        let mut args = vec!["rev-list", "--max-count=1", commit, "--"];
        args.extend(required);
        args.push(READER);
        if !gate.text_git(&args)?.trim().is_empty() {
            return Err(invalid(
                "baseline history already contains a registry, contract or declared-profile reader; initial introduction denied",
            ));
        }
        None
    } else {
        let (records, digests) = gate.snapshot(Some(commit))?;
        let validation = gate.validate_rules(&records, None)?;
        if validation["valid"] != true {
            let issues = validation["violations"]
                .as_array()
                .ok_or_else(|| invalid("registry issue representation"))?
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(invalid(format!(
                "baseline registry invariants failed: {issues}"
            )));
        }
        Some((records, digests))
    };
    let (current, digests) = gate.snapshot(None)?;
    let mut report = gate.validate_rules(&current, previous.as_ref().map(|p| &p.0))?;
    let object = report
        .as_object_mut()
        .ok_or_else(|| invalid("registry report root"))?;
    object.extend([
        (
            "transition_kind".into(),
            json!(if introduction {
                "initial-introduction"
            } else {
                "registry-evolution"
            }),
        ),
        ("compared_previous_registry".into(), json!(!introduction)),
        (
            "initial_introduction_explicitly_allowed".into(),
            json!(introduction && allow),
        ),
        ("baseline_commit".into(), json!(commit)),
        (
            "baseline_sha256".into(),
            json!(previous.map(|p| p.1).unwrap_or_default()),
        ),
        (
            "baseline_absent_refs".into(),
            json!(if introduction {
                let mut refs = required.to_vec();
                refs.push(READER);
                refs
            } else {
                Vec::new()
            }),
        ),
        ("current_source".into(), json!("working-tree")),
        ("current_sha256".into(), json!(digests)),
        ("semantic_acceptance".into(), json!(false)),
    ]);
    gate.check()?;
    Ok(report)
}

// json.dumps(...ensure_ascii=False,sort_keys=True) uses these spaces. This
// formatter changes only separators; values remain inert native JSON data.
struct PythonReportFormatter;
impl serde_json::ser::Formatter for PythonReportFormatter {
    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b", ")?;
        }
        Ok(())
    }
    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b", ")?;
        }
        Ok(())
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }
}
struct BoundedOutput {
    bytes: Vec<u8>,
    cap: usize,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.cap.saturating_sub(self.bytes.len()) {
            return Err(invalid("semantic registry report output limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn emit_error(error: &dyn std::fmt::Display, limit: usize) {
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        cap: limit.min(MEMBER_BYTES),
    };
    let _ = write!(
        &mut output,
        "[error] semantic registry transition: {error}\n"
    );
    let _ = io::stderr().lock().write_all(&output.bytes);
}

/// Explicit native entry, preserving the maintained baseline and introduction
/// env rules. Public mechanics limits can tighten, never expand this profile.
/// Linux execution and hard schema CPU wall remain the outer owner's custody.
pub fn run(
    root: &Path,
    baseline_commit: Option<&str>,
    allow_initial_introduction: bool,
    json_output: bool,
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<i32> {
    let attempt = || -> io::Result<(Value, bool)> {
        let introduction = std::env::var_os(INTRO_ENV).unwrap_or_else(|| "0".into());
        if introduction != "0" && introduction != "1" {
            return Err(invalid(format!("{INTRO_ENV} must be exactly 0 or 1")));
        }
        let environment = std::env::var_os(BASELINE_ENV);
        let commit = match baseline_commit {
            Some(commit) => {
                if commit.len() > 4096 {
                    return Err(invalid("baseline argument exceeds 4096 bytes"));
                }
                commit.to_owned()
            }
            None => environment
                .map(|v| {
                    if v.as_encoded_bytes().len() > 4096 {
                        return Err(invalid("baseline environment exceeds 4096 bytes"));
                    }
                    v.into_string()
                        .map_err(|_| invalid("baseline environment must be UTF-8"))
                })
                .transpose()?
                .unwrap_or_default(),
        };
        let wall = limits
            .command_wall
            .min(limits.lane_wall)
            .min(Duration::from_secs(300));
        if wall.is_zero() || limits.output_bytes == 0 {
            return Err(invalid(
                "semantic registry time/output limits must be nonzero",
            ));
        }
        let deadline = Instant::now()
            .checked_add(wall)
            .ok_or_else(|| invalid("semantic registry deadline overflow"))?;
        let root = root.canonicalize()?;
        let mut gate = Gate {
            root: &root,
            root_dir: open_root(&root)?,
            limits,
            deadline,
            cancel,
            git_bytes: 0,
            source_bytes: 0,
            retained: 0,
        };
        let report = transition(
            &mut gate,
            &commit,
            allow_initial_introduction || introduction == "1",
        )?;
        gate.check()?;
        Ok((report, json_output))
    };
    let (report, json_output) = match attempt() {
        Ok(value) => value,
        Err(error) => {
            emit_error(&error, limits.output_bytes);
            let signal = cancel.load(Ordering::Relaxed);
            return Ok(if signal != 0 { 128 + signal } else { 1 });
        }
    };
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        cap: limits.output_bytes.min(MEMBER_BYTES),
    };
    let output_result = if json_output {
        use serde::Serialize;
        report
            .serialize(&mut serde_json::Serializer::with_formatter(
                &mut output,
                PythonReportFormatter,
            ))
            .map_err(io::Error::other)
            .and_then(|_| output.write_all(b"\n"))
    } else {
        writeln!(
            &mut output,
            "[{}] semantic registry {}: exact baseline {}, current working-tree bytes",
            if report["valid"] == true {
                "ok"
            } else {
                "error"
            },
            report["transition_kind"].as_str().unwrap_or(""),
            report["baseline_commit"].as_str().unwrap_or("")
        )
    };
    if let Err(error) = output_result {
        emit_error(&error, limits.output_bytes);
        return Ok(1);
    }
    let signal = cancel.load(Ordering::Relaxed);
    if signal != 0 {
        return Ok(128 + signal);
    }
    io::stdout().lock().write_all(&output.bytes)?;
    if !json_output {
        let mut stderr = BoundedOutput {
            bytes: Vec::new(),
            cap: limits
                .output_bytes
                .min(MEMBER_BYTES)
                .saturating_sub(output.bytes.len()),
        };
        for issue in report["violations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if let Err(error) = writeln!(&mut stderr, "- {issue}") {
                emit_error(&error, limits.output_bytes);
                return Ok(1);
            }
        }
        io::stderr().lock().write_all(&stderr.bytes)?;
    }
    let signal = cancel.load(Ordering::Relaxed);
    Ok(if signal != 0 {
        128 + signal
    } else if report["valid"] == true {
        0
    } else {
        1
    })
}
