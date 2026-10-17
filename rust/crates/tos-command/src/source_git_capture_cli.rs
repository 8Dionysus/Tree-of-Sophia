//! Owner-selected existing Git capture transport; bytes never grant admission.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, JsonLimits, JsonMode, JsonValue, RelativePath, SourceRevision};
use tos_source_store::{
    CaptureGitRequest, CaptureImportRequestV1, CaptureRestoreLimits, GitCaptureLimits,
    MAX_RETAINED_CAPTURE_INPUTS_V1, ReadLimits, RetainedCaptureSelectionV1,
    SoftwareCaptureSelectionV1,
};

const RETAINED_FLAGS: [&str; 4] = [
    "--retained-store",
    "--retained-revision",
    "--retained-binding",
    "--retained-receipt",
];

pub const HELP: &str =
    "usage: tos-native-owner-command source-capture capture-git|verify|restore|import-corpus OPTIONS
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
import-corpus also requires --output ABS, a new store child under a private owned parent,
  and --validator-sha256 HEX64 pinned to the executing importer image.
  Optional retained input: --retained-store ABS --retained-revision HEX64
  --retained-binding ABS, with --retained-receipt LEAF when selecting its
  revision-scoped original receipt instead of capture-import.json.
  Up to 16 selections: repeat store/revision/binding in the same per-flag order.
  An explicit receipt list must name every selected input's original receipt.
  Retained subtrees share the original aggregate member/source/decoded limits;
  they must be disjoint, and incoming Git members must be outside them.
  Every old snapshot and receipt remains exact in the fresh shared object pool.
  retained_member_bytes_read counts logical member bytes, not complete physical I/O.
Its capture_read_usage reports parser logical byte counts, not complete physical I/O;
the launch supervisor measures process/cgroup I/O separately.
Capture roots and output/destination parents must be exclusively owner-controlled.
The declaration does not stop writers. Source/recovery resources and cleanup remain
with the caller's whole supervisor. Restore/import may leave partial owned output on refusal.
Ignored payload is not Git material. Byte verification/restoration grants no source
admission, ready epoch, currentness, rights, executable selection or publication.
import-corpus creates an exact immutable V1 snapshot and capture-import.json receipt;
it writes no current.json and establishes neither semantic admission nor whole-source completeness.
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

fn retained_selection<'a>(
    values: &BTreeMap<&str, &'a str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<RetainedCaptureSelectionV1<'a>>, &'static str> {
    if !RETAINED_FLAGS.iter().any(|flag| values.contains_key(flag)) {
        return Ok(None);
    }
    let get = |name| {
        values
            .get(name)
            .copied()
            .ok_or("retained input selection is incomplete")
    };
    let root = Path::new(get("--retained-store")?);
    let revision = SourceRevision(
        Digest256::from_hex(get("--retained-revision")?)
            .map_err(|_| "invalid retained revision")?,
    );
    let path = Path::new(get("--retained-binding")?);
    if !path.is_absolute() || !root.is_absolute() {
        return Err("retained root and binding must be absolute");
    }
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err("retained binding cutoff expired");
    }
    let parent = path.parent().ok_or("retained binding parent absent")?;
    let held_parent = hold(parent, false)?;
    let leaf = path.file_name().ok_or("retained binding leaf absent")?;
    let mut file = tos_fd_open::open_regular_at(&held_parent, Path::new(leaf))
        .map_err(|_| "retained binding is not a regular selected file")?;
    let before = file.metadata().map_err(|_| "retained binding stat")?;
    if before.len() > 8192 {
        return Err("retained binding exceeds its fixed metadata bound");
    }
    let mut raw = Vec::new();
    file.by_ref()
        .take(8193)
        .read_to_end(&mut raw)
        .map_err(|_| "retained binding read")?;
    let after = file.metadata().map_err(|_| "retained binding restat")?;
    let named = tos_fd_open::open_regular_at(&held_parent, Path::new(leaf))
        .map_err(|_| "retained binding name changed")?
        .metadata()
        .map_err(|_| "retained binding name stat")?;
    let identity = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if raw.len() as u64 != before.len()
        || identity(&before) != identity(&after)
        || identity(&before) != identity(&named)
    {
        return Err("retained binding changed during read");
    }
    recheck(parent, &held_parent, false)?;
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err("retained binding cutoff expired");
    }
    let binding = tos_foundation::parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(8192, 8, 1024, 64).map_err(|_| "retained binding limits")?,
    )
    .map_err(|_| "retained binding is not strict bounded JSON")?
    .into_root();
    let fields = [
        "$schema",
        "schema_version",
        "scope",
        "restore_origin_git_commit",
        "membership_sha256",
        "member_count",
        "source_bytes",
        "member_mode",
    ];
    if binding
        .as_object()
        .is_none_or(|object| object.len() != fields.len())
        || fields
            .into_iter()
            .any(|name| binding.object_get(name).is_none())
        || binding
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            != Some("tos_retained_corpus_binding_v1")
        || binding.object_get("$schema").and_then(JsonValue::as_str)
            != Some(
                "https://tree-of-sophia.local/ToS/contracts/retained-corpus-binding.schema.json",
            )
        || !binding
            .object_get("restore_origin_git_commit")
            .and_then(JsonValue::as_str)
            .is_some_and(|commit| hex(commit, 40))
    {
        return Err("retained binding contract differs");
    }
    let scope = RelativePath::parse(
        binding
            .object_get("scope")
            .and_then(JsonValue::as_str)
            .ok_or("retained binding scope absent")?,
    )
    .map_err(|_| "unsafe retained binding scope")?;
    if scope.as_str().chars().count() > 4096 {
        return Err("retained scope exceeds its contract bound");
    }
    let membership_sha256 = Digest256::from_hex(
        binding
            .object_get("membership_sha256")
            .and_then(JsonValue::as_str)
            .ok_or("retained membership absent")?,
    )
    .map_err(|_| "invalid retained membership digest")?;
    let member_mode =
        u32::try_from(scalar(&binding, "member_mode")?).map_err(|_| "retained mode overflow")?;
    Ok(Some(RetainedCaptureSelectionV1 {
        root,
        revision,
        receipt_file: values
            .get("--retained-receipt")
            .copied()
            .unwrap_or("capture-import.json"),
        scope,
        membership_sha256,
        member_count: scalar(&binding, "member_count")?,
        source_bytes: scalar(&binding, "source_bytes")?,
        member_mode,
        binding_sha256: Digest256::of_bytes(&raw),
    }))
}

fn retained_selections<'a>(
    values: &BTreeMap<&str, Vec<&'a str>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<RetainedCaptureSelectionV1<'a>>, &'static str> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let stores = values
        .get("--retained-store")
        .ok_or("retained stores absent")?;
    let revisions = values
        .get("--retained-revision")
        .ok_or("retained revisions absent")?;
    let bindings = values
        .get("--retained-binding")
        .ok_or("retained bindings absent")?;
    let receipts = values.get("--retained-receipt");
    if stores.is_empty()
        || stores.len() > MAX_RETAINED_CAPTURE_INPUTS_V1
        || revisions.len() != stores.len()
        || bindings.len() != stores.len()
        || receipts.is_some_and(|rows| rows.len() != stores.len())
    {
        return Err("retained selection lists differ or exceed their bound");
    }
    let mut selected = Vec::with_capacity(stores.len());
    for index in 0..stores.len() {
        let mut one = BTreeMap::from([
            ("--retained-store", stores[index]),
            ("--retained-revision", revisions[index]),
            ("--retained-binding", bindings[index]),
        ]);
        if let Some(receipts) = receipts {
            one.insert("--retained-receipt", receipts[index]);
        }
        selected.push(
            retained_selection(&one, deadline, cancelled)?.ok_or("retained selection absent")?,
        );
    }
    Ok(selected)
}

fn import_response(
    receipt: tos_source_store::CaptureImportReceipt,
) -> Result<serde_json::Value, &'static str> {
    let mut response = serde_json::json!({
        "operation": "import-corpus",
        "schema_version": receipt.schema_version,
        "capture_manifest_sha256": receipt.capture_manifest_sha256.to_hex(),
        "source_git_commit": receipt.source_git_commit,
        "source_git_tree": receipt.source_git_tree,
        "include_prefix_count": receipt.include_prefixes.len(),
        "exclude_prefix_count": receipt.exclude_prefixes.len(),
        "exclude_path_part_count": receipt.exclude_path_parts.len(),
        "revision": receipt.revision.0.to_hex(),
        "membership_sha256": receipt.membership_sha256.to_hex(),
        "member_count": receipt.member_count,
        "source_bytes": receipt.source_bytes,
        "validator_sha256": receipt.validator_sha256.to_hex(),
        "validator_role": "capture-import-byte-integrity-v1",
        "import_receipt_sha256": receipt.import_receipt_sha256.to_hex(),
        "capture_read_usage": {
            "measurement": "capture-parser-logical-read-bytes-v1",
            "metadata_bytes": receipt.usage.metadata_bytes,
            "archive_read_bytes": receipt.usage.archive_read_bytes,
            "decoded_bytes": receipt.usage.decoded_bytes,
            "capture_total_read_bytes": receipt.usage.total_read_bytes()
                .map_err(|_| "read usage overflow")?,
            "complete_physical_io": false
        },
        "custody_only": true,
        "semantic_admission": false,
        "rights_change": false,
        "source_admitted": false,
        "source_currentness_selected": false,
        "publication_authorized": false,
        "whole_authored_inventory_established": false,
        "identity_index_derived": false,
        "dependency_index_derived": false,
        "current_pointer_written": false
    });
    if !receipt.retained.is_empty() {
        let (members, bytes, read) = receipt.retained.iter().try_fold(
            (0u64, 0u64, 0u64),
            |(members, bytes, read), input| {
                Ok::<_, &'static str>((
                    members
                        .checked_add(input.member_count)
                        .ok_or("retained member count overflow")?,
                    bytes
                        .checked_add(input.source_bytes)
                        .ok_or("retained source bytes overflow")?,
                    read.checked_add(input.member_bytes_read)
                        .ok_or("retained read bytes overflow")?,
                ))
            },
        )?;
        response["retained_source_count"] = serde_json::json!(receipt.retained.len());
        response["retained_member_count"] = serde_json::json!(members);
        response["retained_source_bytes"] = serde_json::json!(bytes);
        response["retained_member_bytes_read"] = serde_json::json!(read);
    }
    Ok(response)
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
    if !matches!(
        operation,
        "capture-git" | "verify" | "restore" | "import-corpus"
    ) || (args.len() - 1) % 2 != 0
    {
        return Err("invalid operation/options");
    }
    let mut values = BTreeMap::new();
    let mut retained_values: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
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
        if operation == "import-corpus" && RETAINED_FLAGS.contains(&k) {
            retained_values.entry(k).or_default().push(v);
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
        } else if operation == "import-corpus" {
            matches!(
                k,
                "--capture-root"
                    | "--tree"
                    | "--manifest-sha256"
                    | "--max-decoded-bytes"
                    | "--json-max-depth"
                    | "--json-max-visits"
                    | "--json-max-integer-digits"
                    | "--output"
                    | "--validator-sha256"
            )
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
    if operation == "import-corpus" {
        let output = Path::new(get("--output")?);
        if !output.is_absolute() {
            return Err("absolute output store path required");
        }
        let parent = output.parent().ok_or("output parent required")?;
        let parent_fd = hold(parent, true)?;
        let expected_validator_sha256 = Digest256::from_hex(get("--validator-sha256")?)
            .map_err(|_| "invalid executing validator image pin")?;
        let retained = retained_selections(&retained_values, deadline, &cancel)?;
        let retained_paths = retained.iter().map(|input| input.root).collect::<Vec<_>>();
        let retained_guards = retained_paths
            .iter()
            .map(|path| hold(path, true))
            .collect::<Result<Vec<_>, _>>()?;
        let receipt = tos_source_store::import_git_capture_corpus_v1(
            CaptureImportRequestV1 {
                capture_root: root,
                output,
                selection: &selection,
                limits,
                expected_validator_sha256,
                retained,
            },
            deadline,
            &cancel,
        )
        .map_err(|_| "capture corpus import refused; retain unselected partial custody")?;
        recheck(root, &held, true)?;
        for (path, guard) in retained_paths.iter().zip(&retained_guards) {
            recheck(path, guard, true)?;
        }
        recheck(parent, &parent_fd, true)?;
        return import_response(receipt);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::SourceRevision;
    use tos_source_store::CaptureReadUsage;

    #[test]
    fn import_response_stays_inside_native_command_output_bound() {
        // 110 filters plus the import-corpus required options fit the native
        // command's 256-argument envelope; each list also fits its selector
        // metadata budget. The detailed values live in the durable sidecar.
        let includes = (0..37)
            .map(|index| format!("ToS/include-{index:02}-{}", "x".repeat(24)))
            .collect();
        let excludes = (0..37)
            .map(|index| format!("ToS/exclude-{index:02}-{}", "x".repeat(24)))
            .collect();
        let path_parts = (0..36)
            .map(|index| format!("part-{index:02}-{}", "x".repeat(24)))
            .collect();
        let receipt = tos_source_store::CaptureImportReceipt {
            schema_version: "tos_corpus_capture_import_v1",
            capture_manifest_sha256: Digest256::of_bytes(b"capture"),
            source_git_commit: "a".repeat(40),
            source_git_tree: "b".repeat(40),
            include_prefixes: includes,
            exclude_prefixes: excludes,
            exclude_path_parts: path_parts,
            validator_sha256: Digest256::of_bytes(b"validator"),
            revision: SourceRevision(Digest256::of_bytes(b"revision")),
            membership_sha256: Digest256::of_bytes(b"membership"),
            member_count: u64::MAX - 1,
            source_bytes: u64::MAX - 1,
            import_receipt_sha256: Digest256::of_bytes(b"receipt"),
            usage: CaptureReadUsage {
                metadata_bytes: u64::MAX - 1,
                archive_read_bytes: 0,
                decoded_bytes: 0,
            },
            retained: Vec::new(),
        };
        let response = import_response(receipt.clone()).unwrap();
        let encoded = serde_json::to_vec(&response).unwrap();
        assert!(
            encoded.len() <= 2048,
            "response was {} bytes",
            encoded.len()
        );
        assert_eq!(response["include_prefix_count"], 37);
        assert_eq!(response["exclude_prefix_count"], 37);
        assert_eq!(response["exclude_path_part_count"], 36);
        assert!(response.get("include_prefixes").is_none());
        assert_eq!(
            response["capture_read_usage"]["measurement"],
            "capture-parser-logical-read-bytes-v1"
        );
        assert_eq!(
            response["capture_read_usage"]["complete_physical_io"],
            false
        );
        let mut composed = receipt;
        composed.schema_version = "tos_corpus_capture_import_v2";
        let old_revision = SourceRevision(Digest256::of_bytes(b"old revision"));
        let max_inputs = MAX_RETAINED_CAPTURE_INPUTS_V1 as u64;
        composed.retained = vec![
            tos_source_store::RetainedCaptureReceiptV1 {
                revision: old_revision,
                validator_sha256: Digest256::of_bytes(b"old validator"),
                scope: RelativePath::parse(&format!("ToS/{}", "x".repeat(4090))).unwrap(),
                membership_sha256: Digest256::of_bytes(b"old membership"),
                member_count: u64::MAX / max_inputs - 1,
                source_bytes: u64::MAX / max_inputs - 1,
                member_mode: 0o644,
                binding_sha256: Digest256::of_bytes(b"binding"),
                original_receipt_sha256: Digest256::of_bytes(b"old receipt"),
                receipt_ref: format!("capture-import.{}.json", old_revision.0.to_hex()),
                member_bytes_read: u64::MAX / max_inputs - 1,
            };
            MAX_RETAINED_CAPTURE_INPUTS_V1
        ];
        let response = import_response(composed).unwrap();
        assert!(serde_json::to_vec(&response).unwrap().len() <= 2048);
        assert!(response.get("retained_sources").is_none());
        assert_eq!(response["schema_version"], "tos_corpus_capture_import_v2");
        assert_eq!(
            response["retained_source_count"],
            MAX_RETAINED_CAPTURE_INPUTS_V1
        );
    }

    #[test]
    fn retained_binding_selection_refuses_duplicates_aliases_and_partial_selection() {
        let directory = tempfile::tempdir().unwrap();
        let binding_path = directory.path().join("binding.json");
        let store = directory.path().join("selected-store");
        let binding = serde_json::json!({
            "$schema":"https://tree-of-sophia.local/ToS/contracts/retained-corpus-binding.schema.json",
            "schema_version":"tos_retained_corpus_binding_v1",
            "scope":"ToS/history",
            "restore_origin_git_commit":"a".repeat(40),
            "membership_sha256":"b".repeat(64),
            "member_count":2,"source_bytes":40,"member_mode":420,
        });
        let original = serde_json::to_vec(&binding).unwrap();
        std::fs::write(&binding_path, &original).unwrap();
        let revision = "c".repeat(64);
        let values = BTreeMap::from([
            ("--retained-store", store.to_str().unwrap()),
            ("--retained-revision", revision.as_str()),
            ("--retained-binding", binding_path.to_str().unwrap()),
        ]);
        let deadline = Instant::now() + Duration::from_secs(10);
        let cancelled = AtomicBool::new(false);
        let selection = retained_selection(&values, deadline, &cancelled)
            .unwrap()
            .unwrap();
        assert_eq!(selection.scope.as_str(), "ToS/history");
        assert_eq!(selection.member_count, 2);
        assert_eq!(selection.member_mode, 0o644);
        assert_eq!(selection.binding_sha256, Digest256::of_bytes(&original));
        let mut multiple = values
            .iter()
            .map(|(key, value)| (*key, vec![*value, *value]))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            retained_selections(&multiple, deadline, &cancelled)
                .unwrap()
                .len(),
            2
        );
        multiple.insert("--retained-receipt", vec!["capture-import.json"]);
        assert!(retained_selections(&multiple, deadline, &cancelled).is_err());
        multiple.remove("--retained-receipt");
        multiple.get_mut("--retained-revision").unwrap().pop();
        assert!(retained_selections(&multiple, deadline, &cancelled).is_err());
        let excessive = values
            .iter()
            .map(|(key, value)| (*key, vec![*value; MAX_RETAINED_CAPTURE_INPUTS_V1 + 1]))
            .collect::<BTreeMap<_, _>>();
        assert!(retained_selections(&excessive, deadline, &cancelled).is_err());
        let mut partial = values.clone();
        partial.remove("--retained-revision");
        assert!(retained_selection(&partial, deadline, &cancelled).is_err());
        let mut duplicate = original.clone();
        duplicate.pop();
        duplicate.extend_from_slice(b",\"member_count\":2}");
        std::fs::write(&binding_path, duplicate).unwrap();
        assert!(retained_selection(&values, deadline, &cancelled).is_err());
        let mut traversal = binding.clone();
        traversal["scope"] = serde_json::json!("ToS/../private");
        std::fs::write(&binding_path, serde_json::to_vec(&traversal).unwrap()).unwrap();
        assert!(retained_selection(&values, deadline, &cancelled).is_err());
        std::fs::write(&binding_path, vec![b' '; 8193]).unwrap();
        assert!(retained_selection(&values, deadline, &cancelled).is_err());
        std::fs::write(&binding_path, &original).unwrap();
        let alias = directory.path().join("alias.json");
        std::os::unix::fs::symlink(&binding_path, &alias).unwrap();
        let mut aliased = values.clone();
        aliased.insert("--retained-binding", alias.to_str().unwrap());
        assert!(retained_selection(&aliased, deadline, &cancelled).is_err());
        cancelled.store(true, Ordering::Relaxed);
        assert!(retained_selection(&values, deadline, &cancelled).is_err());
    }
}
