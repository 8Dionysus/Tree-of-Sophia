//! Protected dual-root transport for the owner-local TextLayer/TextUnit writer.
//! A context routes bytes; separate command grants and rights still decide reads
//! and mutations. Reserved owner-local references never fall back to ToS.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, protected_configuration_parents, raw};
use std::collections::BTreeMap;
use std::fs::{File, Metadata};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const CONTEXT_SCHEMA_ID: &str =
    "https://tree-of-sophia.local/ToS/contracts/owner-local-source-context.schema.json";
const OWNER_HOME: &str = "ToS/source-witnesses/owner-local/";
const MAX_CONTEXT_BYTES: usize = 1_048_576;
const MAX_PRIVATE_PACKAGE_FILES: usize = 12;
const MAX_PRIVATE_PACKAGE_BYTES: usize = 12 * 1024 * 1024;

pub(crate) fn normalized_absolute(value: &str) -> SourceCommandResult<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute()
        || path == Path::new("/")
        || value.contains('\\')
        || value.contains('\0')
        || value.ends_with('/')
        || value[1..]
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path
            .components()
            .skip(1)
            .any(|part| !matches!(part, Component::Normal(_)))
        || path.to_str() != Some(value)
    {
        return Err(SourceCommandError::Denied("owner-local absolute path"));
    }
    Ok(path.to_path_buf())
}

fn identity(meta: &Metadata) -> (u64, u64, u32, u32) {
    (meta.dev(), meta.ino(), meta.uid(), meta.mode() & 0o7777)
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

fn checked_file(file: &File, uid: u32, confidential: bool) -> SourceCommandResult<Metadata> {
    let meta = file
        .metadata()
        .map_err(|_| SourceCommandError::Denied("owner-local file metadata"))?;
    if !meta.is_file()
        || meta.uid() != uid && (confidential || meta.uid() != 0)
        || meta.mode() & 0o022 != 0
        || confidential && meta.mode() & 0o7777 != 0o600
    {
        return Err(SourceCommandError::Denied(
            "owner-local file ownership or mode",
        ));
    }
    Ok(meta)
}

fn checked_directory(file: &File, uid: u32, confidential: bool) -> SourceCommandResult<Metadata> {
    let meta = file
        .metadata()
        .map_err(|_| SourceCommandError::Denied("owner-local directory metadata"))?;
    if !meta.is_dir()
        || meta.uid() != uid && (confidential || meta.uid() != 0)
        || meta.mode() & 0o022 != 0
        || confidential && meta.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Denied(
            "owner-local directory ownership or mode",
        ));
    }
    Ok(meta)
}

pub(crate) fn read_absolute(
    path: &Path,
    uid: u32,
    confidential: bool,
    max: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    active(deadline, cancelled)?;
    protected_configuration_parents(path, uid)?;
    let mut file = tos_fd_open::open_absolute_regular(path, max as u64)
        .map_err(|_| SourceCommandError::Denied("owner-local path absent or unsafe"))?;
    let before = checked_file(&file, uid, confidential)?;
    let bytes = raw(&mut file, max, deadline, cancelled)?;
    let after = checked_file(&file, uid, confidential)?;
    let current = tos_fd_open::open_absolute_regular(path, max as u64)
        .map_err(|_| SourceCommandError::Conflict("owner-local path changed"))?;
    let at_path = checked_file(&current, uid, confidential)?;
    if stamp(&before) != stamp(&after) || stamp(&before) != stamp(&at_path) {
        return Err(SourceCommandError::Conflict(
            "owner-local path changed during read",
        ));
    }
    Ok(bytes)
}

pub(crate) struct OwnerTextContext {
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    schema_raw: Vec<u8>,
    public_root: PathBuf,
    private_root: PathBuf,
    private_prefix: String,
    public_identity: (u64, u64, u32, u32),
    private_identity: (u64, u64, u32, u32),
    uid: u32,
}

/// One protected initial-layer delegation. It selects an operation and exact
/// inputs, but the separate current metadata/rights/identity checks remain
/// mandatory before any payload or private package read.
pub(crate) struct OwnerTextInitialLayerSelection {
    pub(crate) config: JsonValue,
    pub(crate) raw: Vec<u8>,
    pub(crate) path: PathBuf,
}

pub(crate) struct OwnerTextUnitSelection {
    pub(crate) config: JsonValue,
    pub(crate) raw: Vec<u8>,
    pub(crate) path: PathBuf,
}

pub(crate) struct OwnerTextDerivedSelection {
    pub(crate) config: JsonValue,
    pub(crate) raw: Vec<u8>,
    pub(crate) path: PathBuf,
    pub(crate) operation: String,
}

impl OwnerTextDerivedSelection {
    pub(crate) fn select(
        context: &OwnerTextContext,
        owner_config: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let path = normalized_absolute(owner_config.to_str().ok_or(
            SourceCommandError::Invalid("native derived TextLayer config UTF-8"),
        )?)?;
        let raw = read_absolute(
            &path,
            context.uid,
            true,
            MAX_CONTEXT_BYTES,
            deadline,
            cancelled,
        )?;
        let config = cmd::parse(&raw)?;
        cmd::exact_keys(
            &config,
            &[
                "schema_version",
                "uid",
                "principal_id",
                "authority_ref",
                "expires_at",
                "source_context_ref",
                "source_path",
                "allowed_operations",
                "source_scope",
                "source_record_refs",
                "source_record_sha256",
                "manifest_sha256",
                "source_access",
                "derivation_access",
                "input",
                "material",
                "policy",
                "identities",
                "maker",
                "language",
                "limits",
            ],
        )?;
        let operation = cmd::array(&config, "allowed_operations")?;
        if operation.len() != 1 {
            return Err(SourceCommandError::Denied(
                "native derived TextLayer operation",
            ));
        }
        let operation = operation[0].as_str().ok_or(SourceCommandError::Invalid(
            "native derived TextLayer operation",
        ))?;
        let schema = cmd::text(&config, "schema_version")?;
        let expected = match operation {
            "text-layer.correct"
            | "text-layer.normalize"
            | "text-layer.record-transcription"
            | "text-layer.record-ocr" => "tos_local_text_layer_derive_owner_v1",
            "text-layer.record-owner-ocr" => "tos_local_text_layer_record_owner_ocr_v1",
            "text-layer.record-owner-page-ocr" => "tos_local_text_layer_record_owner_page_ocr_v1",
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "native derived TextLayer operation",
                ));
            }
        };
        if schema != expected
            || cmd::integer(&config, "uid")? != u64::from(context.uid)
            || cmd::text(&config, "source_context_ref")?
                != context
                    .configuration_path
                    .to_str()
                    .ok_or(SourceCommandError::Invalid(
                        "native derived TextLayer context path",
                    ))?
            || cmd::text(&config, "principal_id")?.trim().is_empty()
            || cmd::text(&config, "authority_ref")?.trim().is_empty()
        {
            return Err(SourceCommandError::Denied(
                "native derived TextLayer delegation",
            ));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let source_path = cmd::text(&config, "source_path")?;
        let (target, private) = context.physical(source_path)?;
        if !private
            || target.file_name().and_then(|name| name.to_str())
                != Some("source-text-layer.v1.json")
            || source_path.split('/').count() < 7
            || source_path.split('/').any(|part| {
                part.starts_with('.') || matches!(part, "payload" | "local-content" | "catalog")
            })
        {
            return Err(SourceCommandError::Denied(
                "native derived TextLayer destination",
            ));
        }
        context.check_private_parents(
            source_path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid(
                    "native derived TextLayer package home",
                ))?
                .0,
        )?;
        let supplied = matches!(
            operation,
            "text-layer.record-transcription"
                | "text-layer.record-ocr"
                | "text-layer.record-owner-ocr"
                | "text-layer.record-owner-page-ocr"
        );
        let access = cmd::field(&config, "source_access")?;
        cmd::exact_keys(
            access,
            if supplied {
                &[
                    "read_scope",
                    "access_allowed",
                    "byte_size",
                    "payload_root",
                    "authority_ref",
                    "expires_at",
                ]
            } else {
                &[
                    "read_scope",
                    "access_allowed",
                    "byte_size",
                    "authority_ref",
                    "expires_at",
                ]
            },
        )?;
        if cmd::field(access, "access_allowed")? != &JsonValue::Bool(true)
            || cmd::text(access, "read_scope")?
                != if supplied {
                    "exact_acquired_file"
                } else {
                    "exact_text_layer"
                }
            || cmd::text(access, "authority_ref")?.trim().is_empty()
            || cmd::integer(access, "byte_size")? == 0
            || cmd::integer(access, "byte_size")?
                > if supplied { 512 * 1024 * 1024 } else { 131_072 }
        {
            return Err(SourceCommandError::Denied(
                "native derived TextLayer read grant",
            ));
        }
        cmd::validate_expiry(
            cmd::text(access, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        if supplied {
            let payload_root = normalized_absolute(cmd::text(access, "payload_root")?)?;
            if payload_root.starts_with(&context.private_root)
                || context.private_root.starts_with(&payload_root)
            {
                return Err(SourceCommandError::Denied(
                    "native derived TextLayer distinct payload root",
                ));
            }
            protected_configuration_parents(&payload_root.join("probe"), context.uid)?;
            let payload = tos_fd_open::open_absolute_directory(&payload_root)
                .map_err(|_| SourceCommandError::Denied("native derived TextLayer payload root"))?;
            checked_directory(&payload, context.uid, false)?;
        }
        let derive = cmd::field(&config, "derivation_access")?;
        cmd::exact_keys(
            derive,
            &[
                "derivation_allowed",
                "operation",
                "rights_record_refs",
                "content_visibility",
                "authority_ref",
                "expires_at",
            ],
        )?;
        if cmd::field(derive, "derivation_allowed")? != &JsonValue::Bool(true)
            || cmd::text(derive, "authority_ref")?.trim().is_empty()
            || cmd::text(derive, "content_visibility")? != "local_only"
            || !(1..=16).contains(&cmd::array(derive, "rights_record_refs")?.len())
            || cmd::text(derive, "operation")?
                != match operation {
                    "text-layer.correct" => "correction",
                    "text-layer.normalize" => "unicode_normalization",
                    "text-layer.record-transcription" => {
                        cmd::text(cmd::field(&config, "policy")?, "method")?
                    }
                    _ => "ocr",
                }
        {
            return Err(SourceCommandError::Denied(
                "native derived TextLayer derivation grant",
            ));
        }
        cmd::validate_expiry(
            cmd::text(derive, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let scope = cmd::field(&config, "source_scope")?;
        cmd::exact_keys(
            scope,
            &[
                "work_ref",
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
                "file_sha256",
            ],
        )?;
        let file_sha = cmd::text(scope, "file_sha256")?;
        if !lower_sha(file_sha)
            || cmd::text(scope, "file_ref")? != format!("tos.file.sha256.{file_sha}")
            || !lower_sha(cmd::text(&config, "manifest_sha256")?)
        {
            return Err(SourceCommandError::Invalid(
                "native derived TextLayer File identity",
            ));
        }
        let refs = cmd::field(&config, "source_record_refs")?;
        let digests = cmd::field(&config, "source_record_sha256")?;
        cmd::exact_keys(refs, &["work", "expression", "edition", "item"])?;
        cmd::exact_keys(digests, &["work", "expression", "edition", "item"])?;
        for kind in ["work", "expression", "edition", "item"] {
            let reference = cmd::text(refs, kind)?;
            let (record_path, private) = context.physical(reference)?;
            if private
                || record_path.file_name().and_then(|name| name.to_str())
                    != Some(format!("{kind}.json").as_str())
                || !lower_sha(cmd::text(digests, kind)?)
            {
                return Err(SourceCommandError::Denied(
                    "native derived TextLayer source record",
                ));
            }
        }
        let input = cmd::field(&config, "input")?;
        cmd::exact_keys(
            input,
            if supplied {
                &["kind", "anchor"]
            } else {
                &["kind", "binding"]
            },
        )?;
        if cmd::text(input, "kind")?
            != if supplied {
                if operation == "text-layer.record-owner-page-ocr" {
                    "retained_pdf_page"
                } else {
                    "acquired_file"
                }
            } else {
                "text_layer"
            }
        {
            return Err(SourceCommandError::Denied(
                "native derived TextLayer selected input",
            ));
        }
        if supplied {
            let anchor = cmd::field(input, "anchor")?;
            cmd::exact_keys(anchor, &["anchor_id", "record_ref", "record_sha256"])?;
            let (anchor_path, _) = context.physical(cmd::text(anchor, "record_ref")?)?;
            if !cmd::text(anchor, "anchor_id")?.starts_with("tos.anchor.")
                || !lower_sha(cmd::text(anchor, "record_sha256")?)
                || anchor_path.extension().and_then(|part| part.to_str()) != Some("json")
                || anchor_path.starts_with(target.parent().ok_or(SourceCommandError::Invalid(
                    "native derived TextLayer target",
                ))?)
            {
                return Err(SourceCommandError::Denied(
                    "native derived TextLayer exact anchor",
                ));
            }
        }
        let ids = cmd::field(&config, "identities")?;
        cmd::exact_keys(ids, &["layer_id", "provenance_event_id"])?;
        if !opaque_id(cmd::text(ids, "layer_id")?, "text-layer")
            || !opaque_id(cmd::text(ids, "provenance_event_id")?, "event")
        {
            return Err(SourceCommandError::Invalid(
                "native derived TextLayer opaque identity",
            ));
        }
        let maker = cmd::field(&config, "maker")?;
        cmd::exact_keys(maker, &["maker_type", "agent_ref", "method", "version"])?;
        if cmd::text(maker, "agent_ref")? != cmd::text(&config, "principal_id")?
            || !["human", "model", "software", "mixed"].contains(&cmd::text(maker, "maker_type")?)
            || cmd::text(maker, "method")?.trim().is_empty()
            || cmd::text(maker, "version")?.trim().is_empty()
        {
            return Err(SourceCommandError::Denied("native derived TextLayer maker"));
        }
        if operation == "text-layer.normalize"
            && (cmd::text(maker, "maker_type")? != "software"
                || cmd::text(maker, "method")? != "tos.unicode.normalize.v1"
                || cmd::text(maker, "version")? != "16.0.0")
        {
            return Err(SourceCommandError::Denied("native TextLayer Unicode maker"));
        }
        if operation.starts_with("text-layer.record-owner-") {
            let material = cmd::field(&config, "material")?;
            crate::source_text_owner_ocr::validate_material(
                material,
                operation == "text-layer.record-owner-page-ocr",
            )?;
            cmd::validate_expiry(
                cmd::text(material, "expires_at")?,
                &crate::source_serialization::instant()?,
            )?;
            if cmd::text(maker, "maker_type")? != "software"
                || !["de", "deu", "ru", "rus"].contains(&cmd::text(&config, "language")?)
            {
                return Err(SourceCommandError::Denied(
                    "native owner OCR maker or language",
                ));
            }
        }
        if supplied && !operation.starts_with("text-layer.record-owner-") {
            let material = cmd::field(&config, "material")?;
            cmd::exact_keys(
                material,
                &[
                    "authority_ref",
                    "expires_at",
                    "content_ref",
                    "content_sha256",
                    "byte_size",
                    "access_allowed",
                    "reported_maker",
                    "provider_execution",
                ],
            )?;
            cmd::validate_expiry(
                cmd::text(material, "expires_at")?,
                &crate::source_serialization::instant()?,
            )?;
            let material_ref = cmd::text(material, "content_ref")?;
            let (physical, private) = context.physical(material_ref)?;
            if !private
                || physical.starts_with(
                    target
                        .parent()
                        .ok_or(SourceCommandError::Invalid("native derived TextLayer home"))?,
                )
                || !lower_sha(cmd::text(material, "content_sha256")?)
                || !(1..=131_072).contains(&cmd::integer(material, "byte_size")?)
                || cmd::field(material, "access_allowed")? != &JsonValue::Bool(true)
                || cmd::text(material, "authority_ref")?.trim().is_empty()
                || cmd::text(material, "provider_execution")? != "not_observed"
            {
                return Err(SourceCommandError::Denied(
                    "native derived TextLayer separate supplied material",
                ));
            }
            let reported = cmd::field(material, "reported_maker")?;
            cmd::exact_keys(reported, &["maker_type", "agent_ref", "method", "version"])?;
            let method = cmd::text(cmd::field(&config, "policy")?, "method")?;
            let expected = if method == "manual_transcription" {
                &["human"][..]
            } else if method == "model_transcription" {
                &["model"][..]
            } else {
                &["software", "model", "mixed"][..]
            };
            if !expected.contains(&cmd::text(reported, "maker_type")?)
                || ["agent_ref", "method", "version"].iter().any(|key| {
                    cmd::text(reported, key)
                        .map_or(true, |value| value.trim().is_empty() || value.len() > 2048)
                })
            {
                return Err(SourceCommandError::Invalid(
                    "native derived TextLayer reported maker",
                ));
            }
        }
        let language = cmd::text(&config, "language")?;
        let mut parts = language.split('-');
        let base = parts.next().unwrap_or("");
        if !(2..=3).contains(&base.len())
            || !base.bytes().all(|b| b.is_ascii_lowercase())
            || parts.any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric()))
        {
            return Err(SourceCommandError::Invalid(
                "native derived TextLayer language",
            ));
        }
        let limits = cmd::field(&config, "limits")?;
        cmd::exact_keys(limits, &["max_output_bytes", "max_seconds"])?;
        if !(1..=131_072).contains(&cmd::integer(limits, "max_output_bytes")?)
            || !(1..=60).contains(&cmd::integer(limits, "max_seconds")?)
        {
            return Err(SourceCommandError::Unsupported(
                "native derived TextLayer limits",
            ));
        }
        Ok(Self {
            config,
            raw,
            path,
            operation: operation.to_owned(),
        })
    }
}

impl OwnerTextUnitSelection {
    pub(crate) fn select(
        context: &OwnerTextContext,
        owner_config: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let path = normalized_absolute(owner_config.to_str().ok_or(
            SourceCommandError::Invalid("native TextUnit configuration path UTF-8"),
        )?)?;
        let raw = read_absolute(
            &path,
            context.uid,
            true,
            MAX_CONTEXT_BYTES,
            deadline,
            cancelled,
        )?;
        let config = cmd::parse(&raw)?;
        cmd::exact_keys(
            &config,
            &[
                "schema_version",
                "uid",
                "principal_id",
                "authority_ref",
                "expires_at",
                "source_context_ref",
                "source_path",
                "allowed_operations",
                "source_binding",
                "source_access",
                "allowed_text_scope",
                "packet_id",
                "scheme_id",
                "segmentation_id",
                "scope_anchor_ref",
                "unit_slots",
                "gap_anchor_refs",
                "scheme",
                "method",
                "provenance_event_id",
            ],
        )?;
        let version = cmd::text(&config, "schema_version")?;
        if ![
            "tos_local_text_unit_create_owner_v1",
            "tos_local_text_unit_create_owner_v2",
        ]
        .contains(&version)
            || cmd::integer(&config, "uid")? != u64::from(context.uid)
            || cmd::text(&config, "source_context_ref")?
                != context
                    .configuration_path
                    .to_str()
                    .ok_or(SourceCommandError::Invalid(
                        "native TextUnit context path UTF-8",
                    ))?
            || cmd::text(&config, "principal_id")?.trim().is_empty()
            || cmd::text(&config, "authority_ref")?.trim().is_empty()
            || cmd::array(&config, "allowed_operations")? != [cmd::string("text-unit.create")]
        {
            return Err(SourceCommandError::Denied("native TextUnit delegation"));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let source_path = cmd::text(&config, "source_path")?;
        let (target, private) = context.physical(source_path)?;
        if !private
            || target.file_name().and_then(|name| name.to_str()) != Some("source-text-unit.v1.json")
            || source_path.split('/').count() < 7
            || source_path.split('/').any(|part| {
                part.starts_with('.') || matches!(part, "payload" | "local-content" | "catalog")
            })
        {
            return Err(SourceCommandError::Denied("native TextUnit private target"));
        }
        context.check_private_parents(
            source_path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid(
                    "native TextUnit package parent",
                ))?
                .0,
        )?;
        let access = cmd::field(&config, "source_access")?;
        cmd::exact_keys(access, &["read_scope", "access_allowed", "authority_ref"])?;
        if cmd::text(access, "read_scope")? != "exact_owner_local"
            || cmd::field(access, "access_allowed")? != &JsonValue::Bool(true)
            || cmd::text(access, "authority_ref")?.trim().is_empty()
        {
            return Err(SourceCommandError::Denied(
                "native TextUnit exact read grant",
            ));
        }
        let scope = cmd::field(&config, "allowed_text_scope")?;
        cmd::exact_keys(scope, &["start", "end"])?;
        if cmd::integer(scope, "start")? >= cmd::integer(scope, "end")? {
            return Err(SourceCommandError::Invalid(
                "native TextUnit nonempty scope",
            ));
        }
        if version == "tos_local_text_unit_create_owner_v2" {
            cmd::exact_keys(
                cmd::field(&config, "source_binding")?,
                &["schema_version", "text_layer", "source_record_refs"],
            )?;
        }
        let home = source_path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("native TextUnit package home"))?
            .0;
        let method = cmd::field(&config, "method")?;
        cmd::exact_keys(
            method,
            &[
                "maker_kind",
                "agent_ref",
                "method_name",
                "method_version",
                "software_refs",
                "model_ref",
                "configuration_ref",
                "locale",
                "unicode_version",
                "unicode_revision",
                "tailoring_ref",
                "provenance_event_ref",
                "made_at",
                "output_posture",
            ],
        )?;
        if cmd::text(method, "agent_ref")? != cmd::text(&config, "principal_id")?
            || cmd::text(method, "provenance_event_ref")?
                != cmd::text(&config, "provenance_event_id")?
            || cmd::text(method, "configuration_ref")?
                != format!("{home}/source-create-owner-configuration.json")
            || cmd::text(method, "maker_kind")? == "synthetic_fixture"
        {
            return Err(SourceCommandError::Denied("native TextUnit maker binding"));
        }
        let scheme = cmd::field(&config, "scheme")?;
        cmd::exact_keys(
            scheme,
            &["scheme_name", "analysis_role", "boundary_basis", "policies"],
        )?;
        let delegated = [
            ("packet_id", "source-text-unit-packet"),
            ("scheme_id", "text-unit-scheme"),
            ("segmentation_id", "text-segmentation"),
        ];
        if delegated
            .iter()
            .any(|(name, kind)| cmd::text(&config, name).map_or(true, |id| !opaque_id(id, kind)))
        {
            return Err(SourceCommandError::Invalid(
                "native TextUnit opaque identities",
            ));
        }
        if cmd::array(&config, "unit_slots")?.is_empty()
            || cmd::array(&config, "unit_slots")?.len() > 256
            || cmd::array(&config, "gap_anchor_refs")?.len() > 257
        {
            return Err(SourceCommandError::Unsupported(
                "native TextUnit delegated count",
            ));
        }
        Ok(Self { config, raw, path })
    }
}

fn lower_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn opaque_id(value: &str, kind: &str) -> bool {
    let Some(suffix) = value.strip_prefix(&format!("tos.{kind}.sid-")) else {
        return false;
    };
    suffix.len() == 32
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl OwnerTextInitialLayerSelection {
    pub(crate) fn select(
        context: &OwnerTextContext,
        owner_config: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let path = normalized_absolute(owner_config.to_str().ok_or(
            SourceCommandError::Invalid("native TextLayer configuration path UTF-8"),
        )?)?;
        let raw = read_absolute(
            &path,
            context.uid,
            true,
            MAX_CONTEXT_BYTES,
            deadline,
            cancelled,
        )?;
        let config = cmd::parse(&raw)?;
        cmd::exact_keys(
            &config,
            &[
                "schema_version",
                "uid",
                "principal_id",
                "authority_ref",
                "expires_at",
                "source_context_ref",
                "source_path",
                "allowed_operations",
                "source_scope",
                "source_record_refs",
                "source_record_sha256",
                "manifest_sha256",
                "source_access",
                "derivation_access",
                "member",
                "selector",
                "policy",
                "identities",
                "maker",
                "language",
                "limits",
            ],
        )?;
        if cmd::text(&config, "schema_version")? != "tos_local_text_layer_create_owner_v1"
            || cmd::integer(&config, "uid")? != u64::from(context.uid)
            || cmd::text(&config, "principal_id")?.trim().is_empty()
            || cmd::text(&config, "authority_ref")?.trim().is_empty()
            || cmd::text(&config, "source_context_ref")?
                != context
                    .configuration_path
                    .to_str()
                    .ok_or(SourceCommandError::Invalid(
                        "native TextLayer context path UTF-8",
                    ))?
            || cmd::array(&config, "allowed_operations")? != [cmd::string("text-layer.create")]
        {
            return Err(SourceCommandError::Denied("native TextLayer delegation"));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let source_path = cmd::text(&config, "source_path")?;
        let (target, private) = context.physical(source_path)?;
        if !private
            || target.file_name().and_then(|part| part.to_str())
                != Some("source-text-layer.v1.json")
            || source_path.split('/').count() < 7
            || source_path.split('/').any(|part| {
                part.starts_with('.') || matches!(part, "payload" | "local-content" | "catalog")
            })
        {
            return Err(SourceCommandError::Denied(
                "native TextLayer private destination",
            ));
        }
        let package_ref = source_path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "native TextLayer package parent",
            ))?
            .0;
        context.check_private_parents(package_ref)?;
        let access = cmd::field(&config, "source_access")?;
        cmd::exact_keys(
            access,
            &[
                "read_scope",
                "access_allowed",
                "authority_ref",
                "expires_at",
                "payload_root",
                "byte_size",
            ],
        )?;
        if cmd::text(access, "read_scope")? != "exact_acquired_file"
            || cmd::field(access, "access_allowed")? != &JsonValue::Bool(true)
            || cmd::text(access, "authority_ref")?.trim().is_empty()
            || !(1..=512 * 1024 * 1024).contains(&cmd::integer(access, "byte_size")?)
        {
            return Err(SourceCommandError::Denied(
                "native TextLayer exact File read grant",
            ));
        }
        cmd::validate_expiry(
            cmd::text(access, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let payload_root = normalized_absolute(cmd::text(access, "payload_root")?)?;
        if payload_root.starts_with(&context.private_root)
            || context.private_root.starts_with(&payload_root)
        {
            return Err(SourceCommandError::Denied(
                "native TextLayer distinct payload root",
            ));
        }
        protected_configuration_parents(&payload_root.join("probe"), context.uid)?;
        let payload = tos_fd_open::open_absolute_directory(&payload_root)
            .map_err(|_| SourceCommandError::Denied("native TextLayer payload root"))?;
        checked_directory(&payload, context.uid, false)?;
        let derivation = cmd::field(&config, "derivation_access")?;
        cmd::exact_keys(
            derivation,
            &[
                "authority_ref",
                "expires_at",
                "derivation_allowed",
                "operation",
                "rights_record_refs",
                "content_visibility",
            ],
        )?;
        if cmd::text(derivation, "authority_ref")?.trim().is_empty()
            || cmd::field(derivation, "derivation_allowed")? != &JsonValue::Bool(true)
            || cmd::text(derivation, "operation")? != "structural_extraction"
            || cmd::text(derivation, "content_visibility")? != "local_only"
            || !(1..=16).contains(&cmd::array(derivation, "rights_record_refs")?.len())
        {
            return Err(SourceCommandError::Denied(
                "native TextLayer separate derivation grant",
            ));
        }
        cmd::validate_expiry(
            cmd::text(derivation, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let scope = cmd::field(&config, "source_scope")?;
        cmd::exact_keys(
            scope,
            &[
                "work_ref",
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
                "file_sha256",
            ],
        )?;
        let file_sha = cmd::text(scope, "file_sha256")?;
        if !lower_sha(file_sha)
            || cmd::text(scope, "file_ref")? != format!("tos.file.sha256.{file_sha}")
            || !lower_sha(cmd::text(&config, "manifest_sha256")?)
        {
            return Err(SourceCommandError::Invalid(
                "native TextLayer exact File identity",
            ));
        }
        let digests = cmd::field(&config, "source_record_sha256")?;
        let refs = cmd::field(&config, "source_record_refs")?;
        cmd::exact_keys(refs, &["work", "expression", "edition", "item"])?;
        cmd::exact_keys(digests, &["work", "expression", "edition", "item"])?;
        if ["work", "expression", "edition", "item"]
            .iter()
            .any(|kind| cmd::text(digests, kind).map_or(true, |sha| !lower_sha(sha)))
        {
            return Err(SourceCommandError::Invalid(
                "native TextLayer record digests",
            ));
        }
        for kind in ["work", "expression", "edition", "item"] {
            let reference = cmd::text(refs, kind)?;
            let (record_path, confidential) = context.physical(reference)?;
            let basename = format!("{kind}.json");
            if confidential
                || record_path.file_name().and_then(|part| part.to_str()) != Some(basename.as_str())
            {
                return Err(SourceCommandError::Denied(
                    "native TextLayer public record locator",
                ));
            }
        }
        let member = cmd::field(&config, "member")?;
        cmd::exact_keys(member, &["member_path", "member_sha256"])?;
        let selected = cmd::text(member, "member_path")?;
        if selected.is_empty()
            || selected.len() > 1024
            || selected.contains('\0')
            || selected.contains('\\')
            || selected.starts_with('/')
            || selected
                .trim_end_matches('/')
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
            || !lower_sha(cmd::text(member, "member_sha256")?)
        {
            return Err(SourceCommandError::Invalid(
                "native TextLayer selected member",
            ));
        }
        let identities = cmd::field(&config, "identities")?;
        cmd::exact_keys(
            identities,
            &["layer_id", "anchor_id", "passage_id", "provenance_event_id"],
        )?;
        for (key, kind) in [
            ("layer_id", "text-layer"),
            ("anchor_id", "anchor"),
            ("passage_id", "passage"),
            ("provenance_event_id", "event"),
        ] {
            if !opaque_id(cmd::text(identities, key)?, kind) {
                return Err(SourceCommandError::Invalid(
                    "native TextLayer delegated identity",
                ));
            }
        }
        let maker = cmd::field(&config, "maker")?;
        cmd::exact_keys(maker, &["maker_type", "agent_ref", "method", "version"])?;
        if cmd::text(maker, "maker_type")? != "software"
            || cmd::field(maker, "agent_ref")? != cmd::field(&config, "principal_id")?
            || cmd::text(maker, "method")?.trim().is_empty()
            || cmd::text(maker, "version")?.trim().is_empty()
        {
            return Err(SourceCommandError::Invalid(
                "native TextLayer software maker",
            ));
        }
        let language = cmd::text(&config, "language")?;
        let mut pieces = language.split('-');
        let base = pieces.next().unwrap_or("");
        if !(2..=3).contains(&base.len())
            || !base.bytes().all(|b| b.is_ascii_lowercase())
            || pieces
                .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric()))
        {
            return Err(SourceCommandError::Invalid("native TextLayer language tag"));
        }
        let limits = cmd::field(&config, "limits")?;
        cmd::exact_keys(limits, &["max_output_bytes", "max_seconds"])?;
        if !(1..=8_388_608).contains(&cmd::integer(limits, "max_output_bytes")?)
            || !(1..=60).contains(&cmd::integer(limits, "max_seconds")?)
        {
            return Err(SourceCommandError::Unsupported(
                "native TextLayer delegated limits",
            ));
        }
        active(deadline, cancelled)?;
        Ok(Self { config, raw, path })
    }
}

impl OwnerTextContext {
    pub(crate) fn private_root(&self) -> &Path {
        &self.private_root
    }

    pub(crate) fn private_identity_home(&self) -> PathBuf {
        self.private_root.join(&self.private_prefix)
    }

    pub(crate) fn public_root(&self) -> &Path {
        &self.public_root
    }

    pub(crate) fn public_root_handle(&self) -> SourceCommandResult<File> {
        let root = tos_fd_open::open_absolute_directory(&self.public_root)
            .map_err(|_| SourceCommandError::Conflict("owner-local public root changed"))?;
        if identity(&checked_directory(&root, self.uid, false)?) != self.public_identity {
            return Err(SourceCommandError::Conflict(
                "owner-local public root changed",
            ));
        }
        Ok(root)
    }

    pub(crate) fn account_uid(&self) -> u32 {
        self.uid
    }

    pub(crate) fn select_publication(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<crate::source_creation_store::work_transaction::PublicationSnapshot>
    {
        let root = self.public_root_handle()?;
        crate::source_creation_store::work_transaction::PublicationSnapshot::select_at(
            &root, self.uid, deadline, cancelled,
        )
    }

    pub(crate) fn verify_publication(
        &self,
        selected: &crate::source_creation_store::work_transaction::PublicationSnapshot,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let root = self.public_root_handle()?;
        selected.verify_at(&root, self.uid, deadline, cancelled)
    }

    /// The caller has selected an exact owner-local source file through its
    /// protected grant. Resolve the new package DIRECTORY containing it;
    /// this does not issue a write grant or create an absent parent.
    pub(crate) fn private_new_package_target(
        &self,
        reference: &str,
    ) -> SourceCommandResult<PathBuf> {
        let (path, private) = self.physical(reference)?;
        if !private {
            return Err(SourceCommandError::Denied("owner-local private target"));
        }
        let package_ref = reference
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("owner-local package parent"))?
            .0;
        self.check_private_parents(package_ref)?;
        path.parent()
            .map(Path::to_path_buf)
            .ok_or(SourceCommandError::Invalid("owner-local package directory"))
    }

    /// `selected_schema` must be the actual selected public-cut member; this
    /// transport checks exact bytes, while the caller's schema worker checks
    /// the configuration against that selected contract before using it.
    pub(crate) fn select(
        configuration_path: &Path,
        selected_schema: &[u8],
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Self, JsonValue)> {
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied("owner-local setuid context"));
        }
        let path = normalized_absolute(
            configuration_path
                .to_str()
                .ok_or(SourceCommandError::Denied("owner-local configuration path"))?,
        )?;
        let configuration_raw =
            read_absolute(&path, uid, true, MAX_CONTEXT_BYTES, deadline, cancelled)?;
        let config = cmd::parse(&configuration_raw)?;
        cmd::exact_keys(
            &config,
            &[
                "schema_version",
                "store_id",
                "public_root",
                "private_root",
                "private_prefix",
            ],
        )?;
        if cmd::text(&config, "schema_version")? != "tos_owner_local_source_context_v1" {
            return Err(SourceCommandError::Invalid("owner-local context profile"));
        }
        let store_id = cmd::text(&config, "store_id")?;
        if store_id.len() != 36
            || !store_id.starts_with("sid-")
            || !store_id.as_bytes()[4..].iter().all(u8::is_ascii_hexdigit)
            || store_id.as_bytes()[4..].iter().any(u8::is_ascii_uppercase)
        {
            return Err(SourceCommandError::Invalid("owner-local store identity"));
        }
        let public_root = normalized_absolute(cmd::text(&config, "public_root")?)?;
        let private_root = normalized_absolute(cmd::text(&config, "private_root")?)?;
        if public_root.starts_with(&private_root) || private_root.starts_with(&public_root) {
            return Err(SourceCommandError::Denied("owner-local roots overlap"));
        }
        let private_prefix = format!("{OWNER_HOME}{store_id}/");
        if cmd::text(&config, "private_prefix")? != private_prefix {
            return Err(SourceCommandError::Invalid("owner-local prefix"));
        }
        protected_configuration_parents(&public_root.join("probe"), uid)?;
        protected_configuration_parents(&private_root.join("probe"), uid)?;
        let public = tos_fd_open::open_absolute_directory(&public_root)
            .map_err(|_| SourceCommandError::Denied("owner-local public root"))?;
        let private = tos_fd_open::open_absolute_directory(&private_root)
            .map_err(|_| SourceCommandError::Denied("owner-local private root"))?;
        let public_identity = identity(&checked_directory(&public, uid, false)?);
        let private_identity = identity(&checked_directory(&private, uid, true)?);
        if public_root
            .join(OWNER_HOME.trim_end_matches('/'))
            .symlink_metadata()
            .is_ok()
        {
            return Err(SourceCommandError::Conflict(
                "owner-local public namespace collision",
            ));
        }
        let schema_path = public_root.join(CONTEXT_SCHEMA);
        let schema_raw = read_absolute(
            &schema_path,
            uid,
            false,
            MAX_CONTEXT_BYTES,
            deadline,
            cancelled,
        )?;
        if schema_raw != selected_schema
            || cmd::text(&cmd::parse(&schema_raw)?, "$id")? != CONTEXT_SCHEMA_ID
            || worker.contract_digest(CONTEXT_SCHEMA) != Some(Digest256::of_bytes(&schema_raw))
        {
            return Err(SourceCommandError::Conflict(
                "owner-local selected context schema",
            ));
        }
        match worker.check_reusing_scalar(
            path.to_str().ok_or(SourceCommandError::Invalid(
                "owner-local configuration path UTF-8",
            ))?,
            &cmd::canonical(&config)?,
            CONTEXT_SCHEMA,
            deadline,
            cancelled,
        ) {
            Ok(true) => (),
            Ok(false) => return Err(SourceCommandError::Invalid("owner-local context schema")),
            Err(reason) => {
                return Err(SourceCommandError::SchemaExecution {
                    path: path.to_string_lossy().into_owned(),
                    root: CONTEXT_SCHEMA.to_owned(),
                    reason,
                });
            }
        }
        Ok((
            Self {
                configuration_path: path,
                configuration_raw,
                schema_raw,
                public_root,
                private_root,
                private_prefix,
                public_identity,
                private_identity,
                uid,
            },
            config,
        ))
    }

    fn physical(&self, reference: &str) -> SourceCommandResult<(PathBuf, bool)> {
        let ref_path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("owner-local logical reference"))?;
        if !reference.starts_with("ToS/") {
            return Err(SourceCommandError::Invalid("owner-local source namespace"));
        }
        if reference.starts_with(OWNER_HOME) {
            let relative = reference
                .strip_prefix(&self.private_prefix)
                .ok_or(SourceCommandError::Denied("owner-local wrong store"))?;
            if relative.is_empty() {
                return Err(SourceCommandError::Invalid("owner-local file absent"));
            }
            Ok((self.private_root.join(ref_path.as_str()), true))
        } else {
            Ok((self.public_root.join(ref_path.as_str()), false))
        }
    }

    fn check_private_parents(&self, reference: &str) -> SourceCommandResult<()> {
        reference
            .strip_prefix(&self.private_prefix)
            .ok_or(SourceCommandError::Denied("owner-local wrong store"))?;
        let mut parent = tos_fd_open::open_absolute_directory(&self.private_root)
            .map_err(|_| SourceCommandError::Conflict("owner-local private root changed"))?;
        if identity(&checked_directory(&parent, self.uid, true)?) != self.private_identity {
            return Err(SourceCommandError::Conflict(
                "owner-local private root changed",
            ));
        }
        let components = reference.split('/').collect::<Vec<_>>();
        for name in &components[..components.len().saturating_sub(1)] {
            parent = tos_fd_open::open_directory_at(&parent, Path::new(name))
                .map_err(|_| SourceCommandError::Denied("owner-local private parent unsafe"))?;
            checked_directory(&parent, self.uid, true)?;
        }
        Ok(())
    }

    pub(crate) fn read(
        &self,
        reference: &str,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let (path, confidential) = self.physical(reference)?;
        if confidential {
            self.check_private_parents(reference)?;
        }
        read_absolute(&path, self.uid, confidential, max, deadline, cancelled)
    }

    /// Read an exact already selected private package. The caller chooses the
    /// operation-specific file set; a directory entry cannot become authority
    /// merely by being present. This method is read-only and issues no replay
    /// or mutation right.
    pub(crate) fn private_package(
        &self,
        directory_ref: &str,
        expected: &[&str],
        state_remaining: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
        if expected.is_empty()
            || expected.len() > MAX_PRIVATE_PACKAGE_FILES
            || expected.iter().any(|name| {
                name.is_empty()
                    || name.contains('/')
                    || name.starts_with('.')
                    || expected.iter().filter(|other| *other == name).count() != 1
            })
        {
            return Err(SourceCommandError::Invalid("owner-local package file set"));
        }
        let (path, private) = self.physical(directory_ref)?;
        if !private {
            return Err(SourceCommandError::Denied(
                "owner-local private package route",
            ));
        }
        self.check_private_parents(directory_ref)?;
        let directory = tos_fd_open::open_absolute_directory(&path)
            .map_err(|_| SourceCommandError::Denied("owner-local package directory"))?;
        let before = checked_directory(&directory, self.uid, true)?;
        let mut seen = vec![false; expected.len()];
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Denied("owner-local package enumeration"))?;
        for entry in entries {
            active(deadline, cancelled)?;
            let entry =
                entry.map_err(|_| SourceCommandError::Denied("owner-local package entry"))?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or(SourceCommandError::Invalid("owner-local package name"))?;
            let index = expected.iter().position(|item| *item == name).ok_or(
                SourceCommandError::Conflict("owner-local package extra member"),
            )?;
            if seen[index] {
                return Err(SourceCommandError::Conflict(
                    "owner-local package duplicate member",
                ));
            }
            seen[index] = true;
        }
        active(deadline, cancelled)?;
        if seen.iter().any(|found| !found) {
            return Err(SourceCommandError::Conflict(
                "owner-local package missing member",
            ));
        }
        let key_state = expected.iter().try_fold(0usize, |total, name| {
            total
                .checked_add(name.len())
                .and_then(|n| n.checked_add(128))
                .ok_or(SourceCommandError::Unsupported(
                    "owner-local package state overflow",
                ))
        })?;
        let mut files = BTreeMap::new();
        let mut remaining = state_remaining
            .checked_sub(key_state)
            .ok_or(SourceCommandError::Unsupported(
                "owner-local package state budget",
            ))?
            .min(MAX_PRIVATE_PACKAGE_BYTES);
        for name in expected {
            active(deadline, cancelled)?;
            let reference = format!("{directory_ref}/{name}");
            let bytes = self.read(&reference, remaining, deadline, cancelled)?;
            remaining -= bytes.len();
            files.insert((*name).to_string(), bytes);
        }
        let current = tos_fd_open::open_absolute_directory(&path)
            .map_err(|_| SourceCommandError::Conflict("owner-local package directory changed"))?;
        if stamp(&before) != stamp(&checked_directory(&directory, self.uid, true)?)
            || stamp(&before) != stamp(&checked_directory(&current, self.uid, true)?)
        {
            return Err(SourceCommandError::Conflict(
                "owner-local package changed during read",
            ));
        }
        Ok(files)
    }

    pub(crate) fn snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Digest256> {
        if read_absolute(
            &self.configuration_path,
            self.uid,
            true,
            MAX_CONTEXT_BYTES,
            deadline,
            cancelled,
        )? != self.configuration_raw
            || read_absolute(
                &self.public_root.join(CONTEXT_SCHEMA),
                self.uid,
                false,
                MAX_CONTEXT_BYTES,
                deadline,
                cancelled,
            )? != self.schema_raw
        {
            return Err(SourceCommandError::Conflict(
                "owner-local configuration or contract changed",
            ));
        }
        let public = tos_fd_open::open_absolute_directory(&self.public_root)
            .map_err(|_| SourceCommandError::Conflict("owner-local public root changed"))?;
        let private = tos_fd_open::open_absolute_directory(&self.private_root)
            .map_err(|_| SourceCommandError::Conflict("owner-local private root changed"))?;
        if identity(&checked_directory(&public, self.uid, false)?) != self.public_identity
            || identity(&checked_directory(&private, self.uid, true)?) != self.private_identity
            || self
                .public_root
                .join(OWNER_HOME.trim_end_matches('/'))
                .symlink_metadata()
                .is_ok()
        {
            return Err(SourceCommandError::Conflict(
                "owner-local context root changed",
            ));
        }
        let tuple = |(dev, ino, uid, mode): (u64, u64, u32, u32)| {
            JsonValue::Array(vec![
                cmd::number(dev),
                cmd::number(ino),
                cmd::number(u64::from(uid)),
                cmd::number(u64::from(mode)),
            ])
        };
        // Same exact context-snapshot fields as the maintained source owner.
        // The digest remains transport evidence, never a read/write grant.
        let binding = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_owner_local_source_context_v1"),
            ),
            (
                "configuration_path",
                cmd::string(
                    self.configuration_path
                        .to_str()
                        .ok_or(SourceCommandError::Invalid("owner-local path UTF-8"))?,
                ),
            ),
            (
                "configuration_sha256",
                cmd::string(&Digest256::of_bytes(&self.configuration_raw).to_hex()),
            ),
            (
                "schema_sha256",
                cmd::string(&Digest256::of_bytes(&self.schema_raw).to_hex()),
            ),
            (
                "roots",
                cmd::object(vec![
                    ("source-contract-root", tuple(self.public_identity)),
                    ("owner-local-root", tuple(self.private_identity)),
                ]),
            ),
            (
                "routing",
                cmd::object(vec![
                    ("private_prefix", cmd::string(&self.private_prefix)),
                    (
                        "public_root",
                        cmd::string(
                            self.public_root
                                .to_str()
                                .ok_or(SourceCommandError::Invalid("owner-local root UTF-8"))?,
                        ),
                    ),
                    (
                        "private_root",
                        cmd::string(
                            self.private_root
                                .to_str()
                                .ok_or(SourceCommandError::Invalid("owner-local root UTF-8"))?,
                        ),
                    ),
                ]),
            ),
        ]);
        Ok(Digest256::of_bytes(&cmd::canonical(&binding)?))
    }
}

// This is byte transport for the concrete Text owner entry. The Sign-native
// resolver still selects its separate Sign route and refuses private members;
// implementing the reader does not issue source access or derivation grants.
impl crate::source_sign_native::SignNativeRead for OwnerTextContext {
    fn read(
        &mut self,
        reference: &str,
        _kind: crate::source_sign_native::NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        OwnerTextContext::read(self, reference, max_bytes, deadline, cancelled)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.snapshot(deadline, cancelled).map(|_| ())
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        self.physical(reference)
            .map(|(_, confidential)| confidential)
    }
}
