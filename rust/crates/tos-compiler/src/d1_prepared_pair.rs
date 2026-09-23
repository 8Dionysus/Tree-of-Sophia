//! Typed, fail-closed selected-pair boundary for the private D1 SQL emitter.
//!
//! The present compiler can check shape and lineage. It cannot mint or retain
//! the source/rights/currentness and selected-D1 owner grants required to
//! promote an addressed pair. `try_emit_selected_pair` therefore refuses
//! before touching paths until a STO/CMD guarded receipt is implemented.

use crate::d1::{D1PairFailure, D1PairInput, D1PairReceipt, D1RowTransition, target_d1_revision};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};
use tos_foundation::Digest256;

const SOURCE_INPUTS_SCHEMA: &str = "tos_prepared_source_inputs_v1";
const PREPARED_BINDING_SCHEMA: &str = "tos_published_knowledge_snapshot_v1";
const PREPARED_READER_SCHEMA: &str = "tos_local_prepared_read_model_v1";
const D1_READER_SCHEMA: &str = "tos_cloudflare_edge_read_model_v9";
const MAX_SOURCE_INPUTS_BYTES: usize = 1_048_576;
const MAX_ROOTS: usize = 16;
const MAX_DEPENDENCIES: usize = 1024;
const MAX_NAME_BYTES: usize = 4096;
const MAX_ADDRESS: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug)]
pub struct D1OwnerRootClaim {
    /// Digest of exact immutable projection-root JSON bytes, not admission.
    pub snapshot_sha256: String,
    /// Caller-supplied locator for the missing independent owner receipt.
    pub owner_receipt_sha256: String,
    pub bound_prepared_source_revision: String,
    pub source_cut: String,
    pub logical_schema: String,
    pub collections: Vec<String>,
    pub owner_epoch: u64,
    pub withdrawn: bool,
}

#[derive(Clone, Debug)]
pub struct D1PreparedSide {
    pub source_revision: String,
    pub prepared_binding_json: String,
    pub prepared_data_revision: String,
    pub prepared_epoch: u64,
    pub source_inputs_raw: Vec<u8>,
    pub generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub navigation: D1OwnerRootClaim,
    pub rights: D1OwnerRootClaim,
}

#[derive(Clone, Debug)]
pub struct D1SelectedPredecessor {
    pub d1_revision: String,
    pub d1_publication_epoch: u64,
    pub selected_generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub reader_top_sha256: String,
    pub catalog_sha256: String,
    pub lens_sha256: String,
    /// External selected-D1 receipt identity, still unverified by CMP.
    pub owner_receipt_sha256: String,
}

#[derive(Clone, Debug)]
pub struct D1RollbackLineage {
    pub predecessor_d1_revision: String,
    pub successor_d1_revision: String,
    pub forward_publication_epoch: u64,
    pub rollback_generation: String,
    pub rollback_publication_epoch: u64,
}

#[derive(Clone, Debug)]
pub struct D1PreparedPair {
    pub selected: D1SelectedPredecessor,
    pub before: D1PreparedSide,
    pub after: D1PreparedSide,
    pub after_descriptor_mode: String,
    pub parent_prepared_data_revision: String,
    pub rollback: D1RollbackLineage,
}

#[derive(Clone, Debug)]
pub struct D1PairPreview {
    pub base_d1_revision: String,
    pub target_d1_revision: String,
    pub lineage_sha256: String,
    pub rollback_generation: String,
    pub owner_admitted: bool,
}

#[derive(Debug)]
pub enum D1GateFailure {
    Pair(D1PairFailure),
    Invalid(&'static str),
    Budget(&'static str),
    OwnerAdmissionUnavailable,
}
impl From<D1PairFailure> for D1GateFailure {
    fn from(value: D1PairFailure) -> Self {
        Self::Pair(value)
    }
}
pub type D1GateResult<T> = std::result::Result<T, D1GateFailure>;

fn digest(value: &[u8]) -> String {
    Digest256::of_bytes(value).to_hex()
}
fn exact_digest(value: &str) -> D1GateResult<()> {
    Digest256::from_hex(value).map_err(|_| D1GateFailure::Invalid("D1 owner digest"))?;
    Ok(())
}
fn bounded_name(value: &str) -> D1GateResult<()> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES || value.contains('\0') {
        return Err(D1GateFailure::Budget("D1 owner name bytes"));
    }
    Ok(())
}
fn field<'a>(value: &'a Value, key: &str) -> D1GateResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(D1GateFailure::Invalid("D1 paired JSON field"))
}
fn parse_json(raw: &[u8], max: usize) -> D1GateResult<Value> {
    if raw.is_empty() || raw.len() > max {
        return Err(D1GateFailure::Budget("D1 paired JSON bytes"));
    }
    serde_json::from_slice(raw).map_err(|_| D1GateFailure::Invalid("D1 paired JSON"))
}

fn verify_binding(side: &D1PreparedSide) -> D1GateResult<()> {
    let value = parse_json(
        side.prepared_binding_json.as_bytes(),
        MAX_SOURCE_INPUTS_BYTES,
    )?;
    if field(&value, "schema")? != PREPARED_BINDING_SCHEMA
        || field(&value, "read_model_schema")? != PREPARED_READER_SCHEMA
        || field(&value, "source_revision")? != side.source_revision
        || field(&value, "data_revision")? != side.prepared_data_revision
        || value.get("publication_epoch").and_then(Value::as_u64) != Some(side.prepared_epoch)
        || side.prepared_epoch == 0
        || side.prepared_epoch > MAX_ADDRESS
    {
        return Err(D1GateFailure::Invalid("exact prepared binding mismatch"));
    }
    exact_digest(&side.prepared_data_revision)?;
    exact_digest(field(&value, "metadata_sha256")?)?;
    Ok(())
}

fn verify_root(
    root: &D1OwnerRootClaim,
    side: &D1PreparedSide,
    navigation: bool,
) -> D1GateResult<()> {
    for value in [&root.snapshot_sha256, &root.owner_receipt_sha256] {
        exact_digest(value)?;
    }
    bounded_name(&root.source_cut)?;
    if root.bound_prepared_source_revision != side.source_revision
        || root.owner_epoch == 0
        || root.owner_epoch > MAX_ADDRESS
        || root.withdrawn
    {
        return Err(D1GateFailure::Invalid("D1 root owner claim binding"));
    }
    let expected = if navigation {
        &["nodes", "edges"][..]
    } else {
        &["rights"][..]
    };
    if root.collections.len() != expected.len()
        || root
            .collections
            .iter()
            .map(String::as_str)
            .ne(expected.iter().copied())
    {
        return Err(D1GateFailure::Invalid("D1 root collection coverage"));
    }
    if navigation {
        if !matches!(
            root.logical_schema.as_str(),
            "tos_agent_source_navigation_rows_v1" | "tos_source_navigation_v1"
        ) {
            return Err(D1GateFailure::Invalid("D1 navigation root schema"));
        }
    } else if root.logical_schema != "tos_source_navigation_rights_v1" {
        return Err(D1GateFailure::Invalid("D1 rights root schema"));
    }
    Ok(())
}

fn verify_inputs(
    side: &D1PreparedSide,
    expected_sha: &str,
) -> D1GateResult<(BTreeMap<String, String>, BTreeMap<String, String>)> {
    exact_digest(expected_sha)?;
    if digest(&side.source_inputs_raw) != expected_sha {
        return Err(D1GateFailure::Invalid(
            "exact prepared source inputs digest",
        ));
    }
    let value = parse_json(&side.source_inputs_raw, MAX_SOURCE_INPUTS_BYTES)?;
    let mut canonical = serde_json::to_vec(&value)
        .map_err(|_| D1GateFailure::Invalid("source inputs canonical JSON"))?;
    canonical.push(b'\n');
    if canonical != side.source_inputs_raw {
        return Err(D1GateFailure::Invalid("source inputs canonical bytes"));
    }
    let object = value
        .as_object()
        .ok_or(D1GateFailure::Invalid("source inputs object"))?;
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != BTreeSet::from([
            "schema",
            "source_revision",
            "source_publication",
            "dependencies",
            "roots",
        ])
        || field(&value, "schema")? != SOURCE_INPUTS_SCHEMA
        || field(&value, "source_revision")? != side.source_revision
    {
        return Err(D1GateFailure::Invalid("source inputs identity"));
    }
    if let Some(token) = value.get("source_publication").and_then(Value::as_str) {
        let digest = token
            .strip_prefix("sha256:")
            .ok_or(D1GateFailure::Invalid("source publication token"))?;
        exact_digest(digest)?;
    } else if !value.get("source_publication").is_some_and(Value::is_null) {
        return Err(D1GateFailure::Invalid("source publication token"));
    }
    let dependencies = value
        .get("dependencies")
        .and_then(Value::as_object)
        .ok_or(D1GateFailure::Invalid("source dependencies"))?;
    if dependencies.len() > MAX_DEPENDENCIES {
        return Err(D1GateFailure::Budget("source dependency count"));
    }
    let mut deps = BTreeMap::new();
    for (name, value) in dependencies {
        bounded_name(name)?;
        let sha = value
            .as_str()
            .ok_or(D1GateFailure::Invalid("source dependency digest"))?;
        exact_digest(sha)?;
        deps.insert(name.clone(), sha.to_owned());
    }
    let roots = value
        .get("roots")
        .and_then(Value::as_object)
        .ok_or(D1GateFailure::Invalid("source roots"))?;
    if roots.is_empty() || roots.len() > MAX_ROOTS {
        return Err(D1GateFailure::Budget("source root count"));
    }
    let mut selected = BTreeMap::new();
    for (name, root) in roots {
        bounded_name(name)?;
        let path = Path::new(field(root, "namespace_path")?);
        if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            return Err(D1GateFailure::Invalid("source root namespace path"));
        }
        let raw = field(root, "root_json")?;
        if raw.len() > MAX_SOURCE_INPUTS_BYTES {
            return Err(D1GateFailure::Budget("source root JSON bytes"));
        }
        let sha = field(root, "snapshot_sha256")?;
        exact_digest(sha)?;
        if digest(raw.as_bytes()) != sha {
            return Err(D1GateFailure::Invalid("source root bytes digest"));
        }
        selected.insert(name.clone(), sha.to_owned());
    }
    if selected.get("source-navigation") != Some(&side.navigation.snapshot_sha256) {
        return Err(D1GateFailure::Invalid("paired navigation root differs"));
    }
    Ok((selected, deps))
}

fn verify_reader_top(raw: &str, revision: &str, source_revision: &str) -> D1GateResult<()> {
    let value = parse_json(raw.as_bytes(), 32_000)?;
    if field(&value, "read_model_schema")? != D1_READER_SCHEMA
        || field(&value, "data_revision")? != revision
        || field(&value, "source_revision")? != source_revision
    {
        return Err(D1GateFailure::Invalid("selected D1 reader top"));
    }
    Ok(())
}

/// Check exact mechanical relationships. This preview is never an admission.
pub fn inspect_prepared_pair(
    spec: &D1PairInput,
    pair: &D1PreparedPair,
) -> D1GateResult<D1PairPreview> {
    let target = target_d1_revision(spec)?;
    verify_reader_top(
        &spec.before_reader_top,
        &spec.base_d1_revision,
        &spec.before_source_revision,
    )?;
    verify_reader_top(&spec.after_reader_top, &target, &spec.after_source_revision)?;
    let selected = &pair.selected;
    for value in [
        &selected.d1_revision,
        &selected.reader_top_sha256,
        &selected.catalog_sha256,
        &selected.lens_sha256,
        &selected.owner_receipt_sha256,
    ] {
        exact_digest(value)?;
    }
    if selected.d1_revision != spec.base_d1_revision
        || selected.reader_top_sha256 != digest(spec.before_reader_top.as_bytes())
        || selected.selected_generation != pair.before.generation
        || selected.route_map_version != pair.before.route_map_version
        || selected.reader_abi != pair.before.reader_abi
    {
        return Err(D1GateFailure::Invalid("selected D1 predecessor mismatch"));
    }
    let before_top = parse_json(spec.before_reader_top.as_bytes(), 32_000)?;
    if field(&before_top, "catalog_sha256")? != selected.catalog_sha256
        || field(&before_top, "lens_sha256")? != selected.lens_sha256
    {
        return Err(D1GateFailure::Invalid(
            "selected D1 catalog or lens mismatch",
        ));
    }
    for (side, binding, source, inputs, navigation, rights) in [
        (
            &pair.before,
            &spec.before_prepared_binding,
            &spec.before_source_revision,
            &spec.before_source_inputs_sha256,
            &spec.before_navigation_sha256,
            &spec.before_rights_sha256,
        ),
        (
            &pair.after,
            &spec.after_prepared_binding,
            &spec.after_source_revision,
            &spec.after_source_inputs_sha256,
            &spec.after_navigation_sha256,
            &spec.after_rights_sha256,
        ),
    ] {
        if &side.prepared_binding_json != binding
            || &side.source_revision != source
            || &side.navigation.snapshot_sha256 != navigation
            || rights.as_ref() != Some(&side.rights.snapshot_sha256)
        {
            return Err(D1GateFailure::Invalid("prepared pair side mismatch"));
        }
        bounded_name(&side.generation)?;
        bounded_name(&side.route_map_version)?;
        bounded_name(&side.reader_abi)?;
        verify_binding(side)?;
        verify_root(&side.navigation, side, true)?;
        verify_root(&side.rights, side, false)?;
        exact_digest(inputs)?;
    }
    if pair.before.route_map_version != pair.after.route_map_version
        || pair.before.reader_abi != pair.after.reader_abi
        || pair.before.generation == pair.after.generation
        || pair.after.prepared_epoch <= pair.before.prepared_epoch
        || pair.after_descriptor_mode != "delta-history"
        || pair.parent_prepared_data_revision != pair.before.prepared_data_revision
    {
        return Err(D1GateFailure::Invalid("prepared parent or ABI transition"));
    }
    let (before_roots, before_deps) =
        verify_inputs(&pair.before, &spec.before_source_inputs_sha256)?;
    let (after_roots, after_deps) = verify_inputs(&pair.after, &spec.after_source_inputs_sha256)?;
    if before_roots.keys().ne(after_roots.keys())
        || before_roots.iter().any(|(name, sha)| {
            !matches!(
                name.as_str(),
                "source-catalog" | "bibliographic-claims" | "source-navigation"
            ) && after_roots.get(name) != Some(sha)
        })
        || before_deps.keys().ne(after_deps.keys())
        || before_deps.iter().any(|(name, sha)| {
            !matches!(
                name.as_str(),
                "claim-publication-profile" | "metadata-addition-publication-profile"
            ) && after_deps.get(name) != Some(sha)
        })
    {
        return Err(D1GateFailure::Invalid(
            "nonparticipating D1 source scope changed",
        ));
    }
    if pair.before.rights.snapshot_sha256 != pair.after.rights.snapshot_sha256 {
        return Err(D1GateFailure::Pair(D1PairFailure::FullOnlyRightsTransition));
    }
    let rollback = &pair.rollback;
    bounded_name(&rollback.rollback_generation)?;
    if rollback.predecessor_d1_revision != spec.base_d1_revision
        || rollback.successor_d1_revision != target
        || rollback.rollback_generation == pair.before.generation
        || rollback.rollback_generation == pair.after.generation
        || selected.d1_publication_epoch == 0
        || selected.d1_publication_epoch > MAX_ADDRESS
        || rollback.forward_publication_epoch <= selected.d1_publication_epoch
        || rollback.forward_publication_epoch >= rollback.rollback_publication_epoch
        || rollback.rollback_publication_epoch > MAX_ADDRESS
    {
        return Err(D1GateFailure::Invalid("D1 rollback lineage"));
    }
    let lineage = json!({
        "schema":"tos_d1_prepared_pair_gate_v1",
        "selected_d1_revision":selected.d1_revision,
        "selected_d1_publication_epoch":selected.d1_publication_epoch,
        "selected_owner_receipt_sha256":selected.owner_receipt_sha256,
        "selected_reader_top_sha256":selected.reader_top_sha256,
        "selected_catalog_sha256":selected.catalog_sha256,
        "selected_lens_sha256":selected.lens_sha256,
        "target_d1_revision":target,
        "before_prepared_data_revision":pair.before.prepared_data_revision,
        "after_prepared_data_revision":pair.after.prepared_data_revision,
        "before_source_inputs_sha256":spec.before_source_inputs_sha256,
        "after_source_inputs_sha256":spec.after_source_inputs_sha256,
        "before_navigation_sha256":pair.before.navigation.snapshot_sha256,
        "after_navigation_sha256":pair.after.navigation.snapshot_sha256,
        "before_navigation_owner_receipt_sha256":pair.before.navigation.owner_receipt_sha256,
        "after_navigation_owner_receipt_sha256":pair.after.navigation.owner_receipt_sha256,
        "before_rights_sha256":pair.before.rights.snapshot_sha256,
        "after_rights_sha256":pair.after.rights.snapshot_sha256,
        "before_rights_owner_receipt_sha256":pair.before.rights.owner_receipt_sha256,
        "after_rights_owner_receipt_sha256":pair.after.rights.owner_receipt_sha256,
        "route_map_version":pair.before.route_map_version,
        "reader_abi":pair.before.reader_abi,
        "before_generation":pair.before.generation,
        "after_generation":pair.after.generation,
        "rollback_generation":rollback.rollback_generation,
        "forward_publication_epoch":rollback.forward_publication_epoch,
        "rollback_publication_epoch":rollback.rollback_publication_epoch,
    });
    let raw =
        serde_json::to_vec(&lineage).map_err(|_| D1GateFailure::Invalid("D1 pair lineage JSON"))?;
    Ok(D1PairPreview {
        base_d1_revision: spec.base_d1_revision.clone(),
        target_d1_revision: target,
        lineage_sha256: digest(&raw),
        rollback_generation: rollback.rollback_generation.clone(),
        owner_admitted: false,
    })
}

/// Explicit fail-closed selected entry. Neither a self-described receipt hash
/// nor the mechanical `d1::emit_d1_pair` proves an admitted live pair, complete
/// affected closure, rights currentness, or reverse publication epoch.
pub fn try_emit_selected_pair<I>(
    spec: &D1PairInput,
    pair: &D1PreparedPair,
    _rows: I,
    _forward: &Path,
    _rollback: &Path,
    _manifest: &Path,
) -> D1GateResult<D1PairReceipt>
where
    I: IntoIterator<Item = D1RowTransition>,
{
    inspect_prepared_pair(spec, pair)?;
    Err(D1GateFailure::OwnerAdmissionUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::d1::D1PairLimits;
    use std::path::PathBuf;

    fn sha(s: &str) -> String {
        digest(s.as_bytes())
    }
    fn binding(source: &str, data: &str, epoch: u64) -> String {
        json!({"schema":PREPARED_BINDING_SCHEMA,"read_model_schema":PREPARED_READER_SCHEMA,
            "source_revision":source,"data_revision":data,"metadata_sha256":sha("metadata"),
            "publication_epoch":epoch})
        .to_string()
    }
    fn root(source: &str, snapshot: &str, navigation: bool) -> D1OwnerRootClaim {
        D1OwnerRootClaim {
            snapshot_sha256: snapshot.into(),
            owner_receipt_sha256: sha(if navigation {
                "navigation-owner"
            } else {
                "rights-owner"
            }),
            bound_prepared_source_revision: source.into(),
            source_cut: "sealed-cut".into(),
            logical_schema: if navigation {
                "tos_agent_source_navigation_rows_v1"
            } else {
                "tos_source_navigation_rights_v1"
            }
            .into(),
            collections: if navigation {
                vec!["nodes".into(), "edges".into()]
            } else {
                vec!["rights".into()]
            },
            owner_epoch: 1,
            withdrawn: false,
        }
    }
    fn source_inputs(source: &str, nav_root: &str) -> Vec<u8> {
        let root_json = "{\"schema\":\"tos_agent_source_navigation_rows_v1\"}\n";
        assert_eq!(digest(root_json.as_bytes()), nav_root);
        let value = json!({"schema":SOURCE_INPUTS_SCHEMA,"source_revision":source,
            "source_publication":format!("sha256:{}",sha("source-publication")),
            "dependencies":{"stable-profile":sha("stable")},
            "roots":{"source-navigation":{"namespace_path":"/private/source-navigation",
                "root_json":root_json,"snapshot_sha256":nav_root}}});
        let mut raw = serde_json::to_vec(&value).unwrap();
        raw.push(b'\n');
        raw
    }
    fn selected() -> (D1PairInput, D1PreparedPair) {
        let before_source = sha("before-source");
        let after_source = sha("after-source");
        let before_data = sha("prepared-before");
        let after_data = sha("prepared-after");
        let nav = sha("{\"schema\":\"tos_agent_source_navigation_rows_v1\"}\n");
        let rights = sha("rights-root");
        let before_inputs = source_inputs(&before_source, &nav);
        let after_inputs = source_inputs(&after_source, &nav);
        let before_binding = binding(&before_source, &before_data, 1);
        let after_binding = binding(&after_source, &after_data, 2);
        let base = sha("selected-d1");
        let mut spec = D1PairInput {
            base_d1_revision: base.clone(),
            before_source_revision: before_source.clone(),
            after_source_revision: after_source.clone(),
            before_prepared_binding: before_binding.clone(),
            after_prepared_binding: after_binding.clone(),
            before_source_inputs_sha256: digest(&before_inputs),
            after_source_inputs_sha256: digest(&after_inputs),
            before_navigation_sha256: nav.clone(),
            after_navigation_sha256: nav.clone(),
            before_rights_sha256: Some(rights.clone()),
            after_rights_sha256: Some(rights.clone()),
            implementation_sha256: sha("implementation"),
            auxiliary_installed: false,
            before_reader_top: json!({"data_revision":base,"source_revision":before_source,
                "read_model_schema":D1_READER_SCHEMA,"catalog_sha256":sha("catalog"),
                "lens_sha256":sha("lens")})
            .to_string(),
            after_reader_top: String::new(),
            limits: D1PairLimits::default(),
        };
        let target = target_d1_revision(&spec).unwrap();
        spec.after_reader_top = json!({"data_revision":target,"source_revision":after_source,
            "read_model_schema":D1_READER_SCHEMA})
        .to_string();
        let before = D1PreparedSide {
            source_revision: before_source.clone(),
            prepared_binding_json: before_binding,
            prepared_data_revision: before_data.clone(),
            prepared_epoch: 1,
            source_inputs_raw: before_inputs,
            generation: "generation-before".into(),
            route_map_version: "route-map-v1".into(),
            reader_abi: "reader-v1".into(),
            navigation: root(&before_source, &nav, true),
            rights: root(&before_source, &rights, false),
        };
        let after = D1PreparedSide {
            source_revision: after_source.clone(),
            prepared_binding_json: after_binding,
            prepared_data_revision: after_data,
            prepared_epoch: 2,
            source_inputs_raw: after_inputs,
            generation: "generation-after".into(),
            route_map_version: "route-map-v1".into(),
            reader_abi: "reader-v1".into(),
            navigation: root(&after_source, &nav, true),
            rights: root(&after_source, &rights, false),
        };
        let pair = D1PreparedPair {
            selected: D1SelectedPredecessor {
                d1_revision: base.clone(),
                d1_publication_epoch: 1,
                selected_generation: before.generation.clone(),
                route_map_version: before.route_map_version.clone(),
                reader_abi: before.reader_abi.clone(),
                reader_top_sha256: digest(spec.before_reader_top.as_bytes()),
                catalog_sha256: sha("catalog"),
                lens_sha256: sha("lens"),
                owner_receipt_sha256: sha("selected-owner-receipt"),
            },
            before,
            after,
            after_descriptor_mode: "delta-history".into(),
            parent_prepared_data_revision: before_data,
            rollback: D1RollbackLineage {
                predecessor_d1_revision: base,
                successor_d1_revision: target,
                forward_publication_epoch: 2,
                rollback_generation: "generation-rollback".into(),
                rollback_publication_epoch: 3,
            },
        };
        (spec, pair)
    }

    #[test]
    fn exact_pair_preview_is_mechanical_only() {
        let (spec, pair) = selected();
        let a = inspect_prepared_pair(&spec, &pair).unwrap();
        let b = inspect_prepared_pair(&spec, &pair).unwrap();
        assert_eq!(a.lineage_sha256, b.lineage_sha256);
        assert_eq!(a.target_d1_revision, target_d1_revision(&spec).unwrap());
        assert!(!a.owner_admitted);
    }

    #[test]
    fn unchanged_navigation_changed_rights_is_full_only() {
        let (mut spec, mut pair) = selected();
        let changed = sha("withdrawn-rights");
        spec.after_rights_sha256 = Some(changed.clone());
        pair.after.rights.snapshot_sha256 = changed;
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Pair(D1PairFailure::FullOnlyRightsTransition))
        ));
    }

    #[test]
    fn mixed_predecessor_and_parent_or_abi_drift_refuse() {
        let (spec, mut pair) = selected();
        pair.selected.d1_revision = sha("foreign-d1");
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Invalid(_))
        ));
        let (_, mut pair) = selected();
        pair.parent_prepared_data_revision = sha("foreign-parent");
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Invalid(_))
        ));
        let (_, mut pair) = selected();
        pair.after.reader_abi = "reader-v2".into();
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Invalid(_))
        ));
        let (_, mut pair) = selected();
        pair.rollback.rollback_publication_epoch = pair.selected.d1_publication_epoch;
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Invalid(_))
        ));
        let (_, mut pair) = selected();
        pair.selected.catalog_sha256 = sha("foreign-catalog");
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Invalid(_))
        ));
    }

    #[test]
    fn noncanonical_source_input_bytes_refuse() {
        let (spec, mut pair) = selected();
        pair.before.source_inputs_raw.pop();
        assert!(matches!(
            inspect_prepared_pair(&spec, &pair),
            Err(D1GateFailure::Invalid(_))
        ));
    }

    #[test]
    fn selected_emit_refuses_before_files_and_iterator_use() {
        let (spec, pair) = selected();
        let root = PathBuf::from("/definitely-not-created/tos-d1-selected-gate");
        let rows = std::iter::once_with(|| -> D1RowTransition {
            panic!("selected gate must not consume rows")
        });
        let error = try_emit_selected_pair(
            &spec,
            &pair,
            rows,
            &root.join("forward.sql"),
            &root.join("reverse.sql"),
            &root.join("pair.json"),
        )
        .unwrap_err();
        assert!(matches!(error, D1GateFailure::OwnerAdmissionUnavailable));
        assert!(!root.exists());
    }
}
