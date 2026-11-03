//! Read-only exact native unit disclosure. Metadata handles confer no text grant.
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
pub use crate::source_sign_native::{NativeReadKind, SignNativeRead};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::source_cut::CutSchemaExecutor;
const SCHEMA: &str = "ToS/contracts/native-local-text-read.schema.json";
pub(crate) const PG_PRIVATE_SCHEMA: &str = "ToS/contracts/native-pg-private-text-read.schema.json";

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
    pg_selected_schema: bool,
    uid: u32,
}
impl LocalTextReadSelection {
    pub fn load(
        path: &Path,
        source_root: &Path,
        worker: &mut dyn CutSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::load_profile(path, source_root, SCHEMA, worker, deadline, cancelled)
    }
    fn load_profile(
        path: &Path,
        source_root: &Path,
        schema_ref: &'static str,
        worker: &mut dyn CutSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let schema_raw = crate::source_text_owner::read_absolute(
            &source_root.join(schema_ref),
            rustix::process::getuid().as_raw(),
            false,
            65_536,
            deadline,
            cancelled,
        )?;
        Self::load_selected_profile(
            path,
            source_root,
            schema_ref,
            schema_raw,
            false,
            worker,
            deadline,
            cancelled,
        )
    }
    pub(crate) fn load_pg_private_profile(
        path: &Path,
        source_root: &Path,
        selected_schema_raw: &[u8],
        worker: &mut dyn CutSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::load_selected_profile(
            path,
            source_root,
            PG_PRIVATE_SCHEMA,
            selected_schema_raw.to_vec(),
            true,
            worker,
            deadline,
            cancelled,
        )
    }
    fn load_selected_profile(
        path: &Path,
        source_root: &Path,
        schema_ref: &'static str,
        schema_raw: Vec<u8>,
        pg_selected_schema: bool,
        worker: &mut dyn CutSchemaExecutor,
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
        let schema = cmd::parse(&schema_raw)?;
        if cmd::text(&schema, "$id")? != format!("https://tree-of-sophia.local/{schema_ref}")
            || worker.selected_contract_digest(schema_ref) != Some(Digest256::of_bytes(&schema_raw))
            || !worker
                .check(
                    if pg_selected_schema {
                        "native-pg-private-text-read-selection"
                    } else {
                        "native-local-text-read-selection"
                    },
                    &raw,
                    schema_ref,
                    deadline,
                    cancelled,
                )
                .map_err(|_| SourceCommandError::Denied("local text selection grammar execution"))?
            || !pg_selected_schema
                && cmd::text(&config, "source_root")?
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
            if pg_selected_schema {
                let mut packages = BTreeSet::new();
                for package in cmd::array(row, "packages")? {
                    let target = cmd::text(package, "target_ref")?;
                    if !target.starts_with("ToS/")
                        || RelativePath::parse(target).is_err()
                        || !packages.insert(target.to_owned())
                    {
                        return Err(SourceCommandError::Denied(
                            "PG private text duplicate or invalid package pin",
                        ));
                    }
                }
            }
        }
        let result = Self {
            path: path.to_owned(),
            source_root: source_root.to_owned(),
            raw,
            config,
            schema_raw,
            schema_ref,
            pg_selected_schema,
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
            || !self.pg_selected_schema
                && read(&self.source_root.join(self.schema_ref), false, 65_536)? != self.schema_raw
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
    pub(crate) fn selected_pg_packages(
        &self,
        binding: &JsonValue,
        rights: Option<&JsonValue>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<JsonValue>> {
        if self.schema_ref != PG_PRIVATE_SCHEMA {
            return Err(SourceCommandError::Denied(
                "PostgreSQL private package reads require a PG profile",
            ));
        }
        let selected = self.selected_binding(binding, rights, deadline, cancelled)?;
        Ok(cmd::array(selected, "packages")?.to_vec())
    }
    pub(crate) fn config(&self) -> &JsonValue {
        &self.config
    }
    pub(crate) fn protected_selection_digest(&self) -> String {
        Digest256::of_bytes(&self.raw).to_hex()
    }
    pub(crate) fn protected_selection_schema_digest(&self) -> String {
        Digest256::of_bytes(&self.schema_raw).to_hex()
    }

    pub(crate) fn selected_legacy_dta(
        &self,
        binding: &JsonValue,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<JsonValue>> {
        if self.schema_ref != PRIVATE_SCHEMA {
            return Ok(None);
        }
        self.selected_binding(binding, None, deadline, cancelled)?;
        let Some(value) = self.config.object_get("legacy_dta") else {
            return Ok(None);
        };
        let rows = value
            .as_array()
            .ok_or(SourceCommandError::Invalid("legacy DTA selection"))?;
        let packet = cmd::text(binding, "packet_ref")?;
        let sha = cmd::text(binding, "packet_sha256")?;
        for row in rows {
            if cmd::text(row, "packet_ref")? == packet {
                if cmd::text(row, "packet_sha256")? != sha {
                    return Err(SourceCommandError::Denied(
                        "legacy DTA packet digest selection differs",
                    ));
                }
                return Ok(Some(row.clone()));
            }
        }
        Ok(None)
    }

    pub(crate) fn private_conditions(
        &self,
        selected: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
        if ![PRIVATE_SCHEMA, PG_PRIVATE_SCHEMA].contains(&self.schema_ref) {
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
    worker: &mut dyn CutSchemaExecutor,
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
        worker: &mut tos_validation::source_cut::InProcessCutSchemaExecutor<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        // Inspect the protected grant before the first schema execution. A
        // selected legacy route uses the bounded Python-compatible input of
        // the in-process engine; no general scalar limit is changed.
        let uid = rustix::process::getuid().as_raw();
        let hint_raw =
            crate::source_text_owner::read_absolute(path, uid, true, 262_144, deadline, cancelled)?;
        let hint = cmd::parse(&hint_raw)?;
        if hint.object_get("legacy_dta").is_some() {
            worker
                .select_legacy_selected_input(
                    tos_validation::executor::LegacySelectedDiagnosticsLimits {
                        max_instance_bytes: 4_194_304,
                        max_visits: 2_000_000,
                        parser_state_bytes: 192 * 1024 * 1024,
                        conversion_state_bytes: 192 * 1024 * 1024,
                    },
                )
                .map_err(|_| SourceCommandError::Denied("legacy DTA bounded input"))?;
        }
        let grant = LocalTextReadSelection::load_profile(
            path,
            source_root,
            PRIVATE_SCHEMA,
            worker,
            deadline,
            cancelled,
        )?;
        if grant.raw != hint_raw {
            return Err(SourceCommandError::Conflict(
                "private text selection changed during worker selection",
            ));
        }
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
        if let Some(rows) = grant.config.object_get("legacy_dta") {
            let rows = rows
                .as_array()
                .ok_or(SourceCommandError::Invalid("legacy DTA mappings"))?;
            let mut mappings = BTreeMap::new();
            for row in rows {
                let packet = cmd::text(row, "packet_ref")?;
                let body = cmd::text(row, "body_ref")?;
                let body_sha = cmd::text(row, "body_sha256")?;
                if !legacy_dta_exact_pair(packet, body) {
                    return Err(SourceCommandError::Denied(
                        "legacy DTA identity not selected profile",
                    ));
                }
                let path =
                    crate::source_text_owner::normalized_absolute(cmd::text(row, "body_path")?)?;
                if mappings
                    .insert(body.to_owned(), (path, body_sha.to_owned()))
                    .is_some()
                {
                    return Err(SourceCommandError::Denied(
                        "duplicate legacy DTA body mapping",
                    ));
                }
            }
            context.install_legacy_dta_mapping(mappings)?;
        }
        if context.public_root() != source_root {
            return Err(SourceCommandError::Denied(
                "private text owner context source differs",
            ));
        }
        let result = Self { grant, context };
        result.verify(deadline, cancelled)?;
        Ok(result)
    }
    pub(crate) fn into_owner_parts(
        self,
    ) -> (
        LocalTextReadSelection,
        crate::source_text_owner::OwnerTextContext,
    ) {
        (self.grant, self.context)
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
    worker: &mut dyn CutSchemaExecutor,
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

// The selection supplies the exact digest pins; code admits only the four
// identified technical-markup logical path pairs, not a general gold-set home.
fn legacy_dta_exact_pair(packet: &str, body: &str) -> bool {
    const PACKET_HOME: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/dta-first-editions-parts-1-4-v1";
    const BODY_HOME: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/technical-markup-v1";
    (1..=4).any(|part| {
        packet == format!("{PACKET_HOME}/part-{part}.source-text-unit.v1.json")
            && body == format!("{BODY_HOME}/dta-part-{part}-body.v1.txt")
    })
}

#[cfg(test)]
mod dta_exact_profile_tests {
    use super::legacy_dta_exact_pair;

    #[test]
    fn only_matching_part_paths_are_selected() {
        let packet = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/dta-first-editions-parts-1-4-v1/part-1.source-text-unit.v1.json";
        let body = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/technical-markup-v1/dta-part-1-body.v1.txt";
        assert!(legacy_dta_exact_pair(packet, body));
        assert!(!legacy_dta_exact_pair(packet, "ToS/unselected"));
        assert!(!legacy_dta_exact_pair("ToS/unselected", body));
        assert!(!legacy_dta_exact_pair(
            packet,
            &body.replace("part-1-body", "part-2-body")
        ));
    }
}
