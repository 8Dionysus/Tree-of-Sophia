//! Read-only observation of a hash-selected, committed initial Claim package.
//! The held maintained writer lock spans the caller's derived publication.
//! Historical expiry applies only after the complete package is authenticated.
use super::{
    CORPUS_LOCK, CreationFilesystem, active, inode, owned, protected_configuration_parents, raw,
    stamp, walk,
};
use crate::source_command::{
    self as cmd, SourceCommandError as Error, SourceCommandResult as Result,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
const FILES: [&str; 5] = [
    "source-claims.jsonl",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];
const MAX_READ: usize = 33_554_432;
fn value(bytes: &[u8]) -> Result<Value> {
    cmd::parse(bytes)?; // strict bounded parser rejects duplicate keys and malformed numbers
    serde_json::from_slice(bytes).map_err(|_| Error::Invalid("Claim observer JSON"))
}
fn canonical(v: &Value) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("Claim observer JSON emission"))?;
    cmd::canonical(&cmd::parse(&raw)?)
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("Claim observer required string"))
}
fn keys(v: &Value, expected: &[&str]) -> Result<()> {
    let actual = v
        .as_object()
        .ok_or(Error::Invalid("Claim observer object"))?;
    if actual.len() != expected.len() || expected.iter().any(|k| !actual.contains_key(*k)) {
        return Err(Error::Invalid("Claim observer field closure"));
    }
    Ok(())
}
fn identity(v: &str, prefix: &str) -> bool {
    let Some(tail) = v.strip_prefix(prefix) else {
        return false;
    };
    !tail.is_empty()
        && tail.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
struct Capture {
    descriptor: File,
    signature: (u64, u64, u64, i64, i64, i64, i64),
    bytes: Arc<[u8]>,
}
/// No source writer or isolated-root constructor escapes this observer.
pub(crate) struct CommittedClaimObservation {
    filesystem: CreationFilesystem,
    witness: File,
    lock: File,
    configuration: Value,
    configuration_capture: File,
    configuration_signature: (u64, u64, u64, i64, i64, i64, i64),
    request: Value,
    receipt: Value,
    source_path: RelativePath,
    home: String,
    receipt_sha256: Digest256,
    request_digest: Digest256,
    captured: BTreeMap<String, Capture>,
    read_bytes: usize,
    publication: super::work_transaction::PublicationSnapshot,
    absences: BTreeMap<String, (File, (u64, u64))>,
}
impl CommittedClaimObservation {
    pub(crate) fn select(
        owner_config: &Path,
        receipt_sha256: Digest256,
        request_digest: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        active(deadline, cancelled)?;
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(Error::Denied("Claim observer account differs"));
        }
        protected_configuration_parents(owner_config, uid)?;
        let mut fd = tos_fd_open::open_absolute_regular(owner_config, 1_048_576)
            .map_err(|_| Error::Denied("Claim observer protected configuration"))?;
        let configuration_signature = stamp(&owned(&fd, uid, false)?); // maintained read permits 0644; this is not a private writer grant
        let configuration_raw = raw(&mut fd, 1_048_576, deadline, cancelled)?;
        let configuration = value(&configuration_raw)?;
        keys(
            &configuration,
            &[
                "schema_version",
                "uid",
                "principal_id",
                "maker_type",
                "source_root",
                "source_path",
                "authority_ref",
                "expires_at",
                "provenance_event_id",
                "allowed_operations",
                "allowed_claim_ids",
                "allowed_subject_refs",
                "allowed_object_refs",
                "allowed_predicates",
                "allowed_evidence_refs",
            ],
        )?;
        if configuration["schema_version"] != "tos_local_claim_create_owner_v1"
            || configuration["uid"].as_u64() != Some(u64::from(uid))
            || !["human", "software", "model"].contains(&text(&configuration, "maker_type")?)
        {
            return Err(Error::Denied(
                "initial identity-only Claim observer delegation",
            ));
        }
        for k in ["principal_id", "authority_ref"] {
            if text(&configuration, k)?.trim().is_empty() {
                return Err(Error::Invalid("Claim observer blank authority"));
            }
        }
        for (key, max) in [
            ("allowed_operations", 1),
            ("allowed_claim_ids", 32),
            ("allowed_subject_refs", 128),
            ("allowed_object_refs", 128),
            ("allowed_predicates", 32),
            ("allowed_evidence_refs", 128),
        ] {
            let list = configuration[key]
                .as_array()
                .ok_or(Error::Invalid("Claim observer scope list"))?;
            let mut seen = BTreeSet::new();
            if list.len() > max
                || list.iter().any(|v| {
                    v.as_str()
                        .is_none_or(|s| s.trim().is_empty() || !seen.insert(s))
                })
            {
                return Err(Error::Invalid("Claim observer bounded unique scope"));
            }
        }
        if !configuration["allowed_operations"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("claims.create"))
            || configuration["allowed_operations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v != "claims.create")
            || !identity(text(&configuration, "provenance_event_id")?, "tos.event.")
            || configuration["allowed_claim_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| !identity(v.as_str().unwrap(), "tos.claim."))
        {
            return Err(Error::Invalid("Claim observer delegated identity"));
        }
        let root_path = PathBuf::from(text(&configuration, "source_root")?);
        protected_configuration_parents(&root_path.join("observer-boundary"), uid)?;
        let root = tos_fd_open::open_absolute_directory(&root_path)
            .map_err(|_| Error::Denied("Claim observer absolute root"))?;
        let root_identity = inode(&owned(&root, uid, true)?);
        let source_path = RelativePath::parse(text(&configuration, "source_path")?)
            .map_err(|_| Error::Denied("Claim observer source path"))?;
        let parts = source_path.as_str().split('/').collect::<Vec<_>>();
        if parts.len() != 5
            || parts[..3] != ["ToS", "source-witnesses", "relations"]
            || !identity(parts[3], "")
            || ["catalog", "payload", "local-content"].contains(&parts[3])
            || parts[4] != "source-claims.jsonl"
        {
            return Err(Error::Denied("Claim observer exact named package"));
        }
        let home = parts[..4].join("/");
        let filesystem = CreationFilesystem {
            root_path,
            root,
            root_identity,
            configuration_path: owner_config.to_path_buf(),
            configuration_raw,
            uid,
        };
        let witness = walk(&filesystem.root, "ToS/source-witnesses", uid)?;
        let lock = filesystem.lock(&witness, deadline, cancelled)?;
        let publication =
            super::work_transaction::PublicationSnapshot::select(&filesystem, deadline, cancelled)?;
        let mut this = Self {
            filesystem,
            witness,
            lock,
            configuration,
            configuration_capture: fd,
            configuration_signature,
            request: Value::Null,
            receipt: Value::Null,
            source_path,
            home,
            receipt_sha256,
            request_digest,
            captured: BTreeMap::new(),
            read_bytes: 0,
            publication,
            absences: BTreeMap::new(),
        };
        for name in FILES {
            let path = RelativePath::parse(&format!("{}/{name}", this.home))
                .map_err(|_| Error::Invalid("Claim observer package member"))?;
            this.read_selected(
                &path,
                if name == "source-create-receipt.json" || name == "source-create-request.json" {
                    1_048_576
                } else {
                    MAX_READ
                },
                deadline,
                cancelled,
            )?;
        }
        this.request =
            value(&this.captured[&format!("{}/source-create-request.json", this.home)].bytes)?;
        this.receipt =
            value(&this.captured[&format!("{}/source-create-receipt.json", this.home)].bytes)?;
        this.verify_package()?;
        this.verify_current(deadline, cancelled)?;
        Ok(this)
    }
    pub(crate) fn source_publication(&self) -> Value {
        serde_json::json!({"protocol":"tos_selected_source_metadata_v1","token":self.publication.token,"generation":self.publication.generation})
    }
    pub(crate) fn command_context(&self, revision: SourceRevision) -> Result<cmd::CommandContext> {
        Ok(cmd::CommandContext {
            base_revision: revision,
            configuration_raw: self.filesystem.configuration_raw.clone(),
            request_raw: self.captured[&format!("{}/source-create-request.json", self.home)]
                .bytes
                .to_vec(),
            recorded_at: text(&self.receipt, "recorded_at")?.to_owned(),
            effective_uid: u64::from(self.filesystem.uid),
            files: self
                .captured
                .iter()
                .map(|(path, c)| {
                    Ok(cmd::SourceFile {
                        path: RelativePath::parse(path)
                            .map_err(|_| Error::Invalid("Claim observer context path"))?,
                        raw: c.bytes.to_vec(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }
    pub(crate) fn read_optional(
        &mut self,
        path: &RelativePath,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Arc<[u8]>>> {
        active(deadline, cancelled)?;
        let reference = path.as_str();
        if !reference.starts_with("ToS/")
            || reference.split('/').any(|p| {
                p.starts_with('.')
                    || ["payload", "private", "owner-local", "local-content"].contains(&p)
            })
        {
            return Err(Error::Denied("Claim observer optional public metadata"));
        }
        if self.absences.contains_key(reference) {
            return Ok(None);
        }
        let (parent, name) = reference
            .rsplit_once('/')
            .ok_or(Error::Invalid("Claim observer optional path"))?;
        let directory = walk(&self.filesystem.root, parent, self.filesystem.uid)?;
        match rustix::fs::statat(&directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {
                if self.absences.len() + self.captured.len() >= 256 {
                    return Err(Error::Unsupported("Claim observer optional file count"));
                }
                let selected = inode(&owned(&directory, self.filesystem.uid, true)?);
                self.absences
                    .insert(reference.to_owned(), (directory, selected));
                Ok(None)
            }
            Err(_) => Err(Error::Denied("Claim observer optional metadata stat")),
            Ok(_) => self.read_selected(path, cap, deadline, cancelled).map(Some),
        }
    }
    pub(crate) fn request(&self) -> &Value {
        &self.request
    }
    pub(crate) fn receipt(&self) -> &Value {
        &self.receipt
    }
    pub(crate) fn configuration(&self) -> &Value {
        &self.configuration
    }
    pub(crate) fn source_path(&self) -> &RelativePath {
        &self.source_path
    }
    pub(crate) fn bindings(&self) -> Value {
        Value::Object(self.captured.iter().map(|(p,c)|(p.clone(),serde_json::json!({"sha256":Digest256::of_bytes(&c.bytes).to_hex(),"bytes":c.bytes.len()}))).collect())
    }
    pub(crate) fn read_selected(
        &mut self,
        path: &RelativePath,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Arc<[u8]>> {
        active(deadline, cancelled)?;
        let reference = path.as_str();
        if !reference.starts_with("ToS/")
            || reference.split('/').any(|p| {
                p.starts_with('.')
                    || ["payload", "private", "owner-local", "local-content"].contains(&p)
            })
        {
            return Err(Error::Denied("Claim observer public exact metadata"));
        }
        if let Some(c) = self.captured.get(reference) {
            if c.bytes.len() > cap {
                return Err(Error::Unsupported("Claim observer selected file cap"));
            }
            return Ok(c.bytes.clone());
        }
        if self.captured.len() >= 256 {
            return Err(Error::Unsupported("Claim observer file count"));
        }
        let (parent, name) = reference
            .rsplit_once('/')
            .ok_or(Error::Invalid("Claim observer file path"))?;
        let parent = walk(&self.filesystem.root, parent, self.filesystem.uid)?;
        let mut descriptor = tos_fd_open::open_regular_at(&parent, Path::new(name))
            .map_err(|_| Error::Denied("Claim observer metadata descriptor"))?;
        let signature = stamp(&owned(&descriptor, self.filesystem.uid, false)?);
        let cap = cap.min(
            MAX_READ
                .checked_sub(self.read_bytes)
                .ok_or(Error::Unsupported("Claim observer read budget"))?,
        );
        let bytes: Arc<[u8]> = raw(&mut descriptor, cap, deadline, cancelled)?.into();
        self.read_bytes += bytes.len();
        self.captured.insert(
            reference.to_owned(),
            Capture {
                descriptor,
                signature,
                bytes: bytes.clone(),
            },
        );
        Ok(bytes)
    }
    fn verify_package(&self) -> Result<()> {
        let get = |name: &str| &*self.captured[&format!("{}/{name}", self.home)].bytes;
        if Digest256::of_bytes(get("source-create-receipt.json")) != self.receipt_sha256
            || Digest256::of_bytes(&canonical(&self.request)?) != self.request_digest
        {
            return Err(Error::Conflict(
                "Claim observer hash-selected package differs",
            ));
        }
        keys(
            &self.receipt,
            &[
                "schema_version",
                "command_id",
                "request_digest",
                "principal_id",
                "authority_ref",
                "owner_configuration",
                "recorded_at",
                "source_path",
                "dependencies",
                "source_bindings",
                "claims",
                "files",
                "grants_admission",
            ],
        )?;
        keys(
            &self.request,
            &[
                "schema_version",
                "operation",
                "claims",
                "command_id",
                "expected_configuration",
                "expected_revision",
                "expected_dependencies",
                "expected_inputs",
            ],
        )?;
        if self.request["schema_version"] != "tos_local_source_command_v1" {
            return Err(Error::Invalid("Claim observer request schema"));
        }
        keys(&self.request["expected_inputs"], &["objects", "evidence"])?;
        let configuration_digest =
            Digest256::of_bytes(&canonical(&self.configuration)?).to_prefixed();
        if self.receipt["schema_version"] != "tos_local_claim_create_receipt_v1"
            || self.request["operation"] != "claims.create"
            || !self
                .request
                .get("expected_revision")
                .is_some_and(Value::is_null)
            || self.receipt["grants_admission"] != false
            || self.receipt["request_digest"] != self.request_digest.to_prefixed()
            || self.receipt["source_path"] != self.source_path.as_str()
            || self.receipt["owner_configuration"] != configuration_digest
            || self.request["expected_configuration"] != configuration_digest
        {
            return Err(Error::Conflict("Claim observer initial receipt identity"));
        }
        for (left, right) in [
            ("command_id", "command_id"),
            ("dependencies", "expected_dependencies"),
            ("source_bindings", "expected_inputs"),
        ] {
            if self.receipt.get(left) != self.request.get(right) {
                return Err(Error::Conflict("Claim observer receipt request binding"));
            }
        }
        for key in ["principal_id", "authority_ref"] {
            if self.receipt.get(key) != self.configuration.get(key) {
                return Err(Error::Denied("Claim observer current authority differs"));
            }
        }
        let recorded_at = text(&self.receipt, "recorded_at")?;
        cmd::validate_instant(recorded_at)?;
        let now = crate::source_serialization::instant()?;
        if tos_validation::retirement_rules::observed_instant_order(recorded_at, &now)
            .map_err(|_| Error::Invalid("Claim observer aware receipt clock"))?
            == std::cmp::Ordering::Greater
        {
            return Err(Error::Denied("Claim observer future receipt"));
        }
        cmd::validate_expiry(text(&self.configuration, "expires_at")?, recorded_at)?;
        let files = self.receipt["files"]
            .as_object()
            .ok_or(Error::Invalid("Claim observer file bindings"))?;
        if files.len() != 4 || FILES[..4].iter().any(|n| !files.contains_key(*n)) {
            return Err(Error::Conflict("Claim observer receipt file closure"));
        }
        for name in &FILES[..4] {
            let bytes = get(name);
            if files[*name]
                != serde_json::json!({"sha256":Digest256::of_bytes(bytes).to_prefixed(),"bytes":bytes.len()})
            {
                return Err(Error::Conflict("Claim observer original package changed"));
            }
        }
        let mut request = canonical(&self.request)?;
        request.push(b'\n');
        if get("source-create-request.json") != request {
            return Err(Error::Conflict("Claim observer canonical request changed"));
        }
        let claims = self.request["claims"]
            .as_array()
            .ok_or(Error::Invalid("Claim observer claim list"))?;
        if claims.is_empty() || claims.len() > 32 {
            return Err(Error::Unsupported("Claim observer claim count"));
        }
        let mut stream = Vec::new();
        let mut refs = Vec::new();
        let mut ids = BTreeSet::new();
        for claim in claims {
            let id = text(claim, "claim_id")?;
            if [
                "has_expression",
                "embodied_by",
                "exemplified_by",
                "translated_by",
                "contains_work",
                "described_by",
                "metadata_at",
                "downloadable_at",
                "rights_statement_at",
            ]
            .contains(&text(claim, "predicate")?)
            {
                return Err(Error::Denied("Claim observer compound operation required"));
            }
            if !ids.insert(id)
                || claim["claim_version"] != 1
                || claim
                    .get("alternative_claim_refs")
                    .is_some_and(|v| v.as_array().is_none_or(|v| !v.is_empty()))
                || claim
                    .get("supersedes_claim_ref")
                    .is_some_and(|v| !v.is_null())
                || claim
                    .get("assessment_refs")
                    .is_some_and(|v| v.as_array().is_none_or(|v| !v.is_empty()))
            {
                return Err(Error::Unsupported(
                    "Claim observer unchanged initial profile",
                ));
            }
            for (scope, key) in [
                ("allowed_claim_ids", "claim_id"),
                ("allowed_subject_refs", "subject_ref"),
                ("allowed_object_refs", "object"),
                ("allowed_predicates", "predicate"),
            ] {
                if !self.configuration[scope]
                    .as_array()
                    .unwrap()
                    .contains(&claim[key])
                {
                    return Err(Error::Denied("Claim observer current scope"));
                }
            }
            if claim.pointer("/maker/agent_ref") != self.configuration.get("principal_id")
                || claim.pointer("/maker/maker_type") != self.configuration.get("maker_type")
                || claim["provenance_event_ref"] != self.configuration["provenance_event_id"]
            {
                return Err(Error::Denied("Claim observer maker scope"));
            }
            for key in ["evidence_refs", "counterevidence_refs"] {
                if let Some(values) = claim.get(key) {
                    for item in values
                        .as_array()
                        .ok_or(Error::Invalid("Claim observer evidence list"))?
                    {
                        if !self.configuration["allowed_evidence_refs"]
                            .as_array()
                            .unwrap()
                            .contains(item)
                        {
                            return Err(Error::Denied("Claim observer evidence scope"));
                        }
                    }
                }
            }
            let bytes = canonical(claim)?;
            refs.push(serde_json::json!({"id":id,"version":1,"digest":Digest256::of_bytes(&bytes).to_prefixed()}));
            stream.extend(bytes);
            stream.push(b'\n');
        }
        if get("source-claims.jsonl") != stream || self.receipt["claims"] != Value::Array(refs) {
            return Err(Error::Conflict("Claim observer canonical initial stream"));
        }
        Ok(())
    }
    pub(crate) fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        active(deadline, cancelled)?;
        let fs = &self.filesystem;
        if rustix::process::geteuid().as_raw() != fs.uid
            || rustix::process::getuid().as_raw() != fs.uid
        {
            return Err(Error::Denied("Claim observer account changed"));
        }
        let root = tos_fd_open::open_absolute_directory(&fs.root_path)
            .map_err(|_| Error::Conflict("Claim observer root path changed"))?;
        if inode(&owned(&root, fs.uid, true)?) != fs.root_identity
            || inode(&owned(&fs.root, fs.uid, true)?) != fs.root_identity
        {
            return Err(Error::Conflict("Claim observer root detached"));
        }
        protected_configuration_parents(&fs.configuration_path, fs.uid)?;
        let mut config = tos_fd_open::open_absolute_regular(&fs.configuration_path, 1_048_576)
            .map_err(|_| Error::Denied("Claim observer current config path"))?;
        if stamp(&owned(&config, fs.uid, false)?) != self.configuration_signature
            || stamp(&owned(&self.configuration_capture, fs.uid, false)?)
                != self.configuration_signature
        {
            return Err(Error::Conflict(
                "Claim observer configuration descriptor detached",
            ));
        }
        if raw(&mut config, 1_048_576, deadline, cancelled)? != fs.configuration_raw {
            return Err(Error::Conflict("Claim observer current config differs"));
        }
        let witness = walk(&root, "ToS/source-witnesses", fs.uid)?;
        if inode(&owned(&witness, fs.uid, true)?) != inode(&owned(&self.witness, fs.uid, true)?) {
            return Err(Error::Conflict("Claim observer lock directory detached"));
        }
        let current = tos_fd_open::open_regular_at(&witness, Path::new(CORPUS_LOCK))
            .map_err(|_| Error::Conflict("Claim observer lock path changed"))?;
        if inode(&owned(&current, fs.uid, false)?) != inode(&owned(&self.lock, fs.uid, false)?) {
            return Err(Error::Conflict("Claim observer held lock detached"));
        }
        let directory = walk(&root, &self.home, fs.uid)?;
        let names = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| Error::Invalid("Claim observer package enumeration"))?;
        let mut seen = BTreeSet::new();
        for name in names {
            let name = name
                .map_err(|_| Error::Invalid("Claim observer directory entry"))?
                .file_name()
                .into_string()
                .map_err(|_| Error::Invalid("Claim observer non-UTF8 package"))?;
            if seen.len() >= FILES.len() || !FILES.contains(&name.as_str()) || !seen.insert(name) {
                return Err(Error::Unsupported(
                    "Claim observer initial package has extensions",
                ));
            }
        }
        if seen.len() != FILES.len() {
            return Err(Error::Conflict("Claim observer package closure changed"));
        }
        for (reference, capture) in &self.captured {
            active(deadline, cancelled)?;
            let (parent, name) = reference
                .rsplit_once('/')
                .ok_or(Error::Invalid("Claim observer member path"))?;
            let parent = walk(&root, parent, fs.uid)?;
            let mut selected = tos_fd_open::open_regular_at(&parent, Path::new(name))
                .map_err(|_| Error::Conflict("Claim observer member path changed"))?;
            if stamp(&owned(&capture.descriptor, fs.uid, false)?) != capture.signature
                || stamp(&owned(&selected, fs.uid, false)?) != capture.signature
                || raw(&mut selected, capture.bytes.len(), deadline, cancelled)?
                    != capture.bytes.as_ref()
            {
                return Err(Error::Conflict("Claim observer retained metadata changed"));
            }
        }
        for (reference, (retained, identity)) in &self.absences {
            active(deadline, cancelled)?;
            let (parent, name) = reference
                .rsplit_once('/')
                .ok_or(Error::Invalid("Claim observer absent path"))?;
            let current = walk(&root, parent, fs.uid)?;
            if inode(&owned(retained, fs.uid, true)?) != *identity
                || inode(&owned(&current, fs.uid, true)?) != *identity
                || !matches!(
                    rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW),
                    Err(rustix::io::Errno::NOENT)
                )
            {
                return Err(Error::Conflict("Claim observer absent metadata appeared"));
            }
        }
        self.publication.verify_current(fs, deadline, cancelled)?;
        self.verify_package()
    }
}
