//! Installed lexical maintainer command over an exact held source cut.
//! All generated artifacts remain in an explicitly fresh private candidate.
use serde_json::{Value, json};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_compiler::zarathustra_lexical::{self as lexical, LexicalCapture, LexicalLimits};
use tos_compiler::zarathustra_lexical_schema::{LexicalSchemaExecutor, LexicalSchemaLimits};
use tos_foundation::{Digest256, JsonLimits, JsonMode, RelativePath, SourceRevision, parse_json};
use tos_source_store::{CorpusReader, CutReadLimits, ReadLimits};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    max_seconds: u64,
    max_manifest_bytes: usize,
    max_manifest_entries: usize,
    max_revisions: usize,
    max_cut_members: u64,
    max_cut_bytes: u64,
    max_member_bytes: u64,
    lexical: LexicalLimits,
    schema: LexicalSchemaLimits,
}
fn raw(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let mut f = tos_fd_open::open_absolute_regular(path, max as u64).map_err(|e| e.to_string())?;
    let mut b = vec![];
    Read::by_ref(&mut f)
        .take(max as u64 + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() > max {
        return Err("lexical command file budget".into());
    }
    Ok(b)
}
fn path(options: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(options.get(key).ok_or_else(|| format!("missing {key}"))?);
    if !p.is_absolute() {
        return Err(format!("absolute path required: {key}"));
    }
    Ok(p)
}
fn write_new(path: &Path, b: &[u8]) -> Result<(), String> {
    tos_fd_open::open_absolute_directory(path.parent().ok_or("candidate parent")?)
        .map_err(|e| e.to_string())?;
    let mut f = fs::OpenOptions::new()
        .mode(0o600)
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("fresh private output required: {e}"))?;
    f.write_all(b).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}
fn cut_eof(
    cut: &tos_source_store::CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value, String> {
    let mut stream = cut
        .stream(cut.current().revision())
        .map_err(|e| e.to_string())?;
    let expected = stream.expectation();
    while stream
        .next_member(deadline, cancelled)
        .map_err(|e| e.to_string())?
        .is_some()
    {}
    if stream.coverage() != Some(expected) {
        return Err("lexical source cut EOF mismatch".into());
    }
    Ok(json!({"members":expected.count,"membership_sha256":expected.digest.to_hex()}))
}
fn run(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mode = args.get(1).ok_or("lexical-index needs build or validate")?;
    if !["build", "validate", "validate-legacy"].contains(&mode.as_str()) {
        return Err("unsupported lexical-index action".into());
    }
    let mut options = BTreeMap::new();
    let mut it = args.iter().skip(2);
    while let Some(k) = it.next() {
        if ![
            "--source-store",
            "--derived-input-root",
            "--source-revision",
            "--software-root",
            "--payload-source-root",
            "--candidate-root",
            "--projection",
            "--local-output-root",
            "--schema-worker",
            "--schema-worker-sha256",
            "--limits",
            "--event-time",
        ]
        .contains(&k.as_str())
        {
            return Err(format!("unknown lexical-index option: {k}"));
        }
        let v = it.next().ok_or("lexical-index option needs value")?;
        if options.insert(k.clone(), v.clone()).is_some() {
            return Err(format!("duplicate lexical-index option: {k}"));
        }
    }
    let limits_raw = raw(&path(&options, "--limits")?, 65536)?;
    let json_limits = JsonLimits::new(65536, 16, 10000, 4300).map_err(|e| e.to_string())?;
    parse_json(&limits_raw, JsonMode::PublishedStrict, json_limits).map_err(|e| e.to_string())?;
    let l: Limits =
        serde_json::from_slice(&limits_raw).map_err(|e| format!("lexical limits: {e}"))?;
    if l.max_seconds == 0 || l.max_seconds > 3600 {
        return Err("finite lexical wall budget required".into());
    }
    let deadline = Instant::now() + Duration::from_secs(l.max_seconds);
    let cancelled = AtomicBool::new(false);
    let root = path(&options, "--source-store")?;
    let software = path(&options, "--software-root")?;
    let revision = SourceRevision(
        Digest256::from_hex(
            options
                .get("--source-revision")
                .ok_or("missing source revision")?,
        )
        .map_err(|e| e.to_string())?,
    );
    let worker = path(&options, "--schema-worker")?;
    let worker_sha = Digest256::from_hex(
        options
            .get("--schema-worker-sha256")
            .ok_or("missing worker digest")?,
    )
    .map_err(|e| e.to_string())?;
    // Snapshot lookup indexes retain several path/identity/dependency maps.
    // Each JSON input byte bounds a slot; 512 bytes/byte conservatively covers
    // the pinned BTree node slots, cloned keys/strings and Vec capacities.
    // No lexical DOM has been built yet. This is logical live setup, not RSS.
    let setup_state_bytes = l
        .max_manifest_bytes
        .checked_mul(l.max_revisions)
        .and_then(|n| n.checked_mul(512))
        .and_then(|n| n.checked_add(65536 * 4))
        .and_then(|n| n.checked_add(root.as_os_str().as_encoded_bytes().len().checked_mul(16)?))
        .and_then(|n| {
            n.checked_add(
                software
                    .as_os_str()
                    .as_encoded_bytes()
                    .len()
                    .checked_mul(16)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                worker
                    .as_os_str()
                    .as_encoded_bytes()
                    .len()
                    .checked_mul(16)?,
            )
        })
        .ok_or("lexical setup state overflow")?;
    if setup_state_bytes > l.schema.max_preparation_bytes {
        return Err("lexical cut setup exceeds preparation declaration".into());
    }
    let reader = CorpusReader::open_existing(
        &root,
        ReadLimits {
            max_manifest_bytes: l.max_manifest_bytes,
            max_manifest_entries: l.max_manifest_entries,
            max_selected_object_bytes: l.max_member_bytes,
            json: JsonLimits::new(
                l.max_manifest_bytes,
                96,
                l.max_manifest_entries.saturating_mul(64),
                4300,
            )
            .map_err(|e| e.to_string())?,
        },
    )
    .map_err(|e| e.to_string())?;
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: l.max_revisions,
                max_members: l.max_cut_members,
                max_total_bytes: l.max_cut_bytes,
                max_member_bytes: l.max_member_bytes,
            },
            deadline,
            &cancelled,
        )
        .map_err(|e| e.to_string())?;
    let mut schema = LexicalSchemaExecutor::new(
        &cut,
        &worker,
        worker_sha,
        l.schema,
        setup_state_bytes,
        deadline,
        &cancelled,
    )?;
    let mut capture = LexicalCapture::from_cut(&software, &cut, l.lexical, deadline, &cancelled)?;
    if options.contains_key("--derived-input-root") {
        capture.set_derived_root(&path(&options, "--derived-input-root")?)?;
    }
    let plan = capture.json(lexical::PLAN_REF)?;
    let tracked = plan["tracked_projection"]["relative_path"]
        .as_str()
        .ok_or("tracked projection path")?;
    let local = plan["local_projection"]["relative_path"]
        .as_str()
        .ok_or("local projection path")?;
    RelativePath::parse(tracked).map_err(|e| e.to_string())?;
    RelativePath::parse(local).map_err(|e| e.to_string())?;
    let report: Value = if mode == "build" {
        let output = path(&options, "--candidate-root")?;
        tos_fd_open::open_absolute_directory(output.parent().ok_or("candidate root parent")?)
            .map_err(|e| e.to_string())?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&output)
            .map_err(|e| format!("fresh candidate root required: {e}"))?;
        let projection = output.join(tracked);
        let database = output.join(local);
        fs::create_dir_all(database.parent().ok_or("database parent")?)
            .map_err(|e| e.to_string())?;
        fs::create_dir_all(projection.parent().ok_or("projection parent")?)
            .map_err(|e| e.to_string())?;
        let result = lexical::build_from_cut(
            &cut,
            &software,
            &path(&options, "--payload-source-root")?,
            lexical::PLAN_REF,
            &database,
            &mut schema,
            l.lexical,
            deadline,
            &cancelled,
        )?;
        let provenance = lexical::candidate_provenance(
            &mut capture,
            &result,
            options
                .get("--event-time")
                .ok_or("build requires --event-time")?,
            &mut schema,
            l.lexical,
        )?;
        write_new(&projection, &result.projection_bytes)?;
        write_new(&output.join("native-lexical-provenance.jsonl"), &provenance)?;
        schema.finish()?;
        let source_eof = cut_eof(&cut, deadline, &cancelled)?;
        capture.revalidate()?;
        lexical::revalidate_build_inputs(
            &result,
            &software,
            &path(&options, "--payload-source-root")?,
            l.lexical,
            deadline,
            &cancelled,
        )?;
        let receipt = json!({"schema":"tos_native_lexical_candidate_receipt_v1","status":"built-private-candidate","source_eof":source_eof,"source_revision":revision.0.to_hex(),"projection_sha256":Digest256::of_bytes(&result.projection_bytes).to_hex(),"local_database_sha256":result.projection["local_projection_receipt"]["database_sha256"],"source_digests":result.source_digests,"payload_digests":result.payload_digests,"schema_execution":schema.receipt()?,"summary":result.projection["summary"],"authority_boundary":lexical::AUTHORITY});
        write_new(
            &output.join("native-lexical-build.json"),
            &serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
        )?;
        receipt
    } else {
        let (projection, local_root, provenance) = if mode == "validate" {
            let output = path(&options, "--candidate-root")?;
            (
                output.join(tracked),
                Some(output.clone()),
                Some(raw(
                    &output.join("native-lexical-provenance.jsonl"),
                    l.lexical.max_file_bytes,
                )?),
            )
        } else {
            (
                path(&options, "--projection")?,
                options
                    .get("--local-output-root")
                    .map(|_| path(&options, "--local-output-root"))
                    .transpose()?,
                None,
            )
        };
        let report = tos_compiler::zarathustra_lexical_validate::validate_with_capture(
            &software,
            &projection,
            local_root.as_deref(),
            provenance.as_deref(),
            &mut capture,
            &mut schema,
            deadline,
            &cancelled,
        )?;
        schema.finish()?;
        let source_eof = cut_eof(&cut, deadline, &cancelled)?;
        capture.revalidate()?;
        json!({"source_eof":source_eof,"schema":"tos_native_lexical_validation_receipt_v1","status":if mode=="validate-legacy"{"validated-legacy-observation"}else{"validated-private-candidate"},"source_revision":revision.0.to_hex(),"validation":report,"schema_execution":schema.receipt()?,"authority_boundary":lexical::AUTHORITY})
    };
    writeln!(stdout, "{report}").map_err(|e| e.to_string())?;
    Ok(())
}
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|s| s != "lexical-index") {
        return None;
    }
    Some(match run(args, stdout) {
        Ok(()) => 0,
        Err(e) => {
            let message: String = e.chars().take(1024).collect();
            let _ = writeln!(
                stderr,
                "{}",
                json!({"schema":"tos_native_lexical_command_error_v1","status":"refused","message":message,"authority_boundary":lexical::AUTHORITY})
            );
            1
        }
    })
}
