//! Descriptor-held lexical source custody for research stages.
//! Remaining lexical-derivative callers move here with their stages.
pub(crate) use crate::research_support::{ensure, s, sha};
use serde_json::Value;
use std::{fs::File, os::unix::fs::PermissionsExt};
use tos_foundation::{JsonLimits, JsonMode, parse_json};
pub(crate) use tos_source_store::execution::Execution as ResearchExecution;

pub(crate) type Result<T> = std::result::Result<T, String>;
pub(crate) const META_CAP: u64 = 4 * 1024 * 1024;
pub(crate) const PACKET_CAP: usize = 64 * 1024 * 1024;
pub(crate) struct Held {
    reference: String,
    file: File,
    metadata: std::fs::Metadata,
    pub(crate) digest: String,
}
impl Held {
    pub(crate) fn open(ctx: &ResearchExecution, reference: &str, cap: u64) -> Result<Self> {
        let mut file = ctx.source_file(reference, cap)?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        let digest = ctx.hash_file(&mut file, cap)?;
        Ok(Self {
            reference: reference.into(),
            file,
            metadata,
            digest,
        })
    }
    pub(crate) fn verify(&mut self, ctx: &ResearchExecution) -> Result<()> {
        ctx.verify_file_unchanged(&self.file, &self.metadata)?;
        let current = ctx.source_file(&self.reference, self.metadata.len())?;
        ctx.verify_file_unchanged(&current, &self.metadata)?;
        ensure(
            ctx.hash_file(&mut self.file, self.metadata.len())? == self.digest,
            "held source bytes changed",
        )
    }
}
pub(crate) fn canonical(mut value: Value) -> Result<Vec<u8>> {
    value.sort_all_objects();
    let mut raw = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    raw.push(b'\n');
    ensure(raw.len() <= PACKET_CAP, "lexical derivative byte bound")?;
    Ok(raw)
}
pub(crate) fn read_json_bounded(
    ctx: &ResearchExecution,
    reference: &str,
    cap: u64,
    held: &mut Vec<Held>,
) -> Result<Value> {
    ensure(cap <= PACKET_CAP as u64, "lexical JSON input byte bound")?;
    let mut source = Held::open(ctx, reference, cap)?;
    let raw = ctx.read_file(&mut source.file, cap)?;
    let limits = JsonLimits::new(cap as usize, 96, 2_000_000, 4300).map_err(|e| e.to_string())?;
    parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("unsupported lexical JSON representation: {e}"))?;
    ensure(value.is_object(), "lexical derivative object required")?;
    held.push(source);
    Ok(value)
}
pub(crate) fn schemas(
    ctx: &ResearchExecution,
    names: &[&str],
    held: &mut Vec<Held>,
) -> Result<tos_validation::SchemaBackendProbe> {
    let mut resources = Vec::new();
    for name in names {
        let reference = format!("ToS/contracts/{name}.schema.json");
        let mut source = Held::open(ctx, &reference, META_CAP)?;
        resources.push(tos_validation::SchemaResource {
            uri: format!("https://tree-of-sophia.local/{reference}"),
            raw: ctx.read_file(&mut source.file, META_CAP)?,
        });
        held.push(source);
    }
    tos_validation::SchemaBackendProbe::new(
        resources,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("lexical schema preparation: {e:?}"))
}
pub(crate) fn generated_path(reference: &str, generation: &str) -> Result<String> {
    let (stem, suffix) = reference.rsplit_once('.').ok_or("output suffix required")?;
    Ok(format!("{stem}.native-{generation}.{suffix}"))
}
pub(crate) fn valid_generation(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
pub(crate) fn present(
    ctx: &ResearchExecution,
    reference: &str,
    expected: &[u8],
    mode: u32,
) -> Result<bool> {
    match std::fs::symlink_metadata(ctx.root().join(reference)) {
        Ok(_) => {
            let mut held = Held::open(ctx, reference, PACKET_CAP as u64)?;
            ensure(
                held.metadata.permissions().mode() & 0o777 == mode,
                "derivative output mode drift",
            )?;
            ensure(
                ctx.read_file(&mut held.file, PACKET_CAP as u64)? == expected,
                "derivative output bytes conflict",
            )?;
            held.verify(ctx)?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
