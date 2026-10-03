//! Installed software delivery. Explicit data selection cannot supply code.
use crate::{AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence};
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
pub const MAX_STATIC_BYTES: usize = 16 * 1024 * 1024;
const PROGRAM: &str = "access/src/tos_access/tos-access";
const STATIC_PREFIX: &str = "access/src/tos_access/web_dist/";
const INDEX: &str = "<!doctype html>\n<html lang=\"ru\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Древо Софии</title><link rel=\"stylesheet\" href=\"/static/assets/tos-graph.css\"></head><body><div id=\"app\"></div><script nonce=\"__CSP_NONCE__\">window.__TOS_GRAPH_BOOT__=__BOOT__;</script><script type=\"module\" src=\"/static/assets/tos-graph.js\"></script></body></html>";
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
pub(crate) const WORD_TOOL: &str = "tos_zarathustra_prepare_word_analysis";
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
            file: None,
            probe,
            holds: vec![],
            absent: Some(absent),
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
    pub(crate) fn software(&self) -> Arc<SoftwareSite> {
        Arc::clone(&self.site)
    }
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
    /// claiming an installed provider. A recognized installed layout must verify;
    /// integrity failures never fall back to absent software authority.
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
        Self::open_running(&executable, probe).map(Some)
    }
    pub fn installed(probe: Arc<dyn AbortProbe>) -> Result<Arc<Self>, AccessError> {
        let executable =
            std::env::current_exe().map_err(|_| unavailable("installed executable unavailable"))?;
        Self::open_running(&executable, probe)
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
        let proof = manifest
            .object_get("native_access")
            .ok_or_else(|| unavailable("installed native build proof absent"))?;
        let keys = [
            "schema_version",
            "sha256",
            "size_bytes",
            "target",
            "source_commit",
            "source_tree",
            "lock_sha256",
            "toolchain",
            "profile",
        ];
        let fields = proof
            .as_object()
            .ok_or_else(|| unavailable("installed native proof invalid"))?;
        if fields.len() != keys.len()
            || fields
                .iter()
                .any(|(key, _)| !key.as_str().is_some_and(|key| keys.contains(&key)))
            || !matches!(field(proof, "profile")?, "debug" | "release")
        {
            return Err(unavailable("installed native proof fields differ"));
        }
        if field(proof, "schema_version")? != "tos_native_access_build_v1"
            || field(proof, "target")? != "x86_64-unknown-linux-gnu"
            || field(proof, "source_commit")? != field(&manifest, "software_ref")?
        {
            return Err(unavailable("installed native build source binding differs"));
        }
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
        let expected_size = size(proof)?;
        let native_sha = field(proof, "sha256")?.to_owned();
        if executable_identity.2 != expected_size as u64
            || hash_file(&executable, expected_size, &probe)?.to_hex() != field(proof, "sha256")?
            || identity(&executable)? != executable_identity
        {
            return Err(unavailable("installed native executable binding differs"));
        }
        let mut site = Self {
            root,
            root_path: root_path.to_owned(),
            manifest,
            manifest_file,
            manifest_identity,
            executable,
            executable_identity,
            required_assets: proof_guards,
        };
        // Assembly must declare finite size for every installed static member;
        // no count from today's fixture is treated as a production rule.
        let mut total = 0u64;
        for item in site.members()? {
            if field(item, "path")?.starts_with(STATIC_PREFIX) {
                let bytes = size(item)?;
                if bytes > MAX_STATIC_BYTES {
                    return Err(unavailable("installed static member exceeds software cap"));
                }
                total = total
                    .checked_add(bytes as u64)
                    .ok_or_else(|| unavailable("installed static aggregate size overflows"))?;
            }
        }
        // Only the manifest integer sum is representable. This is not an
        // aggregate resource quota; the actual body cap is per request.
        let _encoded_static_size_sum = total;
        for required in [
            PROGRAM,
            "access/src/tos_access/web_dist/assets/tos-graph.js",
            "access/src/tos_access/web_dist/assets/tos-graph.css",
        ] {
            let matches = site
                .members()?
                .iter()
                .filter(|item| field(item, "path").ok() == Some(required))
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(unavailable("installed site member closure differs"));
            }
            let item = matches[0];
            if required == PROGRAM {
                if size(item)? != expected_size || field(item, "sha256")? != native_sha {
                    return Err(unavailable(
                        "installed native manifest member binding differs",
                    ));
                }
            } else {
                let file = child(&site.root, required)?;
                let before = identity(&file)?;
                let declared = size(item)?;
                if declared == 0
                    || before.2 != declared as u64
                    || hash_file(&file, declared, &probe)?.to_hex() != field(item, "sha256")?
                    || identity(&file)? != before
                {
                    return Err(unavailable("installed site asset integrity differs"));
                }
                site.required_assets
                    .push((file, required.to_owned(), before));
            }
        }
        site.check()?;
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
        let proof = self
            .manifest
            .object_get("native_commands")
            .and_then(|roles| roles.object_get("tos-schema-worker"))
            .ok_or_else(|| unavailable("installed schema worker proof absent"))?;
        let access = self
            .manifest
            .object_get("native_access")
            .ok_or_else(|| unavailable("installed native build proof absent"))?;
        crate::software_archive::command_proof(proof, access)
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
        let mut header = [0u8; 64];
        file.read_exact_at(&mut header, 0)
            .map_err(|_| unavailable("installed schema worker header unavailable"))?;
        if &header[..7] != b"\x7fELF\x02\x01\x01"
            || header[18..20] != [0x3e, 0]
            || original.2 != declared as u64
            || hash_file(&file, declared, &probe)? != sha256
        {
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
    fn check(&self) -> Result<(), AccessError> {
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
                return Err(unavailable("installed required site asset changed"));
            }
        }
        Ok(())
    }
    pub(crate) fn asset(
        self: &Arc<Self>,
        relative: &str,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<crate::PreparedPacket<'static>, AccessError> {
        RelativePath::parse(relative).map_err(|_| {
            AccessError::new(AccessErrorCode::InvalidRequest, "invalid static path")
        })?;
        let path = format!("{STATIC_PREFIX}{relative}");
        let matches = self
            .members()?
            .iter()
            .filter(|item| field(item, "path").ok() == Some(path.as_str()))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(AccessError::new(
                AccessErrorCode::UnknownExactId,
                "static member is not declared software",
            ));
        }
        let declared = size(matches[0])?;
        if declared > MAX_STATIC_BYTES {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "static software byte cap exceeded",
            ));
        }
        self.check()?;
        let file = child(&self.root, &path)?;
        let original = identity(&file)?;
        if original.2 != declared as u64 {
            return Err(unavailable("static software size differs"));
        }
        crate::knowledge::check_abort(&probe)?;
        let mut body = Vec::new();
        body.try_reserve_exact(declared).map_err(|_| {
            AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "static body allocation unavailable",
            )
        })?;
        body.resize(declared, 0);
        let mut stream = &file;
        for part in body.chunks_mut(65536) {
            crate::knowledge::check_abort(&probe)?;
            stream
                .read_exact(part)
                .map_err(|_| unavailable("static software read failed"))?;
        }
        let mut tail = [0u8; 1];
        if stream
            .read(&mut tail)
            .map_err(|_| unavailable("static software read failed"))?
            != 0
        {
            return Err(unavailable("static software size changed"));
        }
        if body.len() != declared
            || identity(&file)? != original
            || Digest256::of_bytes(&body).to_hex() != field(matches[0], "sha256")?
        {
            return Err(unavailable("static software integrity differs"));
        }
        Ok(crate::PreparedPacket {
            body,
            fence: Box::new(SoftwareFence {
                site: Arc::clone(self),
                file: Some((file, path, original)),
                probe,
                holds: vec![],
                absent: None,
            }),
        })
    }
}
struct SoftwareFence {
    site: Arc<SoftwareSite>,
    file: Option<(File, String, Identity)>,
    probe: Arc<dyn AbortProbe>,
    holds: Vec<Box<dyn DisclosureFence>>,
    absent: Option<AbsentSoftwareMember>,
}
impl DisclosureFence for SoftwareFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        self.site.check()?;
        if let Some(absent) = &self.absent {
            absent.verify(&self.site.root)?;
        }
        if let Some((file, path, original)) = &self.file {
            if identity(file)? != *original
                || identity(&child(&self.site.root, path)?)? != *original
            {
                return Err(unavailable("static software changed before delivery"));
            }
        }
        for hold in &mut self.holds {
            hold.recheck()?;
        }
        crate::knowledge::check_abort(&self.probe)
    }
}
pub(crate) fn mime(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "html" | "htm" => "text/html",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/vnd.microsoft.icon",
        "txt" => "text/plain",
        "jpg" | "jpeg" => "image/jpeg",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

impl SoftwareSite {
    pub(crate) fn shell(
        self: &Arc<Self>,
        executor: &dyn AccessExecutor,
        profile: AccessProfile,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<(crate::PreparedPacket<'static>, String), AccessError> {
        crate::knowledge::check_abort(&probe)?;
        self.check()?;
        let mut holds = Vec::new();
        let mut first = |request, list: &str| -> Result<(String, bool), AccessError> {
            match crate::common::checked_execute(Arc::clone(&probe), |probe| {
                executor.knowledge(request, probe)
            }) {
                Ok(packet) => {
                    let document = parse_json(
                        &packet.body,
                        JsonMode::PublishedStrict,
                        JsonLimits {
                            max_bytes: profile.max_response_bytes,
                            max_depth: 64,
                            max_visits: 300_000,
                            max_integer_digits: 4_300,
                        },
                    )
                    .map_err(|_| {
                        AccessError::new(
                            AccessErrorCode::CorruptSelectedCarrier,
                            "boot metadata packet invalid",
                        )
                    })?;
                    let rows = document
                        .root()
                        .object_get(list)
                        .and_then(JsonValue::as_array)
                        .ok_or_else(|| {
                            AccessError::new(
                                AccessErrorCode::CorruptSelectedCarrier,
                                "boot metadata projection absent",
                            )
                        })?;
                    let id = rows
                        .iter()
                        .find_map(|row| {
                            row.object_get("view_id")
                                .and_then(JsonValue::as_str)
                                .filter(|id| !id.is_empty())
                        })
                        .unwrap_or("")
                        .to_owned();
                    holds.push(packet.fence);
                    Ok((id, true))
                }
                Err(error)
                    if matches!(
                        error.code,
                        AccessErrorCode::Unavailable | AccessErrorCode::PolicyDenied
                    ) =>
                {
                    Ok((String::new(), false))
                }
                Err(error) => Err(error),
            }
        };
        let (corpus, corpus_available) =
            first(crate::KnowledgeRequest::CorpusViewIds, "graph_views")?;
        let (philosophy, philosophy_available) =
            first(crate::KnowledgeRequest::PhilosophyViewIds, "views")?;
        let string = |s: &str| JsonValue::String(JsonString::from_utf8(s));
        let object = |fields: Vec<(&str, JsonValue)>| {
            JsonValue::Object(
                fields
                    .into_iter()
                    .map(|(k, v)| (JsonString::from_utf8(k), v))
                    .collect(),
            )
        };
        let boot = object(vec![
            ("service", string("tree-of-sophia-access")),
            ("default_view", string(&corpus)),
            ("default_philosophy_view", string(&philosophy)),
            ("write_enabled", JsonValue::Bool(false)),
            ("projection_mode", string("json")),
            (
                "neo4j",
                object(vec![
                    ("configured", JsonValue::Bool(false)),
                    ("ready", JsonValue::Bool(false)),
                    ("note", string("Standalone JSON backend")),
                ]),
            ),
            (
                "capabilities",
                object(vec![
                    ("corpus", JsonValue::Bool(corpus_available)),
                    ("philosophy", JsonValue::Bool(philosophy_available)),
                ]),
            ),
        ]);
        let raw = tos_foundation::emit_value_preserved_json(&boot, profile.json_limits())
            .map_err(|_| unavailable("boot packet exceeds byte budget"))?;
        // JSON inside an executable script needs HTML-safe separators, even
        // though the selected values are only metadata IDs.
        let boot = String::from_utf8(raw)
            .map_err(|_| unavailable("boot JSON is not UTF-8"))?
            .replace('&', "\\u0026")
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029");
        let mut entropy = [0u8; 18];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut entropy))
            .map_err(|_| unavailable("site nonce unavailable"))?;
        let nonce = entropy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let body = INDEX
            .replace("__CSP_NONCE__", &nonce)
            .replace("__BOOT__", &boot)
            .into_bytes();
        if body.len() > profile.max_response_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "site shell byte budget exceeded",
            ));
        }
        Ok((
            crate::PreparedPacket {
                body,
                fence: Box::new(SoftwareFence {
                    site: Arc::clone(self),
                    file: None,
                    probe,
                    holds,
                    absent: None,
                }),
            },
            nonce,
        ))
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
}
