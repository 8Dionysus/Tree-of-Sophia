//! Thin, trusted-local CLI for exact V1 or native V2 corpus reads. It grants no public access.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tos_foundation::{Digest256, JsonLimits, SourceRevision};
use tos_segment_store::{AuthenticatedTreeLimitsV1, SegmentLimits};
use tos_source_store::{CorpusReader, PinnedSqliteIoBudget, ReadLimits, Selector};

const USAGE: &str = "tos-reader --store ABSOLUTE_ROOT --revision SHA256 --source-id ID \
--stage-dir ABSOLUTE_PRIVATE_DIR [--format v1|v2] --max-manifest-bytes N \
--max-manifest-entries N --max-selected-object-bytes N --json-max-depth N \
--json-max-visits N --json-max-integer-digits N \
V2 additionally requires --max-read-bytes N --max-write-bytes N \
--max-segment-bytes N --max-frame-bytes N --max-frames N --max-journal-bytes N \
--tree-max-key-bytes N --tree-max-value-bytes N --tree-max-kind-bytes N \
--tree-max-node-bytes N --tree-max-children N --tree-max-nodes N \
--tree-max-total-bytes N --tree-max-rows N --max-state-bytes N \
--deadline-seconds N";
const CAPABILITIES: &str = "{\"schema_version\":\"tos_reader_capabilities_v1\",\"store_format\":\"tos_corpus_snapshot_v1\",\"supported_store_formats\":[\"tos_corpus_snapshot_v1\",\"tos_native_admission_v2\"],\"default_format\":\"v1\",\"selection\":\"exact_revision_and_source_id\",\"platform\":\"linux\",\"minimum_kernel\":\"5.6\",\"required_open_api\":\"openat2\",\"path_traversal\":\"beneath_no_symlinks\",\"unsafe_fallback\":false}";
const MAX_ARGUMENTS: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const OUTPUT_BUFFER_BYTES: usize = 64 * 1024;
const MAX_STAGE_PATH_BYTES: usize = 4096;
const MAX_STAGE_PATH_COMPONENTS: usize = 128;
static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct ParsedArguments {
    values: BTreeMap<String, OsString>,
    retained_state_bytes: usize,
}

fn required(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<OsString, String> {
    values
        .remove(name)
        .ok_or_else(|| format!("missing {name}; usage: {USAGE}"))
}

fn required_text(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<String, String> {
    required(values, name)?
        .into_string()
        .map_err(|_| format!("{name} must be UTF-8 text"))
}

fn required_usize(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<usize, String> {
    required_text(values, name)?
        .parse::<usize>()
        .map_err(|_| format!("{name} must be a positive integer"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{name} must be positive"))
            } else {
                Ok(value)
            }
        })
}

fn required_u64(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<u64, String> {
    required_text(values, name)?
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a positive integer"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{name} must be positive"))
            } else {
                Ok(value)
            }
        })
}

fn required_u32(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<u32, String> {
    required_text(values, name)?
        .parse::<u32>()
        .map_err(|_| format!("{name} must be a positive integer"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{name} must be positive"))
            } else {
                Ok(value)
            }
        })
}

fn estimate_string_storage(bytes: usize, header_bytes: usize) -> Result<usize, String> {
    bytes
        .checked_mul(2)
        .and_then(|n| n.checked_add(header_bytes))
        .and_then(|n| n.checked_add(32))
        .ok_or_else(|| "argument string storage estimate overflow".to_owned())
}

fn arguments() -> Result<ParsedArguments, String> {
    let mut args = env::args_os().skip(1);
    let mut values = BTreeMap::new();
    let mut argument_count = 0usize;
    let mut argument_bytes = 0usize;
    let mut retained_state_bytes = 0usize;
    while let Some(raw_name) = args.next() {
        let raw_name_bytes = raw_name.as_encoded_bytes().len();
        if raw_name_bytes > MAX_ARGUMENT_BYTES {
            return Err("argument exceeds the reader command-line bound".to_owned());
        }
        let name = raw_name
            .into_string()
            .map_err(|_| format!("option names must be UTF-8; usage: {USAGE}"))?;
        if name == "--help" {
            println!(
                "{USAGE}\nRequires Linux 5.6+ with openat2; no weaker path-open fallback. Run --capabilities for the versioned platform contract."
            );
            std::process::exit(0);
        }
        if name == "--capabilities" {
            println!("{CAPABILITIES}");
            std::process::exit(0);
        }
        if !name.starts_with("--") {
            return Err(format!("unexpected argument {name}; usage: {USAGE}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {name}"))?;
        let value_bytes = value.as_encoded_bytes().len();
        argument_count = argument_count
            .checked_add(1)
            .ok_or_else(|| "argument count overflow".to_owned())?;
        argument_bytes = argument_bytes
            .checked_add(raw_name_bytes)
            .and_then(|n| n.checked_add(value_bytes))
            .ok_or_else(|| "argument byte count overflow".to_owned())?;
        if argument_count > MAX_ARGUMENTS || argument_bytes > MAX_ARGUMENT_BYTES {
            return Err("reader command line exceeds its finite argument profile".to_owned());
        }
        // Include both the map node and a transient cloned key. This upper
        // bound remains attached to the invocation when V2 computes its
        // simultaneous caller-state allowance.
        let key_state = estimate_string_storage(raw_name_bytes, std::mem::size_of::<String>())?
            .checked_mul(2)
            .ok_or_else(|| "argument state estimate overflow".to_owned())?;
        let value_state = estimate_string_storage(value_bytes, std::mem::size_of::<OsString>())?;
        retained_state_bytes = retained_state_bytes
            .checked_add(key_state)
            .and_then(|n| n.checked_add(value_state))
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, OsString)>() + 64))
            .ok_or_else(|| "argument state estimate overflow".to_owned())?;
        if values.insert(name.clone(), value).is_some() {
            return Err(format!("duplicate option {name}"));
        }
    }
    Ok(ParsedArguments {
        values,
        retained_state_bytes,
    })
}

fn run() -> Result<(), String> {
    let parsed = arguments()?;
    let mut values = parsed.values;
    let argument_state_bytes = parsed.retained_state_bytes;
    let format = values
        .remove("--format")
        .unwrap_or_else(|| OsString::from("v1"))
        .into_string()
        .map_err(|_| "--format must be UTF-8 text".to_owned())?;
    if format != "v1" && format != "v2" {
        return Err("--format must be v1 or v2".to_owned());
    }
    let store = PathBuf::from(required(&mut values, "--store")?);
    let stage_dir = PathBuf::from(required(&mut values, "--stage-dir")?);
    if !store.is_absolute() || !stage_dir.is_absolute() {
        return Err("--store and --stage-dir must be absolute paths".to_owned());
    }
    let revision = SourceRevision(
        Digest256::from_hex(&required_text(&mut values, "--revision")?)
            .map_err(|error| error.to_string())?,
    );
    let source_id = required_text(&mut values, "--source-id")?;
    let max_manifest_bytes = required_usize(&mut values, "--max-manifest-bytes")?;
    let max_manifest_entries = required_usize(&mut values, "--max-manifest-entries")?;
    let max_selected_object_bytes = required_u64(&mut values, "--max-selected-object-bytes")?;
    let max_depth = required_usize(&mut values, "--json-max-depth")?;
    let max_visits = required_usize(&mut values, "--json-max-visits")?;
    let max_integer_digits = required_usize(&mut values, "--json-max-integer-digits")?;
    let json = JsonLimits::new(
        max_manifest_bytes,
        max_depth,
        max_visits,
        max_integer_digits,
    )
    .map_err(|error| error.to_string())?;
    let limits = ReadLimits {
        max_manifest_bytes,
        max_manifest_entries,
        max_selected_object_bytes,
        json,
    };

    if format == "v2" {
        return run_v2(
            &mut values,
            store,
            stage_dir,
            revision,
            source_id,
            limits,
            max_selected_object_bytes,
            argument_state_bytes,
        );
    }

    if let Some(name) = values.keys().next() {
        return Err(format!("unknown option {name}; usage: {USAGE}"));
    }
    let reader = CorpusReader::open_existing(&store, limits).map_err(|error| error.to_string())?;
    let snapshot = reader
        .load_exact(revision)
        .map_err(|error| error.to_string())?;
    let descriptor = reader
        .resolve(&snapshot, Selector::SourceId(&source_id))
        .map_err(|error| error.to_string())?;
    // The selected bytes remain private until the complete object passes fixity checks.
    let mut stage = tempfile::tempfile_in(stage_dir).map_err(|error| error.to_string())?;
    reader
        .read_selected(
            &snapshot,
            &descriptor,
            max_selected_object_bytes,
            &mut stage,
        )
        .map_err(|error| error.to_string())?;
    stage.flush().map_err(|error| error.to_string())?;
    stage
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    io::copy(&mut stage, &mut output).map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn run_v2(
    values: &mut BTreeMap<String, OsString>,
    store: PathBuf,
    stage_dir: PathBuf,
    revision: SourceRevision,
    source_id: String,
    pointer: ReadLimits,
    max_object_bytes: u64,
    argument_state_bytes: usize,
) -> Result<(), String> {
    let max_read_bytes = required_u64(values, "--max-read-bytes")?;
    let max_write_bytes = required_u64(values, "--max-write-bytes")?;
    let segment = SegmentLimits {
        max_segment_bytes: required_u64(values, "--max-segment-bytes")?,
        max_frame_bytes: required_u64(values, "--max-frame-bytes")?,
        max_frames: required_u32(values, "--max-frames")?,
        max_journal_bytes: required_usize(values, "--max-journal-bytes")?,
    };
    let tree = AuthenticatedTreeLimitsV1 {
        max_key_bytes: required_usize(values, "--tree-max-key-bytes")?,
        max_value_bytes: required_usize(values, "--tree-max-value-bytes")?,
        max_kind_bytes: required_usize(values, "--tree-max-kind-bytes")?,
        max_node_bytes: required_usize(values, "--tree-max-node-bytes")?,
        max_children: required_usize(values, "--tree-max-children")?,
        max_nodes: required_u64(values, "--tree-max-nodes")?,
        max_total_bytes: required_u64(values, "--tree-max-total-bytes")?,
        max_rows: required_u64(values, "--tree-max-rows")?,
    };
    let max_state_bytes = required_usize(values, "--max-state-bytes")?;
    let deadline_seconds = required_u64(values, "--deadline-seconds")?;
    if let Some(name) = values.keys().next() {
        return Err(format!("unknown option {name}; usage: {USAGE}"));
    }
    let max_object_bytes = usize::try_from(max_object_bytes)
        .map_err(|_| "--max-selected-object-bytes exceeds address space".to_owned())?;
    let caller_retained_state_bytes = argument_state_bytes
        .checked_add(OUTPUT_BUFFER_BYTES)
        .and_then(|n| n.checked_add(store.as_os_str().as_encoded_bytes().len()))
        .and_then(|n| n.checked_add(stage_dir.as_os_str().as_encoded_bytes().len()))
        .and_then(|n| n.checked_add(source_id.len()))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| "V2 caller state estimate overflow".to_owned())?;
    let point = tos_command::source_admission_v2_reader::V2PointReadLimits {
        pointer,
        segment,
        tree,
        max_object_bytes,
        max_state_bytes,
        caller_retained_state_bytes,
    };
    let io_budget = PinnedSqliteIoBudget::new(max_read_bytes, max_write_bytes)
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(deadline_seconds))
        .ok_or_else(|| "--deadline-seconds exceeds supported range".to_owned())?;
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stage_root = open_private_stage_root(&stage_dir, deadline, cancelled.as_ref())?;
    check_active(deadline, cancelled.as_ref())?;
    let mut reader = tos_command::source_admission_v2_reader::V2ReadSession::open(
        &store,
        point,
        io_budget.clone(),
        deadline,
        cancelled.clone(),
    )
    .map_err(|error| error.to_string())?;
    let observation = reader
        .read_identity(revision, &source_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "selected V2 identity is absent at the requested revision".to_owned())?;
    if observation.revision != revision
        || observation.bytes.len() > max_object_bytes
        || observation.bytes.len() as u64 != observation.size_bytes
    {
        return Err("selected V2 member differs from its exact bounded observation".to_owned());
    }
    let payload_bytes = observation.bytes.len() as u64;

    // Payload allocation is already bounded by the point profile. Reserve its
    // full stage write before creating a member in the held private directory.
    check_active(deadline, cancelled.as_ref())?;
    io_budget
        .charge_write(payload_bytes)
        .map_err(|error| error.to_string())?;
    let mut stage = PrivateStage::create(&stage_root, deadline, cancelled.as_ref())
        .map_err(|error| error.to_string())?;
    check_active(deadline, cancelled.as_ref())?;
    write_all_precharged(
        &mut stage,
        &observation.bytes,
        &io_budget,
        deadline,
        cancelled.as_ref(),
    )?;
    check_active(deadline, cancelled.as_ref())?;
    stage.flush().map_err(|error| error.to_string())?;
    check_active(deadline, cancelled.as_ref())?;
    if stage
        .file
        .metadata()
        .map_err(|error| error.to_string())?
        .len()
        != payload_bytes
    {
        return Err("V2 staging file length differs from verified payload".to_owned());
    }
    drop(observation);
    check_active(deadline, cancelled.as_ref())?;
    verify_private_stage_root_name(&stage_dir, &stage_root, deadline, cancelled.as_ref())
        .map_err(|error| error.to_string())?;
    verify_private_stage_name(&stage, deadline, cancelled.as_ref())
        .map_err(|error| error.to_string())?;
    reader
        .verify_current_fence()
        .map_err(|error| error.to_string())?;
    check_active(deadline, cancelled.as_ref())?;
    stage
        .file
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    check_active(deadline, cancelled.as_ref())?;

    // Reserve the staged reread and stdout transfer together before exposing
    // the first byte. The source reads and stage write already used this ledger.
    io_budget
        .charge_read(payload_bytes)
        .map_err(|error| error.to_string())?;
    io_budget
        .charge_write(payload_bytes)
        .map_err(|error| error.to_string())?;
    check_active(deadline, cancelled.as_ref())?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut buffer = [0u8; OUTPUT_BUFFER_BYTES];
    let mut remaining = payload_bytes;
    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| "V2 output block size exceeds address space".to_owned())?;
        read_exact_precharged(
            &mut stage.file,
            &mut buffer[..wanted],
            &io_budget,
            deadline,
            cancelled.as_ref(),
        )?;
        write_all_precharged(
            &mut output,
            &buffer[..wanted],
            &io_budget,
            deadline,
            cancelled.as_ref(),
        )?;
        remaining -= wanted as u64;
    }
    check_active(deadline, cancelled.as_ref())?;
    output.flush().map_err(|error| error.to_string())?;
    check_active(deadline, cancelled.as_ref())?;
    Ok(())
}

struct PrivateStage {
    file: File,
    root: File,
    name: String,
}

impl PrivateStage {
    fn create(
        root: &File,
        deadline: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> io::Result<Self> {
        let held_root = root.try_clone()?;
        for _ in 0..16 {
            check_active_io(deadline, cancelled)?;
            let sequence = STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = format!(".tos-reader-{}-{sequence}.stage", std::process::id());
            match rustix::fs::openat(
                root,
                name.as_str(),
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            ) {
                Ok(fd) => {
                    let stage = Self {
                        file: File::from(fd),
                        root: held_root,
                        name,
                    };
                    check_active_io(deadline, cancelled)?;
                    return Ok(stage);
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a private staging name",
        ))
    }
}

impl Write for PrivateStage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Drop for PrivateStage {
    fn drop(&mut self) {
        let named = tos_fd_open::open_regular_at(&self.root, Path::new(&self.name));
        let same_inode = named.is_ok_and(|named| {
            let Ok(expected) = self.file.metadata() else {
                return false;
            };
            let Ok(actual) = named.metadata() else {
                return false;
            };
            expected.dev() == actual.dev() && expected.ino() == actual.ino()
        });
        if same_inode {
            let _ =
                rustix::fs::unlinkat(&self.root, self.name.as_str(), rustix::fs::AtFlags::empty());
        }
    }
}

fn open_private_stage_root(
    path: &Path,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<File, String> {
    let encoded = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || encoded.len() > MAX_STAGE_PATH_BYTES
        || path.components().count() > MAX_STAGE_PATH_COMPONENTS
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err("V2 staging directory path is not bounded and normalized".to_owned());
    }
    check_active(deadline, cancelled)?;
    let root = tos_fd_open::open_absolute_directory(path).map_err(|error| error.to_string())?;
    check_active(deadline, cancelled)?;
    let metadata = root.metadata().map_err(|error| error.to_string())?;
    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o077 != 0 {
        return Err("V2 staging directory must be owned and private to this user".to_owned());
    }
    check_active(deadline, cancelled)?;
    Ok(root)
}

fn verify_private_stage_root_name(
    path: &Path,
    held: &File,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<()> {
    check_active_io(deadline, cancelled)?;
    let named = tos_fd_open::open_absolute_directory(path)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
    let held_metadata = held.metadata()?;
    let named_metadata = named.metadata()?;
    if held_metadata.dev() != named_metadata.dev()
        || held_metadata.ino() != named_metadata.ino()
        || held_metadata.uid() != named_metadata.uid()
        || held_metadata.mode() & 0o077 != 0
        || named_metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "named V2 staging directory differs from its held private root",
        ));
    }
    check_active_io(deadline, cancelled)
}

fn verify_private_stage_name(
    stage: &PrivateStage,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<()> {
    check_active_io(deadline, cancelled)?;
    let named = tos_fd_open::open_regular_at(&stage.root, Path::new(&stage.name))
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
    let held_metadata = stage.file.metadata()?;
    let named_metadata = named.metadata()?;
    if held_metadata.dev() != named_metadata.dev()
        || held_metadata.ino() != named_metadata.ino()
        || held_metadata.uid() != rustix::process::geteuid().as_raw()
        || named_metadata.uid() != rustix::process::geteuid().as_raw()
        || held_metadata.mode() & 0o077 != 0
        || named_metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "named V2 staging member differs from its held private file",
        ));
    }
    check_active_io(deadline, cancelled)
}

fn write_all_precharged(
    mut output: impl Write,
    mut bytes: &[u8],
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    while !bytes.is_empty() {
        check_active(deadline, cancelled)?;
        let written = output.write(bytes).map_err(|error| error.to_string())?;
        if written == 0 {
            return Err("write returned no progress".to_owned());
        }
        io.record_write_returned(written as u64)
            .map_err(|error| error.to_string())?;
        check_active(deadline, cancelled)?;
        bytes = &bytes[written..];
    }
    Ok(())
}

fn read_exact_precharged(
    mut input: impl Read,
    mut bytes: &mut [u8],
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    while !bytes.is_empty() {
        check_active(deadline, cancelled)?;
        let read = input.read(bytes).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("staging file ended before the verified payload".to_owned());
        }
        io.record_read_returned(read as u64)
            .map_err(|error| error.to_string())?;
        check_active(deadline, cancelled)?;
        let (_, rest) = bytes.split_at_mut(read);
        bytes = rest;
    }
    Ok(())
}

fn check_active(
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    check_active_io(deadline, cancelled).map_err(|error| error.to_string())
}

fn check_active_io(deadline: Instant, cancelled: &std::sync::atomic::AtomicBool) -> io::Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "V2 reader cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "V2 reader deadline exceeded",
        ));
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tos-reader: {error}");
            ExitCode::from(2)
        }
    }
}
