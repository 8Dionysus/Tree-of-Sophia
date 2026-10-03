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
    archive_directories: BTreeMap<String, (File, (u64, u64), BTreeSet<String>)>,
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
            archive_directories: BTreeMap::new(),
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
        self.read_selected_internal(path, cap, false, deadline, cancelled)
    }
    fn read_selected_internal(
        &mut self,
        path: &RelativePath,
        cap: usize,
        retained: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Arc<[u8]>> {
        active(deadline, cancelled)?;
        let reference = path.as_str();
        if !reference.starts_with("ToS/")
            || reference.split('/').any(|p| {
                (p.starts_with('.') && !(retained && p == ".record-revisions"))
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
    /// Select only retained record packages named by this exact current
    /// metadata history. No generic hidden-path reader is exposed. The cold
    /// owner resolver subsequently authenticates lineage, package digests,
    /// successor reconstruction and source schemas using these retained bytes.
    pub(crate) fn retain_endpoint_history(
        &mut self,
        source_path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<tos_foundation::JsonValue>> {
        let source_ref = source_path.as_str();
        if !source_ref.starts_with("ToS/source-witnesses/")
            || !source_ref.ends_with(".json")
            || source_ref.split('/').any(|p| {
                p.starts_with('.')
                    || [
                        "payload",
                        "private",
                        "owner-local",
                        "local-content",
                        "catalog",
                    ]
                    .contains(&p)
            })
        {
            return Err(Error::Denied("Claim history exact metadata owner path"));
        }
        let current = self.read_selected(source_path, 2_097_152, deadline, cancelled)?;
        let record = cmd::parse(&current)?;
        let subject = crate::source_forms::metadata_subject(&record)?;
        let id = cmd::text(&subject, "id")?.to_owned();
        let parent = source_ref
            .rsplit_once('/')
            .ok_or(Error::Invalid("Claim history owner parent"))?
            .0;
        let history_ref = RelativePath::parse(&format!("{parent}/source-revision-history.json"))
            .map_err(|_| Error::Invalid("Claim history path"))?;
        let Some(history_raw) = self.read_optional(&history_ref, 2_097_152, deadline, cancelled)?
        else {
            return Ok(vec![]);
        };
        let history = cmd::parse(&history_raw)?;
        cmd::exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
        if ![
            "tos_source_revision_history_v1",
            "tos_source_revision_history_v2",
        ]
        .contains(&cmd::text(&history, "schema_version")?)
            || cmd::text(&history, "record_id")? != id
        {
            return Err(Error::Conflict(
                "Claim current metadata history identity differs",
            ));
        }
        let receipts = cmd::array(&history, "receipts")?;
        if receipts.is_empty() || receipts.len() > 128 {
            return Err(Error::Unsupported("Claim metadata history count budget"));
        }
        let descriptor = cmd::object(vec![
            ("record_id", cmd::string(&id)),
            ("source_path", cmd::string(source_ref)),
        ]);
        let mut exact_refs = Vec::new();
        for receipt in receipts {
            active(deadline, cancelled)?;
            // The maintained cold reader authenticates receipts and replay. Keep
            // its accepted corrections within the same metadata field grammar
            // used by MetadataVersionReader, rather than allowing arbitrary JSON.
            let request = cmd::field(receipt, "request")?;
            let mut request_keys = vec![
                "schema_version",
                "operation",
                "fields",
                "forms",
                "reason",
                "command_id",
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
            ];
            if receipt.object_get("publication").is_some() {
                request_keys.push("expected_publication");
            }
            cmd::exact_keys(request, &request_keys)?;
            if cmd::text(request, "schema_version")? != "tos_local_source_command_v1"
                || cmd::text(request, "operation")? != "record.revise"
            {
                return Err(Error::Unsupported(
                    "Claim retained operation requires its typed owner verifier",
                ));
            }
            let allowed: &[&str] = match cmd::text(&record, "schema_version")? {
                "tos_corpus_record_v1" => {
                    &["preferred_label", "notes", "field_languages", "source_refs"]
                }
                "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => &[
                    "path_identity",
                    "physical_description",
                    "find_context",
                    "bibliography",
                ],
                "tos_scholarly_composite_witness_v1" => &["preferred_label", "editorial_object"],
                "tos_source_link_v1" => &[
                    "preferred_label",
                    "variant_labels",
                    "notes",
                    "source_refs",
                    "provider_label",
                ],
                "tos_historical_record_v1" => &[
                    "preferred_label",
                    "variant_labels",
                    "notes",
                    "field_languages",
                    "source_refs",
                    "extensions",
                    "semantic_content",
                ],
                _ => &[
                    "preferred_label",
                    "variant_labels",
                    "notes",
                    "field_languages",
                    "source_refs",
                    "extensions",
                    "semantic_content",
                    "semantic_scope",
                ],
            };
            let fields = cmd::field(request, "fields")?
                .as_object()
                .ok_or(Error::Invalid("Claim retained correction fields"))?;
            if fields.is_empty()
                || fields
                    .iter()
                    .any(|(k, _)| k.as_str().is_none_or(|k| !allowed.contains(&k)))
            {
                return Err(Error::Denied(
                    "Claim retained correction exceeds metadata fields",
                ));
            }
            let previous = cmd::field(receipt, "previous_source")?;
            cmd::exact_keys(previous, &["id", "version", "digest"])?;
            if cmd::text(previous, "id")? != id {
                return Err(Error::Conflict(
                    "Claim retained metadata belongs to another identity",
                ));
            }
            let revision = cmd::text(receipt, "previous_revision")?;
            let suffix = revision
                .strip_prefix("sha256:")
                .ok_or(Error::Invalid("Claim historical package digest"))?;
            if suffix.len() != 64
                || !suffix
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Error::Invalid("Claim historical package digest"));
            }
            let archive = crate::source_revisions::archive_path(&descriptor, revision)?;
            if cmd::text(receipt, "archive_path")? != archive {
                return Err(Error::Conflict("Claim history archive locator differs"));
            }
            let directory = walk(&self.filesystem.root, &archive, self.filesystem.uid)?;
            let directory_identity = inode(&owned(&directory, self.filesystem.uid, true)?);
            let manifest_ref = RelativePath::parse(&format!("{archive}/manifest.json"))
                .map_err(|_| Error::Invalid("Claim history manifest path"))?;
            let manifest_raw =
                self.read_selected_internal(&manifest_ref, 2_097_152, true, deadline, cancelled)?;
            let manifest = cmd::parse(&manifest_raw)?;
            let selected =
                cmd::text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
            let mut keys = vec![
                "schema_version",
                "source_path",
                "source",
                "revision",
                "files",
            ];
            if selected {
                keys.push("publication_protocol");
            }
            cmd::exact_keys(&manifest, &keys)?;
            if (!selected
                && cmd::text(&manifest, "schema_version")? != "tos_source_package_archive_v1")
                || (selected
                    && cmd::text(&manifest, "publication_protocol")?
                        != "tos_selected_source_metadata_v1")
                || cmd::text(&manifest, "source_path")? != source_ref
                || !cmd::same(cmd::field(&manifest, "source")?, previous)?
                || cmd::text(&manifest, "revision")? != revision
            {
                return Err(Error::Conflict("Claim retained manifest identity differs"));
            }
            let members = cmd::field(&manifest, "files")?
                .as_object()
                .ok_or(Error::Invalid("Claim history manifest members"))?;
            if members.is_empty() || members.len() > 64 {
                return Err(Error::Unsupported("Claim history package member count"));
            }
            let mut names = BTreeSet::from(["manifest.json".to_owned()]);
            let mut package_bytes = 0usize;
            for (name, binding) in members {
                let name = name
                    .as_str()
                    .ok_or(Error::Invalid("Claim history package member name"))?;
                if name.is_empty()
                    || name.contains(['/', '\\', '\0'])
                    || [".", ".."].contains(&name)
                {
                    return Err(Error::Invalid("Claim history exact package basename"));
                }
                cmd::exact_keys(binding, &["blob", "sha256", "bytes"])?;
                let digest = cmd::text(binding, "sha256")?;
                let suffix = digest
                    .strip_prefix("sha256:")
                    .ok_or(Error::Invalid("Claim history member digest"))?;
                if suffix.len() != 64
                    || !suffix
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Error::Invalid("Claim history member digest"));
                }
                let blob = cmd::text(binding, "blob")?;
                if blob != format!("{suffix}.blob") {
                    return Err(Error::Conflict("Claim retained blob locator differs"));
                }
                let declared = cmd::integer(binding, "bytes")?;
                if declared > 2_097_152 {
                    return Err(Error::Unsupported("Claim history member bytes"));
                }
                let blob_ref = RelativePath::parse(&format!("{archive}/{blob}"))
                    .map_err(|_| Error::Invalid("Claim history blob path"))?;
                let raw = self.read_selected_internal(
                    &blob_ref,
                    declared as usize,
                    true,
                    deadline,
                    cancelled,
                )?;
                if raw.len() as u64 != declared || Digest256::of_bytes(&raw).to_prefixed() != digest
                {
                    return Err(Error::Conflict("Claim retained blob bytes differ"));
                }
                package_bytes = package_bytes
                    .checked_add(raw.len())
                    .filter(|n| *n <= 8_388_608)
                    .ok_or(Error::Unsupported("Claim history package byte budget"))?;
                names.insert(blob.to_owned());
            }
            let actual = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
                .map_err(|_| Error::Invalid("Claim retained package enumeration"))?;
            let mut observed = BTreeSet::new();
            for entry in actual {
                active(deadline, cancelled)?;
                let name = entry
                    .map_err(|_| Error::Invalid("Claim retained package entry"))?
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::Invalid("Claim retained package UTF8"))?;
                if observed.len() >= 65 || !observed.insert(name) {
                    return Err(Error::Unsupported(
                        "Claim retained package enumeration budget",
                    ));
                }
            }
            if observed != names {
                return Err(Error::Conflict(
                    "Claim retained package pathname closure differs",
                ));
            }
            if let Some((retained, identity, closure)) = self.archive_directories.get(&archive) {
                if *identity != directory_identity
                    || inode(&owned(retained, self.filesystem.uid, true)?) != directory_identity
                    || *closure != names
                {
                    return Err(Error::Conflict("Claim retained archive directory changed"));
                }
            } else {
                self.archive_directories
                    .insert(archive, (directory, directory_identity, names));
            }
            exact_refs.push(previous.clone());
        }
        self.verify_current(deadline, cancelled)?;
        Ok(exact_refs)
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
        for (reference, (retained, identity, closure)) in &self.archive_directories {
            active(deadline, cancelled)?;
            let current = walk(&root, reference, fs.uid)?;
            if inode(&owned(retained, fs.uid, true)?) != *identity
                || inode(&owned(&current, fs.uid, true)?) != *identity
            {
                return Err(Error::Conflict("Claim retained archive pathname detached"));
            }
            let entries = std::fs::read_dir(format!("/proc/self/fd/{}", current.as_raw_fd()))
                .map_err(|_| Error::Invalid("Claim current archive enumeration"))?;
            let mut names = BTreeSet::new();
            for entry in entries {
                active(deadline, cancelled)?;
                let name = entry
                    .map_err(|_| Error::Invalid("Claim current archive entry"))?
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::Invalid("Claim current archive UTF8"))?;
                if names.len() >= 65 || !names.insert(name) {
                    return Err(Error::Unsupported("Claim current archive entry budget"));
                }
            }
            if &names != closure {
                return Err(Error::Conflict(
                    "Claim retained archive member closure changed",
                ));
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
