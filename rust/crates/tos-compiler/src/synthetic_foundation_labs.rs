//! Public synthetic fixture definitions and the native owner validation route.
//! These are authored test inputs, never evidence of a translation or review act.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{encode, ensure, s, sha},
    transfer_target_passages::read_optional,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs::File, time::Instant};
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    item_rules::{ItemLimits, ItemRefusal},
    layer_family_rules::LayerFamilySource,
    source_foundation_labs::{SourceFoundationLab, inspect_source_foundation_lab},
};
type Result<T> = std::result::Result<T, String>;
const BUILDER: &str = "rust/crates/tos-compiler/src/synthetic_foundation_labs.rs";
const CAP: usize = 4 * 1024 * 1024;
struct Recipe {
    legacy: &'static str,
    manifest: &'static str,
    contract: &'static str,
    research: &'static str,
    lab: SourceFoundationLab,
    files: &'static [(&'static str, &'static str)],
}
fn substitute(value: &mut Value, old: &str) {
    match value {
        Value::String(text) if text == old => *text = BUILDER.into(),
        Value::Array(rows) => {
            for row in rows {
                substitute(row, old);
            }
        }
        Value::Object(fields) => {
            for row in fields.values_mut() {
                substitute(row, old);
            }
        }
        _ => {}
    }
}
struct Prepared {
    files: BTreeMap<String, Vec<u8>>,
    inputs: Vec<(String, File, String)>,
    manifest: String,
}
fn prepare(ctx: &ResearchExecution, name: &str) -> Result<Prepared> {
    let recipe = recipe(name)?;
    let mut inputs = Vec::new();
    let mut external = BTreeMap::new();
    for reference in [recipe.contract, recipe.research] {
        let mut file = ctx.source_file(reference, CAP as u64)?;
        let raw = ctx.read_file(&mut file, CAP as u64)?;
        inputs.push((reference.into(), file, sha(&raw)));
        external.insert(reference.to_string(), raw);
    }
    let contract = format!("https://tree-of-sophia.local/{}", recipe.contract);
    let schema = SchemaBackendProbe::new(
        [SchemaResource {
            uri: contract.clone(),
            raw: external[recipe.contract].clone(),
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("lab schema: {e:?}"))?;
    let mut files = BTreeMap::new();
    let mut manifest = None;
    for &(reference, raw) in recipe.files {
        ctx.tick(raw.len() as u64)?;
        if reference == recipe.manifest {
            manifest = Some(serde_json::from_str::<Value>(raw).map_err(|e| e.to_string())?);
            continue;
        }
        let bytes = if reference.ends_with(".json") {
            let mut value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            substitute(&mut value, recipe.legacy);
            encode(&value, true)?
        } else {
            raw.as_bytes().to_vec()
        };
        ensure(
            reference.starts_with("ToS/research-packets/foundation-laboratory-2026-07/")
                && bytes.len() <= CAP,
            "synthetic output scope",
        )?;
        files.insert(reference.to_string(), bytes);
    }
    let mut manifest = manifest.ok_or("missing lab manifest definition")?;
    let builder = include_bytes!("synthetic_foundation_labs.rs");
    manifest["builder"] = json!({"ref":BUILDER,"sha256":sha(builder)});
    for key in ["contract", "research"] {
        let reference = s(&manifest[key]["ref"])?;
        manifest[key]["sha256"] = json!(sha(external
            .get(reference)
            .ok_or("unselected manifest dependency")?));
    }
    for key in ["plan", "source", "input_fixture"] {
        if manifest.get(key).is_some() {
            let reference = s(&manifest[key]["ref"])?;
            manifest[key]["sha256"] = json!(sha(files
                .get(reference)
                .ok_or("manifest fixture binding")?));
        }
    }
    for key in ["inputs", "analysis_artifacts"] {
        if let Some(rows) = manifest.get_mut(key).and_then(Value::as_array_mut) {
            for row in rows {
                let reference = s(&row["ref"])?;
                row["sha256"] = json!(sha(files
                    .get(reference)
                    .ok_or("manifest analysis binding")?));
            }
        }
    }
    for row in manifest["variants"].as_array_mut().ok_or("lab variants")? {
        let reference = s(&row["packet_ref"])?;
        row["packet_sha256"] = json!(sha(files.get(reference).ok_or("variant bytes")?));
    }
    files.insert(recipe.manifest.into(), encode(&manifest, true)?);
    let mut source = Overlay {
        ctx,
        files: &files,
        external,
        builder,
        schema: &schema,
        contract: recipe.contract,
        read_bytes: 0,
    };
    let limits = ItemLimits {
        max_member_bytes: CAP,
        max_total_bytes: (32 * CAP) as u64,
        max_state_bytes: 32 * CAP,
        max_issues: 128,
        deadline: ctx.deadline(),
    };
    let report = inspect_source_foundation_lab(
        &mut source,
        limits,
        recipe.lab,
        &files.keys().cloned().collect::<Vec<_>>(),
    )
    .map_err(|e| format!("synthetic lab closure: {e:?}"))?;
    ensure(
        report.unimplemented.is_empty() && report.ordered_issues.is_empty(),
        &format!("synthetic lab closure: {:?}", report.ordered_issues),
    )?;
    for request in report.schema_checks {
        ctx.check()?;
        ensure(
            request.contract == recipe.contract,
            "unselected schema request",
        )?;
        let valid = schema
            .is_valid_raw(&contract, &encode(&request.instance, false)?)
            .map_err(|e| format!("synthetic schema execution: {e:?}"))?;
        if let Some(expected) = request.expected_valid {
            ensure(valid == expected, "synthetic schema verdict changed")?;
        }
        if let Some(expected) = request.expected_rejected {
            ensure(
                (!valid || request.semantic_rejected.unwrap_or(false)) == expected,
                "synthetic negative control changed",
            )?;
        }
        if request.expected_valid.is_none() && request.expected_rejected.is_none() {
            ensure(valid, "synthetic packet schema invalid")?;
        }
    }
    Ok(Prepared {
        files,
        inputs,
        manifest: recipe.manifest.into(),
    })
}
struct Overlay<'a> {
    ctx: &'a ResearchExecution,
    files: &'a BTreeMap<String, Vec<u8>>,
    external: BTreeMap<String, Vec<u8>>,
    builder: &'static [u8],
    schema: &'a SchemaBackendProbe,
    contract: &'static str,
    read_bytes: u64,
}
impl LayerFamilySource for Overlay<'_> {
    fn current(
        &mut self,
        path: &str,
        max: usize,
        deadline: Instant,
    ) -> std::result::Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        let value = if path == BUILDER {
            Some(self.builder)
        } else {
            self.files
                .get(path)
                .or_else(|| self.external.get(path))
                .map(Vec::as_slice)
        };
        if let Some(raw) = value {
            if raw.len() > max {
                return Err(ItemRefusal::Budget);
            }
            self.read_bytes += raw.len() as u64;
            if self.read_bytes > (32 * CAP) as u64 {
                return Err(ItemRefusal::Budget);
            }
        }
        Ok(value.map(<[u8]>::to_vec))
    }
    fn recorded(
        &mut self,
        path: &str,
        digest: &str,
        max: usize,
        deadline: Instant,
    ) -> std::result::Result<Option<Vec<u8>>, ItemRefusal> {
        Ok(self
            .current(path, max, deadline)?
            .filter(|raw| sha(raw) == digest))
    }
    fn schema(
        &mut self,
        _path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> std::result::Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        if contract != self.contract {
            return Err(ItemRefusal::Source("unexpected lab contract".into()));
        }
        self.schema
            .is_valid_raw(&format!("https://tree-of-sophia.local/{contract}"), raw)
            .map_err(|e| ItemRefusal::Source(format!("schema: {e:?}")))
    }
    fn checkpoint(&mut self, deadline: Instant) -> std::result::Result<(), ItemRefusal> {
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        self.ctx.check().map_err(ItemRefusal::Source)
    }
}
pub fn run(ctx: &ResearchExecution, name: &str, build: bool) -> Result<Value> {
    let mut prepared = prepare(ctx, name)?;
    // All bindings and negative controls are resolved before any generated write.
    let mut changes = Vec::new();
    for (reference, raw) in &prepared.files {
        let prior = read_optional(ctx, reference)?;
        if prior.as_deref() != Some(raw) {
            ensure(build, &format!("synthetic fixture stale: {reference}"))?;
            changes.push((reference, raw, prior));
        }
    }
    for (reference, file, digest) in &mut prepared.inputs {
        ensure(
            ctx.hash_file(file, CAP as u64)? == *digest,
            "held lab dependency changed",
        )?;
        let mut current = ctx.source_file(reference, CAP as u64)?;
        ensure(
            ctx.hash_file(&mut current, CAP as u64)? == *digest,
            "lab dependency path changed",
        )?;
    }
    let count = changes.len();
    // Manifest is committed last. A partial interruption remains visibly stale
    // and can be completed by an idempotent repeat using the same recipe.
    changes.sort_by_key(|(reference, _, _)| *reference == &prepared.manifest);
    for (reference, raw, prior) in changes {
        match prior {
            Some(old) => ctx.write_replacing_exact(reference, raw, 0o644, &old)?,
            None => ctx.write(reference, raw, 0o644, true)?,
        }
    }
    Ok(
        json!({"status":"passed","laboratory":name,"records":prepared.files.len(),"written":count,"variants":3,"schema_and_semantic_negative_controls":"passed","source_posture":"public_synthetic_only","human_review_performed":false,"source_admission_performed":false,"canon_effect":false}),
    )
}

fn recipe(name: &str) -> Result<Recipe> {
    match name {
        "source-text-unit-v1" => Ok(Recipe {
            legacy: "scripts/build_source_text_unit_v1_lab.py",
            manifest: "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/lab.manifest.json",
            contract: "ToS/contracts/source-text-unit-packet-v1.schema.json",
            research: "ToS/research-packets/foundation-laboratory-2026-07/SOURCE_TEXT_UNIT_SEGMENTATION_RESEARCH_2026-08-11.md",
            lab: SourceFoundationLab::SourceTextUnitV1,
            files: &[
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
                    r####"mark A. ember-turns.
mark B: ember turns.
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
                    r####"{
  "authority_limits": {
    "canon_effect": false,
    "graph_truth_established": false,
    "human_review_performed": false,
    "legacy_migration_performed": false,
    "lexeme_established": false,
    "linguistic_word_established": false,
    "model_invoked": false,
    "private_source_used": false,
    "real_language_boundary_accepted": false,
    "semantic_truth_established": false
  },
  "input_posture": "public_synthetic_only",
  "language": "x-tos-unit",
  "negative_controls": [
    "text-derived-unit-id",
    "duplicate-unit-id",
    "duplicate-scheme-id",
    "duplicate-segmentation-id",
    "missing-scheme-ref",
    "missing-unit-ref",
    "missing-anchor-ref",
    "source-layer-digest-drift",
    "anchor-exact-digest-drift",
    "anchor-out-of-bounds",
    "anchor-reversed-range",
    "unit-anchor-order-overlap",
    "one-way-parent-child",
    "self-parent-unit",
    "one-way-competing-segmentation",
    "self-competing-segmentation",
    "hidden-exhaustive-coverage-gap",
    "undeclared-overlap",
    "accepted-machine-result-without-review",
    "accepted-with-sample-only-review",
    "accepted-without-language-competence",
    "model-subword-promoted-to-linguistic-authority",
    "graph-projection-of-proposed-segmentation",
    "projection-visibility-widening",
    "self-supersession"
  ],
  "question": "Can ToS preserve exact source text units and reciprocal competing segmentations without turning a method result into source or linguistic truth?",
  "schema_version": "tos_source_text_unit_v1_lab_plan_v1",
  "variants": [
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "purpose": "Exact physical-line and line-break observation only",
      "variant_id": "A"
    },
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "purpose": "Reciprocal sentence and hyphen-tokenization alternatives, all unresolved",
      "variant_id": "B"
    },
    {
      "expected_schema_valid": false,
      "expected_semantic_valid": false,
      "purpose": "Invalid model-shaped acceptance without review and with hidden coverage gap",
      "variant_id": "C"
    }
  ]
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-a-source-layout-observation.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
  "anchors": [
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
      "anchor_role": "scope",
      "exact_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
      "ordinal": 1,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-a-line-1",
      "anchor_role": "content",
      "exact_sha256": "18d75c93d7be519be9df557867cedcc1f3259d3020a55776fc848835a902ef98",
      "ordinal": 2,
      "selector": {
        "end": 20,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-a-break-1",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 3,
      "selector": {
        "end": 21,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 20,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-a-line-2",
      "anchor_role": "content",
      "exact_sha256": "763ebbc06bd6bc83d28a1eb5a0b0530df4c0e93a4f76049b52deafa04201296c",
      "ordinal": 4,
      "selector": {
        "end": 41,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 21,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-a-break-2",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 5,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 41,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    }
  ],
  "authority_boundary": {
    "graph_role": "relation",
    "legacy_bulk_migration_authorized": false,
    "model_token_is_not_linguistic_token": true,
    "projection_is_owner_truth": false,
    "segmentation_is_method_result_not_source_truth": true,
    "source_role": "authority",
    "source_unit_is_not_lexeme_sign_or_concept": true,
    "tree_role": "orientation",
    "validators_prove_mechanics_not_truth": true
  },
  "content_posture": "public_synthetic_contract_exercise",
  "packet_id": "tos.source-text-unit-packet.sid-6f5a85dde63609e96988498c51b32fea",
  "packet_version": 1,
  "projections": [],
  "reviews": [],
  "rights_and_visibility": {
    "effective_visibility": "public",
    "inheritance_policy": "most-restrictive-source-packet-and-destination-wins",
    "packet_visibility": "public",
    "private_source_used": false,
    "publication_authorized": true,
    "rights_record_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json"
    ],
    "source_visibility": "public"
  },
  "schema_version": "tos_source_text_unit_packet_v1",
  "schemes": [
    {
      "analysis_role": "source_layout",
      "authority_limit": {
        "algorithmic_output_is_linguistic_truth": false,
        "algorithmic_output_is_source_truth": false,
        "model_subword_is_lexeme": false,
        "text_mutation_allowed": false,
        "unit_identity_is_semantic_identity": false
      },
      "boundary_basis": "source_layout",
      "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
      "method": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic exact physical-line observation",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "policies": {
        "hyphenation": "preserve_source",
        "line_break": "standalone_units",
        "normalization": "no-text-mutation-separate-successor-layer",
        "overlap": "forbid",
        "punctuation": "included_in_neighbor",
        "unreported_gaps_allowed": false,
        "whitespace": "included_in_neighbor"
      },
      "scheme_id": "tos.text-unit-scheme.sid-784a470be3a4391c209b16e609b1ffa0",
      "scheme_name": "public-synthetic exact physical-line observation",
      "scheme_version": 1,
      "supersedes_scheme_ref": null,
      "unit_kinds": [
        "physical_line",
        "whitespace"
      ]
    }
  ],
  "segmentations": [
    {
      "competing_segmentation_refs": [],
      "coverage": {
        "coverage_posture": "exhaustive_nonoverlapping",
        "excluded_anchor_refs": [],
        "overlap_requires_declaration": true,
        "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
        "source_reconstruction_required": true,
        "unreported_gaps_allowed": false
      },
      "declared_uses": [
        "navigation",
        "source_observation"
      ],
      "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
      "linguistic_authority": false,
      "maker": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic segmentation result a-layout",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "ordered_unit_refs": [
        "tos.text-unit.sid-2ef02a16df2655822eeb24b440202543",
        "tos.text-unit.sid-af3343ef65a521f9ea9a1bd13b6b716e",
        "tos.text-unit.sid-4410b51d4c5f427ba4ec0999095775dc",
        "tos.text-unit.sid-cd8f419a920564c06840ab7d3a0d96b6"
      ],
      "review_refs": [],
      "scheme_ref": "tos.text-unit-scheme.sid-784a470be3a4391c209b16e609b1ffa0",
      "segmentation_id": "tos.text-segmentation.sid-feb12ad230934b6ca0fc1cced0780419",
      "segmentation_version": 1,
      "semantic_authority": false,
      "source_text_authority": false,
      "status": "observed_source_structure",
      "status_reason": "Only exact physical line content and line-break code points are observed.",
      "supersedes_segmentation_ref": null
    }
  ],
  "source_layer": {
    "immutable": true,
    "interval": "half_open",
    "language": "x-tos-unit",
    "media_type": "text/plain; charset=utf-8",
    "position_unit": "unicode_code_point",
    "publication_authorized": true,
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
    "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
    "unicode_form": "source_preserved",
    "visibility": "public"
  },
  "source_scope": {
    "edition_ref": "tos.edition.source-text-unit-v1.synthetic-miniature",
    "expression_ref": "tos.expression.source-text-unit-v1.synthetic-miniature",
    "file_ref": "tos.file.source-text-unit-v1.synthetic-miniature.utf8",
    "file_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
    "item_ref": "tos.item.source-text-unit-v1.synthetic-miniature",
    "work_ref": "tos.work.source-text-unit-v1.synthetic-miniature"
  },
  "supersedes_packet_ref": null,
  "units": [
    {
      "boundary_posture": "source_attested",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 1.0
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-a-line-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-2ef02a16df2655822eeb24b440202543",
      "unit_kind": "physical_line",
      "unit_version": 1
    },
    {
      "boundary_posture": "source_attested",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 1.0
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-a-break-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-af3343ef65a521f9ea9a1bd13b6b716e",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "source_attested",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 1.0
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-a-line-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-4410b51d4c5f427ba4ec0999095775dc",
      "unit_kind": "physical_line",
      "unit_version": 1
    },
    {
      "boundary_posture": "source_attested",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 1.0
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-a-break-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-cd8f419a920564c06840ab7d3a0d96b6",
      "unit_kind": "whitespace",
      "unit_version": 1
    }
  ]
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-b-competing-segmentations.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
  "anchors": [
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
      "anchor_role": "scope",
      "exact_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
      "ordinal": 1,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-1",
      "anchor_role": "content",
      "exact_sha256": "18d75c93d7be519be9df557867cedcc1f3259d3020a55776fc848835a902ef98",
      "ordinal": 2,
      "selector": {
        "end": 20,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-break-1",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 3,
      "selector": {
        "end": 21,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 20,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-2",
      "anchor_role": "content",
      "exact_sha256": "763ebbc06bd6bc83d28a1eb5a0b0530df4c0e93a4f76049b52deafa04201296c",
      "ordinal": 4,
      "selector": {
        "end": 41,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 21,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-break-2",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 5,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 41,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-split-sentence-1a",
      "anchor_role": "content",
      "exact_sha256": "9039ae2ed72e97c4c9ce85681d4c9fb5f48171a2eb744ff8ad96ce6ebf86023e",
      "ordinal": 6,
      "selector": {
        "end": 7,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-split-space-1",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 7,
      "selector": {
        "end": 8,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 7,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-split-sentence-1b",
      "anchor_role": "content",
      "exact_sha256": "00cc09fe13ed9d372ae2819c29e5794551564b8a8a8303c88901a8c5668f4186",
      "ordinal": 8,
      "selector": {
        "end": 20,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 8,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-split-break-1",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 9,
      "selector": {
        "end": 21,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 20,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-split-sentence-2",
      "anchor_role": "content",
      "exact_sha256": "763ebbc06bd6bc83d28a1eb5a0b0530df4c0e93a4f76049b52deafa04201296c",
      "ordinal": 10,
      "selector": {
        "end": 41,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 21,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-split-break-2",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 11,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 41,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-mark-1",
      "anchor_role": "content",
      "exact_sha256": "6201eb4dccc956cc4fa3a78dca0c2888177ec52efd48f125df214f046eb43138",
      "ordinal": 12,
      "selector": {
        "end": 4,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-1",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 13,
      "selector": {
        "end": 5,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 4,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-a",
      "anchor_role": "content",
      "exact_sha256": "559aead08264d5795d3909718cdd05abd49572e84fe55590eef31a88a08fdffd",
      "ordinal": 14,
      "selector": {
        "end": 6,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 5,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-period-1",
      "anchor_role": "punctuation",
      "exact_sha256": "cdb4ee2aea69cc6a83331bbe96dc2caa9a299d21329efb0336fc02a82e1839a8",
      "ordinal": 15,
      "selector": {
        "end": 7,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 6,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-2",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 16,
      "selector": {
        "end": 8,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 7,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-ember-turns",
      "anchor_role": "content",
      "exact_sha256": "fdcae3e82fddff9121e3a8475b8296ace91537f796660b2efc46f2b818ad0932",
      "ordinal": 17,
      "selector": {
        "end": 19,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 8,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-period-2",
      "anchor_role": "punctuation",
      "exact_sha256": "cdb4ee2aea69cc6a83331bbe96dc2caa9a299d21329efb0336fc02a82e1839a8",
      "ordinal": 18,
      "selector": {
        "end": 20,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 19,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-break-1",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 19,
      "selector": {
        "end": 21,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 20,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-mark-2",
      "anchor_role": "content",
      "exact_sha256": "6201eb4dccc956cc4fa3a78dca0c2888177ec52efd48f125df214f046eb43138",
      "ordinal": 20,
      "selector": {
        "end": 25,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 21,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-3",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 21,
      "selector": {
        "end": 26,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 25,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-b",
      "anchor_role": "content",
      "exact_sha256": "df7e70e5021544f4834bbee64a9e3789febc4be81470df629cad6ddb03320a5c",
      "ordinal": 22,
      "selector": {
        "end": 27,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 26,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-colon",
      "anchor_role": "punctuation",
      "exact_sha256": "e7ac0786668e0ff0f02b62bd04f45ff636fd82db63b1104601c975dc005f3a67",
      "ordinal": 23,
      "selector": {
        "end": 28,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 27,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-4",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 24,
      "selector": {
        "end": 29,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 28,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-ember-2",
      "anchor_role": "content",
      "exact_sha256": "7cadc15d609c4ae9b4be6265b8e1cace16e6fa78a81ab0c7db82e687a7c867a5",
      "ordinal": 25,
      "selector": {
        "end": 34,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 29,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-5",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 26,
      "selector": {
        "end": 35,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 34,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-turns-2",
      "anchor_role": "content",
      "exact_sha256": "6976a029d065bf7dd5a64c4a61ff9258550e18abe1838fb9c6ecda43c908147a",
      "ordinal": 27,
      "selector": {
        "end": 40,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 35,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-period-3",
      "anchor_role": "punctuation",
      "exact_sha256": "cdb4ee2aea69cc6a83331bbe96dc2caa9a299d21329efb0336fc02a82e1839a8",
      "ordinal": 28,
      "selector": {
        "end": 41,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 40,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-break-2",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 29,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 41,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-mark-1",
      "anchor_role": "content",
      "exact_sha256": "6201eb4dccc956cc4fa3a78dca0c2888177ec52efd48f125df214f046eb43138",
      "ordinal": 30,
      "selector": {
        "end": 4,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-1",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 31,
      "selector": {
        "end": 5,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 4,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-a",
      "anchor_role": "content",
      "exact_sha256": "559aead08264d5795d3909718cdd05abd49572e84fe55590eef31a88a08fdffd",
      "ordinal": 32,
      "selector": {
        "end": 6,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 5,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-period-1",
      "anchor_role": "punctuation",
      "exact_sha256": "cdb4ee2aea69cc6a83331bbe96dc2caa9a299d21329efb0336fc02a82e1839a8",
      "ordinal": 33,
      "selector": {
        "end": 7,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 6,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-2",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 34,
      "selector": {
        "end": 8,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 7,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-ember-1",
      "anchor_role": "content",
      "exact_sha256": "7cadc15d609c4ae9b4be6265b8e1cace16e6fa78a81ab0c7db82e687a7c867a5",
      "ordinal": 35,
      "selector": {
        "end": 13,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 8,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-hyphen",
      "anchor_role": "punctuation",
      "exact_sha256": "3973e022e93220f9212c18d0d0c543ae7c309e46640da93a4a0314de999f5112",
      "ordinal": 36,
      "selector": {
        "end": 14,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 13,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-turns-1",
      "anchor_role": "content",
      "exact_sha256": "6976a029d065bf7dd5a64c4a61ff9258550e18abe1838fb9c6ecda43c908147a",
      "ordinal": 37,
      "selector": {
        "end": 19,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 14,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-period-2",
      "anchor_role": "punctuation",
      "exact_sha256": "cdb4ee2aea69cc6a83331bbe96dc2caa9a299d21329efb0336fc02a82e1839a8",
      "ordinal": 38,
      "selector": {
        "end": 20,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 19,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-break-1",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 39,
      "selector": {
        "end": 21,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 20,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-mark-2",
      "anchor_role": "content",
      "exact_sha256": "6201eb4dccc956cc4fa3a78dca0c2888177ec52efd48f125df214f046eb43138",
      "ordinal": 40,
      "selector": {
        "end": 25,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 21,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-3",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 41,
      "selector": {
        "end": 26,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 25,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-b",
      "anchor_role": "content",
      "exact_sha256": "df7e70e5021544f4834bbee64a9e3789febc4be81470df629cad6ddb03320a5c",
      "ordinal": 42,
      "selector": {
        "end": 27,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 26,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-colon",
      "anchor_role": "punctuation",
      "exact_sha256": "e7ac0786668e0ff0f02b62bd04f45ff636fd82db63b1104601c975dc005f3a67",
      "ordinal": 43,
      "selector": {
        "end": 28,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 27,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-4",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 44,
      "selector": {
        "end": 29,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 28,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-ember-2",
      "anchor_role": "content",
      "exact_sha256": "7cadc15d609c4ae9b4be6265b8e1cace16e6fa78a81ab0c7db82e687a7c867a5",
      "ordinal": 45,
      "selector": {
        "end": 34,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 29,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-5",
      "anchor_role": "whitespace",
      "exact_sha256": "36a9e7f1c95b82ffb99743e0c5c4ce95d83c9a430aac59f84ef3cbfab6145068",
      "ordinal": 46,
      "selector": {
        "end": 35,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 34,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-turns-2",
      "anchor_role": "content",
      "exact_sha256": "6976a029d065bf7dd5a64c4a61ff9258550e18abe1838fb9c6ecda43c908147a",
      "ordinal": 47,
      "selector": {
        "end": 40,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 35,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-period-3",
      "anchor_role": "punctuation",
      "exact_sha256": "cdb4ee2aea69cc6a83331bbe96dc2caa9a299d21329efb0336fc02a82e1839a8",
      "ordinal": 48,
      "selector": {
        "end": 41,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 40,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-token-split-break-2",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 49,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 41,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    }
  ],
  "authority_boundary": {
    "graph_role": "relation",
    "legacy_bulk_migration_authorized": false,
    "model_token_is_not_linguistic_token": true,
    "projection_is_owner_truth": false,
    "segmentation_is_method_result_not_source_truth": true,
    "source_role": "authority",
    "source_unit_is_not_lexeme_sign_or_concept": true,
    "tree_role": "orientation",
    "validators_prove_mechanics_not_truth": true
  },
  "content_posture": "public_synthetic_contract_exercise",
  "packet_id": "tos.source-text-unit-packet.sid-28b405d8e5fac9c3eb5a6954b9bca42a",
  "packet_version": 1,
  "projections": [],
  "reviews": [],
  "rights_and_visibility": {
    "effective_visibility": "public",
    "inheritance_policy": "most-restrictive-source-packet-and-destination-wins",
    "packet_visibility": "public",
    "private_source_used": false,
    "publication_authorized": true,
    "rights_record_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json"
    ],
    "source_visibility": "public"
  },
  "schema_version": "tos_source_text_unit_packet_v1",
  "schemes": [
    {
      "analysis_role": "linguistic",
      "authority_limit": {
        "algorithmic_output_is_linguistic_truth": false,
        "algorithmic_output_is_source_truth": false,
        "model_subword_is_lexeme": false,
        "text_mutation_allowed": false,
        "unit_identity_is_semantic_identity": false
      },
      "boundary_basis": "public_synthetic_fixture",
      "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
      "method": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic sentence candidate b-whole",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "policies": {
        "hyphenation": "preserve_source",
        "line_break": "standalone_units",
        "normalization": "no-text-mutation-separate-successor-layer",
        "overlap": "forbid",
        "punctuation": "included_in_neighbor",
        "unreported_gaps_allowed": false,
        "whitespace": "standalone_units"
      },
      "scheme_id": "tos.text-unit-scheme.sid-d03b0064030838d64314142f422b3ec1",
      "scheme_name": "public-synthetic sentence candidate b-whole",
      "scheme_version": 1,
      "supersedes_scheme_ref": null,
      "unit_kinds": [
        "sentence",
        "whitespace"
      ]
    },
    {
      "analysis_role": "linguistic",
      "authority_limit": {
        "algorithmic_output_is_linguistic_truth": false,
        "algorithmic_output_is_source_truth": false,
        "model_subword_is_lexeme": false,
        "text_mutation_allowed": false,
        "unit_identity_is_semantic_identity": false
      },
      "boundary_basis": "public_synthetic_fixture",
      "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
      "method": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic sentence candidate b-split",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "policies": {
        "hyphenation": "preserve_source",
        "line_break": "standalone_units",
        "normalization": "no-text-mutation-separate-successor-layer",
        "overlap": "forbid",
        "punctuation": "included_in_neighbor",
        "unreported_gaps_allowed": false,
        "whitespace": "standalone_units"
      },
      "scheme_id": "tos.text-unit-scheme.sid-0221652f661431c9a0c182d0b626f381",
      "scheme_name": "public-synthetic sentence candidate b-split",
      "scheme_version": 1,
      "supersedes_scheme_ref": null,
      "unit_kinds": [
        "sentence",
        "whitespace"
      ]
    },
    {
      "analysis_role": "orthographic",
      "authority_limit": {
        "algorithmic_output_is_linguistic_truth": false,
        "algorithmic_output_is_source_truth": false,
        "model_subword_is_lexeme": false,
        "text_mutation_allowed": false,
        "unit_identity_is_semantic_identity": false
      },
      "boundary_basis": "public_synthetic_fixture",
      "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
      "method": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic orthographic token candidate b-token-whole",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "policies": {
        "hyphenation": "preserve_source",
        "line_break": "standalone_units",
        "normalization": "no-text-mutation-separate-successor-layer",
        "overlap": "forbid",
        "punctuation": "standalone_units",
        "unreported_gaps_allowed": false,
        "whitespace": "standalone_units"
      },
      "scheme_id": "tos.text-unit-scheme.sid-9d544a918b9032759857d91121e1714d",
      "scheme_name": "public-synthetic orthographic token candidate b-token-whole",
      "scheme_version": 1,
      "supersedes_scheme_ref": null,
      "unit_kinds": [
        "surface_token",
        "punctuation",
        "whitespace"
      ]
    },
    {
      "analysis_role": "orthographic",
      "authority_limit": {
        "algorithmic_output_is_linguistic_truth": false,
        "algorithmic_output_is_source_truth": false,
        "model_subword_is_lexeme": false,
        "text_mutation_allowed": false,
        "unit_identity_is_semantic_identity": false
      },
      "boundary_basis": "public_synthetic_fixture",
      "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
      "method": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic orthographic token candidate b-token-split",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "policies": {
        "hyphenation": "preserve_source",
        "line_break": "standalone_units",
        "normalization": "no-text-mutation-separate-successor-layer",
        "overlap": "forbid",
        "punctuation": "standalone_units",
        "unreported_gaps_allowed": false,
        "whitespace": "standalone_units"
      },
      "scheme_id": "tos.text-unit-scheme.sid-2e32e80b443e1f865666b5f044290fd7",
      "scheme_name": "public-synthetic orthographic token candidate b-token-split",
      "scheme_version": 1,
      "supersedes_scheme_ref": null,
      "unit_kinds": [
        "surface_token",
        "punctuation",
        "whitespace"
      ]
    }
  ],
  "segmentations": [
    {
      "competing_segmentation_refs": [
        "tos.text-segmentation.sid-c3c9b3cf24dff7621dd2679375e566db"
      ],
      "coverage": {
        "coverage_posture": "exhaustive_nonoverlapping",
        "excluded_anchor_refs": [],
        "overlap_requires_declaration": true,
        "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
        "source_reconstruction_required": true,
        "unreported_gaps_allowed": false
      },
      "declared_uses": [
        "linguistic_analysis",
        "translation_alignment"
      ],
      "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
      "linguistic_authority": false,
      "maker": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic segmentation result b-sentence-whole",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "ordered_unit_refs": [
        "tos.text-unit.sid-14eb4e66ad881bcd5ff0930f9a5145a6",
        "tos.text-unit.sid-b72bb948a7fda9f521d0cfd5e9482f72",
        "tos.text-unit.sid-eaa6d7f9624d919ee30de16ec53a3210",
        "tos.text-unit.sid-ff8375406392ee0fce1600eae1078f07"
      ],
      "review_refs": [],
      "scheme_ref": "tos.text-unit-scheme.sid-d03b0064030838d64314142f422b3ec1",
      "segmentation_id": "tos.text-segmentation.sid-ca2377237f5f7340bb8f1387a7666a7d",
      "segmentation_version": 1,
      "semantic_authority": false,
      "source_text_authority": false,
      "status": "ambiguous",
      "status_reason": "Whole-first-line sentence candidate remains unresolved.",
      "supersedes_segmentation_ref": null
    },
    {
      "competing_segmentation_refs": [
        "tos.text-segmentation.sid-ca2377237f5f7340bb8f1387a7666a7d"
      ],
      "coverage": {
        "coverage_posture": "exhaustive_nonoverlapping",
        "excluded_anchor_refs": [],
        "overlap_requires_declaration": true,
        "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
        "source_reconstruction_required": true,
        "unreported_gaps_allowed": false
      },
      "declared_uses": [
        "linguistic_analysis",
        "translation_alignment"
      ],
      "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
      "linguistic_authority": false,
      "maker": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic segmentation result b-sentence-split",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "ordered_unit_refs": [
        "tos.text-unit.sid-553b9ef8a0b061d679b5e66bdafd2256",
        "tos.text-unit.sid-05baec3c94d517d85d4fc35fc260b24a",
        "tos.text-unit.sid-a1ff64ad7aec3b9e23ee20eeffe7a604",
        "tos.text-unit.sid-278593677455b903b32f74e68d36a9b3",
        "tos.text-unit.sid-79b95920e5564abf48c73ca526ffb9c4",
        "tos.text-unit.sid-66d55bc59401c353829778def6ed7b97"
      ],
      "review_refs": [],
      "scheme_ref": "tos.text-unit-scheme.sid-0221652f661431c9a0c182d0b626f381",
      "segmentation_id": "tos.text-segmentation.sid-c3c9b3cf24dff7621dd2679375e566db",
      "segmentation_version": 1,
      "semantic_authority": false,
      "source_text_authority": false,
      "status": "ambiguous",
      "status_reason": "Split-first-line sentence candidate remains unresolved.",
      "supersedes_segmentation_ref": null
    },
    {
      "competing_segmentation_refs": [
        "tos.text-segmentation.sid-ba3324161b0a2f253e49cbc6b0f588a2"
      ],
      "coverage": {
        "coverage_posture": "exhaustive_nonoverlapping",
        "excluded_anchor_refs": [],
        "overlap_requires_declaration": true,
        "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
        "source_reconstruction_required": true,
        "unreported_gaps_allowed": false
      },
      "declared_uses": [
        "linguistic_analysis",
        "model_input"
      ],
      "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
      "linguistic_authority": false,
      "maker": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic segmentation result b-token-hyphen-whole",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "ordered_unit_refs": [
        "tos.text-unit.sid-4a14f255dfc87d9b33f8cfee3ed4b751",
        "tos.text-unit.sid-ceac6680e6288b372e7c8f19826acc31",
        "tos.text-unit.sid-16518b36745175f1ef6c37e6956c5464",
        "tos.text-unit.sid-d57c9791b147c41f79c20e7031a03eb6",
        "tos.text-unit.sid-061c6a1bd4c8c23153f68bd50520fbe8",
        "tos.text-unit.sid-8eb5abee444b7d7c004e1fa61cb6493e",
        "tos.text-unit.sid-5e2d1158ee052b02cec67744cfe5b482",
        "tos.text-unit.sid-c9458883da2088c03e8b2ad9f3a391ad",
        "tos.text-unit.sid-d58517d329244c71c85e9bbe393bc9d7",
        "tos.text-unit.sid-9f93ef373a2a38d2e6f011b540b959d0",
        "tos.text-unit.sid-f2401d18ecde9d3bc3386d08f94c299b",
        "tos.text-unit.sid-f1e31dea552c06e4a6826924113ab30c",
        "tos.text-unit.sid-9bcda7bec32b443de4411f2f281cfe2b",
        "tos.text-unit.sid-8f4b1462275fb5b90330f25cf153701c",
        "tos.text-unit.sid-7e3c74e459bab3ecec8faee6cd9cc62a",
        "tos.text-unit.sid-58007cac44f69709c926a93c2dfd1fe9",
        "tos.text-unit.sid-86992423b36f82aa90eac21411175e1d",
        "tos.text-unit.sid-224e9325d2407af77dd085bd6999185d"
      ],
      "review_refs": [],
      "scheme_ref": "tos.text-unit-scheme.sid-9d544a918b9032759857d91121e1714d",
      "segmentation_id": "tos.text-segmentation.sid-0100b314e4a9eba42d99c1872101a398",
      "segmentation_version": 1,
      "semantic_authority": false,
      "source_text_authority": false,
      "status": "ambiguous",
      "status_reason": "Hyphenated surface-token candidate remains unresolved.",
      "supersedes_segmentation_ref": null
    },
    {
      "competing_segmentation_refs": [
        "tos.text-segmentation.sid-0100b314e4a9eba42d99c1872101a398"
      ],
      "coverage": {
        "coverage_posture": "exhaustive_nonoverlapping",
        "excluded_anchor_refs": [],
        "overlap_requires_declaration": true,
        "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
        "source_reconstruction_required": true,
        "unreported_gaps_allowed": false
      },
      "declared_uses": [
        "linguistic_analysis",
        "model_input"
      ],
      "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
      "linguistic_authority": false,
      "maker": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic segmentation result b-token-hyphen-split",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "ordered_unit_refs": [
        "tos.text-unit.sid-6c4f0f1bd4d2dfda3f7625b24422f6b2",
        "tos.text-unit.sid-5f8b9aa3f927488225a0a0c61a086e0a",
        "tos.text-unit.sid-e368559cb88bb418bf3e1ec36e9cc2e3",
        "tos.text-unit.sid-0b57cfd96ebe646d37aa24ced101c75c",
        "tos.text-unit.sid-e9762bb11dade57372ce14d3397bc69c",
        "tos.text-unit.sid-3350daefb900c3d9fc9fcb936ea73ae2",
        "tos.text-unit.sid-ef32e3ef68d4bcfa7ae203dea169a482",
        "tos.text-unit.sid-d9a924a37baaed16ffe2ac4b3b64941e",
        "tos.text-unit.sid-b99a14ddd2dd34b5e0b1435f9fd7b03a",
        "tos.text-unit.sid-b430cdcf39a587f1cf02c5c5fe1048d8",
        "tos.text-unit.sid-01cf3955c8ee1d9750b7df51c3bb3488",
        "tos.text-unit.sid-6c55109ab05a5eadf268ca4c35e4897a",
        "tos.text-unit.sid-e186ef86499ee22931e538db57e45378",
        "tos.text-unit.sid-54176e02d2d1b0c34a1451e00d6279d2",
        "tos.text-unit.sid-2276f6e59c0866078b09b60770c64902",
        "tos.text-unit.sid-0720f5e98fe9e27cdb296bc9811fb69a",
        "tos.text-unit.sid-6a288be7ae208a262e3ead7e739b905b",
        "tos.text-unit.sid-d7f52d4ea1ae519c18a3eb4e7f27e7f7",
        "tos.text-unit.sid-bb083dd126b6b7d1b722d272432d7731",
        "tos.text-unit.sid-d912a55852d85de1b95fd8a9dbfd333c"
      ],
      "review_refs": [],
      "scheme_ref": "tos.text-unit-scheme.sid-2e32e80b443e1f865666b5f044290fd7",
      "segmentation_id": "tos.text-segmentation.sid-ba3324161b0a2f253e49cbc6b0f588a2",
      "segmentation_version": 1,
      "semantic_authority": false,
      "source_text_authority": false,
      "status": "ambiguous",
      "status_reason": "Split-hyphen surface-token candidate remains unresolved.",
      "supersedes_segmentation_ref": null
    }
  ],
  "source_layer": {
    "immutable": true,
    "interval": "half_open",
    "language": "x-tos-unit",
    "media_type": "text/plain; charset=utf-8",
    "position_unit": "unicode_code_point",
    "publication_authorized": true,
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
    "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
    "unicode_form": "source_preserved",
    "visibility": "public"
  },
  "source_scope": {
    "edition_ref": "tos.edition.source-text-unit-v1.synthetic-miniature",
    "expression_ref": "tos.expression.source-text-unit-v1.synthetic-miniature",
    "file_ref": "tos.file.source-text-unit-v1.synthetic-miniature.utf8",
    "file_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
    "item_ref": "tos.item.source-text-unit-v1.synthetic-miniature",
    "work_ref": "tos.work.source-text-unit-v1.synthetic-miniature"
  },
  "supersedes_packet_ref": null,
  "units": [
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-14eb4e66ad881bcd5ff0930f9a5145a6",
      "unit_kind": "sentence",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-break-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-b72bb948a7fda9f521d0cfd5e9482f72",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-eaa6d7f9624d919ee30de16ec53a3210",
      "unit_kind": "sentence",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-break-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-ff8375406392ee0fce1600eae1078f07",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-split-sentence-1a"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-553b9ef8a0b061d679b5e66bdafd2256",
      "unit_kind": "sentence",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-split-space-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-05baec3c94d517d85d4fc35fc260b24a",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-split-sentence-1b"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-a1ff64ad7aec3b9e23ee20eeffe7a604",
      "unit_kind": "sentence",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-split-break-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-278593677455b903b32f74e68d36a9b3",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-split-sentence-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-79b95920e5564abf48c73ca526ffb9c4",
      "unit_kind": "sentence",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-split-break-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-66d55bc59401c353829778def6ed7b97",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-mark-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-4a14f255dfc87d9b33f8cfee3ed4b751",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-ceac6680e6288b372e7c8f19826acc31",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-a"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-16518b36745175f1ef6c37e6956c5464",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-period-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-d57c9791b147c41f79c20e7031a03eb6",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-061c6a1bd4c8c23153f68bd50520fbe8",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-ember-turns"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-8eb5abee444b7d7c004e1fa61cb6493e",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-period-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-5e2d1158ee052b02cec67744cfe5b482",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-break-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-c9458883da2088c03e8b2ad9f3a391ad",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-mark-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-d58517d329244c71c85e9bbe393bc9d7",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-3"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-9f93ef373a2a38d2e6f011b540b959d0",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-b"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-f2401d18ecde9d3bc3386d08f94c299b",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-colon"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-f1e31dea552c06e4a6826924113ab30c",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-4"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-9bcda7bec32b443de4411f2f281cfe2b",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-ember-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-8f4b1462275fb5b90330f25cf153701c",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-space-5"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-7e3c74e459bab3ecec8faee6cd9cc62a",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-turns-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-58007cac44f69709c926a93c2dfd1fe9",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-period-3"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-86992423b36f82aa90eac21411175e1d",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-whole-break-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-224e9325d2407af77dd085bd6999185d",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-mark-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-6c4f0f1bd4d2dfda3f7625b24422f6b2",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-5f8b9aa3f927488225a0a0c61a086e0a",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-a"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-e368559cb88bb418bf3e1ec36e9cc2e3",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-period-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-0b57cfd96ebe646d37aa24ced101c75c",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-e9762bb11dade57372ce14d3397bc69c",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-ember-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-3350daefb900c3d9fc9fcb936ea73ae2",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-hyphen"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-ef32e3ef68d4bcfa7ae203dea169a482",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-turns-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-d9a924a37baaed16ffe2ac4b3b64941e",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-period-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-b99a14ddd2dd34b5e0b1435f9fd7b03a",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-break-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-b430cdcf39a587f1cf02c5c5fe1048d8",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-mark-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-01cf3955c8ee1d9750b7df51c3bb3488",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-3"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-6c55109ab05a5eadf268ca4c35e4897a",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-b"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-e186ef86499ee22931e538db57e45378",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-colon"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-54176e02d2d1b0c34a1451e00d6279d2",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-4"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-2276f6e59c0866078b09b60770c64902",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-ember-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-0720f5e98fe9e27cdb296bc9811fb69a",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-space-5"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-6a288be7ae208a262e3ead7e739b905b",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-turns-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-d7f52d4ea1ae519c18a3eb4e7f27e7f7",
      "unit_kind": "surface_token",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-period-3"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-bb083dd126b6b7d1b722d272432d7731",
      "unit_kind": "punctuation",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-token-split-break-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-d912a55852d85de1b95fd8a9dbfd333c",
      "unit_kind": "whitespace",
      "unit_version": 1
    }
  ]
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-c-invalid-acceptance.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
  "anchors": [
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
      "anchor_role": "scope",
      "exact_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
      "ordinal": 1,
      "selector": {
        "end": 42,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-1",
      "anchor_role": "content",
      "exact_sha256": "18d75c93d7be519be9df557867cedcc1f3259d3020a55776fc848835a902ef98",
      "ordinal": 2,
      "selector": {
        "end": 20,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 0,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-break-1",
      "anchor_role": "whitespace",
      "exact_sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
      "ordinal": 3,
      "selector": {
        "end": 21,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 20,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    },
    {
      "anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-2",
      "anchor_role": "content",
      "exact_sha256": "763ebbc06bd6bc83d28a1eb5a0b0530df4c0e93a4f76049b52deafa04201296c",
      "ordinal": 4,
      "selector": {
        "end": 41,
        "interval": "half_open",
        "position_unit": "unicode_code_point",
        "start": 21,
        "type": "text_position"
      },
      "source_return": {
        "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "required": true
      },
      "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
      "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
    }
  ],
  "authority_boundary": {
    "graph_role": "relation",
    "legacy_bulk_migration_authorized": false,
    "model_token_is_not_linguistic_token": true,
    "projection_is_owner_truth": false,
    "segmentation_is_method_result_not_source_truth": true,
    "source_role": "authority",
    "source_unit_is_not_lexeme_sign_or_concept": true,
    "tree_role": "orientation",
    "validators_prove_mechanics_not_truth": true
  },
  "content_posture": "public_synthetic_contract_exercise",
  "packet_id": "tos.source-text-unit-packet.sid-886d35f7944901f61abd8e467360e5ac",
  "packet_version": 1,
  "projections": [],
  "reviews": [],
  "rights_and_visibility": {
    "effective_visibility": "public",
    "inheritance_policy": "most-restrictive-source-packet-and-destination-wins",
    "packet_visibility": "public",
    "private_source_used": false,
    "publication_authorized": true,
    "rights_record_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json"
    ],
    "source_visibility": "public"
  },
  "schema_version": "tos_source_text_unit_packet_v1",
  "schemes": [
    {
      "analysis_role": "linguistic",
      "authority_limit": {
        "algorithmic_output_is_linguistic_truth": false,
        "algorithmic_output_is_source_truth": false,
        "model_subword_is_lexeme": false,
        "text_mutation_allowed": false,
        "unit_identity_is_semantic_identity": false
      },
      "boundary_basis": "public_synthetic_fixture",
      "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
      "method": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method_name": "public-synthetic sentence candidate b-whole",
        "method_version": "1",
        "model_ref": null,
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "policies": {
        "hyphenation": "preserve_source",
        "line_break": "standalone_units",
        "normalization": "no-text-mutation-separate-successor-layer",
        "overlap": "forbid",
        "punctuation": "included_in_neighbor",
        "unreported_gaps_allowed": false,
        "whitespace": "standalone_units"
      },
      "scheme_id": "tos.text-unit-scheme.sid-d03b0064030838d64314142f422b3ec1",
      "scheme_name": "public-synthetic sentence candidate b-whole",
      "scheme_version": 1,
      "supersedes_scheme_ref": null,
      "unit_kinds": [
        "sentence",
        "whitespace"
      ]
    }
  ],
  "segmentations": [
    {
      "competing_segmentation_refs": [],
      "coverage": {
        "coverage_posture": "exhaustive_nonoverlapping",
        "excluded_anchor_refs": [],
        "overlap_requires_declaration": true,
        "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
        "source_reconstruction_required": true,
        "unreported_gaps_allowed": false
      },
      "declared_uses": [
        "linguistic_analysis",
        "translation_alignment"
      ],
      "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
      "linguistic_authority": false,
      "maker": {
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "configuration_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
        "locale": "x-tos-unit",
        "made_at": "2026-08-11T18:00:00Z",
        "maker_kind": "model",
        "method_name": "invalid model-shaped sentence boundary output",
        "method_version": "synthetic-invalid-control",
        "model_ref": "model:synthetic-invalid-control-not-invoked",
        "output_posture": "method_result_not_source_or_linguistic_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "software_refs": [
          "scripts/build_source_text_unit_v1_lab.py"
        ],
        "tailoring_ref": null,
        "unicode_revision": 47,
        "unicode_version": "17.0.0"
      },
      "ordered_unit_refs": [
        "tos.text-unit.sid-14eb4e66ad881bcd5ff0930f9a5145a6",
        "tos.text-unit.sid-b72bb948a7fda9f521d0cfd5e9482f72",
        "tos.text-unit.sid-eaa6d7f9624d919ee30de16ec53a3210"
      ],
      "review_refs": [],
      "scheme_ref": "tos.text-unit-scheme.sid-d03b0064030838d64314142f422b3ec1",
      "segmentation_id": "tos.text-segmentation.sid-ea5a29c5f1502564f22544229f032610",
      "segmentation_version": 1,
      "semantic_authority": false,
      "source_text_authority": false,
      "status": "accepted",
      "status_reason": "Invalid control: a synthetic model-shaped result claims acceptance without review. Removed unit tos.text-unit.sid-ff8375406392ee0fce1600eae1078f07.",
      "supersedes_segmentation_ref": null
    }
  ],
  "source_layer": {
    "immutable": true,
    "interval": "half_open",
    "language": "x-tos-unit",
    "media_type": "text/plain; charset=utf-8",
    "position_unit": "unicode_code_point",
    "publication_authorized": true,
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
    "text_layer_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
    "unicode_form": "source_preserved",
    "visibility": "public"
  },
  "source_scope": {
    "edition_ref": "tos.edition.source-text-unit-v1.synthetic-miniature",
    "expression_ref": "tos.expression.source-text-unit-v1.synthetic-miniature",
    "file_ref": "tos.file.source-text-unit-v1.synthetic-miniature.utf8",
    "file_sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3",
    "item_ref": "tos.item.source-text-unit-v1.synthetic-miniature",
    "work_ref": "tos.work.source-text-unit-v1.synthetic-miniature"
  },
  "supersedes_packet_ref": null,
  "units": [
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-14eb4e66ad881bcd5ff0930f9a5145a6",
      "unit_kind": "sentence",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-break-1"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-b72bb948a7fda9f521d0cfd5e9482f72",
      "unit_kind": "whitespace",
      "unit_version": 1
    },
    {
      "boundary_posture": "method_proposed",
      "certainty": {
        "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        "value": 0.5
      },
      "continuity": "contiguous",
      "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
      "ordered_anchor_refs": [
        "tos.anchor.source-text-unit-v1.synthetic-b-whole-sentence-2"
      ],
      "ordered_child_unit_refs": [],
      "parent_unit_refs": [],
      "semantic_promotion": false,
      "source_text_mutated": false,
      "status_reason": "Public-synthetic unit exists only to exercise exact boundary mechanics.",
      "supersedes_unit_ref": null,
      "surface_posture": "source_bearing",
      "unit_id": "tos.text-unit.sid-eaa6d7f9624d919ee30de16ec53a3210",
      "unit_kind": "sentence",
      "unit_version": 1
    }
  ]
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/lab.manifest.json",
                    r####"{
  "authority_limits": {
    "canon_effect": false,
    "graph_truth_established": false,
    "human_review_performed": false,
    "legacy_migration_performed": false,
    "lexeme_established": false,
    "linguistic_word_established": false,
    "model_invoked": false,
    "private_source_used": false,
    "real_language_boundary_accepted": false,
    "semantic_truth_established": false
  },
  "authority_posture": "public_synthetic_contract_mechanics_only",
  "builder": {
    "ref": "scripts/build_source_text_unit_v1_lab.py",
    "sha256": "4da34c86754934fe44b0e24188cd1016df2a296101c63d163d7587108cf9c930"
  },
  "contract": {
    "ref": "ToS/contracts/source-text-unit-packet-v1.schema.json",
    "sha256": "6ba96f4298cf5e88ae261c53c46c7d51681d0f0f0faf3bcecce2e309a0e071ab"
  },
  "plan": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/plan.json",
    "sha256": "0b9434f0e21f20073b23d85faa002f8a684ab40c9c694628349b4491baa627e8"
  },
  "research": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/SOURCE_TEXT_UNIT_SEGMENTATION_RESEARCH_2026-08-11.md",
    "sha256": "ff73e7a7053f7eeb710d1d183b2ecab08401cdba3604c82843ba37556ffcb9bc"
  },
  "schema_version": "tos_source_text_unit_v1_lab_manifest_v1",
  "source": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
    "sha256": "312864bbb2666e8cb5130797996afcf8debb5b4fb3234e4cad81bcb82bff53b3"
  },
  "variants": [
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-a-source-layout-observation.json",
      "packet_sha256": "c7065c753a9b7b1df46c579041d39cdaa89db007408bf3d597f8539d1c586de4",
      "variant_id": "A"
    },
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-b-competing-segmentations.json",
      "packet_sha256": "653f3663aa1fc04f20a8f5d2e412dc5b19e121fada1f584837f36d9a06142e3b",
      "variant_id": "B"
    },
    {
      "expected_schema_valid": false,
      "expected_semantic_valid": false,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-c-invalid-acceptance.json",
      "packet_sha256": "5edca499ad3d1775b043ed51c33179ebfab503496a16205e4b90166326b0ae53",
      "variant_id": "C"
    }
  ]
}
"####,
                ),
            ],
        }),
        "translation-alignment-v1" => Ok(Recipe {
            legacy: "scripts/build_translation_alignment_v1_lab.py",
            manifest: "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/lab.manifest.json",
            contract: "ToS/contracts/translation-alignment-packet-v1.schema.json",
            research: "ToS/research-packets/foundation-laboratory-2026-07/TRANSLATION_ALIGNMENT_IDENTITY_RESEARCH_2026-08-11.md",
            lab: SourceFoundationLab::TranslationAlignmentV1,
            files: &[
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
                    r####"ember-one remains.
ember-two turns.
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
                    r####"echo-one stays.
echo-two shifts and opens.
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-segmentation.json",
                    r####"{
  "analysis_kind": "segmentation",
  "authority": "synthetic mechanical fixture only",
  "schema_version": "tos_translation_alignment_synthetic_analysis_v1",
  "side": "source",
  "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
  "source_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
  "state": "frozen"
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-segmentation.json",
                    r####"{
  "analysis_kind": "segmentation",
  "authority": "synthetic mechanical fixture only",
  "schema_version": "tos_translation_alignment_synthetic_analysis_v1",
  "side": "target",
  "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
  "source_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
  "state": "frozen"
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-tokenization.json",
                    r####"{
  "analysis_kind": "tokenization",
  "authority": "synthetic mechanical fixture only",
  "schema_version": "tos_translation_alignment_synthetic_analysis_v1",
  "side": "source",
  "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
  "source_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
  "state": "frozen"
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-tokenization.json",
                    r####"{
  "analysis_kind": "tokenization",
  "authority": "synthetic mechanical fixture only",
  "schema_version": "tos_translation_alignment_synthetic_analysis_v1",
  "side": "target",
  "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
  "source_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
  "state": "frozen"
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
                    r####"{
  "authority_limits": {
    "accepted_alignment_established": false,
    "aligner_invoked": false,
    "canon_effect": false,
    "graph_truth_established": false,
    "human_review_performed": false,
    "lexical_equivalence_established": false,
    "model_invoked": false,
    "private_source_used": false,
    "semantic_truth_established": false,
    "translation_performed": false
  },
  "input_posture": "public_synthetic_only",
  "languages": [
    "x-tos-src",
    "x-tos-tgt"
  ],
  "negative_controls": [
    "text-derived-alignment-id",
    "duplicate-alignment-id",
    "duplicate-claim-id",
    "missing-source-anchor",
    "missing-target-anchor",
    "source-layer-digest-drift",
    "target-layer-digest-drift",
    "naked-unresolved-anchor",
    "shape-cardinality-mismatch",
    "source-omission-with-target-members",
    "target-addition-with-source-members",
    "one-way-competing-mapping",
    "accepted-model-proposal-without-review",
    "accepted-without-language-competence",
    "projection-of-proposed-alignment",
    "projection-visibility-widening",
    "self-supersession"
  ],
  "question": "Can ToS bind exact bilingual sides and preserve competing mappings without turning a method output into translation truth?",
  "schema_version": "tos_translation_alignment_v1_lab_plan_v1",
  "variants": [
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "purpose": "One deterministic one-to-one structural proposal",
      "variant_id": "A"
    },
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "purpose": "Reciprocal one-to-one and one-to-many competing proposals",
      "variant_id": "B"
    },
    {
      "expected_schema_valid": false,
      "expected_semantic_valid": false,
      "purpose": "Invalid synthetic/model-shaped acceptance without review or competence",
      "variant_id": "C"
    }
  ]
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-a-one-to-one-proposal.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json",
  "alignments": [
    {
      "alignment_id": "tos.translation-alignment.sid-07aa1bb0118735d997fd03a91c9c7865",
      "alignment_version": 1,
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.5
      },
      "claim_id": "tos.translation-alignment-claim.sid-8d4a15786446aa295888a356dd199ea3",
      "claim_version": 1,
      "competing_alignment_refs": [],
      "correspondence_shape": "one_to_one",
      "direction": "source_to_target",
      "epistemic_status": "observed_structure",
      "evidence": [
        {
          "description": "Exact invented spans are present only to exercise ordered stand-off mapping mechanics.",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
          "role": "method_input",
          "source_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-source-one"
          ],
          "target_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-target-one"
          ]
        }
      ],
      "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
      "maker": {
        "agent_ref": "software:tos-translation-alignment-v1-lab-builder",
        "made_at": "2026-08-11T14:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic mapping fixture construction",
        "method_output_posture": "proposal_not_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, model, translation, or review act occurred"
      },
      "order_posture": "monotonic",
      "ordered_source_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-source-one"
      ],
      "ordered_target_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-target-one"
      ],
      "review_refs": [],
      "status": "proposed",
      "status_reason": "Public-synthetic proposal exercises mapping mechanics and establishes no translation truth.",
      "supersedes_alignment_ref": null,
      "supersedes_claim_ref": null,
      "translation_techniques": [
        "unresolved"
      ]
    }
  ],
  "authority_boundary": {
    "alignment_is_claim_not_translation_truth": true,
    "canon_effect": false,
    "exports_are_not_owner_truth": true,
    "graph_role": "relation",
    "lexical_equivalence_not_inferred": true,
    "machine_or_model_may_propose_not_accept": true,
    "source_role": "authority",
    "tree_role": "orientation",
    "unaligned_members_are_first_class": true,
    "validators_prove_mechanics_not_truth": true
  },
  "content_posture": "public_synthetic_contract_exercise",
  "granularity": "mixed",
  "packet_id": "tos.translation-alignment-packet.sid-e9e9adbc7950449b519456df5241f0c0",
  "packet_version": 1,
  "projections": [],
  "reviews": [],
  "rights_and_visibility": {
    "effective_visibility": "public",
    "inheritance_policy": "most_restrictive_side_or_packet_wins",
    "packet_visibility": "public",
    "private_source_used": false,
    "publication_authorized": true,
    "rights_record_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "source_visibility": "public",
    "target_visibility": "public"
  },
  "schema_version": "tos_translation_alignment_packet_v1",
  "source_side": {
    "anchors": [
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-source-one",
        "exact_sha256": "002826403682778533cd800ace70b8b5f096cd0a95741f588e4a7bcde810c1cc",
        "ordinal": 1,
        "selector": {
          "end": 18,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 0,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
        "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-source-two",
        "exact_sha256": "ed26907b651487d367a1b1ae65c51412d05e6665ba64c384b6f721f1c50a9c64",
        "ordinal": 2,
        "selector": {
          "end": 35,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 19,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
        "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
      }
    ],
    "edition_ref": "tos.edition.translation-alignment-v1.synthetic-miniature.source",
    "expression_ref": "tos.expression.translation-alignment-v1.synthetic-miniature.source",
    "file_ref": "tos.file.translation-alignment-v1.synthetic-miniature.source.utf8",
    "file_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
    "item_ref": "tos.item.translation-alignment-v1.synthetic-miniature.source",
    "language": "x-tos-src",
    "publication_authorized": true,
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "segmentation": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-segmentation.json",
      "sha256": "3500417ea57222ede1575fe7039965293466cf3cd7f2f62b0e49110641c7a453",
      "state": "frozen"
    },
    "side_role": "source",
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
    "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
    "tokenization": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-tokenization.json",
      "sha256": "fa81efa5b6d61a1ea27c395326214f01730990116244900af0de2cac4c46c157",
      "state": "frozen"
    },
    "visibility": "public",
    "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature"
  },
  "supersedes_packet_ref": null,
  "target_side": {
    "anchors": [
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-one",
        "exact_sha256": "ee17a53dbdac53ce14195a30d5924153011bf2fe143c6f224d702e6d94d98752",
        "ordinal": 1,
        "selector": {
          "end": 15,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 0,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-whole",
        "exact_sha256": "a105d074c679a0fa88a81ced8554fbcc5a6c79569baa835a5ff0f4d73e78dde5",
        "ordinal": 2,
        "selector": {
          "end": 42,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 16,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
        "exact_sha256": "1d632cf9cb195eafce3048141962b4818fe50574f94e9118329ec3651ec70a24",
        "ordinal": 3,
        "selector": {
          "end": 31,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 16,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b",
        "exact_sha256": "1a5de230ee2acd014581ec19a1534009880732b74594f7aac9085663a3745aa5",
        "ordinal": 4,
        "selector": {
          "end": 42,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 32,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      }
    ],
    "edition_ref": "tos.edition.translation-alignment-v1.synthetic-miniature.target",
    "expression_ref": "tos.expression.translation-alignment-v1.synthetic-miniature.target",
    "file_ref": "tos.file.translation-alignment-v1.synthetic-miniature.target.utf8",
    "file_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
    "item_ref": "tos.item.translation-alignment-v1.synthetic-miniature.target",
    "language": "x-tos-tgt",
    "publication_authorized": true,
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "segmentation": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-segmentation.json",
      "sha256": "c9d46ea0722a1ae1c36a3990f70968acbd96b43b01afa787db4a3ce3270fafe2",
      "state": "frozen"
    },
    "side_role": "target",
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
    "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
    "tokenization": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-tokenization.json",
      "sha256": "f79f066ccca7e633465e011f8f2ada0cad7c83409df8bcb82b389634ba018720",
      "state": "frozen"
    },
    "visibility": "public",
    "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature"
  }
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-b-competing-mappings.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json",
  "alignments": [
    {
      "alignment_id": "tos.translation-alignment.sid-2b4c976a205b3a983696b6a205981564",
      "alignment_version": 1,
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.5
      },
      "claim_id": "tos.translation-alignment-claim.sid-5d6273060edb4bcdcc0ec3e274840339",
      "claim_version": 1,
      "competing_alignment_refs": [
        "tos.translation-alignment.sid-93821a1c1b9e77415d716df04f9f8cc3"
      ],
      "correspondence_shape": "one_to_one",
      "direction": "source_to_target",
      "epistemic_status": "observed_structure",
      "evidence": [
        {
          "description": "Exact invented spans are present only to exercise ordered stand-off mapping mechanics.",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
          "role": "method_input",
          "source_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-source-two"
          ],
          "target_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-target-two-whole"
          ]
        }
      ],
      "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
      "maker": {
        "agent_ref": "software:tos-translation-alignment-v1-lab-builder",
        "made_at": "2026-08-11T14:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic mapping fixture construction",
        "method_output_posture": "proposal_not_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, model, translation, or review act occurred"
      },
      "order_posture": "monotonic",
      "ordered_source_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-source-two"
      ],
      "ordered_target_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-target-two-whole"
      ],
      "review_refs": [],
      "status": "proposed",
      "status_reason": "Public-synthetic proposal exercises mapping mechanics and establishes no translation truth.",
      "supersedes_alignment_ref": null,
      "supersedes_claim_ref": null,
      "translation_techniques": [
        "unresolved"
      ]
    },
    {
      "alignment_id": "tos.translation-alignment.sid-93821a1c1b9e77415d716df04f9f8cc3",
      "alignment_version": 1,
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.5
      },
      "claim_id": "tos.translation-alignment-claim.sid-035353c476774772d31ad130b157e533",
      "claim_version": 1,
      "competing_alignment_refs": [
        "tos.translation-alignment.sid-2b4c976a205b3a983696b6a205981564"
      ],
      "correspondence_shape": "one_to_many",
      "direction": "source_to_target",
      "epistemic_status": "observed_structure",
      "evidence": [
        {
          "description": "Exact invented spans are present only to exercise ordered stand-off mapping mechanics.",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
          "role": "method_input",
          "source_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-source-two"
          ],
          "target_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
            "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b"
          ]
        }
      ],
      "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
      "maker": {
        "agent_ref": "software:tos-translation-alignment-v1-lab-builder",
        "made_at": "2026-08-11T14:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic mapping fixture construction",
        "method_output_posture": "proposal_not_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, model, translation, or review act occurred"
      },
      "order_posture": "monotonic",
      "ordered_source_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-source-two"
      ],
      "ordered_target_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
        "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b"
      ],
      "review_refs": [],
      "status": "proposed",
      "status_reason": "Public-synthetic proposal exercises mapping mechanics and establishes no translation truth.",
      "supersedes_alignment_ref": null,
      "supersedes_claim_ref": null,
      "translation_techniques": [
        "unresolved"
      ]
    }
  ],
  "authority_boundary": {
    "alignment_is_claim_not_translation_truth": true,
    "canon_effect": false,
    "exports_are_not_owner_truth": true,
    "graph_role": "relation",
    "lexical_equivalence_not_inferred": true,
    "machine_or_model_may_propose_not_accept": true,
    "source_role": "authority",
    "tree_role": "orientation",
    "unaligned_members_are_first_class": true,
    "validators_prove_mechanics_not_truth": true
  },
  "content_posture": "public_synthetic_contract_exercise",
  "granularity": "mixed",
  "packet_id": "tos.translation-alignment-packet.sid-59abf8308b222271ab03e5b2d8db6039",
  "packet_version": 1,
  "projections": [],
  "reviews": [],
  "rights_and_visibility": {
    "effective_visibility": "public",
    "inheritance_policy": "most_restrictive_side_or_packet_wins",
    "packet_visibility": "public",
    "private_source_used": false,
    "publication_authorized": true,
    "rights_record_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "source_visibility": "public",
    "target_visibility": "public"
  },
  "schema_version": "tos_translation_alignment_packet_v1",
  "source_side": {
    "anchors": [
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-source-one",
        "exact_sha256": "002826403682778533cd800ace70b8b5f096cd0a95741f588e4a7bcde810c1cc",
        "ordinal": 1,
        "selector": {
          "end": 18,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 0,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
        "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-source-two",
        "exact_sha256": "ed26907b651487d367a1b1ae65c51412d05e6665ba64c384b6f721f1c50a9c64",
        "ordinal": 2,
        "selector": {
          "end": 35,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 19,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
        "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
      }
    ],
    "edition_ref": "tos.edition.translation-alignment-v1.synthetic-miniature.source",
    "expression_ref": "tos.expression.translation-alignment-v1.synthetic-miniature.source",
    "file_ref": "tos.file.translation-alignment-v1.synthetic-miniature.source.utf8",
    "file_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
    "item_ref": "tos.item.translation-alignment-v1.synthetic-miniature.source",
    "language": "x-tos-src",
    "publication_authorized": true,
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "segmentation": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-segmentation.json",
      "sha256": "3500417ea57222ede1575fe7039965293466cf3cd7f2f62b0e49110641c7a453",
      "state": "frozen"
    },
    "side_role": "source",
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
    "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
    "tokenization": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-tokenization.json",
      "sha256": "fa81efa5b6d61a1ea27c395326214f01730990116244900af0de2cac4c46c157",
      "state": "frozen"
    },
    "visibility": "public",
    "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature"
  },
  "supersedes_packet_ref": null,
  "target_side": {
    "anchors": [
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-one",
        "exact_sha256": "ee17a53dbdac53ce14195a30d5924153011bf2fe143c6f224d702e6d94d98752",
        "ordinal": 1,
        "selector": {
          "end": 15,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 0,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-whole",
        "exact_sha256": "a105d074c679a0fa88a81ced8554fbcc5a6c79569baa835a5ff0f4d73e78dde5",
        "ordinal": 2,
        "selector": {
          "end": 42,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 16,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
        "exact_sha256": "1d632cf9cb195eafce3048141962b4818fe50574f94e9118329ec3651ec70a24",
        "ordinal": 3,
        "selector": {
          "end": 31,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 16,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b",
        "exact_sha256": "1a5de230ee2acd014581ec19a1534009880732b74594f7aac9085663a3745aa5",
        "ordinal": 4,
        "selector": {
          "end": 42,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 32,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      }
    ],
    "edition_ref": "tos.edition.translation-alignment-v1.synthetic-miniature.target",
    "expression_ref": "tos.expression.translation-alignment-v1.synthetic-miniature.target",
    "file_ref": "tos.file.translation-alignment-v1.synthetic-miniature.target.utf8",
    "file_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
    "item_ref": "tos.item.translation-alignment-v1.synthetic-miniature.target",
    "language": "x-tos-tgt",
    "publication_authorized": true,
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "segmentation": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-segmentation.json",
      "sha256": "c9d46ea0722a1ae1c36a3990f70968acbd96b43b01afa787db4a3ce3270fafe2",
      "state": "frozen"
    },
    "side_role": "target",
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
    "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
    "tokenization": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-tokenization.json",
      "sha256": "f79f066ccca7e633465e011f8f2ada0cad7c83409df8bcb82b389634ba018720",
      "state": "frozen"
    },
    "visibility": "public",
    "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature"
  }
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-c-invalid-acceptance.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json",
  "alignments": [
    {
      "alignment_id": "tos.translation-alignment.sid-2b4c976a205b3a983696b6a205981564",
      "alignment_version": 1,
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.5
      },
      "claim_id": "tos.translation-alignment-claim.sid-5d6273060edb4bcdcc0ec3e274840339",
      "claim_version": 1,
      "competing_alignment_refs": [
        "tos.translation-alignment.sid-93821a1c1b9e77415d716df04f9f8cc3"
      ],
      "correspondence_shape": "one_to_one",
      "direction": "source_to_target",
      "epistemic_status": "observed_structure",
      "evidence": [
        {
          "description": "Exact invented spans are present only to exercise ordered stand-off mapping mechanics.",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
          "role": "method_input",
          "source_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-source-two"
          ],
          "target_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-target-two-whole"
          ]
        }
      ],
      "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
      "maker": {
        "agent_ref": "software:tos-translation-alignment-v1-lab-builder",
        "made_at": "2026-08-11T14:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic mapping fixture construction",
        "method_output_posture": "proposal_not_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, model, translation, or review act occurred"
      },
      "order_posture": "monotonic",
      "ordered_source_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-source-two"
      ],
      "ordered_target_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-target-two-whole"
      ],
      "review_refs": [],
      "status": "accepted",
      "status_reason": "Deliberately invalid: a synthetic/model-shaped proposal attempts acceptance without a real-human review.",
      "supersedes_alignment_ref": null,
      "supersedes_claim_ref": null,
      "translation_techniques": [
        "unresolved"
      ]
    },
    {
      "alignment_id": "tos.translation-alignment.sid-93821a1c1b9e77415d716df04f9f8cc3",
      "alignment_version": 1,
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.5
      },
      "claim_id": "tos.translation-alignment-claim.sid-035353c476774772d31ad130b157e533",
      "claim_version": 1,
      "competing_alignment_refs": [
        "tos.translation-alignment.sid-2b4c976a205b3a983696b6a205981564"
      ],
      "correspondence_shape": "one_to_many",
      "direction": "source_to_target",
      "epistemic_status": "observed_structure",
      "evidence": [
        {
          "description": "Exact invented spans are present only to exercise ordered stand-off mapping mechanics.",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
          "role": "method_input",
          "source_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-source-two"
          ],
          "target_anchor_refs": [
            "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
            "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b"
          ]
        }
      ],
      "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
      "maker": {
        "agent_ref": "software:tos-translation-alignment-v1-lab-builder",
        "made_at": "2026-08-11T14:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic mapping fixture construction",
        "method_output_posture": "proposal_not_truth",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, model, translation, or review act occurred"
      },
      "order_posture": "monotonic",
      "ordered_source_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-source-two"
      ],
      "ordered_target_anchor_refs": [
        "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
        "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b"
      ],
      "review_refs": [],
      "status": "proposed",
      "status_reason": "Public-synthetic proposal exercises mapping mechanics and establishes no translation truth.",
      "supersedes_alignment_ref": null,
      "supersedes_claim_ref": null,
      "translation_techniques": [
        "unresolved"
      ]
    }
  ],
  "authority_boundary": {
    "alignment_is_claim_not_translation_truth": true,
    "canon_effect": false,
    "exports_are_not_owner_truth": true,
    "graph_role": "relation",
    "lexical_equivalence_not_inferred": true,
    "machine_or_model_may_propose_not_accept": true,
    "source_role": "authority",
    "tree_role": "orientation",
    "unaligned_members_are_first_class": true,
    "validators_prove_mechanics_not_truth": true
  },
  "content_posture": "public_synthetic_contract_exercise",
  "granularity": "mixed",
  "packet_id": "tos.translation-alignment-packet.sid-e6916f53efe77944c9ba595563d4c33a",
  "packet_version": 1,
  "projections": [],
  "reviews": [],
  "rights_and_visibility": {
    "effective_visibility": "public",
    "inheritance_policy": "most_restrictive_side_or_packet_wins",
    "packet_visibility": "public",
    "private_source_used": false,
    "publication_authorized": true,
    "rights_record_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "source_visibility": "public",
    "target_visibility": "public"
  },
  "schema_version": "tos_translation_alignment_packet_v1",
  "source_side": {
    "anchors": [
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-source-one",
        "exact_sha256": "002826403682778533cd800ace70b8b5f096cd0a95741f588e4a7bcde810c1cc",
        "ordinal": 1,
        "selector": {
          "end": 18,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 0,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
        "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-source-two",
        "exact_sha256": "ed26907b651487d367a1b1ae65c51412d05e6665ba64c384b6f721f1c50a9c64",
        "ordinal": 2,
        "selector": {
          "end": 35,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 19,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
        "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
      }
    ],
    "edition_ref": "tos.edition.translation-alignment-v1.synthetic-miniature.source",
    "expression_ref": "tos.expression.translation-alignment-v1.synthetic-miniature.source",
    "file_ref": "tos.file.translation-alignment-v1.synthetic-miniature.source.utf8",
    "file_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
    "item_ref": "tos.item.translation-alignment-v1.synthetic-miniature.source",
    "language": "x-tos-src",
    "publication_authorized": true,
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "segmentation": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-segmentation.json",
      "sha256": "3500417ea57222ede1575fe7039965293466cf3cd7f2f62b0e49110641c7a453",
      "state": "frozen"
    },
    "side_role": "source",
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
    "text_layer_sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a",
    "tokenization": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-tokenization.json",
      "sha256": "fa81efa5b6d61a1ea27c395326214f01730990116244900af0de2cac4c46c157",
      "state": "frozen"
    },
    "visibility": "public",
    "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature"
  },
  "supersedes_packet_ref": null,
  "target_side": {
    "anchors": [
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-one",
        "exact_sha256": "ee17a53dbdac53ce14195a30d5924153011bf2fe143c6f224d702e6d94d98752",
        "ordinal": 1,
        "selector": {
          "end": 15,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 0,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-whole",
        "exact_sha256": "a105d074c679a0fa88a81ced8554fbcc5a6c79569baa835a5ff0f4d73e78dde5",
        "ordinal": 2,
        "selector": {
          "end": 42,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 16,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
        "exact_sha256": "1d632cf9cb195eafce3048141962b4818fe50574f94e9118329ec3651ec70a24",
        "ordinal": 3,
        "selector": {
          "end": 31,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 16,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      },
      {
        "anchor_ref": "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b",
        "exact_sha256": "1a5de230ee2acd014581ec19a1534009880732b74594f7aac9085663a3745aa5",
        "ordinal": 4,
        "selector": {
          "end": 42,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 32,
          "type": "text_position"
        },
        "source_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
          "required": true
        },
        "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
        "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
      }
    ],
    "edition_ref": "tos.edition.translation-alignment-v1.synthetic-miniature.target",
    "expression_ref": "tos.expression.translation-alignment-v1.synthetic-miniature.target",
    "file_ref": "tos.file.translation-alignment-v1.synthetic-miniature.target.utf8",
    "file_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
    "item_ref": "tos.item.translation-alignment-v1.synthetic-miniature.target",
    "language": "x-tos-tgt",
    "publication_authorized": true,
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json"
    ],
    "segmentation": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-segmentation.json",
      "sha256": "c9d46ea0722a1ae1c36a3990f70968acbd96b43b01afa787db4a3ce3270fafe2",
      "state": "frozen"
    },
    "side_role": "target",
    "text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
    "text_layer_sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0",
    "tokenization": {
      "artifact_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-tokenization.json",
      "sha256": "f79f066ccca7e633465e011f8f2ada0cad7c83409df8bcb82b389634ba018720",
      "state": "frozen"
    },
    "visibility": "public",
    "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature"
  }
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/lab.manifest.json",
                    r####"{
  "analysis_artifacts": [
    {
      "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-segmentation.json",
      "sha256": "3500417ea57222ede1575fe7039965293466cf3cd7f2f62b0e49110641c7a453"
    },
    {
      "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-segmentation.json",
      "sha256": "c9d46ea0722a1ae1c36a3990f70968acbd96b43b01afa787db4a3ce3270fafe2"
    },
    {
      "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/source-tokenization.json",
      "sha256": "fa81efa5b6d61a1ea27c395326214f01730990116244900af0de2cac4c46c157"
    },
    {
      "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/target-tokenization.json",
      "sha256": "f79f066ccca7e633465e011f8f2ada0cad7c83409df8bcb82b389634ba018720"
    }
  ],
  "authority_limits": {
    "accepted_alignment_established": false,
    "aligner_invoked": false,
    "canon_effect": false,
    "graph_truth_established": false,
    "human_review_performed": false,
    "lexical_equivalence_established": false,
    "model_invoked": false,
    "private_source_used": false,
    "semantic_truth_established": false,
    "translation_performed": false
  },
  "authority_posture": "public_synthetic_contract_mechanics_only",
  "builder": {
    "ref": "scripts/build_translation_alignment_v1_lab.py",
    "sha256": "4faf54fcae1dd7d891caccb88404d3f852f54a7c2c3ff26c937861cff5a3f166"
  },
  "contract": {
    "ref": "ToS/contracts/translation-alignment-packet-v1.schema.json",
    "sha256": "69be2c70d9eac1cd1ccef165d99b5c7709cd2e52f8ec8022548ce33eb7cca130"
  },
  "inputs": [
    {
      "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-source.x-tos-src.txt",
      "sha256": "63a1081d585f8d6f07d51e8c0c09fd54dc36eee7270971b1ad5f5f0e324e5e1a"
    },
    {
      "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/public-synthetic-target.x-tos-tgt.txt",
      "sha256": "2311244ff57ef7e3e0eee3d5aa5601b8f6d6957d3cd1dc68e2b70399278ed6c0"
    }
  ],
  "plan": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/plan.json",
    "sha256": "548e295e334c5051bef01d5e65ac6506ba5b0b4005ae1b94d0827cb95e5b7591"
  },
  "research": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/TRANSLATION_ALIGNMENT_IDENTITY_RESEARCH_2026-08-11.md",
    "sha256": "987077e03e50a98982b29c7e83208036569959680a8b30056c6d6c662760580c"
  },
  "schema_version": "tos_translation_alignment_v1_lab_manifest_v1",
  "variants": [
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-a-one-to-one-proposal.json",
      "packet_sha256": "846529d1d3f75404122bc65eee65bd3d84feb57719b19930db600a32e54d2f50",
      "variant_id": "A"
    },
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-b-competing-mappings.json",
      "packet_sha256": "c5385616d73858d7ed23a542e9ed7f3c35adccfff3c543af0c5d1df13aa0e424",
      "variant_id": "B"
    },
    {
      "expected_schema_valid": false,
      "expected_semantic_valid": false,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-c-invalid-acceptance.json",
      "packet_sha256": "5cc8df0a99bcf3b7a3d394a98f0bbe3777db89d13a9457b9fccfe6d3b3abc185",
      "variant_id": "C"
    }
  ]
}
"####,
                ),
            ],
        }),
        "semantic-annotation-v2" => Ok(Recipe {
            legacy: "scripts/build_semantic_annotation_v2_lab.py",
            manifest: "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/lab.manifest.json",
            contract: "ToS/contracts/semantic-annotation-packet-v2.schema.json",
            research: "ToS/research-packets/foundation-laboratory-2026-07/SEMANTIC_IDENTITY_ANNOTATION_RESEARCH_2026-08-11.md",
            lab: SourceFoundationLab::SemanticAnnotationV2,
            files: &[
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
                    r####"At dawn, the flame kept the record of a promise.
At dusk, the flame became a question rather than an answer.
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json",
                    r####"{
  "authority_limits": {
    "canon_effect": false,
    "concept_established": false,
    "graph_truth_established": false,
    "human_review_performed": false,
    "model_invoked": false,
    "private_source_used": false,
    "semantic_truth_established": false,
    "stable_sign_established": false
  },
  "input_posture": "public_synthetic_only",
  "negative_controls": [
    "label-derived-id",
    "duplicate-entity-id",
    "missing-target-anchor",
    "unresolved-entity-reference",
    "one-way-competing-claim",
    "accepted-model-claim-without-review",
    "sign-promotion-without-baseline-or-competence",
    "accepted-relation-without-accepted-claim",
    "graph-projection-of-proposed-claim",
    "relation-proposition-mismatch",
    "self-supersession",
    "publication-boundary-widening"
  ],
  "question": "Can ToS keep occurrence, sign proposal, competing reading, review, and graph admission identities separate and fail closed?",
  "schema_version": "tos_semantic_annotation_v2_lab_plan_v1",
  "variants": [
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "purpose": "Exact source occurrences only",
      "variant_id": "A"
    },
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "purpose": "Two reciprocal competing sign proposals without review or graph effect",
      "variant_id": "B"
    },
    {
      "expected_schema_valid": false,
      "expected_semantic_valid": false,
      "purpose": "Invalid model-shaped promotion without a real-human review",
      "variant_id": "C"
    }
  ]
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-a-occurrences-only.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/semantic-annotation-packet-v2.schema.json",
  "annotation_id": "tos.annotation.sid-bf14a98e0e50ca32ed782cf275bcd0cf",
  "annotation_version": 1,
  "authority_boundary": {
    "frequency_is_semantic_evidence_not_semantic_proof": true,
    "graph_role": "relation",
    "model_may_propose_but_not_promote_stable_sign": true,
    "promotion_review_posture": "rare-triggered-source-visible-human-checkpoint",
    "routine_human_review_per_occurrence": false,
    "source_role": "authority",
    "tree_role": "orientation",
    "validators_prove_mechanics_not_truth": true
  },
  "claims": [
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 1.0
      },
      "claim_id": "tos.claim.sid-6b7dc1b1994a838760c9b47d3eb76d60",
      "claim_status": "observed",
      "claim_type": "textual_observation",
      "claim_version": 1,
      "competing_claim_refs": [],
      "epistemic_status": "observed",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1"
          ],
          "description": "Exact public-synthetic source span bound by digest and position selector.",
          "direction": "supports",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "support"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "kind": "digest",
          "media_type": "text/plain; charset=utf-8",
          "sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449"
        },
        "predicate": "has_exact_form_digest",
        "subject_ref": "tos.occurrence.semantic-v2.synthetic-flame-1"
      },
      "review_refs": [],
      "stage": "exact_form",
      "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 1.0
      },
      "claim_id": "tos.claim.sid-1752eb7bea713115e9592416a0974ddb",
      "claim_status": "observed",
      "claim_type": "textual_observation",
      "claim_version": 1,
      "competing_claim_refs": [],
      "epistemic_status": "observed",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "Exact public-synthetic source span bound by digest and position selector.",
          "direction": "supports",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "support"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "kind": "digest",
          "media_type": "text/plain; charset=utf-8",
          "sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449"
        },
        "predicate": "has_exact_form_digest",
        "subject_ref": "tos.occurrence.semantic-v2.synthetic-flame-2"
      },
      "review_refs": [],
      "stage": "exact_form",
      "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    }
  ],
  "content_posture": "public_synthetic_contract_exercise",
  "entities": [
    {
      "admission_review_refs": [],
      "admission_status": "observed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "display",
          "value": "flame"
        }
      ],
      "entity_id": "tos.occurrence.semantic-v2.synthetic-flame-1",
      "entity_kind": "occurrence",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1"
        ],
        "claim_refs": [],
        "parent_entity_refs": []
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "observed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "display",
          "value": "flame"
        }
      ],
      "entity_id": "tos.occurrence.semantic-v2.synthetic-flame-2",
      "entity_kind": "occurrence",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [],
        "parent_entity_refs": []
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    }
  ],
  "graph_projection": {
    "edges": [],
    "node_refs": [],
    "posture": "downstream-derived-projection-only",
    "projection_event_refs": [],
    "source_return_required": true
  },
  "relations": [],
  "reviews": [],
  "rights_and_visibility": {
    "private_source_used": false,
    "publication_authorized": true,
    "record_visibility": "public",
    "rights_basis_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json"
    ],
    "source_content_visibility": "public_synthetic"
  },
  "schema_version": "tos_semantic_annotation_packet_v2",
  "source_scope": {
    "expression_ref": "tos.expression.semantic-v2.synthetic-miniature.en",
    "file_ref": "tos.file.semantic-v2.synthetic-miniature.utf8",
    "file_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3",
    "item_ref": "tos.item.semantic-v2.synthetic-miniature.tracked",
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json"
    ],
    "source_anchors": [
      {
        "anchor_ref": "tos.anchor.semantic-v2.synthetic-flame-1",
        "exact_sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449",
        "page_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "required": true
        },
        "selector": {
          "end": 18,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 13,
          "type": "text_position"
        },
        "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
        "source_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
      },
      {
        "anchor_ref": "tos.anchor.semantic-v2.synthetic-flame-2",
        "exact_sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449",
        "page_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "required": true
        },
        "selector": {
          "end": 67,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 62,
          "type": "text_position"
        },
        "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
        "source_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
      }
    ],
    "source_text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
    "work_ref": "tos.work.semantic-v2.synthetic-miniature"
  },
  "supersedes_annotation_ref": null
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/semantic-annotation-packet-v2.schema.json",
  "annotation_id": "tos.annotation.sid-7f9cb35b759ea3f9ebc73376ab953f96",
  "annotation_version": 1,
  "authority_boundary": {
    "frequency_is_semantic_evidence_not_semantic_proof": true,
    "graph_role": "relation",
    "model_may_propose_but_not_promote_stable_sign": true,
    "promotion_review_posture": "rare-triggered-source-visible-human-checkpoint",
    "routine_human_review_per_occurrence": false,
    "source_role": "authority",
    "tree_role": "orientation",
    "validators_prove_mechanics_not_truth": true
  },
  "claims": [
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 1.0
      },
      "claim_id": "tos.claim.sid-6b7dc1b1994a838760c9b47d3eb76d60",
      "claim_status": "observed",
      "claim_type": "textual_observation",
      "claim_version": 1,
      "competing_claim_refs": [],
      "epistemic_status": "observed",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1"
          ],
          "description": "Exact public-synthetic source span bound by digest and position selector.",
          "direction": "supports",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "support"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "kind": "digest",
          "media_type": "text/plain; charset=utf-8",
          "sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449"
        },
        "predicate": "has_exact_form_digest",
        "subject_ref": "tos.occurrence.semantic-v2.synthetic-flame-1"
      },
      "review_refs": [],
      "stage": "exact_form",
      "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 1.0
      },
      "claim_id": "tos.claim.sid-1752eb7bea713115e9592416a0974ddb",
      "claim_status": "observed",
      "claim_type": "textual_observation",
      "claim_version": 1,
      "competing_claim_refs": [],
      "epistemic_status": "observed",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "Exact public-synthetic source span bound by digest and position selector.",
          "direction": "supports",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "support"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "kind": "digest",
          "media_type": "text/plain; charset=utf-8",
          "sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449"
        },
        "predicate": "has_exact_form_digest",
        "subject_ref": "tos.occurrence.semantic-v2.synthetic-flame-2"
      },
      "review_refs": [],
      "stage": "exact_form",
      "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.55
      },
      "claim_id": "tos.claim.sid-14b9fe36366f506ef55818f7b0be1158",
      "claim_status": "proposed",
      "claim_type": "sign_identity",
      "claim_version": 1,
      "competing_claim_refs": [
        "tos.claim.sid-17ca559d9cc0de97900839cfcecccd37"
      ],
      "epistemic_status": "interpreted",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1",
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "The same two source spans permit competing synthetic readings; recurrence alone does not decide identity.",
          "direction": "does_not_decide",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "context"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "description": "Synthetic reading A treats both occurrences as one sign of continuity.",
          "entity_refs": [
            "tos.occurrence.semantic-v2.synthetic-flame-1",
            "tos.occurrence.semantic-v2.synthetic-flame-2"
          ],
          "kind": "entity_set"
        },
        "predicate": "groups_occurrences_as_candidate_sign",
        "subject_ref": "tos.sign.sid-40f50dd2add1effb00bf5c6b198ccb51"
      },
      "review_refs": [],
      "stage": "stable_sign_candidate",
      "status_reason": "Model-shaped synthetic proposal remains unreviewed and non-authoritative.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1",
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.55
      },
      "claim_id": "tos.claim.sid-17ca559d9cc0de97900839cfcecccd37",
      "claim_status": "proposed",
      "claim_type": "sign_identity",
      "claim_version": 1,
      "competing_claim_refs": [
        "tos.claim.sid-14b9fe36366f506ef55818f7b0be1158"
      ],
      "epistemic_status": "interpreted",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1",
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "The same two source spans permit competing synthetic readings; recurrence alone does not decide identity.",
          "direction": "does_not_decide",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "context"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "description": "Synthetic reading B treats the later occurrence as transforming the earlier sign.",
          "entity_refs": [
            "tos.occurrence.semantic-v2.synthetic-flame-1",
            "tos.occurrence.semantic-v2.synthetic-flame-2"
          ],
          "kind": "entity_set"
        },
        "predicate": "groups_occurrences_as_candidate_sign",
        "subject_ref": "tos.sign.sid-fac28d7c3ca8a1e9af008da93ce58c88"
      },
      "review_refs": [],
      "stage": "stable_sign_candidate",
      "status_reason": "Model-shaped synthetic proposal remains unreviewed and non-authoritative.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1",
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    }
  ],
  "content_posture": "public_synthetic_contract_exercise",
  "entities": [
    {
      "admission_review_refs": [],
      "admission_status": "observed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "display",
          "value": "flame"
        }
      ],
      "entity_id": "tos.occurrence.semantic-v2.synthetic-flame-1",
      "entity_kind": "occurrence",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1"
        ],
        "claim_refs": [],
        "parent_entity_refs": []
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "observed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "display",
          "value": "flame"
        }
      ],
      "entity_id": "tos.occurrence.semantic-v2.synthetic-flame-2",
      "entity_kind": "occurrence",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [],
        "parent_entity_refs": []
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "proposed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "editorial",
          "value": "flame as continuity"
        }
      ],
      "entity_id": "tos.sign.sid-40f50dd2add1effb00bf5c6b198ccb51",
      "entity_kind": "sign",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1",
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [
          "tos.claim.sid-14b9fe36366f506ef55818f7b0be1158"
        ],
        "parent_entity_refs": [
          "tos.occurrence.semantic-v2.synthetic-flame-1",
          "tos.occurrence.semantic-v2.synthetic-flame-2"
        ]
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "proposed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "editorial",
          "value": "flame as transformation"
        }
      ],
      "entity_id": "tos.sign.sid-fac28d7c3ca8a1e9af008da93ce58c88",
      "entity_kind": "sign",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1",
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [
          "tos.claim.sid-17ca559d9cc0de97900839cfcecccd37"
        ],
        "parent_entity_refs": [
          "tos.occurrence.semantic-v2.synthetic-flame-1",
          "tos.occurrence.semantic-v2.synthetic-flame-2"
        ]
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    }
  ],
  "graph_projection": {
    "edges": [],
    "node_refs": [],
    "posture": "downstream-derived-projection-only",
    "projection_event_refs": [],
    "source_return_required": true
  },
  "relations": [],
  "reviews": [],
  "rights_and_visibility": {
    "private_source_used": false,
    "publication_authorized": true,
    "record_visibility": "public",
    "rights_basis_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json"
    ],
    "source_content_visibility": "public_synthetic"
  },
  "schema_version": "tos_semantic_annotation_packet_v2",
  "source_scope": {
    "expression_ref": "tos.expression.semantic-v2.synthetic-miniature.en",
    "file_ref": "tos.file.semantic-v2.synthetic-miniature.utf8",
    "file_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3",
    "item_ref": "tos.item.semantic-v2.synthetic-miniature.tracked",
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json"
    ],
    "source_anchors": [
      {
        "anchor_ref": "tos.anchor.semantic-v2.synthetic-flame-1",
        "exact_sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449",
        "page_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "required": true
        },
        "selector": {
          "end": 18,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 13,
          "type": "text_position"
        },
        "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
        "source_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
      },
      {
        "anchor_ref": "tos.anchor.semantic-v2.synthetic-flame-2",
        "exact_sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449",
        "page_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "required": true
        },
        "selector": {
          "end": 67,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 62,
          "type": "text_position"
        },
        "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
        "source_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
      }
    ],
    "source_text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
    "work_ref": "tos.work.semantic-v2.synthetic-miniature"
  },
  "supersedes_annotation_ref": null
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-c-invalid-model-promotion.json",
                    r####"{
  "$schema": "https://tree-of-sophia.local/ToS/contracts/semantic-annotation-packet-v2.schema.json",
  "annotation_id": "tos.annotation.sid-8d28d9f2f6b662eba75812c550eb6548",
  "annotation_version": 1,
  "authority_boundary": {
    "frequency_is_semantic_evidence_not_semantic_proof": true,
    "graph_role": "relation",
    "model_may_propose_but_not_promote_stable_sign": true,
    "promotion_review_posture": "rare-triggered-source-visible-human-checkpoint",
    "routine_human_review_per_occurrence": false,
    "source_role": "authority",
    "tree_role": "orientation",
    "validators_prove_mechanics_not_truth": true
  },
  "claims": [
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 1.0
      },
      "claim_id": "tos.claim.sid-6b7dc1b1994a838760c9b47d3eb76d60",
      "claim_status": "observed",
      "claim_type": "textual_observation",
      "claim_version": 1,
      "competing_claim_refs": [],
      "epistemic_status": "observed",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1"
          ],
          "description": "Exact public-synthetic source span bound by digest and position selector.",
          "direction": "supports",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "support"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "kind": "digest",
          "media_type": "text/plain; charset=utf-8",
          "sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449"
        },
        "predicate": "has_exact_form_digest",
        "subject_ref": "tos.occurrence.semantic-v2.synthetic-flame-1"
      },
      "review_refs": [],
      "stage": "exact_form",
      "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 1.0
      },
      "claim_id": "tos.claim.sid-1752eb7bea713115e9592416a0974ddb",
      "claim_status": "observed",
      "claim_type": "textual_observation",
      "claim_version": 1,
      "competing_claim_refs": [],
      "epistemic_status": "observed",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "Exact public-synthetic source span bound by digest and position selector.",
          "direction": "supports",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "support"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "software",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "kind": "digest",
          "media_type": "text/plain; charset=utf-8",
          "sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449"
        },
        "predicate": "has_exact_form_digest",
        "subject_ref": "tos.occurrence.semantic-v2.synthetic-flame-2"
      },
      "review_refs": [],
      "stage": "exact_form",
      "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.55
      },
      "claim_id": "tos.claim.sid-14b9fe36366f506ef55818f7b0be1158",
      "claim_status": "accepted",
      "claim_type": "sign_identity",
      "claim_version": 1,
      "competing_claim_refs": [
        "tos.claim.sid-17ca559d9cc0de97900839cfcecccd37"
      ],
      "epistemic_status": "interpreted",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1",
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "The same two source spans permit competing synthetic readings; recurrence alone does not decide identity.",
          "direction": "does_not_decide",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "context"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "description": "Synthetic reading A treats both occurrences as one sign of continuity.",
          "entity_refs": [
            "tos.occurrence.semantic-v2.synthetic-flame-1",
            "tos.occurrence.semantic-v2.synthetic-flame-2"
          ],
          "kind": "entity_set"
        },
        "predicate": "groups_occurrences_as_candidate_sign",
        "subject_ref": "tos.sign.sid-40f50dd2add1effb00bf5c6b198ccb51"
      },
      "review_refs": [],
      "stage": "stable_sign_candidate",
      "status_reason": "Deliberately invalid: synthetic model-shaped proposal attempts promotion without human review.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1",
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    },
    {
      "certainty": {
        "meaning": "maker_declared_uncertainty_not_truth_probability",
        "value": 0.55
      },
      "claim_id": "tos.claim.sid-17ca559d9cc0de97900839cfcecccd37",
      "claim_status": "proposed",
      "claim_type": "sign_identity",
      "claim_version": 1,
      "competing_claim_refs": [
        "tos.claim.sid-14b9fe36366f506ef55818f7b0be1158"
      ],
      "epistemic_status": "interpreted",
      "evidence": [
        {
          "anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1",
            "tos.anchor.semantic-v2.synthetic-flame-2"
          ],
          "description": "The same two source spans permit competing synthetic readings; recurrence alone does not decide identity.",
          "direction": "does_not_decide",
          "evidence_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "role": "context"
        }
      ],
      "maker": {
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": "2026-08-11T12:00:00Z",
        "maker_kind": "synthetic_fixture",
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": "model",
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred"
      },
      "proposition": {
        "object": {
          "description": "Synthetic reading B treats the later occurrence as transforming the earlier sign.",
          "entity_refs": [
            "tos.occurrence.semantic-v2.synthetic-flame-1",
            "tos.occurrence.semantic-v2.synthetic-flame-2"
          ],
          "kind": "entity_set"
        },
        "predicate": "groups_occurrences_as_candidate_sign",
        "subject_ref": "tos.sign.sid-fac28d7c3ca8a1e9af008da93ce58c88"
      },
      "review_refs": [],
      "stage": "stable_sign_candidate",
      "status_reason": "Model-shaped synthetic proposal remains unreviewed and non-authoritative.",
      "supersedes_claim_ref": null,
      "target_anchor_refs": [
        "tos.anchor.semantic-v2.synthetic-flame-1",
        "tos.anchor.semantic-v2.synthetic-flame-2"
      ]
    }
  ],
  "content_posture": "public_synthetic_contract_exercise",
  "entities": [
    {
      "admission_review_refs": [],
      "admission_status": "observed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "display",
          "value": "flame"
        }
      ],
      "entity_id": "tos.occurrence.semantic-v2.synthetic-flame-1",
      "entity_kind": "occurrence",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1"
        ],
        "claim_refs": [],
        "parent_entity_refs": []
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "observed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "display",
          "value": "flame"
        }
      ],
      "entity_id": "tos.occurrence.semantic-v2.synthetic-flame-2",
      "entity_kind": "occurrence",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [],
        "parent_entity_refs": []
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "accepted",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "editorial",
          "value": "flame as continuity"
        }
      ],
      "entity_id": "tos.sign.sid-40f50dd2add1effb00bf5c6b198ccb51",
      "entity_kind": "sign",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1",
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [
          "tos.claim.sid-14b9fe36366f506ef55818f7b0be1158"
        ],
        "parent_entity_refs": [
          "tos.occurrence.semantic-v2.synthetic-flame-1",
          "tos.occurrence.semantic-v2.synthetic-flame-2"
        ]
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    },
    {
      "admission_review_refs": [],
      "admission_status": "proposed",
      "display_labels": [
        {
          "language": "en",
          "mutable": true,
          "role": "editorial",
          "value": "flame as transformation"
        }
      ],
      "entity_id": "tos.sign.sid-fac28d7c3ca8a1e9af008da93ce58c88",
      "entity_kind": "sign",
      "entity_version": 1,
      "identity_basis": {
        "anchor_refs": [
          "tos.anchor.semantic-v2.synthetic-flame-1",
          "tos.anchor.semantic-v2.synthetic-flame-2"
        ],
        "claim_refs": [
          "tos.claim.sid-17ca559d9cc0de97900839cfcecccd37"
        ],
        "parent_entity_refs": [
          "tos.occurrence.semantic-v2.synthetic-flame-1",
          "tos.occurrence.semantic-v2.synthetic-flame-2"
        ]
      },
      "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
      "supersedes_entity_ref": null
    }
  ],
  "graph_projection": {
    "edges": [],
    "node_refs": [],
    "posture": "downstream-derived-projection-only",
    "projection_event_refs": [],
    "source_return_required": true
  },
  "relations": [],
  "reviews": [],
  "rights_and_visibility": {
    "private_source_used": false,
    "publication_authorized": true,
    "record_visibility": "public",
    "rights_basis_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json"
    ],
    "source_content_visibility": "public_synthetic"
  },
  "schema_version": "tos_semantic_annotation_packet_v2",
  "source_scope": {
    "expression_ref": "tos.expression.semantic-v2.synthetic-miniature.en",
    "file_ref": "tos.file.semantic-v2.synthetic-miniature.utf8",
    "file_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3",
    "item_ref": "tos.item.semantic-v2.synthetic-miniature.tracked",
    "rights_refs": [
      "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json"
    ],
    "source_anchors": [
      {
        "anchor_ref": "tos.anchor.semantic-v2.synthetic-flame-1",
        "exact_sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449",
        "page_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "required": true
        },
        "selector": {
          "end": 18,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 13,
          "type": "text_position"
        },
        "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
        "source_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
      },
      {
        "anchor_ref": "tos.anchor.semantic-v2.synthetic-flame-2",
        "exact_sha256": "bd9dc5f78dac9fad83a604327bf29bf368b8921f8cedfb3d3673efbe1f4eb449",
        "page_return": {
          "locator_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
          "required": true
        },
        "selector": {
          "end": 67,
          "interval": "half_open",
          "position_unit": "unicode_code_point",
          "start": 62,
          "type": "text_position"
        },
        "source_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
        "source_sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
      }
    ],
    "source_text_layer_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
    "work_ref": "tos.work.semantic-v2.synthetic-miniature"
  },
  "supersedes_annotation_ref": null
}
"####,
                ),
                (
                    "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/lab.manifest.json",
                    r####"{
  "authority_limits": {
    "canon_effect": false,
    "concept_established": false,
    "graph_truth_established": false,
    "human_review_performed": false,
    "model_invoked": false,
    "private_source_used": false,
    "semantic_truth_established": false,
    "stable_sign_established": false
  },
  "authority_posture": "public_synthetic_contract_mechanics_only",
  "builder": {
    "ref": "scripts/build_semantic_annotation_v2_lab.py",
    "sha256": "e0f42750f53e4b0679d6346ff5199bd4f7cd7a4442834cba62758ca57f339ffe"
  },
  "contract": {
    "ref": "ToS/contracts/semantic-annotation-packet-v2.schema.json",
    "sha256": "1a3e8bf71f47c8c655c00cc9b7401cb621b5136ee52567ab3c34c2022c2b65d8"
  },
  "input_fixture": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt",
    "sha256": "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3"
  },
  "plan": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/plan.json",
    "sha256": "b2b212aab7018891d68f564233f13cf4a49909c1db44ec141485380460486399"
  },
  "research": {
    "ref": "ToS/research-packets/foundation-laboratory-2026-07/SEMANTIC_IDENTITY_ANNOTATION_RESEARCH_2026-08-11.md",
    "sha256": "32aad968739e35f77841d8bd4954480773533e5ec789bc875e524de3f18fe634"
  },
  "schema_version": "tos_semantic_annotation_v2_lab_manifest_v1",
  "variants": [
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-a-occurrences-only.json",
      "packet_sha256": "33846140980864c1cd51bcb5eae739f8ead87fc49508b9f56afd2a8e2f2ffdfe",
      "variant_id": "A"
    },
    {
      "expected_schema_valid": true,
      "expected_semantic_valid": true,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json",
      "packet_sha256": "00a22067da28cd17ab02f2fde2e227b70f5b32ac11c9b68514fcee4133f39003",
      "variant_id": "B"
    },
    {
      "expected_schema_valid": false,
      "expected_semantic_valid": false,
      "packet_ref": "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-c-invalid-model-promotion.json",
      "packet_sha256": "c9725be035ac425cd70fd77c620cf9806443bffccdefeaa8b6a2aeda1f4854fc",
      "variant_id": "C"
    }
  ]
}
"####,
                ),
            ],
        }),
        _ => Err("unknown synthetic laboratory".into()),
    }
}
