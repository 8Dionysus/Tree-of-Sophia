//! Explicit, bounded private lexical derivatives. Source strings stay in the
//! selected local output; the receipt carries only fixity and aggregate counts.
pub mod morphology_result;
pub mod recurrence;
pub mod usage_context;
use crate::{
    jenseits_numbered_structure::Held,
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, s, sha},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, os::unix::fs::PermissionsExt, path::Path};

type Result<T> = std::result::Result<T, String>;
const META_CAP: u64 = 4 * 1024 * 1024;
const PACKET_CAP: usize = 64 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/lexical_derivatives.rs";
pub const MORPHOLOGY_PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-evaluation-plan.v1.json";
const RETAINED_RECEIPT_SHA: &str =
    "b37d789ae4e9439565acc8bc9fb91028cee7943f8ff99a515a75e85a9ffa6d83";
const ROW_FIELDS: [&str; 6] = [
    "schema_version",
    "form_key",
    "exact_form",
    "exact_form_sha256",
    "normalized_form_sha256",
    "occurrence_count",
];

fn canonical(mut value: Value) -> Result<Vec<u8>> {
    value.sort_all_objects();
    let mut raw = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    raw.push(b'\n');
    ensure(raw.len() <= PACKET_CAP, "lexical derivative byte bound")?;
    Ok(raw)
}
fn read_json(ctx: &ResearchExecution, reference: &str, held: &mut Vec<Held>) -> Result<Value> {
    read_json_bounded(ctx, reference, META_CAP, held)
}
fn read_json_bounded(
    ctx: &ResearchExecution,
    reference: &str,
    cap: u64,
    held: &mut Vec<Held>,
) -> Result<Value> {
    ensure(cap <= PACKET_CAP as u64, "lexical JSON input byte bound")?;
    let mut source = Held::open(ctx, reference, cap)?;
    let raw = ctx.read_file(&mut source.file, cap)?;
    let value = crate::zarathustra_lexical::parse(&raw, cap as usize)?;
    ensure(value.is_object(), "lexical derivative object required")?;
    held.push(source);
    Ok(value)
}
fn schemas(
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
fn digest_bound(
    ctx: &ResearchExecution,
    reference: &str,
    digest: &str,
    held: &mut Vec<Held>,
) -> Result<()> {
    // The retained hash-only lexical projection is 12.3 MiB. It is streamed
    // for fixity here, without materializing its full JSON graph.
    let source = Held::open(ctx, reference, PACKET_CAP as u64)?;
    ensure(
        source.digest == digest,
        &format!("source digest drift: {reference}"),
    )?;
    held.push(source);
    Ok(())
}
fn generated_path(reference: &str, generation: &str) -> Result<String> {
    let (stem, suffix) = reference.rsplit_once('.').ok_or("output suffix required")?;
    Ok(format!("{stem}.native-{generation}.{suffix}"))
}
fn valid_generation(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn validate(schemas: &tos_validation::SchemaBackendProbe, name: &str, value: &Value) -> Result<()> {
    ensure(
        schemas
            .is_valid_raw(
                &format!("https://tree-of-sophia.local/ToS/contracts/{name}.schema.json"),
                &canonical(value.clone())?,
            )
            .map_err(|e| format!("schema validation: {e:?}"))?,
        &format!("{name} schema refused"),
    )
}

#[derive(Default)]
struct Census {
    bytes: Vec<u8>,
    normalized: BTreeSet<String>,
    previous: Option<(String, String)>,
    rows: u64,
    tokens: u64,
    singletons: u64,
    joiners: u64,
    max_chars: usize,
}
impl Census {
    fn push(
        &mut self,
        ctx: &ResearchExecution,
        key: String,
        exact: String,
        digest: String,
        normalized: String,
        count: u64,
    ) -> Result<()> {
        ctx.tick(1 + exact.len() as u64)?;
        ensure(
            !exact.is_empty() && exact.len() <= 65536,
            "exact form byte bound",
        )?;
        ensure(
            sha(exact.as_bytes()) == digest,
            "exact form digest mismatch",
        )?;
        ensure(
            key == format!("lexical-form:sha256:{digest}"),
            "form key digest mismatch",
        )?;
        ensure(
            normalized.len() == 64
                && normalized
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "normalized form digest syntax",
        )?;
        ensure(count > 0 && self.rows < 1_000_000, "form count bound")?;
        let order = (digest.clone(), exact.clone());
        ensure(
            self.previous.as_ref().is_none_or(|prior| prior < &order),
            "strict frozen form order",
        )?;
        let row = canonical(json!({
            "schema_version":"tos_morphology_input_row_v1", "form_key":key,
            "exact_form":exact, "exact_form_sha256":digest,
            "normalized_form_sha256":normalized, "occurrence_count":count,
        }))?;
        ensure(
            row.len() <= PACKET_CAP - self.bytes.len(),
            "morphology packet byte bound",
        )?;
        self.tokens = self
            .tokens
            .checked_add(count)
            .ok_or("occurrence count overflow")?;
        self.bytes.extend_from_slice(&row);
        self.normalized.insert(normalized);
        self.rows += 1;
        self.singletons += u64::from(count == 1);
        self.joiners += u64::from(exact.chars().any(|c| "-'’‐‑".contains(c)));
        self.max_chars = self.max_chars.max(exact.chars().count());
        self.previous = Some(order);
        Ok(())
    }
    fn summary(&self) -> Result<Value> {
        ensure(self.rows > 0, "empty morphology census")?;
        Ok(json!({
            "exact_form_row_count":self.rows, "normalized_form_hash_count":self.normalized.len(),
            "token_occurrence_count":self.tokens, "singleton_form_count":self.singletons,
            "joiner_form_count":self.joiners, "maximum_codepoint_length":self.max_chars,
        }))
    }
}

/// Path-based existence is only a preflight hint. Existing bytes are opened
/// through the selected descriptor, with no symlinks, and reverified there.
/// Missing targets still pass through the exclusive descriptor-only writer.
fn present(ctx: &ResearchExecution, reference: &str, expected: &[u8], mode: u32) -> Result<bool> {
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

pub struct Options<'a> {
    pub build: bool,
    pub input_root: &'a Path,
    pub output_root: &'a Path,
    pub plan: &'a str,
    pub generation: Option<&'a str>,
    /// Relative to the selected output root for native generations. Retained
    /// checks always use the exact receipt declared by their frozen plan.
    pub receipt: Option<&'a str>,
}
pub fn morphology_input(ctx: &ResearchExecution, options: Options<'_>) -> Result<Value> {
    ensure(
        !options.build || options.generation.is_some(),
        "build requires a new generation",
    )?;
    if let Some(g) = options.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    ensure(
        options.generation.is_some() || options.receipt.is_none(),
        "retained receipt cannot be redirected",
    )?;
    ensure(
        options.input_root.is_absolute() && options.output_root.is_absolute(),
        "absolute local roots required",
    )?;
    if options.build {
        ensure(
            !options.output_root.starts_with(ctx.root())
                && !ctx.root().starts_with(options.output_root)
                && !options.output_root.starts_with(options.input_root)
                && !options.input_root.starts_with(options.output_root),
            "new generation output must be separate from source and input roots",
        )?;
    }
    let mut held = Vec::new();
    let schemas = schemas(
        ctx,
        &["morphology-evaluation-plan", "morphology-input-receipt"],
        &mut held,
    )?;
    let plan = read_json(ctx, options.plan, &mut held)?;
    let plan_digest = held.last().unwrap().digest.clone();
    validate(&schemas, "morphology-evaluation-plan", &plan)?;
    let source = &plan["source_lexical_index"];
    for (key, digest) in [
        ("index_plan_ref", "index_plan_sha256"),
        ("tracked_projection_ref", "tracked_projection_sha256"),
    ] {
        digest_bound(ctx, s(&source[key])?, s(&source[digest])?, &mut held)?;
    }
    let input = ctx.select_directory(options.input_root)?;
    let mut database = Held::open(
        &input,
        s(&source["local_database_relative_path"])?,
        256 * 1024 * 1024,
    )?;
    ensure(
        database.digest == s(&source["local_database_sha256"])?,
        "local lexical database digest drift",
    )?;
    let mut census = Census::default();
    {
        let db = input.open_sqlite_readonly_for_ordered_scan(&database.file)?;
        ensure(
            db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                == "ok",
            "lexical database quick_check failed",
        )?;
        let mut stmt = db.prepare("SELECT form_key,exact_form,exact_form_sha256,normalized_form_sha256,occurrence_count FROM forms ORDER BY exact_form_sha256,exact_form").map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let get = |i| row.get::<_, String>(i).map_err(|e| e.to_string());
            let count = row.get::<_, i64>(4).map_err(|e| e.to_string())?;
            census.push(
                ctx,
                get(0)?,
                get(1)?,
                get(2)?,
                get(3)?,
                u64::try_from(count).map_err(|_| "negative occurrence count")?,
            )?;
        }
    }
    let summary = census.summary()?;
    for key in [
        "exact_form_row_count",
        "normalized_form_hash_count",
        "token_occurrence_count",
    ] {
        ensure(
            summary[key] == source[key],
            &format!("morphology {key} drift"),
        )?;
    }
    let base_packet = s(&plan["a_census"]["local_packet"]["relative_path"])?;
    let base_receipt = s(&plan["a_census"]["tracked_receipt_ref"])?;
    let packet_ref = options
        .generation
        .map(|g| generated_path(base_packet, g))
        .transpose()?
        .unwrap_or(base_packet.into());
    let receipt_ref = if let Some(reference) = options.receipt {
        reference.into()
    } else {
        options
            .generation
            .map(|g| generated_path(base_receipt, g))
            .transpose()?
            .unwrap_or(base_receipt.into())
    };
    ensure(
        packet_ref != receipt_ref,
        "packet and receipt paths must differ",
    )?;
    let mut builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("lexical_derivatives.rs")),
        "running lexical generator source drift",
    )?;
    let mut receipt = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/morphology-input-receipt.schema.json",
        "schema_version":"tos_morphology_input_receipt_v1", "generated_or_authored":"generated_from_local_lexical_projection",
        "receipt_id":format!("morphology-input-receipt:zarathustra-dta-exact-form-census-v1{}", options.generation.map(|g|format!(".native-{g}")).unwrap_or_default()),
        "plan_id":plan["plan_id"], "plan_ref":options.plan, "plan_sha256":plan_digest,
        "generator_ref":BUILDER, "generator_sha256":builder.digest,
        "source_database":{"relative_path":source["local_database_relative_path"], "sha256":database.digest, "bytes":database.metadata.len()},
        "source_projection":{"index_plan_ref":source["index_plan_ref"], "index_plan_sha256":source["index_plan_sha256"], "tracked_projection_ref":source["tracked_projection_ref"], "tracked_projection_sha256":source["tracked_projection_sha256"]},
        "local_packet":{"relative_path":packet_ref,"format":"jsonl","schema_version":"tos_morphology_input_row_v1","sha256":sha(&census.bytes),"bytes":census.bytes.len(),"mode":"0600","row_count":census.rows,"required_fields":ROW_FIELDS},
        "summary":summary,
        "content_exposure":{"local_exact_strings":true,"tracked_exact_strings":false,"tracked_sequence":false,"tracked_context":false,"tracked_positions":false},
        "semantic_boundary":{"creates_accepted_source":false,"creates_lemma":false,"creates_lexeme":false,"creates_sign":false,"creates_semantic_claim":false,"opens_human_backlog":false},
        "authority_boundary":"This artifact materializes private exact-form input for the selected morphology experiment.",
    });
    if options.generation.is_none() {
        let prior = read_json(ctx, base_receipt, &mut held)?;
        ensure(
            held.last().unwrap().digest == RETAINED_RECEIPT_SHA && options.plan == MORPHOLOGY_PLAN,
            "unknown historical morphology receipt",
        )?;
        // Preserve historical provenance verbatim; this comparison reconstructs
        // its data result and does not relabel the Rust run as the old producer.
        for key in ["generator_ref", "generator_sha256", "authority_boundary"] {
            receipt[key] = prior[key].clone();
        }
        ensure(receipt == prior, "retained morphology receipt data drift")?;
    }
    validate(&schemas, "morphology-input-receipt", &receipt)?;
    for source in &mut held {
        source.verify(ctx)?;
    }
    builder.verify(ctx)?;
    database.verify(&input)?;
    let output = ctx.select_output_directory(options.output_root, options.build)?;
    let receipt_bytes = canonical(receipt.clone())?;
    let packet_present = present(&output, &packet_ref, &census.bytes, 0o600)?;
    let receipt_present = if options.generation.is_some() {
        present(&output, &receipt_ref, &receipt_bytes, 0o644)?
    } else {
        true
    };
    if options.build {
        if !packet_present {
            output.write(&packet_ref, &census.bytes, 0o600, true)?;
        }
        // Receipt is the completion marker. A retry first verifies any already
        // published packet; no earlier generation or conflicting output changes.
        if !receipt_present {
            output.write(&receipt_ref, &receipt_bytes, 0o644, true)?;
        }
    } else {
        ensure(
            packet_present && receipt_present,
            "morphology derivative missing",
        )?;
    }
    ctx.check()?;
    Ok(
        json!({"status":if options.build {"materialized"} else {"verified"}, "packet_ref":packet_ref,"packet_sha256":sha(&census.bytes),"packet_bytes":census.bytes.len(),"receipt_ref":receipt_ref,"receipt_sha256":sha(&receipt_bytes),"summary":summary,"budget":ctx.budget_report()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_private_census_order_counts_and_digest_refusals() {
        let ctx = ResearchExecution::new(Path::new(env!("CARGO_MANIFEST_DIR")), 30).unwrap();
        let mut forms = [("Über-Mensch", "über-mensch", 2), ("Straße", "strasse", 1)].map(
            |(exact, normalized, count)| {
                (
                    sha(exact.as_bytes()),
                    exact,
                    sha(normalized.as_bytes()),
                    count,
                )
            },
        );
        forms.sort();
        let mut census = Census::default();
        for (digest, exact, normalized, count) in &forms {
            census
                .push(
                    &ctx,
                    format!("lexical-form:sha256:{digest}"),
                    (*exact).into(),
                    digest.clone(),
                    normalized.clone(),
                    *count,
                )
                .unwrap();
        }
        assert_eq!(
            census.summary().unwrap(),
            json!({"exact_form_row_count":2,"normalized_form_hash_count":2,"token_occurrence_count":3,"singleton_form_count":1,"joiner_form_count":1,"maximum_codepoint_length":11})
        );
        let rows: Vec<Value> = std::str::from_utf8(&census.bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for (row, (_, exact, _, _)) in rows.iter().zip(&forms) {
            assert_eq!(row["exact_form"], *exact);
        }
        let prior_bytes = census.bytes.clone();
        let (digest, exact, normalized, count) = &forms[1];
        assert!(
            census
                .push(
                    &ctx,
                    format!("lexical-form:sha256:{digest}"),
                    (*exact).into(),
                    digest.clone(),
                    normalized.clone(),
                    *count
                )
                .is_err()
        );
        assert_eq!(census.bytes, prior_bytes);
        assert!(
            Census::default()
                .push(
                    &ctx,
                    format!("lexical-form:sha256:{digest}"),
                    "changed".into(),
                    digest.clone(),
                    normalized.clone(),
                    1
                )
                .is_err()
        );
        assert!(
            Census::default()
                .push(
                    &ctx,
                    "wrong-key".into(),
                    (*exact).into(),
                    digest.clone(),
                    normalized.clone(),
                    1
                )
                .is_err()
        );
        assert!(
            Census::default()
                .push(
                    &ctx,
                    format!("lexical-form:sha256:{digest}"),
                    (*exact).into(),
                    digest.clone(),
                    "x".into(),
                    1
                )
                .is_err()
        );
        assert!(
            Census::default()
                .push(
                    &ctx,
                    format!("lexical-form:sha256:{digest}"),
                    (*exact).into(),
                    digest.clone(),
                    normalized.clone(),
                    0
                )
                .is_err()
        );
        assert!(Census::default().summary().is_err());
        assert_eq!(
            canonical(json!({"z":"ß\n\u{0000}","a":[{"z":2,"a":1}]})).unwrap(),
            b"{\"a\":[{\"a\":1,\"z\":2}],\"z\":\"\xc3\x9f\\n\\u0000\"}\n"
        );
    }
}

pub mod semantic_recurrence;

pub mod morphology_context;

pub mod morphology_context_result;
