//! Owner-selected existing Git capture transport; bytes never grant admission.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::File,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, JsonLimits, JsonValue};
use tos_source_store::{
    CaptureGitRequest, CaptureRestoreLimits, GitCaptureLimits, ReadLimits,
    SoftwareCaptureSelectionV1,
};

pub const HELP: &str =
    "usage: tos-native-owner-command source-capture capture-git|verify|restore OPTIONS
All operations: --max-seconds N --confirm-exclusive-owner yes
capture-git: --repository ABS --commit HEX40 --output ABS --include-prefix PATH
  --max-members N --max-member-bytes N --max-source-bytes N
  --max-metadata-bytes N --max-tree-bytes N --max-archive-bytes N
  Optional repeatable --exclude-prefix PATH and --exclude-path-part NAME.
verify|restore: --capture-root ABS --commit HEX40 --tree HEX40 --manifest-sha256 HEX64
  --max-members N --max-member-bytes N --max-source-bytes N
  --max-metadata-bytes N --max-archive-bytes N --max-decoded-bytes N
  --json-max-depth N --json-max-visits N --json-max-integer-digits N
restore also requires --destination ABS, a new child under a private owned parent.
Capture roots and output/destination parents must be exclusively owner-controlled.
The declaration does not stop writers. Source/recovery resources and cleanup remain
with the caller's whole supervisor. Restore may leave partial owned output on refusal.
Ignored payload is not Git material. Byte verification/restoration grants no source
admission, ready epoch, currentness, rights, executable selection or publication.
";

fn hold(path: &Path, private: bool) -> Result<File, &'static str> {
    let fd = tos_fd_open::open_absolute_directory(path).map_err(|_| "unsafe directory")?;
    let m = fd.metadata().map_err(|_| "directory metadata")?;
    if private && (m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o777 != 0o700) {
        return Err("private owned directory required");
    }
    Ok(fd)
}
fn recheck(path: &Path, fd: &File, private: bool) -> Result<(), &'static str> {
    let now = hold(path, private)?;
    let a = fd.metadata().map_err(|_| "held directory metadata")?;
    let b = now.metadata().map_err(|_| "named directory metadata")?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Err("directory identity changed");
    }
    Ok(())
}
fn hex(s: &str, length: usize) -> bool {
    s.len() == length
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn scalar(m: &JsonValue, name: &str) -> Result<u64, &'static str> {
    m.object_get(name)
        .and_then(JsonValue::as_u64)
        .ok_or("capture manifest count")
}

const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 65536;

/// Check each OS argument before retaining it in the command envelope. The
/// OS argument iterator itself owns one current argument; no oversized
/// aggregate or UTF-8 copy is retained before the bound is checked.
pub fn run_os_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<serde_json::Value, &'static str> {
    let mut selected = Vec::new();
    let mut bytes = 0usize;
    for argument in args {
        if selected.len() >= MAX_ARGUMENTS {
            return Err("argument envelope exceeded");
        }
        bytes = bytes
            .checked_add(argument.len())
            .filter(|n| *n <= MAX_ARGUMENT_BYTES)
            .ok_or("argument envelope exceeded")?;
        selected.push(
            argument
                .into_string()
                .map_err(|_| "arguments must be UTF-8")?,
        );
    }
    run(&selected)
}

pub fn run(args: &[String]) -> Result<serde_json::Value, &'static str> {
    let started = Instant::now();
    if args.len() > MAX_ARGUMENTS
        || args
            .iter()
            .try_fold(0usize, |n, a| n.checked_add(a.len()))
            .filter(|n| *n <= MAX_ARGUMENT_BYTES)
            .is_none()
    {
        return Err("argument envelope exceeded");
    }
    let operation = args
        .first()
        .map(String::as_str)
        .ok_or("operation required")?;
    if !matches!(operation, "capture-git" | "verify" | "restore") || (args.len() - 1) % 2 != 0 {
        return Err("invalid operation/options");
    }
    let mut values = BTreeMap::new();
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    let mut parts = Vec::new();
    for pair in args[1..].chunks_exact(2) {
        let k = pair[0].as_str();
        let v = pair[1].as_str();
        if operation == "capture-git"
            && matches!(
                k,
                "--include-prefix" | "--exclude-prefix" | "--exclude-path-part"
            )
        {
            match k {
                "--include-prefix" => include.push(v.to_owned()),
                "--exclude-prefix" => exclude.push(v.to_owned()),
                _ => parts.push(v.to_owned()),
            }
            continue;
        }
        let common = matches!(
            k,
            "--commit"
                | "--max-seconds"
                | "--confirm-exclusive-owner"
                | "--max-members"
                | "--max-member-bytes"
                | "--max-source-bytes"
                | "--max-metadata-bytes"
                | "--max-archive-bytes"
        );
        let specific = if operation == "capture-git" {
            matches!(k, "--repository" | "--output" | "--max-tree-bytes")
        } else {
            matches!(
                k,
                "--capture-root"
                    | "--tree"
                    | "--manifest-sha256"
                    | "--max-decoded-bytes"
                    | "--json-max-depth"
                    | "--json-max-visits"
                    | "--json-max-integer-digits"
            ) || operation == "restore" && k == "--destination"
        };
        if !(common || specific) || values.insert(k, v).is_some() {
            return Err("unknown/duplicate option");
        }
    }
    let get = |k: &str| values.get(k).copied().ok_or("required option absent");
    let n = |k: &str| -> Result<u64, &'static str> {
        let x = get(k)?.parse::<u64>().map_err(|_| "invalid limit")?;
        if x == 0 || x == u64::MAX {
            Err("finite positive limit required")
        } else {
            Ok(x)
        }
    };
    let usize_n = |k: &str| -> Result<usize, &'static str> {
        let x = usize::try_from(n(k)?).map_err(|_| "limit overflow")?;
        if x == usize::MAX {
            Err("finite limit required")
        } else {
            Ok(x)
        }
    };
    if get("--confirm-exclusive-owner")? != "yes" {
        return Err("exclusive owner declaration required");
    }
    let commit = get("--commit")?;
    if !hex(commit, 40) {
        return Err("exact commit required");
    }
    let deadline = started
        .checked_add(Duration::from_secs(n("--max-seconds")?))
        .ok_or("deadline overflow")?;
    let cancel = AtomicBool::new(false);
    let members = usize_n("--max-members")?;
    let member = n("--max-member-bytes")?;
    let source = n("--max-source-bytes")?;
    let metadata = usize_n("--max-metadata-bytes")?;
    let archive = n("--max-archive-bytes")?;
    if operation == "capture-git" {
        if include.is_empty() {
            return Err("explicit include prefix required");
        }
        let repository = Path::new(get("--repository")?);
        let output = Path::new(get("--output")?);
        let repo = hold(repository, false)?;
        let parent = output.parent().ok_or("output parent required")?;
        let parent_fd = hold(parent, true)?;
        let result = tos_source_store::capture_git(
            CaptureGitRequest {
                repository,
                commit,
                include_prefixes: &include,
                exclude_prefixes: &exclude,
                exclude_path_parts: &parts,
                output,
            },
            GitCaptureLimits {
                max_members: members,
                max_member_bytes: member,
                max_source_bytes: source,
                max_metadata_bytes: metadata,
                max_tree_bytes: n("--max-tree-bytes")?,
                max_archive_bytes: archive,
            },
            deadline,
            &cancel,
        )
        .map_err(|_| "capture refused; preserve owned partial output")?;
        recheck(repository, &repo, false)?;
        recheck(parent, &parent_fd, true)?;
        let tree = result
            .manifest
            .object_get("source_git_tree")
            .and_then(JsonValue::as_str)
            .ok_or("capture tree absent")?;
        return Ok(
            serde_json::json!({"operation":operation,"source_git_commit":commit,"source_git_tree":tree,
            "capture_manifest_sha256":result.manifest_sha256.to_hex(),"members":scalar(&result.manifest,"member_count")?,
            "source_bytes":scalar(&result.manifest,"source_bytes")?,"archive_bytes":scalar(&result.manifest,"archive_size_bytes")?,
            "grants_admission":false,"publication_authorized":false}),
        );
    }
    let tree = get("--tree")?;
    let digest = get("--manifest-sha256")?;
    if !hex(tree, 40) || !hex(digest, 64) {
        return Err("exact tree/manifest identity required");
    }
    let root = Path::new(get("--capture-root")?);
    let held = hold(root, true)?;
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit.to_owned(),
        source_git_tree: tree.to_owned(),
        capture_manifest_sha256: Digest256::from_hex(digest)
            .map_err(|_| "invalid manifest digest")?,
    };
    let limits = CaptureRestoreLimits {
        metadata: ReadLimits {
            max_manifest_bytes: metadata,
            max_manifest_entries: members,
            max_selected_object_bytes: member,
            json: JsonLimits {
                max_bytes: metadata,
                max_depth: usize_n("--json-max-depth")?,
                max_visits: usize_n("--json-max-visits")?,
                max_integer_digits: usize_n("--json-max-integer-digits")?,
            },
        },
        max_archive_bytes: archive,
        max_decoded_bytes: n("--max-decoded-bytes")?,
        max_source_bytes: source,
    };
    let usage = if operation == "verify" {
        let v = tos_source_store::verify_capture_with_usage(
            root, &selection, limits, deadline, &cancel,
        )
        .map_err(|_| "capture verification refused")?;
        Some(
            serde_json::json!({"metadata_bytes":v.usage.metadata_bytes,"archive_read_bytes":v.usage.archive_read_bytes,
            "decoded_bytes":v.usage.decoded_bytes,"total_read_bytes":v.usage.total_read_bytes().map_err(|_| "read usage overflow")?}),
        )
    } else {
        let destination = Path::new(get("--destination")?);
        let parent = destination.parent().ok_or("destination parent required")?;
        let parent_fd = hold(parent, true)?;
        tos_source_store::restore_capture(root, destination, &selection, limits, deadline, &cancel)
            .map_err(|_| "restore refused; preserve owned partial output")?;
        recheck(parent, &parent_fd, true)?;
        None
    };
    recheck(root, &held, true)?;
    Ok(
        serde_json::json!({"operation":operation,"source_git_commit":commit,"source_git_tree":tree,
        "capture_manifest_sha256":digest,"actual_read_usage":usage,"grants_admission":false,"publication_authorized":false}),
    )
}
