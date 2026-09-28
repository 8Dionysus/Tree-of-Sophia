//! Operation-local retained bytes for TextUnit/TextLayer creation. This
//! control is not an owner grant; the writer checks its selected source,
//! rights, identities and current configuration separately.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, raw};
use crate::source_text_owner::OwnerTextContext;
use base64::Engine;
use base64::alphabet::STANDARD as BASE64_ALPHABET;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::collections::BTreeMap;
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1,
    canonical_count_v1, parse_json,
};

const MAX_FILES: usize = 12;
const MAX_PACKAGE: usize = 12 * 1024 * 1024;
const MAX_CONTROL: usize = 18 * 1024 * 1024;
const MAX_FILE: usize = 8 * 1024 * 1024;

fn bad_plan() -> SourceCommandError {
    SourceCommandError::Invalid("native private construction control")
}

fn leaf(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn request_digest(request: &JsonValue) -> SourceCommandResult<String> {
    Ok(Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed())
}

/// Exact maintained `tos_native_construction_stage_v1` plan. The package
/// bytes and encoded control are both admitted before base64 allocation.
pub(crate) fn encode_plan(
    target_ref: &str,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<Vec<u8>> {
    if files.is_empty() || files.len() > MAX_FILES || files.keys().any(|name| !leaf(name)) {
        return Err(SourceCommandError::Invalid(
            "native private package file set",
        ));
    }
    let mut total = 0usize;
    let mut encoded_total = 0usize;
    for (name, bytes) in files {
        total = total.checked_add(bytes.len()).ok_or(bad_plan())?;
        let len = bytes
            .len()
            .checked_add(2)
            .and_then(|n| n.checked_div(3))
            .and_then(|n| n.checked_mul(4))
            .ok_or(bad_plan())?;
        encoded_total = encoded_total
            .checked_add(name.len())
            .and_then(|n| n.checked_add(len))
            .ok_or(bad_plan())?;
        if bytes.len() > MAX_FILE || total > MAX_PACKAGE || encoded_total > MAX_CONTROL {
            return Err(SourceCommandError::Unsupported(
                "native private plan byte budget",
            ));
        }
    }
    // Each accepted leaf is plain ASCII, so its JSON string has no escaping
    // expansion. Reserve the actual fixed fields, target, punctuation and LF
    // before allocating the encoded copies.
    let limits = JsonLimits {
        max_bytes: MAX_CONTROL,
        ..JsonLimits::default()
    };
    let target_len = canonical_count_v1(
        &cmd::string(target_ref),
        CanonicalProfile::SourceCommandInputV1,
        limits,
    )
    .map_err(|_| bad_plan())?;
    let control_upper = encoded_total
        .checked_add(target_len)
        .and_then(|n| n.checked_add(512 + MAX_FILES * 8))
        .ok_or(bad_plan())?;
    if control_upper > MAX_CONTROL {
        return Err(SourceCommandError::Unsupported(
            "native private control byte budget",
        ));
    }
    let mut encoded = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        encoded.push((name.as_str(), cmd::string(&STANDARD.encode(bytes))));
    }
    let plan = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_construction_stage_v1"),
        ),
        ("target_ref", cmd::string(target_ref)),
        ("request_digest", cmd::string(&request_digest(request)?)),
        ("files", cmd::object(encoded)),
    ]);
    let mut raw = canonical_bytes_v1(&plan, CanonicalProfile::SourceCommandInputV1, limits)
        .map_err(|_| bad_plan())?;
    if raw.len().checked_add(1).is_none_or(|len| len > MAX_CONTROL) {
        return Err(SourceCommandError::Unsupported(
            "native private control byte budget",
        ));
    }
    raw.push(b'\n');
    Ok(raw)
}

/// Existing Python plans use `b64decode(validate=True)`. Its trailing-bit
/// behavior is deliberately selected here; Rust's default decoder refuses
/// nonzero unused bits although the maintained reader accepts them.
pub(crate) fn decode_plan(
    raw: &[u8],
    target_ref: &str,
    request: &JsonValue,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    if raw.len() > MAX_CONTROL {
        return Err(bad_plan());
    }
    let plan = parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: MAX_CONTROL,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| bad_plan())?
    .into_root();
    cmd::exact_keys(
        &plan,
        &["schema_version", "target_ref", "request_digest", "files"],
    )?;
    if cmd::text(&plan, "schema_version")? != "tos_native_construction_stage_v1"
        || cmd::text(&plan, "target_ref")? != target_ref
        || cmd::text(&plan, "request_digest")? != request_digest(request)?
    {
        return Err(SourceCommandError::Conflict(
            "native private plan identity differs",
        ));
    }
    let entries = cmd::field(&plan, "files")?.as_object().ok_or(bad_plan())?;
    if entries.is_empty() || entries.len() > MAX_FILES {
        return Err(bad_plan());
    }
    let python_base64 = GeneralPurpose::new(
        &BASE64_ALPHABET,
        GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    );
    let mut remaining = MAX_PACKAGE;
    let mut files = BTreeMap::new();
    for (name, value) in entries {
        let name = name.as_str().ok_or(bad_plan())?;
        let encoded = value.as_str().ok_or(bad_plan())?;
        if !leaf(name) || encoded.len() > MAX_CONTROL {
            return Err(bad_plan());
        }
        let upper = encoded
            .len()
            .checked_div(4)
            .and_then(|n| n.checked_mul(3))
            .ok_or(bad_plan())?;
        if upper > remaining.saturating_add(2) {
            return Err(bad_plan());
        }
        let bytes = python_base64.decode(encoded).map_err(|_| bad_plan())?;
        if bytes.len() > MAX_FILE || bytes.len() > remaining {
            return Err(bad_plan());
        }
        remaining -= bytes.len();
        if files.insert(name.to_owned(), bytes).is_some() {
            return Err(bad_plan());
        }
    }
    Ok(files)
}

fn stamp(meta: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

fn directory(file: &File, uid: u32, private: bool) -> SourceCommandResult<Metadata> {
    let meta = file.metadata().map_err(|_| bad_plan())?;
    if !meta.is_dir()
        || meta.uid() != uid && (private || meta.uid() != 0)
        || meta.mode() & 0o022 != 0
        || private && meta.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Denied("native Text directory unsafe"));
    }
    Ok(meta)
}

fn regular(file: &File, uid: u32) -> SourceCommandResult<Metadata> {
    let meta = file.metadata().map_err(|_| bad_plan())?;
    if !meta.is_file() || meta.uid() != uid || meta.mode() & 0o7777 != 0o600 {
        return Err(SourceCommandError::Denied(
            "native Text private file unsafe",
        ));
    }
    Ok(meta)
}

fn child(parent: &File, name: &str, uid: u32) -> SourceCommandResult<File> {
    let found = tos_fd_open::open_directory_at(parent, Path::new(name)).map_err(|_| {
        SourceCommandError::Denied("native Text private directory absent or unsafe")
    })?;
    directory(&found, uid, true)?;
    Ok(found)
}

fn read_at(
    parent: &File,
    name: &str,
    uid: u32,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    active(deadline, cancelled)?;
    let opened = rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    );
    let mut file: File = match opened {
        Ok(fd) => fd.into(),
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => {
            return Err(SourceCommandError::Denied(
                "native Text private member unsafe",
            ));
        }
    };
    let before = regular(&file, uid)?;
    if before.len() > limit as u64 {
        return Err(SourceCommandError::Unsupported(
            "native Text private member byte budget",
        ));
    }
    let bytes = raw(&mut file, limit, deadline, cancelled)?;
    let after = regular(&file, uid)?;
    let current: File = rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("native Text private member detached"))?;
    if stamp(&before) != stamp(&after) || stamp(&before) != stamp(&regular(&current, uid)?) {
        return Err(SourceCommandError::Conflict(
            "native Text private member changed",
        ));
    }
    Ok(Some(bytes))
}

fn write_new(
    parent: &File,
    name: &str,
    bytes: &[u8],
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let mut file: File = rustix::fs::openat(
        parent,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("native Text staged file occupied"))?;
    regular(&file, uid)?;
    for block in bytes.chunks(65_536) {
        active(deadline, cancelled)?;
        file.write_all(block).map_err(|_| bad_plan())?;
    }
    file.sync_all().map_err(|_| bad_plan())?;
    parent.sync_all().map_err(|_| bad_plan())?;
    if read_at(parent, name, uid, bytes.len(), deadline, cancelled)?.as_deref() != Some(bytes) {
        return Err(SourceCommandError::Conflict(
            "native Text staged readback differs",
        ));
    }
    Ok(())
}

fn enumerate(
    parent: &File,
    expected: &[&str],
    missing_allowed: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut seen = vec![false; expected.len()];
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", parent.as_raw_fd()))
        .map_err(|_| SourceCommandError::Denied("native Text directory enumeration"))?;
    for entry in entries {
        active(deadline, cancelled)?;
        let name = entry.map_err(|_| bad_plan())?.file_name();
        let name = name.to_str().ok_or(bad_plan())?;
        let Some(index) = expected.iter().position(|value| *value == name) else {
            return Err(SourceCommandError::Conflict(
                "native Text foreign staged member",
            ));
        };
        if std::mem::replace(&mut seen[index], true) {
            return Err(SourceCommandError::Conflict(
                "native Text duplicate staged member",
            ));
        }
    }
    active(deadline, cancelled)?;
    if !missing_allowed && seen.iter().any(|found| !found) {
        return Err(SourceCommandError::Conflict(
            "native Text missing staged member",
        ));
    }
    Ok(())
}

fn lock_at(
    parent: &File,
    name: &str,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<File> {
    let lock: File = rustix::fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Denied("native Text lock unsafe"))?;
    regular(&lock, uid)?;
    loop {
        active(deadline, cancelled)?;
        match rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => break,
            Err(Errno::AGAIN) => thread::sleep(Duration::from_millis(5)),
            Err(_) => return Err(SourceCommandError::Denied("native Text lock unavailable")),
        }
    }
    let current: File = rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("native Text lock detached"))?;
    if stamp(&regular(&lock, uid)?) != stamp(&regular(&current, uid)?) {
        return Err(SourceCommandError::Conflict("native Text lock replaced"));
    }
    Ok(lock)
}

pub(crate) struct PrivateTextLocks {
    _historical: File,
    _private: File,
}

pub(crate) enum PrivateTextCustody {
    Absent,
    Published(BTreeMap<String, Vec<u8>>),
    Pending(BTreeMap<String, Vec<u8>>),
}

fn control_name(target: &Path, request: &JsonValue) -> SourceCommandResult<String> {
    let basis = cmd::object(vec![
        ("target", cmd::string(target.to_str().ok_or(bad_plan())?)),
        ("command_id", cmd::field(request, "command_id")?.clone()),
    ]);
    Ok(format!(
        ".native-construction-{}.pending",
        Digest256::of_bytes(&cmd::canonical(&basis)?).to_hex()
    ))
}

/// Exact target/command bytes only. The owning Text caller still verifies
/// their original receipt and current source, rights and publication state.
pub(crate) fn observe_private_text(
    context: &OwnerTextContext,
    target_ref: &str,
    request: &JsonValue,
    expected: &[&str],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PrivateTextCustody> {
    active(deadline, cancelled)?;
    let target = context.private_new_package_target(target_ref)?;
    match target.symlink_metadata() {
        Ok(_) => {
            let package_ref = target_ref.rsplit_once('/').ok_or(bad_plan())?.0;
            let state = expected.iter().try_fold(MAX_PACKAGE, |total, name| {
                total
                    .checked_add(name.len())
                    .and_then(|n| n.checked_add(128))
                    .ok_or(bad_plan())
            })?;
            return context
                .private_package(package_ref, expected, state, deadline, cancelled)
                .map(PrivateTextCustody::Published);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err(SourceCommandError::Denied("native Text target observation")),
    }
    let uid = context.account_uid();
    let root = tos_fd_open::open_absolute_directory(context.private_root())
        .map_err(|_| SourceCommandError::Denied("native Text private root"))?;
    directory(&root, uid, true)?;
    let name = control_name(&target, request)?;
    let control = match rustix::fs::openat(
        &root,
        name.as_str(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) => return Ok(PrivateTextCustody::Absent),
        Err(_) => return Err(SourceCommandError::Denied("native Text control unsafe")),
    };
    directory(&control, uid, true)?;
    enumerate(
        &control,
        &["plan.json", "output"],
        true,
        deadline,
        cancelled,
    )?;
    let raw = read_at(&control, "plan.json", uid, MAX_CONTROL, deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("native Text control lacks plan"),
    )?;
    let target_rel = target
        .strip_prefix(context.private_root())
        .map_err(|_| bad_plan())?
        .to_str()
        .ok_or(bad_plan())?;
    let files = decode_plan(&raw, target_rel, request)?;
    if files.len() != expected.len() || files.keys().any(|name| !expected.contains(&name.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained file set differs",
        ));
    }
    match rustix::fs::openat(
        &control,
        "output",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let output = File::from(fd);
            directory(&output, uid, true)?;
            enumerate(&output, expected, true, deadline, cancelled)?;
            for (name, bytes) in &files {
                if read_at(&output, name, uid, bytes.len(), deadline, cancelled)?
                    .is_some_and(|existing| existing != *bytes)
                {
                    return Err(SourceCommandError::Conflict(
                        "native Text staged bytes differ",
                    ));
                }
            }
        }
        Err(Errno::NOENT) => (),
        Err(_) => return Err(SourceCommandError::Denied("native Text stage unsafe")),
    }
    if read_at(&control, "plan.json", uid, MAX_CONTROL, deadline, cancelled)?.as_deref()
        != Some(raw.as_slice())
    {
        return Err(SourceCommandError::Conflict(
            "native Text control changed during read",
        ));
    }
    Ok(PrivateTextCustody::Pending(files))
}

impl PrivateTextLocks {
    pub(crate) fn acquire(
        context: &OwnerTextContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let uid = context.account_uid();
        let public = tos_fd_open::open_absolute_directory(
            &context.public_root().join("ToS/source-witnesses"),
        )
        .map_err(|_| SourceCommandError::Denied("native Text historical home"))?;
        directory(&public, uid, false)?;
        let historical = lock_at(
            &public,
            ".historical-create.writer.lock",
            uid,
            deadline,
            cancelled,
        )?;
        let private = tos_fd_open::open_absolute_directory(context.private_root())
            .map_err(|_| SourceCommandError::Denied("native Text private home"))?;
        directory(&private, uid, true)?;
        let owner = lock_at(
            &private,
            ".native-create.writer.lock",
            uid,
            deadline,
            cancelled,
        )?;
        Ok(Self {
            _historical: historical,
            _private: owner,
        })
    }
}

/// The caller holds the selected Text grant and supplies its real current
/// source/rights/identity guard. The durable plan precedes all output; failed
/// or foreign stages remain visible to the owner and are never auto-cleaned.
pub(crate) fn publish_private_text(
    context: &OwnerTextContext,
    target_ref: &str,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    mut stage_guard: impl FnMut() -> SourceCommandResult<()>,
    mut final_guard: impl FnMut() -> SourceCommandResult<()>,
    mut verify_staged: Option<&mut dyn FnMut(&Path) -> SourceCommandResult<()>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    // Serialize the complete bounded plan before taking either publication
    // lock. The locked checks below still authenticate current inputs and the
    // exact retained plan before any selected destination mutation.
    let target = context.private_new_package_target(target_ref)?;
    let parent_path = target.parent().ok_or(bad_plan())?;
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(bad_plan())?;
    if !leaf(target_name) {
        return Err(bad_plan());
    }
    let target_rel = target
        .strip_prefix(context.private_root())
        .map_err(|_| bad_plan())?
        .to_str()
        .ok_or(bad_plan())?;
    let plan = encode_plan(target_rel, request, files)?;
    let control_name = control_name(&target, request)?;
    let mut held = Some(PrivateTextLocks::acquire(context, deadline, cancelled)?);
    stage_guard()?;
    if context.private_new_package_target(target_ref)? != target {
        return Err(SourceCommandError::Conflict(
            "native Text destination changed before staging",
        ));
    }
    let uid = context.account_uid();
    let parent = tos_fd_open::open_absolute_directory(parent_path)
        .map_err(|_| SourceCommandError::Denied("native Text destination parent"))?;
    let parent_before = directory(&parent, uid, true)?;
    let root = tos_fd_open::open_absolute_directory(context.private_root())
        .map_err(|_| SourceCommandError::Denied("native Text private root"))?;
    directory(&root, uid, true)?;
    match rustix::fs::mkdirat(&root, control_name.as_str(), Mode::from_raw_mode(0o700)) {
        Ok(()) => {
            root.sync_all().map_err(|_| bad_plan())?;
            let control = child(&root, &control_name, uid)?;
            write_new(&control, "plan.json", &plan, uid, deadline, cancelled)?;
        }
        Err(Errno::EXIST) => (),
        Err(_) => return Err(SourceCommandError::Denied("native Text control occupied")),
    }
    let control = child(&root, &control_name, uid)?;
    enumerate(
        &control,
        &["plan.json", "output"],
        true,
        deadline,
        cancelled,
    )?;
    let retained_raw = read_at(&control, "plan.json", uid, MAX_CONTROL, deadline, cancelled)?
        .ok_or(SourceCommandError::Conflict(
            "native Text control lacks plan",
        ))?;
    let retained = decode_plan(&retained_raw, target_rel, request)?;
    if retained != *files {
        return Err(SourceCommandError::Conflict(
            "native Text retained plan differs",
        ));
    }
    stage_guard()?;
    if read_at(&control, "plan.json", uid, MAX_CONTROL, deadline, cancelled)?.as_deref()
        != Some(retained_raw.as_slice())
    {
        return Err(SourceCommandError::Conflict("native Text plan changed"));
    }
    match rustix::fs::mkdirat(&control, "output", Mode::from_raw_mode(0o700)) {
        Ok(()) => control.sync_all().map_err(|_| bad_plan())?,
        Err(Errno::EXIST) => (),
        Err(_) => return Err(bad_plan()),
    }
    let output = child(&control, "output", uid)?;
    let expected: Vec<_> = retained.keys().map(String::as_str).collect();
    enumerate(&output, &expected, true, deadline, cancelled)?;
    for (name, bytes) in &retained {
        match read_at(&output, name, uid, bytes.len(), deadline, cancelled)? {
            Some(existing) if existing == *bytes => (),
            Some(_) => {
                return Err(SourceCommandError::Conflict(
                    "native Text staged bytes differ",
                ));
            }
            None => write_new(&output, name, bytes, uid, deadline, cancelled)?,
        }
    }
    enumerate(&output, &expected, false, deadline, cancelled)?;
    output.sync_all().map_err(|_| bad_plan())?;
    if let Some(verify_staged) = verify_staged.as_mut() {
        // This is the final source check for the first uninterrupted lock
        // hold. Owner verification runs only after these locks are released.
        final_guard()?;
        let root_before = directory(&root, uid, true)?;
        let control_before = directory(&control, uid, true)?;
        let output_before = directory(&output, uid, true)?;
        // The stronger owner may run Git and signature verification. It must
        // never hold either corpus publication lock while doing that work.
        held = None;
        let output_path = context.private_root().join(&control_name).join("output");
        verify_staged(&output_path)?;
        held = Some(PrivateTextLocks::acquire(context, deadline, cancelled)?);
        let current_root = tos_fd_open::open_absolute_directory(context.private_root())
            .map_err(|_| SourceCommandError::Conflict("native Text private root changed"))?;
        let current_control = child(&root, &control_name, uid)?;
        let current_output = child(&current_control, "output", uid)?;
        if stamp(&root_before) != stamp(&directory(&root, uid, true)?)
            || stamp(&root_before) != stamp(&directory(&current_root, uid, true)?)
            || stamp(&control_before) != stamp(&directory(&control, uid, true)?)
            || stamp(&control_before) != stamp(&directory(&current_control, uid, true)?)
            || stamp(&output_before) != stamp(&directory(&output, uid, true)?)
            || stamp(&output_before) != stamp(&directory(&current_output, uid, true)?)
        {
            return Err(SourceCommandError::Conflict(
                "native Text owner verification stage detached",
            ));
        }
        enumerate(&output, &expected, false, deadline, cancelled)?;
        for (name, bytes) in &retained {
            if read_at(&output, name, uid, bytes.len(), deadline, cancelled)?.as_deref()
                != Some(bytes.as_slice())
            {
                return Err(SourceCommandError::Conflict(
                    "native Text staged bytes changed during owner verification",
                ));
            }
        }
    }
    // A fresh complete check is required after owner verification reacquires
    // the locks; without that release this is the sole complete lock check.
    final_guard()?;
    if read_at(&control, "plan.json", uid, MAX_CONTROL, deadline, cancelled)?.as_deref()
        != Some(retained_raw.as_slice())
    {
        return Err(SourceCommandError::Conflict(
            "native Text plan changed before commit",
        ));
    }
    let current_parent = tos_fd_open::open_absolute_directory(parent_path)
        .map_err(|_| SourceCommandError::Conflict("native Text destination parent changed"))?;
    if stamp(&parent_before) != stamp(&directory(&parent, uid, true)?)
        || stamp(&parent_before) != stamp(&directory(&current_parent, uid, true)?)
    {
        return Err(SourceCommandError::Conflict(
            "native Text destination parent changed",
        ));
    }
    rustix::fs::renameat_with(
        &control,
        "output",
        &parent,
        target_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| {
        SourceCommandError::Conflict("native Text destination occupied or publication uncertain")
    })?;
    parent
        .sync_all()
        .map_err(|_| SourceCommandError::Conflict("native Text publication fsync uncertain"))?;
    control
        .sync_all()
        .map_err(|_| SourceCommandError::Conflict("native Text control fsync uncertain"))?;
    let package_ref = target_ref.rsplit_once('/').ok_or(bad_plan())?.0;
    let selected_state = expected.iter().try_fold(MAX_PACKAGE, |n, name| {
        n.checked_add(name.len())
            .and_then(|n| n.checked_add(128))
            .ok_or(bad_plan())
    })?;
    if context.private_package(package_ref, &expected, selected_state, deadline, cancelled)?
        != retained
    {
        return Err(SourceCommandError::Conflict(
            "native Text published bytes differ",
        ));
    }
    drop(held);
    Ok(())
}

/// Existing packet-mode TextUnit has a flat no-replace exchange, distinct
/// from the first-segmentation retained-plan route. This stage has no replay
/// authority: a foreign or uncertain stage is left visible and refused.
pub(crate) fn publish_flat_text(
    context: &OwnerTextContext,
    target_ref: &str,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    mut stage_guard: impl FnMut() -> SourceCommandResult<()>,
    mut final_guard: impl FnMut() -> SourceCommandResult<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if files.is_empty()
        || files.len() > MAX_FILES
        || files.keys().any(|name| !leaf(name))
        || files
            .values()
            .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
            .is_none_or(|sum| sum > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native Text flat package budget",
        ));
    }
    let _locks = PrivateTextLocks::acquire(context, deadline, cancelled)?;
    stage_guard()?;
    let target = context.private_new_package_target(target_ref)?;
    let parent_path = target.parent().ok_or(bad_plan())?;
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(bad_plan())?;
    let uid = context.account_uid();
    let root = tos_fd_open::open_absolute_directory(context.private_root())
        .map_err(|_| SourceCommandError::Denied("native Text private root"))?;
    directory(&root, uid, true)?;
    let parent = tos_fd_open::open_absolute_directory(parent_path)
        .map_err(|_| SourceCommandError::Denied("native Text flat parent"))?;
    let parent_before = directory(&parent, uid, true)?;
    let name =
        control_name(&target, request)?.replacen(".native-construction-", ".native-flat-", 1);
    rustix::fs::mkdirat(&root, name.as_str(), Mode::from_raw_mode(0o700))
        .map_err(|_| SourceCommandError::Conflict("native Text flat stage occupied"))?;
    root.sync_all().map_err(|_| bad_plan())?;
    let stage = child(&root, &name, uid)?;
    let expected = files.keys().map(String::as_str).collect::<Vec<_>>();
    let mut renamed = false;
    let staged = (|| -> SourceCommandResult<()> {
        for (member, raw) in files {
            active(deadline, cancelled)?;
            write_new(&stage, member, raw, uid, deadline, cancelled)?;
        }
        enumerate(&stage, &expected, false, deadline, cancelled)?;
        final_guard()?;
        for (member, raw) in files {
            if read_at(&stage, member, uid, raw.len(), deadline, cancelled)?.as_deref()
                != Some(raw.as_slice())
            {
                return Err(SourceCommandError::Conflict(
                    "native Text flat stage changed",
                ));
            }
        }
        let current_parent = tos_fd_open::open_absolute_directory(parent_path)
            .map_err(|_| SourceCommandError::Conflict("native Text flat parent changed"))?;
        if stamp(&parent_before) != stamp(&directory(&parent, uid, true)?)
            || stamp(&parent_before) != stamp(&directory(&current_parent, uid, true)?)
        {
            return Err(SourceCommandError::Conflict(
                "native Text flat parent changed",
            ));
        }
        rustix::fs::renameat_with(
            &root,
            name.as_str(),
            &parent,
            target_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|_| {
            SourceCommandError::Conflict("native Text flat destination occupied or uncertain")
        })?;
        renamed = true;
        parent
            .sync_all()
            .map_err(|_| SourceCommandError::Conflict("native Text flat fsync uncertain"))?;
        root.sync_all()
            .map_err(|_| SourceCommandError::Conflict("native Text flat root fsync uncertain"))?;
        let package_ref = target_ref.rsplit_once('/').ok_or(bad_plan())?.0;
        let state = expected.iter().try_fold(MAX_PACKAGE, |n, name| {
            n.checked_add(name.len())
                .and_then(|n| n.checked_add(128))
                .ok_or(bad_plan())
        })?;
        if context.private_package(package_ref, &expected, state, deadline, cancelled)? != *files {
            return Err(SourceCommandError::Conflict(
                "native Text flat published bytes differ",
            ));
        }
        Ok(())
    })();
    if staged.is_err() && !renamed {
        // Cleanup is restricted to our exact newly-created stage. Any
        // unfamiliar member or changed byte keeps the stage for owner review.
        let unchanged = expected.iter().all(|member| {
            files.get(*member).is_some_and(|raw| {
                read_at(&stage, member, uid, raw.len(), deadline, cancelled)
                    .ok()
                    .flatten()
                    .is_some_and(|now| now == *raw)
            })
        });
        if unchanged && enumerate(&stage, &expected, false, deadline, cancelled).is_ok() {
            for member in &expected {
                rustix::fs::unlinkat(&stage, *member, AtFlags::empty()).map_err(|_| bad_plan())?;
            }
            stage.sync_all().map_err(|_| bad_plan())?;
            rustix::fs::unlinkat(&root, name.as_str(), AtFlags::REMOVEDIR)
                .map_err(|_| bad_plan())?;
            root.sync_all().map_err(|_| bad_plan())?;
        }
    }
    staged
}
