//! Read-only exact native unit disclosure. Metadata handles confer no text grant.
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
pub use crate::source_sign_native::{NativeReadKind, SignNativeRead};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
const SCHEMA: &str = "ToS/contracts/native-local-text-read.schema.json";

/// Exact selected source identity supplied by the read-only owner session.
/// The caller pins the pathname, root FD/inode, source vector and full revision.
pub trait NativeUnitRead: SignNativeRead {
    fn source_root(&self) -> &Path;
}

/// Independently selected protected local conditions for already public text.
/// This object never grants private transport or external publication.
pub struct LocalTextReadSelection {
    path: PathBuf,
    source_root: PathBuf,
    raw: Vec<u8>,
    config: JsonValue,
    schema_raw: Vec<u8>,
    schema_ref: &'static str,
    uid: u32,
}
impl LocalTextReadSelection {
    pub fn load(
        path: &Path,
        source_root: &Path,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::load_profile(path, source_root, SCHEMA, worker, deadline, cancelled)
    }
    fn load_profile(
        path: &Path,
        source_root: &Path,
        schema_ref: &'static str,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        for candidate in [path, source_root] {
            let name = candidate
                .to_str()
                .ok_or(SourceCommandError::Invalid("local text absolute path UTF8"))?;
            if crate::source_text_owner::normalized_absolute(name)? != candidate {
                return Err(SourceCommandError::Denied(
                    "local text selection normalized absolute roots",
                ));
            }
        }
        let uid = rustix::process::getuid().as_raw();
        let raw =
            crate::source_text_owner::read_absolute(path, uid, true, 262_144, deadline, cancelled)?;
        let config = cmd::parse(&raw)?;
        let schema_raw = crate::source_text_owner::read_absolute(
            &source_root.join(schema_ref),
            uid,
            false,
            65_536,
            deadline,
            cancelled,
        )?;
        let schema = cmd::parse(&schema_raw)?;
        if cmd::text(&schema, "$id")? != format!("https://tree-of-sophia.local/{schema_ref}")
            || worker.contract_digest(schema_ref) != Some(Digest256::of_bytes(&schema_raw))
            || !worker
                .check(
                    "native-local-text-read-selection",
                    &raw,
                    schema_ref,
                    deadline,
                    cancelled,
                )
                .map_err(|_| SourceCommandError::Denied("local text selection grammar execution"))?
            || cmd::text(&config, "source_root")?
                != source_root
                    .to_str()
                    .ok_or(SourceCommandError::Invalid("local text root UTF8"))?
            || cmd::integer(&config, "owner_uid")? != u64::from(uid)
        {
            return Err(SourceCommandError::Denied(
                "local text selection owner grammar or source differs",
            ));
        }
        let mut bindings = BTreeSet::new();
        for row in cmd::array(&config, "selections")? {
            if !bindings.insert(cmd::text(row, "binding_sha256")?) {
                return Err(SourceCommandError::Denied(
                    "local text duplicate binding selection",
                ));
            }
        }
        let result = Self {
            path: path.to_owned(),
            source_root: source_root.to_owned(),
            raw,
            config,
            schema_raw,
            schema_ref,
            uid,
        };
        result.verify(deadline, cancelled)?;
        Ok(result)
    }
    pub fn verify(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        let now = crate::source_serialization::instant()?;
        let issued = cmd::text(&self.config, "issued_at")?;
        let expires = cmd::text(&self.config, "expires_at")?;
        use tos_validation::retirement_rules::{
            observed_instant_elapsed_micros, observed_instant_order,
        };
        if observed_instant_order(issued, &now)
            .map_err(|_| SourceCommandError::Denied("local text issued instant"))?
            == std::cmp::Ordering::Greater
            || observed_instant_order(&now, expires)
                .map_err(|_| SourceCommandError::Denied("local text expiry instant"))?
                != std::cmp::Ordering::Less
            || !(0..=86_400_000_000).contains(
                &observed_instant_elapsed_micros(issued, expires)
                    .map_err(|_| SourceCommandError::Denied("local text lifetime"))?,
            )
        {
            return Err(SourceCommandError::Denied(
                "local text selection expired or exceeds one day",
            ));
        }
        let read = |path: &Path, confidential, max| {
            crate::source_text_owner::read_absolute(
                path,
                self.uid,
                confidential,
                max,
                deadline,
                cancelled,
            )
        };
        let mandate = cmd::field(&self.config, "mandate")?;
        let mandate_path =
            crate::source_text_owner::normalized_absolute(cmd::text(mandate, "path")?)?;
        if read(&self.path, true, 262_144)? != self.raw
            || read(&self.source_root.join(self.schema_ref), false, 65_536)? != self.schema_raw
            || Digest256::of_bytes(&read(&mandate_path, false, 1_048_576)?).to_hex()
                != cmd::text(mandate, "sha256")?
        {
            return Err(SourceCommandError::Denied(
                "local text selection revoked or changed",
            ));
        }
        Ok(())
    }
    pub(crate) fn selected_binding(
        &self,
        binding: &JsonValue,
        rights: Option<&JsonValue>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<&JsonValue> {
        self.verify(deadline, cancelled)?;
        let key = Digest256::of_bytes(&cmd::canonical(binding)?).to_hex();
        let selected = cmd::array(&self.config, "selections")?
            .iter()
            .find(|row| {
                row.object_get("binding_sha256").and_then(JsonValue::as_str) == Some(key.as_str())
            })
            .ok_or(SourceCommandError::Denied(
                "local text exact binding not selected",
            ))?;
        if let Some(rights) = rights {
            if !cmd::same(cmd::field(selected, "rights_record_refs")?, rights)? {
                return Err(SourceCommandError::Denied(
                    "local text selected rights differ",
                ));
            }
        }
        Ok(selected)
    }
    pub(crate) fn private_conditions(
        &self,
        selected: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
        if self.schema_ref != PRIVATE_SCHEMA {
            return Err(SourceCommandError::Denied(
                "private text requires private selection",
            ));
        }
        Ok(cmd::object(vec![
            (
                "selection_sha256",
                cmd::string(&Digest256::of_bytes(&self.raw).to_hex()),
            ),
            (
                "expires_at",
                cmd::field(&self.config, "expires_at")?.clone(),
            ),
            (
                "condition_review",
                cmd::field(selected, "condition_review")?.clone(),
            ),
        ]))
    }
    pub(crate) fn select<R: SignNativeRead + ?Sized>(
        &self,
        reader: &mut R,
        binding: &JsonValue,
        rights: &JsonValue,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<JsonValue> {
        self.verify(deadline, cancelled)?;
        let selected = self.selected_binding(binding, Some(rights), deadline, cancelled)?;
        let mut remaining = 32_768;
        let mut notices = Vec::new();
        let mut roles = BTreeSet::new();
        for row in cmd::array(selected, "notices")? {
            let name = cmd::text(row, "ref")?;
            RelativePath::parse(name)
                .map_err(|_| SourceCommandError::Denied("local text notice path"))?;
            if !["LICENSE", "NOTICE", "NOTICE.md", "README.md"].contains(&name)
                && !(name.starts_with("ToS/review-ledger/") && name.ends_with(".md"))
            {
                return Err(SourceCommandError::Denied(
                    "local text notice outside public support",
                ));
            }
            if reader.owner_local(name)? {
                return Err(SourceCommandError::Denied(
                    "local text notice private transport",
                ));
            }
            let raw = reader.read(
                name,
                NativeReadKind::Support,
                remaining,
                deadline,
                cancelled,
            )?;
            if raw.len() > remaining
                || Digest256::of_bytes(&raw).to_hex() != cmd::text(row, "sha256")?
            {
                return Err(SourceCommandError::Denied(
                    "local text notice changed or over budget",
                ));
            }
            remaining -= raw.len();
            roles.insert(cmd::text(row, "role")?);
            let mut notice = row.clone();
            let text = std::str::from_utf8(&raw)
                .map_err(|_| SourceCommandError::Denied("local text notice UTF8"))?;
            notice = cmd::object(vec![
                ("ref", cmd::field(&notice, "ref")?.clone()),
                ("sha256", cmd::field(&notice, "sha256")?.clone()),
                ("role", cmd::field(&notice, "role")?.clone()),
                ("text", cmd::string(text)),
            ]);
            notices.push(notice);
        }
        if !roles.contains("license") || !roles.contains("attribution") {
            return Err(SourceCommandError::Denied(
                "local text license and attribution required",
            ));
        }
        reader.verify_current(deadline, cancelled)?;
        Ok(cmd::object(vec![
            (
                "selection_sha256",
                cmd::string(&Digest256::of_bytes(&self.raw).to_hex()),
            ),
            (
                "expires_at",
                cmd::field(&self.config, "expires_at")?.clone(),
            ),
            (
                "condition_review",
                cmd::field(selected, "condition_review")?.clone(),
            ),
            ("notices", JsonValue::Array(notices)),
        ]))
    }
}

pub fn read_native_unit<R: NativeUnitRead + ?Sized>(
    reader: &mut R,
    worker: &mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    local_selection: Option<&LocalTextReadSelection>,
    max_return_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    if let Some(selection) = local_selection {
        if reader.source_root() != selection.source_root {
            return Err(SourceCommandError::Denied(
                "local text selection source differs",
            ));
        }
    }
    crate::source_sign_native::read_disclosed_unit(
        reader,
        worker,
        binding,
        local_selection,
        max_return_bytes,
        deadline,
        cancelled,
    )
}

const PRIVATE_SCHEMA: &str = "ToS/contracts/native-private-text-read.schema.json";
const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";

/// A separately selected private research grant. Its owner context is protected
/// byte transport only. It is never an HTTP credential or a public source handle.
pub struct PrivateTextReadSelection {
    grant: LocalTextReadSelection,
    context: crate::source_text_owner::OwnerTextContext,
}
impl PrivateTextReadSelection {
    pub fn load(
        path: &Path,
        source_root: &Path,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let grant = LocalTextReadSelection::load_profile(
            path,
            source_root,
            PRIVATE_SCHEMA,
            worker,
            deadline,
            cancelled,
        )?;
        let selected = cmd::field(&grant.config, "owner_context")?;
        let context_path =
            crate::source_text_owner::normalized_absolute(cmd::text(selected, "path")?)?;
        let raw = crate::source_text_owner::read_absolute(
            &context_path,
            grant.uid,
            true,
            65_536,
            deadline,
            cancelled,
        )?;
        if Digest256::of_bytes(&raw).to_hex() != cmd::text(selected, "sha256")? {
            return Err(SourceCommandError::Denied(
                "private text owner context differs",
            ));
        }
        let schema = crate::source_text_owner::read_absolute(
            &source_root.join(CONTEXT_SCHEMA),
            grant.uid,
            false,
            65_536,
            deadline,
            cancelled,
        )?;
        let (context, _) = crate::source_text_owner::OwnerTextContext::select(
            &context_path,
            &schema,
            worker,
            deadline,
            cancelled,
        )?;
        if context.public_root() != source_root {
            return Err(SourceCommandError::Denied(
                "private text owner context source differs",
            ));
        }
        let result = Self { grant, context };
        result.verify(deadline, cancelled)?;
        Ok(result)
    }
    pub fn verify(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        self.grant.verify(deadline, cancelled)?;
        let selected = cmd::field(&self.grant.config, "owner_context")?;
        let context_path =
            crate::source_text_owner::normalized_absolute(cmd::text(selected, "path")?)?;
        let raw = crate::source_text_owner::read_absolute(
            &context_path,
            self.grant.uid,
            true,
            65_536,
            deadline,
            cancelled,
        )?;
        if Digest256::of_bytes(&raw).to_hex() != cmd::text(selected, "sha256")? {
            return Err(SourceCommandError::Denied(
                "private text owner context changed",
            ));
        }
        self.context.snapshot(deadline, cancelled)?;
        Ok(())
    }
}

/// Return exact selected spans and their full recorded rights to the local
/// owner process. Source rights and current grant checks precede text I/O and
/// are rechecked before return. No original Item payload is opened.
pub fn read_private_native_unit(
    selection: &mut PrivateTextReadSelection,
    worker: &mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    max_return_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    selection.verify(deadline, cancelled)?;
    let result = crate::source_sign_native::read_private_disclosed_unit(
        &mut selection.context,
        worker,
        binding,
        &selection.grant,
        max_return_bytes,
        deadline,
        cancelled,
    )?;
    selection.verify(deadline, cancelled)?;
    Ok(result)
}
