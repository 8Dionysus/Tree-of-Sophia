//! Installed software delivery. Explicit data selection cannot supply code.
use crate::software_archive::installed::{SelectedRole, selected_role_proof};
use crate::{AccessError, AccessErrorCode, AccessProfile, DisclosureFence};
use std::{
    fs::File,
    io::Read,
    os::unix::fs::{FileExt, MetadataExt},
    path::{Path, PathBuf},
    sync::Arc,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonString, JsonValue, RelativePath,
    parse_json,
};
use tos_query::AbortProbe;

const MANIFEST_BYTES: usize = 1_048_576;
const PROGRAM: &str = "access/src/tos_access/tos-access";
type Identity = (u64, u64, u64, i64, i64, i64, i64);
fn unavailable(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::Unavailable, message)
}
fn word_language(input: &str, max_input_code_points: usize) -> Result<String, AccessError> {
    tos_foundation::python_strip_unicode16_v1(input, max_input_code_points)
        .map(str::to_lowercase)
        .map_err(|_| {
            AccessError::new(
                AccessErrorCode::InvalidRequest,
                "word-analysis language exceeds request budget",
            )
        })
}
fn identity(file: &File) -> Result<Identity, AccessError> {
    let m = file
        .metadata()
        .map_err(|_| unavailable("installed software metadata unavailable"))?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
fn check_executable(file: &File) -> Result<(), AccessError> {
    let mut header = [0u8; 64];
    file.read_exact_at(&mut header, 0)
        .map_err(|_| unavailable("installed executable header unavailable"))?;
    let mode = file
        .metadata()
        .map_err(|_| unavailable("installed executable mode unavailable"))?
        .mode();
    if mode & 0o111 == 0 || &header[..7] != b"\x7fELF\x02\x01\x01" || header[18..20] != [0x3e, 0] {
        return Err(unavailable("installed executable format or mode differs"));
    }
    Ok(())
}
fn child(root: &File, path: &str) -> Result<File, AccessError> {
    RelativePath::parse(path).map_err(|_| {
        AccessError::new(
            AccessErrorCode::InvalidRequest,
            "invalid static member path",
        )
    })?;
    let mut directory = tos_fd_open::reopen_directory(root)
        .map_err(|_| unavailable("installed software directory unavailable"))?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return tos_fd_open::open_regular_at(&directory, Path::new(part)).map_err(|_| {
                AccessError::new(AccessErrorCode::UnknownExactId, "static member unavailable")
            });
        }
        directory = tos_fd_open::open_directory_at(&directory, Path::new(part)).map_err(|_| {
            AccessError::new(
                AccessErrorCode::UnknownExactId,
                "static directory unavailable",
            )
        })?;
    }
    Err(unavailable("installed member absent"))
}
fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str, AccessError> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| unavailable("installed software manifest field invalid"))
}
fn size(value: &JsonValue) -> Result<usize, AccessError> {
    value
        .object_get("size_bytes")
        .and_then(JsonValue::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| unavailable("installed software member size invalid"))
}
fn hash_file(
    file: &File,
    expected: usize,
    probe: &Arc<dyn AbortProbe>,
) -> Result<Digest256, AccessError> {
    let mut stream = file;
    let mut digest = Digest256Hasher::new();
    let mut buffer = [0u8; 65536];
    let mut count = 0usize;
    loop {
        crate::knowledge::check_abort(probe)?;
        let n = stream
            .read(&mut buffer)
            .map_err(|_| unavailable("installed software read failed"))?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n)
            .filter(|count| *count <= expected)
            .ok_or_else(|| unavailable("installed software size changed"))?;
        digest.update(&buffer[..n]);
    }
    if count != expected {
        return Err(unavailable("installed software size changed"));
    }
    Ok(digest.finalize())
}

/// One server-owned software companion, opened once before listening. Its
/// source is the executable layout; neither cwd nor selected data is consulted.
pub struct SoftwareSite {
    root: File,
    root_path: PathBuf,
    manifest: JsonValue,
    manifest_file: File,
    manifest_identity: Identity,
    executable: File,
    executable_identity: Identity,
    required_assets: Vec<(File, String, Identity)>,
}
const SCHEMA_WORKER: &str = "native/bin/tos-schema-worker";
pub(crate) const WORD_OPERATION: &str = "tos.zarathustra.word_analysis_task";
const WORD_PROVIDER: &str = "scripts/prepare_zarathustra_word_analysis_v1.py";
fn missing_software_member(error: &tos_fd_open::OpenError) -> bool {
    error.code == tos_fd_open::OpenErrorCode::Io
        && error
            .source
            .as_ref()
            .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound)
}
/// Pin the directory containing the first absent path component. Its identity
/// includes change stamps, so provider installation cannot race final flush.
struct AbsentSoftwareMember {
    parent: File,
    prefix: String,
    missing: String,
    original: Identity,
}
impl AbsentSoftwareMember {
    fn verify(&self, root: &File) -> Result<(), AccessError> {
        let mut named = tos_fd_open::reopen_directory(root)
            .map_err(|_| unavailable("software absence root unavailable"))?;
        for part in self.prefix.split('/').filter(|p| !p.is_empty()) {
            named = tos_fd_open::open_directory_at(&named, Path::new(part))
                .map_err(|_| unavailable("software absence directory changed"))?;
        }
        if identity(&named)? != self.original || identity(&self.parent)? != self.original {
            return Err(unavailable("software absence directory changed"));
        }
        match tos_fd_open::open_regular_at(&self.parent, Path::new(&self.missing)) {
            Err(error) if missing_software_member(&error) => Ok(()),
            _ => Err(unavailable("software provider appeared or is unsupported")),
        }
    }
}
impl SoftwareSite {
    fn word_provider_absence(&self) -> Result<AbsentSoftwareMember, AccessError> {
        self.check()?;
        if self
            .members()?
            .iter()
            .any(|item| field(item, "path").ok() == Some(WORD_PROVIDER))
        {
            return Err(unavailable(
                "installed private word-analysis provider requires unsupported native task kernel",
            ));
        }
        let mut parent = tos_fd_open::reopen_directory(&self.root)
            .map_err(|_| unavailable("software absence root unavailable"))?;
        let mut prefix = String::new();
        let mut parts = WORD_PROVIDER.split('/').peekable();
        while let Some(part) = parts.next() {
            let original = identity(&parent)?;
            if parts.peek().is_none() {
                let guard = AbsentSoftwareMember {
                    parent,
                    prefix,
                    missing: part.to_owned(),
                    original,
                };
                guard.verify(&self.root)?;
                return Ok(guard);
            }
            match tos_fd_open::open_directory_at(&parent, Path::new(part)) {
                Ok(next) => {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }
                    prefix.push_str(part);
                    parent = next;
                }
                Err(error) if missing_software_member(&error) => {
                    let guard = AbsentSoftwareMember {
                        parent,
                        prefix,
                        missing: part.to_owned(),
                        original,
                    };
                    guard.verify(&self.root)?;
                    return Ok(guard);
                }
                _ => {
                    return Err(unavailable(
                        "installed private word-analysis provider path unsupported",
                    ));
                }
            }
        }
        Err(unavailable("software provider path invalid"))
    }
    pub(crate) fn word_analysis_negative(
        self: &Arc<Self>,
        arguments: &JsonValue,
        probe: Arc<dyn AbortProbe>,
        profile: AccessProfile,
    ) -> Result<crate::PreparedPacket<'static>, AccessError> {
        crate::knowledge::check_abort(&probe)?;
        let invalid = |message| AccessError::new(AccessErrorCode::InvalidRequest, message);
        let Some(JsonValue::String(query)) = arguments.object_get("query") else {
            return Err(invalid("word-analysis query is required"));
        };
        let whitespace = |unit: u16| {
            char::from_u32(unit as u32).is_some_and(|ch| {
                let mut b = [0; 4];
                tos_foundation::python_strip_unicode16_v1(ch.encode_utf8(&mut b), 1)
                    .is_ok_and(str::is_empty)
            })
        };
        let query = query.units();
        let begin = query
            .iter()
            .position(|u| !whitespace(*u))
            .unwrap_or(query.len());
        let end = query
            .iter()
            .rposition(|u| !whitespace(*u))
            .map_or(begin, |i| i + 1);
        if begin == end || char::decode_utf16(query[begin..end].iter().copied()).count() > 256 {
            return Err(invalid(
                "word-analysis query must contain 1 to 256 characters",
            ));
        }
        let language = word_language(
            match arguments.object_get("language") {
                None => "ru",
                Some(value) => value
                    .as_str()
                    .ok_or_else(|| invalid("word-analysis language must be a string"))?,
            },
            profile.max_request_bytes,
        )?;
        if !matches!(language.as_str(), "de" | "ru" | "en") {
            return Err(invalid("unsupported word-analysis language"));
        }
        if arguments.object_get("rank").is_some_and(|v| !match v {
            JsonValue::Bool(_) => true,
            JsonValue::Number(n) if n.kind == tos_foundation::JsonNumberKind::Int => true,
            JsonValue::Number(n) => n
                .lexeme
                .parse::<f64>()
                .is_ok_and(|v| v.is_finite() && v.fract() == 0.0),
            JsonValue::String(v) => v
                .as_str()
                .is_some_and(|v| crate::mcp_prompts::rank(v).is_ok()),
            _ => false,
        }) {
            return Err(invalid("word-analysis rank must be an integer"));
        }
        if arguments
            .object_get("include_semantic_neighbors")
            .is_some_and(|v| !match v {
                JsonValue::Bool(_) => true,
                JsonValue::Number(n) => n.lexeme.parse::<f64>().is_ok_and(|v| v == 0.0 || v == 1.0),
                JsonValue::String(s) => s.as_str().is_some_and(|s| {
                    matches!(
                        s.to_ascii_lowercase().as_str(),
                        "0" | "1"
                            | "off"
                            | "on"
                            | "f"
                            | "t"
                            | "false"
                            | "true"
                            | "n"
                            | "y"
                            | "no"
                            | "yes"
                    )
                }),
                _ => false,
            })
        {
            return Err(invalid(
                "word-analysis semantic-neighbor flag must be boolean",
            ));
        }
        let absent = self.word_provider_absence()?;
        let body = br#"{"schema":"tos_zarathustra_word_analysis_capability_v1","available":false,"reason":"local source-bound word-analysis provider is not installed","provider_ref":"scripts/prepare_zarathustra_word_analysis_v1.py","publication_posture":"excluded_from_public_bundle","task":null,"authority":{"source_owner":"Tree-of-Sophia","access_plane_is_source":false,"is_semantic_truth":false,"writes_to_tree":false,"reviewed":false,"canon":false}}"#.to_vec();
        if body.len() > profile.max_response_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "word-analysis response byte budget",
            ));
        }
        let mut fence = SoftwareFence {
            site: Arc::clone(self),
            probe,
            absent,
        };
        fence.recheck()?;
        Ok(crate::PreparedPacket {
            body,
            fence: Box::new(fence),
        })
    }
}

/// Retain this selection through the source reader's final disclosure fence.
/// The worker launcher still owns execution and its exact-image checks.
pub struct InstalledSchemaWorker {
    site: Arc<SoftwareSite>,
    file: File,
    original: Identity,
    path: PathBuf,
    sha256: Digest256,
}
impl InstalledSchemaWorker {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn sha256(&self) -> Digest256 {
        self.sha256
    }
    pub fn verify(&self) -> Result<(), AccessError> {
        self.site.check()?;
        let current = tos_fd_open::open_absolute_regular(&self.path, self.original.2)
            .map_err(|_| unavailable("installed schema worker path changed"))?;
        if identity(&self.file)? != self.original
            || identity(&current)? != self.original
            || identity(&child(&self.site.root, SCHEMA_WORKER)?)? != self.original
        {
            return Err(unavailable("installed schema worker changed"));
        }
        Ok(())
    }
}
impl SoftwareSite {
    /// Standalone raw binaries retain existing MCP fixture behavior without
    /// claiming an installed provider. A recognized installed layout must verify
    /// its native role. Integrity failures
    /// never fall back to absent software authority.
    pub fn installed_for_mcp(probe: Arc<dyn AbortProbe>) -> Result<Option<Arc<Self>>, AccessError> {
        let executable =
            std::env::current_exe().map_err(|_| unavailable("installed executable unavailable"))?;
        if executable
            .to_str()
            .and_then(|s| s.strip_suffix(" (deleted)"))
            .is_some_and(|s| Path::new(s).ends_with(PROGRAM))
        {
            return Err(unavailable("installed software image was removed"));
        }
        if !executable.ends_with(PROGRAM) {
            return Ok(None);
        }
        let running = File::open("/proc/self/exe")
            .map_err(|_| unavailable("actual running image unavailable"))?;
        Self::open_inner(&executable, probe, Some(running)).map(Some)
    }
    pub fn installed(probe: Arc<dyn AbortProbe>) -> Result<Arc<Self>, AccessError> {
        let executable =
            std::env::current_exe().map_err(|_| unavailable("installed executable unavailable"))?;
        Self::open_running(&executable, probe)
    }
    /// Native exact-source startup selects code and its release bindings.
    pub(crate) fn installed_source(probe: Arc<dyn AbortProbe>) -> Result<Arc<Self>, AccessError> {
        let executable =
            std::env::current_exe().map_err(|_| unavailable("installed executable unavailable"))?;
        let running = File::open("/proc/self/exe")
            .map_err(|_| unavailable("actual running image unavailable"))?;
        Self::open_inner(&executable, probe, Some(running))
    }
    /// Kernel-bound production image. An explicit path can locate the layout,
    /// but cannot substitute a different ELF for the currently running image.
    pub fn open_running(
        executable: &Path,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<Arc<Self>, AccessError> {
        let running = File::open("/proc/self/exe")
            .map_err(|_| unavailable("actual running image unavailable"))?;
        Self::open_inner(executable, probe, Some(running))
    }
    /// Explicit software integrity fixture/inspection route. This does not
    /// assert that its ELF is running; the server uses installed/open_running.
    pub fn open(executable: &Path, probe: Arc<dyn AbortProbe>) -> Result<Arc<Self>, AccessError> {
        Self::open_inner(executable, probe, None)
    }
    fn open_inner(
        executable: &Path,
        probe: Arc<dyn AbortProbe>,
        running: Option<File>,
    ) -> Result<Arc<Self>, AccessError> {
        crate::knowledge::check_abort(&probe)?;
        let root_path = executable
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .ok_or_else(|| unavailable("installed software layout absent"))?;
        if executable.strip_prefix(root_path).ok() != Some(Path::new(PROGRAM)) {
            return Err(unavailable("installed native software layout differs"));
        }
        let root = tos_fd_open::open_absolute_directory(root_path)
            .map_err(|_| unavailable("installed software root unavailable"))?;
        let manifest_file = child(&root, "software.manifest.json")?;
        let manifest_identity = identity(&manifest_file)?;
        if manifest_identity.2 > MANIFEST_BYTES as u64 {
            return Err(unavailable("installed software manifest exceeds byte cap"));
        }
        let mut raw = Vec::new();
        (&manifest_file)
            .take(MANIFEST_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|_| unavailable("installed software manifest read failed"))?;
        let manifest = parse_json(
            &raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: MANIFEST_BYTES,
                max_depth: 32,
                max_visits: 100000,
                max_integer_digits: 20,
            },
        )
        .map_err(|_| unavailable("installed software manifest invalid"))?
        .into_root();
        if raw.len() > MANIFEST_BYTES
            || identity(&manifest_file)? != manifest_identity
            || field(&manifest, "schema_version")? != "tos_software_bundle_manifest_v1"
            || manifest.object_get("data_included") != Some(&JsonValue::Bool(false))
            || manifest.object_get("source_dirty") != Some(&JsonValue::Bool(false))
        {
            return Err(unavailable("installed software manifest profile differs"));
        }
        // Share the existing installed role grammar and exact release binding;
        // transport startup does not invent another proof/admission system.
        let proof = selected_role_proof(&manifest, SelectedRole::Access)
            .map_err(|_| unavailable("installed native build source binding differs"))?;
        let mut proof_guards = Vec::new();
        let lock = child(&root, "Cargo.lock")?;
        let lock_identity = identity(&lock)?;
        if hash_file(
            &lock,
            usize::try_from(lock_identity.2)
                .map_err(|_| unavailable("installed lock size invalid"))?,
            &probe,
        )?
        .to_hex()
            != field(proof, "lock_sha256")?
            || identity(&lock)? != lock_identity
        {
            return Err(unavailable("installed native lock binding differs"));
        }
        proof_guards.push((lock, "Cargo.lock".to_owned(), lock_identity));
        let pin = child(&root, "rust-toolchain.toml")?;
        let pin_identity = identity(&pin)?;
        let compiled_pin = include_bytes!("../../../../rust-toolchain.toml");
        if pin_identity.2 != compiled_pin.len() as u64
            || hash_file(&pin, compiled_pin.len(), &probe)? != Digest256::of_bytes(compiled_pin)
            || identity(&pin)? != pin_identity
        {
            return Err(unavailable("installed native toolchain pin differs"));
        }
        let channel = include_str!("../../../../rust-toolchain.toml")
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("channel = ")
                    .and_then(|value| value.strip_prefix('"'))
                    .and_then(|value| value.strip_suffix('"'))
            })
            .ok_or_else(|| unavailable("compiled toolchain profile invalid"))?;
        if field(proof, "toolchain")? != channel {
            return Err(unavailable("installed native toolchain receipt differs"));
        }
        proof_guards.push((pin, "rust-toolchain.toml".to_owned(), pin_identity));
        let path_image = child(&root, PROGRAM)?;
        let path_identity = identity(&path_image)?;
        let executable = if let Some(running) = running {
            if identity(&running)? != path_identity {
                return Err(unavailable("installed path differs from running image"));
            }
            running // Retain the kernel image FD through every final flush.
        } else {
            path_image
        };
        let executable_identity = identity(&executable)?;
        check_executable(&executable)?;
        let expected_size = size(proof)?;
        let native_sha = field(proof, "sha256")?.to_owned();
        if executable_identity.2 != expected_size as u64
            || hash_file(&executable, expected_size, &probe)?.to_hex() != field(proof, "sha256")?
            || identity(&executable)? != executable_identity
        {
            return Err(unavailable("installed native executable binding differs"));
        }
        let site = Self {
            root,
            root_path: root_path.to_owned(),
            manifest,
            manifest_file,
            manifest_identity,
            executable,
            executable_identity,
            required_assets: proof_guards,
        };
        let matches = site
            .members()?
            .iter()
            .filter(|item| field(item, "path").ok() == Some(PROGRAM))
            .collect::<Vec<_>>();
        if matches.len() != 1
            || size(matches[0])? != expected_size
            || field(matches[0], "sha256")? != native_sha
        {
            return Err(unavailable(
                "installed native manifest member binding differs",
            ));
        }
        site.check()?;
        crate::knowledge::check_abort(&probe)?;
        Ok(Arc::new(site))
    }
    /// Select only the installed schema worker, using the archive's existing
    /// same-cohort proof rules. The caller supplies its admitted image cap.
    pub fn source_schema_worker(
        self: &Arc<Self>,
        max_image_bytes: usize,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<InstalledSchemaWorker, AccessError> {
        self.check()?;
        crate::knowledge::check_abort(&probe)?;
        let proof = selected_role_proof(&self.manifest, SelectedRole::Command("tos-schema-worker"))
            .map_err(|_| unavailable("installed schema worker cohort differs"))?;
        let declared = size(proof)?;
        if declared > max_image_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "installed schema worker exceeds selected image cap",
            ));
        }
        let mut members = self
            .members()?
            .iter()
            .filter(|item| field(item, "path").ok() == Some(SCHEMA_WORKER));
        let member = members
            .next()
            .ok_or_else(|| unavailable("installed schema worker member absent"))?;
        if members.next().is_some()
            || size(member)? != declared
            || field(member, "sha256")? != field(proof, "sha256")?
        {
            return Err(unavailable(
                "installed schema worker member binding differs",
            ));
        }
        let file = child(&self.root, SCHEMA_WORKER)?;
        let original = identity(&file)?;
        let sha256 = Digest256::from_hex(field(proof, "sha256")?)
            .map_err(|_| unavailable("installed schema worker digest invalid"))?;
        check_executable(&file)?;
        if original.2 != declared as u64 || hash_file(&file, declared, &probe)? != sha256 {
            return Err(unavailable("installed schema worker image binding differs"));
        }
        let worker = InstalledSchemaWorker {
            site: Arc::clone(self),
            file,
            original,
            sha256,
            path: self.root_path.join(SCHEMA_WORKER),
        };
        worker.verify()?;
        Ok(worker)
    }
    fn members(&self) -> Result<&[JsonValue], AccessError> {
        self.manifest
            .object_get("members")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| unavailable("installed software member inventory absent"))
    }
    pub(crate) fn check(&self) -> Result<(), AccessError> {
        if identity(&self.manifest_file)? != self.manifest_identity
            || identity(&self.executable)? != self.executable_identity
            || identity(
                &child(&self.root, "software.manifest.json")
                    .map_err(|_| unavailable("installed software manifest changed"))?,
            )? != self.manifest_identity
            || identity(
                &child(&self.root, PROGRAM)
                    .map_err(|_| unavailable("installed software program changed"))?,
            )? != self.executable_identity
        {
            return Err(unavailable("installed software changed"));
        }
        for (file, path, original) in &self.required_assets {
            if identity(file)? != *original
                || identity(
                    &child(&self.root, path)
                        .map_err(|_| unavailable("installed required software member changed"))?,
                )? != *original
            {
                return Err(unavailable("installed required software member changed"));
            }
        }
        Ok(())
    }
}

struct SoftwareFence {
    site: Arc<SoftwareSite>,
    probe: Arc<dyn AbortProbe>,
    absent: AbsentSoftwareMember,
}
impl DisclosureFence for SoftwareFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        self.site.check()?;
        self.absent.verify(&self.site.root)?;
        crate::knowledge::check_abort(&self.probe)
    }
}
#[cfg(test)]
mod mcp_maintained_tests {
    use super::*;
    #[test]
    fn mcp_maintained_software_absence_pins_named_directory_identity() {
        assert_eq!(word_language("\u{001c}RU\u{001f}", 4).unwrap(), "ru");
        assert!(word_language("\u{001c}RU\u{001f}", 3).is_err());
        let path = std::env::temp_dir().join(format!(
            "tos-mcp-absence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        let root = tos_fd_open::open_absolute_directory(&path).unwrap();
        let parent = tos_fd_open::reopen_directory(&root).unwrap();
        let absent = AbsentSoftwareMember {
            original: identity(&parent).unwrap(),
            parent,
            prefix: String::new(),
            missing: "scripts".into(),
        };
        absent.verify(&root).unwrap();
        std::fs::create_dir(path.join("scripts")).unwrap();
        assert!(
            absent.verify(&root).is_err(),
            "appearing provider directory must refuse"
        );
        let parent = tos_fd_open::open_directory_at(&root, Path::new("scripts")).unwrap();
        let absent = AbsentSoftwareMember {
            original: identity(&parent).unwrap(),
            parent,
            prefix: "scripts".into(),
            missing: "prepare_zarathustra_word_analysis_v1.py".into(),
        };
        absent.verify(&root).unwrap();
        std::fs::write(
            path.join("scripts/prepare_zarathustra_word_analysis_v1.py"),
            b"owned test bytes",
        )
        .unwrap();
        assert!(
            absent.verify(&root).is_err(),
            "appearing provider file must refuse"
        );
        std::fs::remove_file(path.join("scripts/prepare_zarathustra_word_analysis_v1.py")).unwrap();
        let parent = tos_fd_open::open_directory_at(&root, Path::new("scripts")).unwrap();
        let absent = AbsentSoftwareMember {
            original: identity(&parent).unwrap(),
            parent,
            prefix: "scripts".into(),
            missing: "prepare_zarathustra_word_analysis_v1.py".into(),
        };
        absent.verify(&root).unwrap();
        std::fs::rename(path.join("scripts"), path.join("previous")).unwrap();
        std::fs::create_dir(path.join("scripts")).unwrap();
        assert!(
            absent.verify(&root).is_err(),
            "same absent name under a substituted directory must refuse"
        );
    }

    #[test]
    fn cloudflare_origin_launcher_forwards_to_installed_native_tos_on_loopback() {
        use std::{os::unix::fs::PermissionsExt, process::Command};

        let deploy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../access/deploy/cloudflare-tunnel");
        let root = std::env::temp_dir().join(format!(
            "tos-origin-native-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("ToS/derived-exports")).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        for path in [
            "AGENTS.md",
            "ToS/source_home.manifest.json",
            "ToS/derived-exports/tos_corpus_index.min.json",
            "ToS/derived-exports/philosophy_graph_projection.min.json",
        ] {
            std::fs::write(root.join(path), b"fixture").unwrap();
        }
        let binary = root.join("native-tos-stub");
        std::fs::write(
            &binary,
            br#"#!/bin/sh
printf '%s\n' "$@" > "$TOS_ORIGIN_CAPTURE_ARGS"
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&binary, permissions).unwrap();
        let args_path = root.join("native-tos-args.txt");
        let output = Command::new("sh")
            .arg(deploy.join("run-origin.sh"))
            .env("TOS_SITE_ROOT", &root)
            .env_remove("TOS_SOURCE_TEXT_SEARCH_SELECTION")
            .env("TOS_ACCESS_BIN", &binary)
            .env("TOS_SITE_PORT", "5439")
            .env("TOS_ORIGIN_CAPTURE_ARGS", &args_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "native origin launcher failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let args = std::fs::read_to_string(args_path).unwrap();
        let mut expected = vec![
            "--root".to_owned(),
            root.to_string_lossy().into_owned(),
            "serve".to_owned(),
            "127.0.0.1:5439".to_owned(),
        ];
        expected.push(String::new());
        assert_eq!(args, expected.join("\n"));
    }

    #[test]
    fn cloudflare_origin_launcher_forwards_pg_selection_without_site_root() {
        use std::{os::unix::fs::PermissionsExt, process::Command};

        let deploy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../access/deploy/cloudflare-tunnel");
        let root = std::env::temp_dir().join(format!(
            "tos-origin-pg-only-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let binary = root.join("native-tos-stub");
        std::fs::write(
            &binary,
            br#"#!/bin/sh
printf '%s\n' "$@" > "$TOS_ORIGIN_CAPTURE_ARGS"
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&binary, permissions).unwrap();
        let selection = root.join("source-text-selection.json");
        std::fs::write(&selection, b"{}").unwrap();
        let args_path = root.join("native-tos-args.txt");
        let output = Command::new("sh")
            .arg(deploy.join("run-origin.sh"))
            .env_remove("TOS_SITE_ROOT")
            .env("TOS_SOURCE_TEXT_SEARCH_SELECTION", &selection)
            .env("TOS_ACCESS_BIN", &binary)
            .env("TOS_SITE_PORT", "5439")
            .env("TOS_ORIGIN_CAPTURE_ARGS", &args_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "PG-only native origin launcher failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let args = std::fs::read_to_string(args_path).unwrap();
        let expected = vec![
            "--source-text-search-selection".to_owned(),
            selection.to_string_lossy().into_owned(),
            "serve".to_owned(),
            "127.0.0.1:5439".to_owned(),
            String::new(),
        ];
        assert_eq!(args, expected.join("\n"));
    }

    #[test]
    fn cloudflare_origin_launcher_refuses_a_non_tos_root() {
        use std::process::Command;

        let deploy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../access/deploy/cloudflare-tunnel");
        let root = std::env::temp_dir().join(format!(
            "tos-origin-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let output = Command::new("sh")
            .arg(deploy.join("run-origin.sh"))
            .env("TOS_SITE_ROOT", &root)
            .env_remove("TOS_SOURCE_TEXT_SEARCH_SELECTION")
            .env("TOS_ACCESS_BIN", "/bin/true")
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[cfg(test)]
mod native_source_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "tos-native-source-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let fixture = Self(root);
            let mut elf = [0u8; 64];
            elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            elf[18..20].copy_from_slice(&[0x3e, 0]);
            fixture.write(PROGRAM, &elf);
            fixture.write(SCHEMA_WORKER, &elf);
            for path in [PROGRAM, SCHEMA_WORKER] {
                std::fs::set_permissions(
                    fixture.0.join(path),
                    std::fs::Permissions::from_mode(0o755),
                )
                .unwrap();
            }
            let lock = b"version = 4\n";
            let pin = include_bytes!("../../../../rust-toolchain.toml");
            fixture.write("Cargo.lock", lock);
            fixture.write("rust-toolchain.toml", pin);
            let channel = include_str!("../../../../rust-toolchain.toml")
                .lines()
                .find_map(|line| line.trim().strip_prefix("channel = \""))
                .unwrap()
                .trim_end_matches('"');
            let commit = "b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d";
            let mut access = serde_json::json!({
                "schema_version": "tos_native_access_build_v1",
                "sha256": Digest256::of_bytes(&elf).to_hex(), "size_bytes": elf.len(),
                "target": "x86_64-unknown-linux-gnu", "source_commit": commit,
                "source_tree": "5f4276ef03d5253160510727dcd76da27b7c57c2",
                "lock_sha256": Digest256::of_bytes(lock).to_hex(),
                "toolchain": channel, "profile": "debug"
            });
            let mut worker = access.clone();
            worker["schema_version"] = "tos_native_software_command_build_v1".into();
            worker["features"] = serde_json::json!(
                tos_foundation::native_software_roles::features("tos-schema-worker").unwrap()
            );
            // Access v2 and command v1 are allowed to use distinct build profiles.
            access["schema_version"] = "tos_native_access_build_v2".into();
            access["opt_level"] = "1".into();
            let member = |path: &str, raw: &[u8]| {
                serde_json::json!({
                    "path": path, "sha256": Digest256::of_bytes(raw).to_hex(), "size_bytes": raw.len()
                })
            };
            fixture.manifest(&serde_json::json!({
                "schema_version": "tos_software_bundle_manifest_v1", "software_ref": commit,
                "data_included": false, "source_dirty": false, "native_access": access,
                "native_commands": {"tos-schema-worker": worker},
                "members": [member(PROGRAM, &elf), member(SCHEMA_WORKER, &elf),
                            member("Cargo.lock", lock), member("rust-toolchain.toml", pin)]
            }));
            fixture
        }
        fn write(&self, path: &str, raw: &[u8]) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, raw).unwrap();
        }
        fn manifest(&self, manifest: &serde_json::Value) {
            self.write(
                "software.manifest.json",
                &serde_json::to_vec(manifest).unwrap(),
            );
        }
        fn read_manifest(&self) -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(self.0.join("software.manifest.json")).unwrap())
                .unwrap()
        }
        fn open(&self, probe: Arc<dyn AbortProbe>) -> Result<Arc<SoftwareSite>, AccessError> {
            SoftwareSite::open_inner(&self.0.join(PROGRAM), probe, None)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn probe() -> Arc<dyn AbortProbe> {
        AccessProfile::new(65_536, 1_048_576, 65_536)
            .with_query_timeout(Duration::from_secs(30))
            .deadline_probe()
    }

    #[test]
    fn native_source_needs_only_its_exact_executables_and_release_bindings() {
        let fixture = Fixture::new();
        let site = fixture.open(probe()).unwrap();
        let worker = site.source_schema_worker(1024, probe()).unwrap();
        assert_eq!(worker.path(), fixture.0.join(SCHEMA_WORKER));
        worker.verify().unwrap();
        assert!(!fixture.0.join("access/src/tos_access/web_dist").exists());
        worker.verify().unwrap();
        assert!(SoftwareSite::open_running(&fixture.0.join(PROGRAM), probe()).is_err());
    }

    #[test]
    fn native_refuses_code_lock_and_cohort_substitution() {
        {
            for path in [PROGRAM, SCHEMA_WORKER, "Cargo.lock", "rust-toolchain.toml"] {
                let fixture = Fixture::new();
                let site = fixture.open(probe()).unwrap();
                let worker = site.source_schema_worker(1024, probe()).unwrap();
                let target = fixture.0.join(path);
                let mut bytes = std::fs::read(&target).unwrap();
                *bytes.last_mut().unwrap() ^= 1;
                std::fs::write(target, bytes).unwrap();
                assert!(worker.verify().is_err(), "retained {path}");
                let reopened = fixture
                    .open(probe())
                    .and_then(|site| site.source_schema_worker(1024, probe()));
                assert!(reopened.is_err(), "reopened {path}");
            }
            for key in [
                "source_commit",
                "source_tree",
                "lock_sha256",
                "toolchain",
                "target",
                "features",
            ] {
                let fixture = Fixture::new();
                let mut manifest = fixture.read_manifest();
                manifest["native_commands"]["tos-schema-worker"][key] = serde_json::json!("wrong");
                fixture.manifest(&manifest);
                let site = fixture.open(probe()).unwrap();
                assert!(site.source_schema_worker(1024, probe()).is_err(), "{key}");
            }
        }
    }

    #[test]
    fn selected_worker_keeps_its_cap_and_exact_member_closure() {
        let fixture = Fixture::new();
        let site = fixture.open(probe()).unwrap();
        assert_eq!(
            site.source_schema_worker(63, probe()).err().unwrap().code,
            AccessErrorCode::BudgetExceeded
        );
        for duplicate in [false, true] {
            let fixture = Fixture::new();
            let mut manifest = fixture.read_manifest();
            let members = manifest["members"].as_array_mut().unwrap();
            let worker = members
                .iter()
                .position(|member| member["path"] == SCHEMA_WORKER)
                .unwrap();
            let member = members.remove(worker);
            if duplicate {
                members.extend([member.clone(), member]);
            }
            fixture.manifest(&manifest);
            let site = fixture.open(probe()).unwrap();
            assert!(site.source_schema_worker(1024, probe()).is_err());
        }
        let fixture = Fixture::new();
        std::fs::remove_file(fixture.0.join(SCHEMA_WORKER)).unwrap();
        let site = fixture.open(probe()).unwrap();
        assert!(site.source_schema_worker(1024, probe()).is_err());
    }

    #[test]
    fn native_role_mode_replacement_and_operation_abort_remain_fenced() {
        for path in [PROGRAM, SCHEMA_WORKER] {
            let fixture = Fixture::new();
            let site = fixture.open(probe()).unwrap();
            let worker = site.source_schema_worker(1024, probe()).unwrap();
            let target = fixture.0.join(path);
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(worker.verify().is_err());
            assert!(
                fixture
                    .open(probe())
                    .and_then(|site| site.source_schema_worker(1024, probe()))
                    .is_err()
            );
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
            let site = fixture.open(probe()).unwrap();
            let worker = site.source_schema_worker(1024, probe()).unwrap();
            std::fs::rename(&target, target.with_extension("old")).unwrap();
            std::fs::copy(target.with_extension("old"), &target).unwrap();
            assert!(
                worker.verify().is_err(),
                "same-byte inode replacement {path}"
            );
        }
        struct Cancelled;
        impl AbortProbe for Cancelled {
            fn reason(&self) -> Option<tos_query::AbortReason> {
                Some(tos_query::AbortReason::Cancelled)
            }
        }
        let fixture = Fixture::new();
        {
            assert_eq!(
                fixture.open(Arc::new(Cancelled)).err().unwrap().code,
                AccessErrorCode::Cancelled
            );
            let expired = AccessProfile::new(65_536, 1_048_576, 65_536)
                .with_query_timeout(Duration::ZERO)
                .deadline_probe();
            assert_eq!(
                fixture.open(expired).err().unwrap().code,
                AccessErrorCode::DeadlineExceeded
            );
        }
    }
}
