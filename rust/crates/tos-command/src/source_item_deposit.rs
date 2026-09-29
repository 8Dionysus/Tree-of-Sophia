//! One actual protected Item File deposit under the existing corpus lock.
//! Original and copied payload custody stays here; metadata validation receives
//! only the observed inventory and the source-safe completed byte receipt.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, owned, work_transaction};
use crate::source_serialization::instant;
use crate::source_text_layer_payload::{owned_file, parents};
use rustix::fs::{Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use serde_json::{Value, json};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, JsonValue};

const STAGE_FILE: &str = "item-deposit.json";
const STAGE_SCHEMA: &str = "tos_item_deposit_stage_v1";
const MAX_BYTES: u64 = 512 * 1024 * 1024;
const CHUNK: usize = 1024 * 1024;
const MAX_STAGE: usize = 2 * 1024 * 1024;

fn denied() -> SourceCommandError {
    SourceCommandError::Denied("Item protected payload custody")
}
fn conflict() -> SourceCommandError {
    SourceCommandError::Conflict("Item retained payload binding changed")
}
fn text<'a>(value: &'a Value, key: &str) -> SourceCommandResult<&'a str> {
    value.get(key).and_then(Value::as_str).ok_or(denied())
}
pub(crate) fn value(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?).map_err(|_| denied())
}
pub(crate) fn foundation(value: &Value) -> SourceCommandResult<JsonValue> {
    cmd::parse(&serde_json::to_vec(value).map_err(|_| denied())?)
}
fn canonical(value: &Value) -> SourceCommandResult<Vec<u8>> {
    cmd::canonical(&foundation(value)?)
}
fn digest(value: &Value) -> SourceCommandResult<String> {
    Ok(Digest256::of_bytes(&canonical(value)?).to_prefixed())
}
fn inode(meta: &Metadata) -> Value {
    json!([meta.dev(), meta.ino(), meta.mode(), meta.uid()])
}
fn identity(meta: &Metadata) -> SourceCommandResult<Value> {
    let nanos = |sec: i64, subsec: i64| {
        sec.checked_mul(1_000_000_000)
            .and_then(|v| v.checked_add(subsec))
            .ok_or(conflict())
    };
    Ok(json!([
        meta.dev(),
        meta.ino(),
        meta.mode(),
        meta.uid(),
        meta.len(),
        nanos(meta.mtime(), meta.mtime_nsec())?,
        nanos(meta.ctime(), meta.ctime_nsec())?
    ]))
}
fn pins(
    path: &Path,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let mut result = serde_json::Map::new();
    for (name, (dev, ino, mode, owner)) in parents(path, uid, deadline, cancelled)? {
        result.insert(name, json!([dev, ino, mode, owner]));
    }
    Ok(Value::Object(result))
}
fn absolute(value: &str) -> SourceCommandResult<&Path> {
    let path = Path::new(value);
    if !path.is_absolute()
        || value.contains(['\\', '\0'])
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        || path.to_str() != Some(value)
        || value.ends_with('/') && value != "/"
    {
        return Err(denied());
    }
    // Component normalization can hide repeated separators or a dot.
    if value != "/"
        && value[1..]
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(denied());
    }
    Ok(path)
}
pub(crate) fn destination(config: &Value) -> SourceCommandResult<PathBuf> {
    let item = Path::new(text(config, "item_source_path")?);
    let home = item
        .parent()
        .ok_or(denied())?
        .strip_prefix("ToS/source-witnesses")
        .map_err(|_| denied())?;
    Ok(absolute(text(config, "payload_root")?)?
        .join(home)
        .join("payload")
        .join(text(config, "payload_basename")?))
}
pub(crate) fn validate_config(config: &Value, uid: u32) -> SourceCommandResult<()> {
    cmd::validate_expiry(text(config, "payload_expires_at")?, &instant()?)?;
    validate_retained_config(config, uid)
}
pub(crate) fn validate_retained_config(config: &Value, uid: u32) -> SourceCommandResult<()> {
    let input = absolute(text(config, "input_path")?)?;
    let payload = absolute(text(config, "payload_root")?)?;
    let recovery = absolute(text(config, "recovery_root")?)?;
    let source = absolute(text(config, "source_root")?)?;
    if text(config, "payload_authority_ref")?.trim().is_empty()
        || !(1..=MAX_BYTES).contains(
            &config
                .get("byte_size")
                .and_then(Value::as_u64)
                .ok_or(denied())?,
        )
    {
        return Err(denied());
    }
    let root = tos_fd_open::open_absolute_directory(payload).map_err(|_| denied())?;
    owned(&root, uid, true)?;
    let mut excluded = vec![source, payload, input];
    if payload.ends_with("ToS/source-witnesses") {
        excluded.push(payload.parent().and_then(Path::parent).ok_or(denied())?);
    }
    if excluded
        .iter()
        .any(|p| recovery.starts_with(p) || p.starts_with(recovery))
        || destination(config)? == input
    {
        return Err(denied());
    }
    let root = tos_fd_open::open_absolute_directory(recovery).map_err(|_| denied())?;
    if owned(&root, uid, true)?.mode() & 0o7777 != 0o700 {
        return Err(denied());
    }
    // Pin/protect all ancestors before these roots are used as custody.
    crate::source_creation_store::protected_configuration_parents(&recovery.join(STAGE_FILE), uid)?;
    crate::source_creation_store::protected_configuration_parents(&payload.join(STAGE_FILE), uid)?;
    Ok(())
}
pub(crate) fn payload_entry(config: &Value) -> SourceCommandResult<Value> {
    Ok(
        json!({"file_id": text(config,"file_id")?, "relative_path": format!("payload/{}",text(config,"payload_basename")?),
        "original_basename": text(config,"original_basename")?, "media_type": text(config,"media_type")?,
        "byte_size": config["byte_size"], "sha256": text(config,"sha256")?}),
    )
}
fn open_file(path: &Path, uid: u32) -> SourceCommandResult<(File, Metadata)> {
    let file = tos_fd_open::open_absolute_regular(path, MAX_BYTES).map_err(|_| denied())?;
    let meta = owned_file(&file, uid)?;
    Ok((file, meta))
}
fn exact_bytes(
    file: &mut File,
    config: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<()> {
    file.seek(SeekFrom::Start(0)).map_err(|_| denied())?;
    let expected = config["byte_size"].as_u64().ok_or(denied())?;
    let mut hash = Digest256Hasher::new();
    let mut total = 0u64;
    let mut block = vec![0; CHUNK];
    loop {
        active(deadline, cancelled)?;
        authorize()?;
        let count = file.read(&mut block).map_err(|_| denied())?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).ok_or(conflict())?;
        if total > expected {
            return Err(conflict());
        }
        hash.update(&block[..count]);
    }
    if total != expected || hash.finalize().to_hex() != text(config, "sha256")? {
        return Err(conflict());
    }
    Ok(())
}
fn current_file(
    path: &Path,
    file: &File,
    before: &Metadata,
    uid: u32,
    selected_parents: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let (_, named) = open_file(path, uid)?;
    if identity(&owned_file(file, uid)?)? != identity(before)?
        || identity(&named)? != identity(before)?
        || pins(path, uid, deadline, cancelled)? != *selected_parents
    {
        return Err(conflict());
    }
    Ok(())
}
pub(crate) fn observe(
    config: &Value,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Value> {
    authorize()?;
    let started_at = instant()?;
    validate_config(config, uid)?;
    let input = absolute(text(config, "input_path")?)?;
    let selected_parents = pins(input, uid, deadline, cancelled)?;
    let (mut file, before) = open_file(input, uid)?;
    if before.len() != config["byte_size"].as_u64().ok_or(denied())? {
        return Err(conflict());
    }
    exact_bytes(&mut file, config, deadline, cancelled, authorize)?;
    authorize()?;
    let mut authority_error = None;
    let inventory_result =
        crate::source_item_inventory::observe(&mut file, config, deadline, cancelled, &mut || {
            authorize().map_err(|error| {
                authority_error = Some(error.clone());
                error
            })
        });
    if let Some(error) = authority_error {
        return Err(error);
    }
    let (inventory, limitation) = match inventory_result {
        Ok(v) if canonical(&v)?.len() <= 256 * 1024 => (v, Value::Null),
        Ok(_)
        | Err(SourceCommandError::Unsupported(_))
        | Err(SourceCommandError::Invalid(_))
        | Err(SourceCommandError::Conflict(_)) => (
            Value::Null,
            json!("inventory-unavailable:InventoryBuildError"),
        ),
        Err(error) => return Err(error),
    };
    current_file(
        input,
        &file,
        &before,
        uid,
        &selected_parents,
        deadline,
        cancelled,
    )?;
    authorize()?;
    Ok(
        json!({"input_identity":identity(&before)?,"input_parents":selected_parents,
        "inventory":inventory,"limitation":limitation,
        "observation_interval":{"started_at":started_at,"ended_at":instant()?}}),
    )
}
fn companion(
    config: &Value,
    id: &str,
    uid: u32,
    create: bool,
) -> SourceCommandResult<Option<File>> {
    if !work_transaction::is_hash(id) {
        return Err(denied());
    }
    let root = tos_fd_open::open_absolute_directory(absolute(text(config, "recovery_root")?)?)
        .map_err(|_| denied())?;
    if owned(&root, uid, true)?.mode() & 0o7777 != 0o700 {
        return Err(denied());
    }
    if create {
        match rustix::fs::mkdirat(&root, &id[7..], Mode::from_raw_mode(0o700)) {
            Ok(()) => root.sync_all().map_err(|_| denied())?,
            Err(Errno::EXIST) => (),
            Err(_) => return Err(denied()),
        }
    }
    let child = match rustix::fs::openat(
        &root,
        &id[7..],
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => return Err(denied()),
    };
    if owned(&child, uid, true)?.mode() & 0o7777 != 0o700 {
        return Err(denied());
    }
    Ok(Some(child))
}
pub(crate) fn read_stage(
    config: &Value,
    id: &str,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Value>> {
    let Some(home) = companion(config, id, uid, false)? else {
        return Ok(None);
    };
    let Some((raw, mode)) =
        work_transaction::read_at_mode(&home, STAGE_FILE, uid, MAX_STAGE, deadline, cancelled)?
    else {
        return Ok(None);
    };
    if mode != 0o600 {
        return Err(denied());
    }
    let stage = value(&cmd::parse(&raw)?)?;
    if text(&stage, "schema_version")? != STAGE_SCHEMA || text(&stage, "transaction_id")? != id {
        return Err(conflict());
    }
    Ok(Some(stage))
}
fn save(
    config: &Value,
    id: &str,
    stage: &Value,
    previous: Option<&Value>,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let home = companion(config, id, uid, true)?.ok_or(denied())?;
    let now = read_stage(config, id, uid, deadline, cancelled)?;
    if now.as_ref() != previous {
        return Err(conflict());
    }
    let mut raw = canonical(stage)?;
    raw.push(b'\n');
    if raw.len() > MAX_STAGE {
        return Err(denied());
    }
    work_transaction::atomic_write(
        &home,
        STAGE_FILE,
        &raw,
        previous.is_none(),
        deadline,
        cancelled,
    )
}
fn target_home(config: &Value, uid: u32, create: bool) -> SourceCommandResult<File> {
    let root_path = absolute(text(config, "payload_root")?)?;
    let target = destination(config)?;
    let relative = target
        .parent()
        .ok_or(denied())?
        .strip_prefix(root_path)
        .map_err(|_| denied())?;
    let mut parent = tos_fd_open::open_absolute_directory(root_path).map_err(|_| denied())?;
    owned(&parent, uid, true)?;
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err(denied());
        };
        if create {
            match rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
                Ok(()) => parent.sync_all().map_err(|_| denied())?,
                Err(Errno::EXIST) => (),
                Err(_) => return Err(denied()),
            }
        }
        parent = tos_fd_open::open_directory_at(&parent, Path::new(name)).map_err(|_| denied())?;
        let meta = owned(&parent, uid, true)?;
        if meta.mode() & 0o7000 != 0 {
            return Err(denied());
        }
    }
    Ok(parent)
}
fn exists(parent: &File, name: &str) -> SourceCommandResult<bool> {
    match rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(Errno::NOENT) => Ok(false),
        Err(_) => Err(denied()),
    }
}
pub(crate) fn verify_deposit(
    config: &Value,
    stage: &Value,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<()> {
    authorize()?;
    let target = destination(config)?;
    let selected_parents = pins(&target, uid, deadline, cancelled)?;
    if selected_parents != stage["target_parents"] {
        return Err(conflict());
    }
    let (mut file, before) = open_file(&target, uid)?;
    if inode(&before) != stage["payload_inode"]
        || before.len() != config["byte_size"].as_u64().ok_or(denied())?
    {
        return Err(conflict());
    }
    exact_bytes(&mut file, config, deadline, cancelled, authorize)?;
    current_file(
        &target,
        &file,
        &before,
        uid,
        &selected_parents,
        deadline,
        cancelled,
    )?;
    authorize()
}
pub(crate) fn ensure_deposit(
    config: &Value,
    request: &Value,
    id: &str,
    uid: u32,
    recovery: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Value> {
    authorize()?;
    let mut observation = observe(config, uid, deadline, cancelled, authorize)?;
    let interval = observation
        .as_object_mut()
        .ok_or(denied())?
        .remove("observation_interval")
        .ok_or(denied())?;
    if observation["inventory"] != request["inventory"]
        || observation["limitation"] != request["inventory_limitation"]
    {
        return Err(conflict());
    }
    let target = destination(config)?;
    let binding = json!({"request":request,"configuration":config});
    let retained = read_stage(config, id, uid, deadline, cancelled)?;
    let mut stage = if let Some(stage) = retained {
        if stage["binding"] != binding {
            let original = &stage["binding"]["configuration"];
            let renewable = [
                "principal_id",
                "maker_type",
                "authority_ref",
                "expires_at",
                "allowed_operations",
                "payload_authority_ref",
                "payload_expires_at",
            ];
            if !recovery
                || stage["binding"]["request"] != *request
                || digest(original)? != text(request, "expected_configuration")?
                || config.as_object().ok_or(denied())?.iter().any(|(key, v)| {
                    !renewable.contains(&key.as_str())
                        && !key.starts_with("allowed_")
                        && original.get(key) != Some(v)
                })
            {
                return Err(conflict());
            }
        }
        if stage["observation"] != observation {
            return Err(conflict());
        }
        stage
    } else {
        match target.symlink_metadata() {
            Ok(_) => return Err(conflict()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(denied()),
        }
        let stage = json!({"schema_version":STAGE_SCHEMA,"transaction_id":id,"binding":binding,
            "observation":observation,"observation_interval":interval,"state":"prepared","created_at":instant()?,
            "payload_inode":null,"target_parents":null,"deposited_at":null,"recovery_authorization":null});
        save(config, id, &stage, None, uid, deadline, cancelled)?;
        stage
    };
    if text(&stage, "state")? == "rolled-back-retained" {
        return Err(conflict());
    }
    if recovery {
        let previous = stage.clone();
        stage["recovery_authorization"] = json!({"owner_configuration":digest(config)?,"principal_id":text(config,"principal_id")?,
            "authority_ref":text(config,"authority_ref")?,"authorized_at":instant()?,"decision":"resume"});
        save(
            config,
            id,
            &stage,
            Some(&previous),
            uid,
            deadline,
            cancelled,
        )?;
    }
    if text(&stage, "state")? == "deposited" {
        verify_deposit(config, &stage, uid, deadline, cancelled, authorize)?;
        return Ok(stage);
    }
    authorize()?;
    let home = target_home(config, uid, true)?;
    let selected_parents = pins(&target, uid, deadline, cancelled)?;
    let name = target
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or(denied())?;
    let partial = format!(".item-{}.partial", &id[7..]);
    authorize()?;
    if text(&stage, "state")? == "prepared" {
        if exists(&home, name)? {
            return Err(conflict());
        }
        let file = File::from(
            rustix::fs::openat(
                &home,
                partial.as_str(),
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| conflict())?,
        );
        let previous = stage.clone();
        stage["state"] = json!("copying");
        stage["payload_inode"] = inode(&owned_file(&file, uid)?);
        stage["target_parents"] = selected_parents.clone();
        file.sync_all().map_err(|_| denied())?;
        home.sync_all().map_err(|_| denied())?;
        save(
            config,
            id,
            &stage,
            Some(&previous),
            uid,
            deadline,
            cancelled,
        )?;
    }
    if text(&stage, "state")? != "copying" || stage["target_parents"] != selected_parents {
        return Err(conflict());
    }
    if exists(&home, name)? {
        verify_deposit(config, &stage, uid, deadline, cancelled, authorize)?;
    } else {
        let mut copied = File::from(
            rustix::fs::openat(
                &home,
                partial.as_str(),
                OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| conflict())?,
        );
        let partial_before = owned_file(&copied, uid)?;
        let input = absolute(text(config, "input_path")?)?;
        authorize()?;
        let (mut original, original_before) = open_file(input, uid)?;
        let size = config["byte_size"].as_u64().ok_or(denied())?;
        if inode(&partial_before) != stage["payload_inode"]
            || partial_before.len() > size
            || identity(&original_before)? != observation["input_identity"]
        {
            return Err(conflict());
        }
        let mut block = vec![0; CHUNK];
        let mut prefix = vec![0; CHUNK];
        let mut offset = 0u64;
        let mut hash = Digest256Hasher::new();
        loop {
            active(deadline, cancelled)?;
            authorize()?;
            let count = original.read(&mut block).map_err(|_| denied())?;
            if count == 0 {
                break;
            }
            if offset.checked_add(count as u64).is_none_or(|n| n > size) {
                return Err(conflict());
            }
            let retained = partial_before
                .len()
                .saturating_sub(offset)
                .min(count as u64) as usize;
            if retained > 0 {
                copied
                    .read_exact(&mut prefix[..retained])
                    .map_err(|_| conflict())?;
                if prefix[..retained] != block[..retained] {
                    return Err(conflict());
                }
            }
            copied
                .write_all(&block[retained..count])
                .map_err(|_| denied())?;
            hash.update(&block[..count]);
            offset += count as u64;
        }
        if offset != size
            || hash.finalize().to_hex() != text(config, "sha256")?
            || identity(&owned_file(&original, uid)?)? != observation["input_identity"]
            || pins(input, uid, deadline, cancelled)? != observation["input_parents"]
        {
            return Err(conflict());
        }
        let (_, named) = open_file(input, uid)?;
        if identity(&named)? != observation["input_identity"]
            || pins(&target, uid, deadline, cancelled)? != selected_parents
        {
            return Err(conflict());
        }
        authorize()?;
        copied.sync_all().map_err(|_| denied())?;
        if inode(&owned_file(&copied, uid)?) != stage["payload_inode"] {
            return Err(conflict());
        }
        // Verify the pathname still selects our retained partial before moving it.
        let named_partial = File::from(
            rustix::fs::openat(
                &home,
                partial.as_str(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| conflict())?,
        );
        if identity(&owned_file(&named_partial, uid)?)? != identity(&owned_file(&copied, uid)?)? {
            return Err(conflict());
        }
        rustix::fs::renameat_with(&home, partial.as_str(), &home, name, RenameFlags::NOREPLACE)
            .map_err(|_| conflict())?;
        home.sync_all().map_err(|_| denied())?;
        verify_deposit(config, &stage, uid, deadline, cancelled, authorize)?;
    }
    let previous = stage.clone();
    stage["state"] = json!("deposited");
    stage["deposited_at"] = json!(instant()?);
    save(
        config,
        id,
        &stage,
        Some(&previous),
        uid,
        deadline,
        cancelled,
    )?;
    Ok(stage)
}
pub(crate) fn public_receipt(stage: &Value) -> SourceCommandResult<Value> {
    if text(stage, "state")? != "deposited" {
        return Err(conflict());
    }
    let config = &stage["binding"]["configuration"];
    Ok(
        json!({"schema_version":"tos_item_deposit_receipt_v1","transaction_id":text(stage,"transaction_id")?,
        "owner_configuration":digest(config)?,"private_stage_digest":digest(stage)?,
        "recovery_configuration":stage["recovery_authorization"].get("owner_configuration").unwrap_or(&Value::Null),
        "file":payload_entry(config)?,"started_at":stage["created_at"],"observation_interval":stage["observation_interval"],
        "deposited_at":stage["deposited_at"],"original_preserved":true,"metadata_committed":false,"grants_admission":false}),
    )
}
pub(crate) fn public_state(stage: &Value) -> SourceCommandResult<Value> {
    Ok(
        json!({"transaction_id":stage["transaction_id"],"state":stage["state"],
        "recovery_handle":{"transaction_id":stage["transaction_id"],"owner_configuration":digest(&stage["binding"]["configuration"])?},
        "file":payload_entry(&stage["binding"]["configuration"])? ,"inventory_limitation":stage["observation"]["limitation"],
        "original_preserved":true,"metadata_committed":false,"grants_admission":false}),
    )
}

/// The full File hash is checked at the existing full publication edges.
/// Intermediate mover checks retain the complete observed File identity,
/// protected namespace and exact private stage, without hashing 512 MiB once
/// for every selected metadata leaf.
pub(crate) fn deposited_identity(
    config: &Value,
    stage: &Value,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let target = destination(config)?;
    let (_, meta) = open_file(&target, uid)?;
    let target_parents = pins(&target, uid, deadline, cancelled)?;
    if inode(&meta) != stage["payload_inode"] || target_parents != stage["target_parents"] {
        return Err(conflict());
    }
    Ok(json!({"identity":identity(&meta)?,"parents":target_parents}))
}
pub(crate) fn stage_current(
    config: &Value,
    stage: &Value,
    selected_identity: &Value,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let id = text(stage, "transaction_id")?;
    if read_stage(config, id, uid, deadline, cancelled)?.as_ref() != Some(stage)
        || deposited_identity(config, stage, uid, deadline, cancelled)? != *selected_identity
    {
        return Err(conflict());
    }
    let input = absolute(text(config, "input_path")?)?;
    let (_, meta) = open_file(input, uid)?;
    if identity(&meta)? != stage["observation"]["input_identity"]
        || pins(input, uid, deadline, cancelled)? != stage["observation"]["input_parents"]
    {
        return Err(conflict());
    }
    Ok(())
}
pub(crate) fn rollback_retained(
    config: &Value,
    id: &str,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Value> {
    authorize()?;
    let mut stage = read_stage(config, id, uid, deadline, cancelled)?.ok_or(conflict())?;
    if text(&stage, "state")? == "deposited" {
        verify_deposit(config, &stage, uid, deadline, cancelled, authorize)?;
    }
    let previous = stage.clone();
    stage["state"] = json!("rolled-back-retained");
    save(
        config,
        id,
        &stage,
        Some(&previous),
        uid,
        deadline,
        cancelled,
    )?;
    Ok(
        json!({"transaction_id":id,"state":"rolled-back-retained","recovery_handle":{"transaction_id":id,"owner_configuration":digest(&stage["binding"]["configuration"])?},
        "file":payload_entry(&stage["binding"]["configuration"])? ,"inventory_limitation":stage["observation"]["limitation"],
        "original_preserved":true,"metadata_committed":false,"grants_admission":false}),
    )
}
