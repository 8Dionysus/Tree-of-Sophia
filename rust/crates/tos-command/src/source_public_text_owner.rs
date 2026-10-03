//! Independent protected public Text selection and bounded byte custody.
//! This grant routes construction; the caller must prove rights and retained
//! publication authority before asking this transport to open source text.
use crate::source_command::{self as cmd, SourceCommandError as E, SourceCommandResult as R};
use crate::source_creation_store::{active, owned, protected_configuration_parents};
use crate::source_text_owner::{normalized_absolute, read_absolute};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, Metadata};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONFIG_SCHEMA: &str = "ToS/contracts/public-native-text-create-owner.schema.json";
const SCHEMAS: [&str; 3] = [
    CONFIG_SCHEMA,
    "ToS/contracts/public-native-text-authority.schema.json",
    "ToS/contracts/source-text-unit-packet-v1.schema.json",
];
const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 131072;
fn identity(m: &Metadata) -> (u64, u64, u32, u32) {
    (m.dev(), m.ino(), m.uid(), m.mode() & 0o7777)
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn directory(path: &Path, uid: u32, private: bool) -> R<File> {
    protected_configuration_parents(&path.join("probe"), uid)?;
    let f = tos_fd_open::open_absolute_directory(path)
        .map_err(|_| E::Denied("public Text directory absent or unsafe"))?;
    let m = owned(&f, uid, true)?;
    if private && m.mode() & 0o7777 != 0o700 {
        return Err(E::Denied("public Text recovery mode"));
    }
    Ok(f)
}
pub(crate) fn public_reference(reference: &str, source_text: bool) -> R<RelativePath> {
    let p =
        RelativePath::parse(reference).map_err(|_| E::Denied("public Text canonical reference"))?;
    if reference.split('/').any(|v| {
        v.starts_with('.') || matches!(v, "owner-local" | "payload" | "local-content" | "catalog")
    }) {
        return Err(E::Denied("public Text private or indirect reference"));
    }
    if source_text
        && !["ToS/review-ledger", "ToS/doctrine", "docs"]
            .iter()
            .any(|home| reference == *home || reference.starts_with(&format!("{home}/")))
    {
        return Err(E::Denied("public Text authored documentation route"));
    }
    Ok(p)
}

pub(crate) struct PublicNativeTextSelection {
    pub(crate) config: JsonValue,
    pub(crate) raw: Vec<u8>,
    pub(crate) path: PathBuf,
    source_root: PathBuf,
    recovery_root: PathBuf,
    source_identity: (u64, u64, u32, u32),
    recovery_identity: (u64, u64, u32, u32),
    schemas: BTreeMap<String, Vec<u8>>,
    digest: Digest256,
    uid: u32,
}
impl PublicNativeTextSelection {
    pub(crate) fn select(
        path: &Path,
        selected_schemas: &BTreeMap<String, Vec<u8>>,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<Self> {
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(E::Denied("public Text setuid account"));
        }
        let path =
            normalized_absolute(path.to_str().ok_or(E::Invalid("public Text grant UTF-8"))?)?;
        let raw = read_absolute(&path, uid, true, 1_048_576, deadline, cancelled)?;
        let config = cmd::parse(&raw)?;
        let source_root = normalized_absolute(cmd::text(&config, "source_root")?)?;
        let recovery_root = normalized_absolute(cmd::text(&config, "recovery_root")?)?;
        if source_root.starts_with(&recovery_root) || recovery_root.starts_with(&source_root) {
            return Err(E::Denied("public Text roots overlap"));
        }
        let source_identity = identity(&owned(&directory(&source_root, uid, false)?, uid, true)?);
        let recovery_identity =
            identity(&owned(&directory(&recovery_root, uid, true)?, uid, true)?);
        let mut schemas = BTreeMap::new();
        let mut contracts = Vec::new();
        for reference in SCHEMAS {
            let bytes = read_absolute(
                &source_root.join(reference),
                uid,
                false,
                1_048_576,
                deadline,
                cancelled,
            )?;
            if selected_schemas.get(reference) != Some(&bytes)
                || worker.contract_digest(reference) != Some(Digest256::of_bytes(&bytes))
                || cmd::text(&cmd::parse(&bytes)?, "$id")?
                    != format!("https://tree-of-sophia.local/{reference}")
            {
                return Err(E::Conflict("public Text selected schema changed"));
            }
            contracts.push((
                reference,
                cmd::string(&format!("sha256:{}", Digest256::of_bytes(&bytes).to_hex())),
            ));
            schemas.insert(reference.to_owned(), bytes);
        }
        match worker.check_reusing_scalar(
            path.to_str().ok_or(E::Invalid("public Text grant UTF-8"))?,
            &cmd::canonical(&config)?,
            CONFIG_SCHEMA,
            deadline,
            cancelled,
        ) {
            Ok(true) => (),
            Ok(false) => return Err(E::Invalid("public Text grant schema")),
            Err(reason) => {
                return Err(E::SchemaExecution {
                    path: path.to_string_lossy().into_owned(),
                    root: CONFIG_SCHEMA.to_owned(),
                    reason,
                });
            }
        }
        if cmd::text(&config, "schema_version")? != "tos_public_native_text_create_owner_v1"
            || cmd::integer(&config, "uid")? != u64::from(uid)
            || cmd::array(&config, "allowed_operations")? != [cmd::string("native-text.create")]
        {
            return Err(E::Denied("public Text grant account or operation"));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let target = public_reference(cmd::text(&config, "source_path")?, false)?;
        if !target.as_str().starts_with("ToS/source-witnesses/works/")
            || target.as_str().split('/').count() < 7
            || Path::new(target.as_str())
                .file_name()
                .and_then(|v| v.to_str())
                != Some("source-text-unit.v1.json")
        {
            return Err(E::Denied("public Text exact output package"));
        }
        let package = source_root
            .join(target.as_str())
            .parent()
            .ok_or(E::Invalid("public Text target parent"))?
            .to_path_buf();
        directory(
            package
                .parent()
                .ok_or(E::Invalid("public Text package parent"))?,
            uid,
            false,
        )?;
        let source = cmd::field(&config, "source")?;
        public_reference(cmd::text(source, "ref")?, true)?;
        let scope = cmd::field(&config, "source_scope")?;
        if cmd::text(source, "sha256")? != cmd::text(scope, "file_sha256")?
            || cmd::text(scope, "file_ref")?
                != format!("tos.file.sha256.{}", cmd::text(source, "sha256")?)
            || cmd::integer(source, "byte_size")?
                > cmd::integer(cmd::field(&config, "limits")?, "max_source_bytes")?
        {
            return Err(E::Invalid("public Text original File binding"));
        }
        for name in ["rights_record_refs", "license_bindings"] {
            let mut seen = BTreeSet::new();
            for binding in cmd::array(&config, name)? {
                let reference = cmd::text(binding, "ref")?;
                public_reference(reference, false)?;
                if !seen.insert(reference) {
                    return Err(E::Invalid("public Text duplicate evidence reference"));
                }
            }
        }
        let authority = cmd::text(cmd::field(&config, "publication_authority")?, "ref")?;
        public_reference(authority, false)?;
        if !authority.starts_with("ToS/") {
            return Err(E::Denied("public Text retained authority owner"));
        }
        for kind in ["work", "expression", "edition", "item"] {
            let reference = cmd::text(cmd::field(&config, "source_record_refs")?, kind)?;
            public_reference(reference, false)?;
            if !reference.starts_with("ToS/source-witnesses/")
                || Path::new(reference).file_name().and_then(|v| v.to_str())
                    != Some(format!("{kind}.json").as_str())
            {
                return Err(E::Denied("public Text metadata route"));
            }
        }
        let ids = cmd::field(&config, "identities")?;
        let mut seen = BTreeSet::new();
        for key in [
            "layer_id",
            "anchor_id",
            "passage_id",
            "provenance_event_id",
            "packet_id",
            "scheme_id",
            "segmentation_id",
            "scope_anchor_ref",
        ] {
            if !seen.insert(cmd::text(ids, key)?) {
                return Err(E::Invalid("public Text duplicate native identity"));
            }
        }
        for slot in cmd::array(ids, "unit_slots")? {
            for key in ["unit_id", "anchor_ref"] {
                if !seen.insert(cmd::text(slot, key)?) {
                    return Err(E::Invalid("public Text duplicate unit identity"));
                }
            }
        }
        for gap in cmd::array(ids, "gap_anchor_refs")? {
            if !seen.insert(gap.as_str().ok_or(E::Invalid("public Text gap identity"))?) {
                return Err(E::Invalid("public Text duplicate gap identity"));
            }
        }
        let method = cmd::field(cmd::field(&config, "unit_proposal")?, "method")?;
        let plan_ref = format!(
            "{}/construction-plan.json",
            target
                .as_str()
                .rsplit_once('/')
                .ok_or(E::Invalid("public Text package"))?
                .0
        );
        if cmd::text(method, "maker_kind")? != "software"
            || cmd::text(method, "agent_ref")? != cmd::text(&config, "principal_id")?
            || cmd::text(method, "provenance_event_ref")? != cmd::text(ids, "provenance_event_id")?
            || cmd::text(method, "configuration_ref")? != plan_ref
        {
            return Err(E::Denied("public Text constructor method"));
        }
        let digest = Digest256::of_bytes(&cmd::canonical(&cmd::object(vec![
            (
                "protected_configuration_bytes",
                cmd::string(&format!("sha256:{}", Digest256::of_bytes(&raw).to_hex())),
            ),
            ("contracts", cmd::object(contracts)),
        ]))?);
        Ok(Self {
            config,
            raw,
            path,
            source_root,
            recovery_root,
            source_identity,
            recovery_identity,
            schemas,
            digest,
            uid,
        })
    }
    pub(crate) fn account_uid(&self) -> u32 {
        self.uid
    }
    pub(crate) fn public_root(&self) -> &Path {
        &self.source_root
    }
    pub(crate) fn target_root(&self) -> &Path {
        &self.source_root
    }
    pub(crate) fn recovery_root(&self) -> &Path {
        &self.recovery_root
    }
    pub(crate) fn target_is_private(&self) -> bool {
        false
    }
    pub(crate) fn configuration_digest(&self) -> Digest256 {
        self.digest
    }
    pub(crate) fn public_plan(&self) -> R<JsonValue> {
        let entries = self
            .config
            .as_object()
            .ok_or(E::Invalid("public Text grant object"))?
            .iter()
            .filter(|(key, _)| {
                !["source_root", "recovery_root", "uid"].contains(&key.as_str().unwrap_or(""))
            })
            .cloned()
            .collect();
        Ok(cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_public_native_construction_plan_v1"),
            ),
            ("selection", JsonValue::Object(entries)),
            (
                "protected_grant_digest",
                cmd::string(&format!("sha256:{}", self.digest.to_hex())),
            ),
            (
                "authority_boundary",
                cmd::string("construction-plan-not-a-grant-license-or-assessment"),
            ),
        ]))
    }
    pub(crate) fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> R<()> {
        active(deadline, cancelled)?;
        if read_absolute(&self.path, self.uid, true, 1_048_576, deadline, cancelled)? != self.raw {
            return Err(E::Conflict("public Text protected grant changed"));
        }
        cmd::validate_expiry(
            cmd::text(&self.config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        if identity(&owned(
            &directory(&self.source_root, self.uid, false)?,
            self.uid,
            true,
        )?) != self.source_identity
            || identity(&owned(
                &directory(&self.recovery_root, self.uid, true)?,
                self.uid,
                true,
            )?) != self.recovery_identity
        {
            return Err(E::Conflict("public Text root changed"));
        }
        for (reference, raw) in &self.schemas {
            if read_absolute(
                &self.source_root.join(reference),
                self.uid,
                false,
                1_048_576,
                deadline,
                cancelled,
            )? != *raw
            {
                return Err(E::Conflict("public Text schema changed"));
            }
        }
        Ok(())
    }
    pub(crate) fn new_package_target(&self, reference: &str) -> R<PathBuf> {
        if reference != cmd::text(&self.config, "source_path")? {
            return Err(E::Denied("public Text unselected destination"));
        }
        let p = public_reference(reference, false)?;
        let package = self
            .source_root
            .join(p.as_str())
            .parent()
            .ok_or(E::Invalid("public Text package"))?
            .to_path_buf();
        directory(
            package
                .parent()
                .ok_or(E::Invalid("public Text package parent"))?,
            self.uid,
            false,
        )?;
        Ok(package)
    }
    pub(crate) fn read(
        &self,
        reference: &str,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<Vec<u8>> {
        self.read_bounded(
            reference,
            max.min(
                if reference == cmd::text(cmd::field(&self.config, "source")?, "ref")? {
                    MAX_SOURCE_BYTES
                } else {
                    1_048_576
                },
            ),
            deadline,
            cancelled,
        )
    }
    fn read_bounded(
        &self,
        reference: &str,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<Vec<u8>> {
        public_reference(reference, false)?;
        if identity(&owned(
            &directory(&self.source_root, self.uid, false)?,
            self.uid,
            true,
        )?) != self.source_identity
        {
            return Err(E::Conflict("public Text source root changed"));
        }
        let mut parent = self.source_root.clone();
        let components: Vec<_> = reference.split('/').collect();
        for name in &components[..components.len().saturating_sub(1)] {
            parent.push(name);
            directory(&parent, self.uid, false)?;
        }
        let path = self.source_root.join(reference);
        let f = tos_fd_open::open_absolute_regular(&path, max as u64)
            .map_err(|_| E::Denied("public Text input file"))?;
        owned(&f, self.uid, false)?;
        let bytes = read_absolute(&path, self.uid, false, max, deadline, cancelled)?;
        let current = tos_fd_open::open_absolute_regular(&path, max as u64)
            .map_err(|_| E::Conflict("public Text input changed"))?;
        if stamp(&owned(&f, self.uid, false)?) != stamp(&owned(&current, self.uid, false)?) {
            return Err(E::Conflict("public Text input changed"));
        }
        Ok(bytes)
    }
    pub(crate) fn read_expected(
        &self,
        reference: &str,
        sha256: &str,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<Vec<u8>> {
        let bytes = self.read(reference, max, deadline, cancelled)?;
        if Digest256::of_bytes(&bytes).to_hex() != sha256 {
            return Err(E::Conflict("public Text input digest"));
        }
        Ok(bytes)
    }
    pub(crate) fn package(
        &self,
        reference: &str,
        expected: &[&str],
        state: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<BTreeMap<String, Vec<u8>>> {
        public_reference(reference, false)?;
        let selected = cmd::text(&self.config, "source_path")?
            .rsplit_once('/')
            .ok_or(E::Invalid("public Text selected package"))?
            .0;
        if reference != selected
            || expected.is_empty()
            || expected.len() > 12
            || expected
                .iter()
                .any(|v| v.is_empty() || v.contains('/') || v.starts_with('.'))
            || expected.iter().copied().collect::<BTreeSet<_>>().len() != expected.len()
        {
            return Err(E::Denied("public Text package selection"));
        }
        let path = self.source_root.join(reference);
        let f = directory(&path, self.uid, false)?;
        let before = stamp(&owned(&f, self.uid, true)?);
        let mut seen = BTreeSet::new();
        for entry in std::fs::read_dir(format!("/proc/self/fd/{}", f.as_raw_fd()))
            .map_err(|_| E::Denied("public Text package enumeration"))?
        {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| E::Denied("public Text package entry"))?
                .file_name()
                .into_string()
                .map_err(|_| E::Invalid("public Text package UTF-8"))?;
            if !expected.contains(&name.as_str()) || !seen.insert(name) {
                return Err(E::Conflict("public Text package extra member"));
            }
        }
        if seen.len() != expected.len() {
            return Err(E::Conflict("public Text package missing member"));
        }
        let overhead = expected.iter().try_fold(0usize, |n, v| {
            n.checked_add(v.len() + 128)
                .ok_or(E::Unsupported("public Text package state overflow"))
        })?;
        let mut remaining = state
            .checked_sub(overhead)
            .ok_or(E::Unsupported("public Text package state"))?
            .min(2 * 1024 * 1024);
        let mut result = BTreeMap::new();
        for name in expected {
            let bytes = self.read_bounded(
                &format!("{reference}/{name}"),
                remaining,
                deadline,
                cancelled,
            )?;
            remaining -= bytes.len();
            result.insert((*name).to_owned(), bytes);
        }
        if before != stamp(&owned(&f, self.uid, true)?)
            || before != stamp(&owned(&directory(&path, self.uid, false)?, self.uid, true)?)
        {
            return Err(E::Conflict("public Text package changed"));
        }
        Ok(result)
    }
}

/// A command-local distinct-input cache. Rechecks remain explicit and live.
pub(crate) struct PublicTextInputCache {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    bytes: usize,
}
impl PublicTextInputCache {
    pub(crate) fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            bytes: 0,
        }
    }
    pub(crate) fn read(
        &mut self,
        selection: &PublicNativeTextSelection,
        reference: &str,
        expected: Option<&str>,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<Vec<u8>> {
        if !self.files.contains_key(reference) {
            if self.files.len() >= 128 {
                return Err(E::Unsupported("public Text input file budget"));
            }
            let bytes = selection.read(reference, max, deadline, cancelled)?;
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .filter(|v| *v <= MAX_INPUT_BYTES)
                .ok_or(E::Unsupported("public Text input byte budget"))?;
            self.files.insert(reference.to_owned(), bytes);
        }
        let bytes = self
            .files
            .get(reference)
            .ok_or(E::Invalid("public Text cached input"))?;
        if bytes.len() > max || expected.is_some_and(|v| Digest256::of_bytes(bytes).to_hex() != v) {
            return Err(E::Conflict("public Text input declaration"));
        }
        Ok(bytes.clone())
    }
    pub(crate) fn verify_current(
        &self,
        selection: &PublicNativeTextSelection,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> R<()> {
        selection.verify_current(deadline, cancelled)?;
        for (reference, bytes) in &self.files {
            if selection.read(reference, bytes.len(), deadline, cancelled)? != *bytes {
                return Err(E::Conflict("public Text cached input changed"));
            }
        }
        Ok(())
    }
}
