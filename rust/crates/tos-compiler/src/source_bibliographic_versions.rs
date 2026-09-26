//! Exact metadata/Claim versions and Collection ordering from the immutable source carrier.
//! Positive staged rows are byte-bound to this independently selected cut;
//! only its complete manifest can establish absence of an adjacent history.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic::{self as graph, BibliographicForms, BibliographicLimits};
use crate::source_bibliographic_render::{array, digest, encode, text};
use crate::source_witness_catalog::{self as catalog, SourceCatalogValidator};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, RelativePath, SourceRevision,
    canonical_raw_bytes_v1,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

/// The caller selects the revision/membership independently of the cut reader.
/// `stage_source_cut` is the caller's exact job label: it is not interpreted as
/// a revision digest or used as permission to read an unselected revision.
pub struct BibliographicSourceCut<'a> {
    pub cut: &'a CorpusCutReader,
    pub expected_revision: SourceRevision,
    pub expected_membership: SourceMembershipV1,
    pub stage_source_cut: &'a str,
    pub max_read_files: usize,
    pub max_read_bytes: usize,
}
pub(crate) struct Versions<'a, 'b> {
    input: &'a BibliographicSourceCut<'b>,
    files: BTreeMap<String, Vec<u8>>,
    bytes: usize,
}
fn path(reference: &str) -> Result<RelativePath> {
    RelativePath::parse(reference).map_err(|_| Error::Invalid("bibliographic cut source path"))
}
impl<'a, 'b> Versions<'a, 'b> {
    pub(crate) fn new(
        input: &'a BibliographicSourceCut<'b>,
        stage: &mut KnowledgeStage<'_>,
        validator: &SourceCatalogValidator<'_>,
        receipt: &catalog::SourceCatalogReceipt,
        l: BibliographicLimits,
    ) -> Result<Self> {
        catalog::verify_catalog(stage, receipt, l.catalog)?;
        if input.cut.current().revision() != input.expected_revision
            || stage.exact_receipt().binding.source_cut != input.stage_source_cut
            || input.max_read_files == 0
            || input.max_read_files > 4096
            || input.max_read_bytes == 0
            || input.max_read_bytes > 64 * 1024 * 1024
        {
            return Err(Error::Invalid(
                "bibliographic independently selected source cut",
            ));
        }
        let stream = input
            .cut
            .stream(input.expected_revision)
            .map_err(|_| Error::Invalid("bibliographic current source membership"))?;
        if stream.expectation() != input.expected_membership {
            return Err(Error::Invalid(
                "bibliographic independent membership expectation",
            ));
        }
        // Metadata is authenticated by SourceRevision. Complete traversal to
        // EOF does not read original payloads; those are outside this carrier.
        let mut membership = Digest256Hasher::new();
        membership.update(b"tos-val-full-membership-v1\0");
        let mut count = 0u64;
        for member in input.cut.current().members() {
            check(validator, l)?;
            let reference = member.path.as_str();
            membership.update(&(reference.len() as u64).to_be_bytes());
            membership.update(reference.as_bytes());
            membership.update(&member.size_bytes.to_be_bytes());
            membership.update(member.sha256.as_bytes());
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("bibliographic cut membership"))?;
        }
        if (SourceMembershipV1 {
            count,
            digest: membership.finalize(),
        }) != input.expected_membership
        {
            return Err(Error::Invalid(
                "bibliographic complete metadata membership root",
            ));
        }
        let mut result = Self {
            input,
            files: BTreeMap::new(),
            bytes: 0,
        };
        // Bind every declared raw input, including selected schemas and native
        // packets, to a member of this exact current source revision.
        for collection in [
            catalog::SOURCE_FILES,
            catalog::CONTRACT_FILES,
            catalog::NATIVE_IDENTITIES,
            catalog::NATIVE_TEXT,
            catalog::BIBLIOGRAPHIC_FILES,
        ] {
            if !stage
                .exact_receipt()
                .collections
                .iter()
                .any(|c| c.source_graph == catalog::CATALOG_SOURCE && c.collection == collection)
            {
                continue;
            }
            let mut after = None;
            loop {
                let page =
                    stage.scan_input(catalog::CATALOG_SOURCE, collection, after.as_deref(), 1)?;
                for row in page.rows {
                    check(validator, l)?;
                    let metadata =
                        input
                            .cut
                            .current()
                            .member(&path(&row.id)?)
                            .ok_or(Error::Invalid(
                                "bibliographic staged source outside selected current cut",
                            ))?;
                    if metadata.size_bytes != row.payload.len() as u64
                        || metadata.sha256 != Digest256::of_bytes(&row.payload)
                    {
                        return Err(Error::Invalid(
                            "bibliographic staged source/cut bytes differ",
                        ));
                    }
                }
                after = page.next_id;
                if after.is_none() {
                    break;
                }
            }
        }
        // The read cache starts empty: binding staged bytes reserves no second
        // copy and does not claim the selected source is publicly admissible.
        result.bytes = 0;
        Ok(result)
    }
    pub(crate) fn binding(&self) -> Value {
        json!({"source_revision":self.input.expected_revision.0.to_hex(),
            "membership_count":self.input.expected_membership.count,
            "membership_sha256":self.input.expected_membership.digest.to_hex(),
            "stage_source_cut":self.input.stage_source_cut})
    }
    fn optional(
        &mut self,
        reference: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Option<Vec<u8>>> {
        check(validator, l)?;
        let parsed = path(reference)?;
        if self.input.cut.current().member(&parsed).is_none() {
            return Ok(None);
        }
        if let Some(raw) = self.files.get(reference) {
            return Ok(Some(raw.clone()));
        }
        if self.files.len() >= self.input.max_read_files {
            return Err(Error::Budget("bibliographic version source file count"));
        }
        let member = self
            .input
            .cut
            .read_member(
                self.input.expected_revision,
                &parsed,
                l.catalog.max_file_bytes as u64,
                l.deadline,
                validator.cancelled,
            )
            .map_err(|_| Error::Invalid("bibliographic exact current source member read"))?;
        self.bytes = self
            .bytes
            .checked_add(member.raw.len())
            .filter(|n| *n <= self.input.max_read_bytes)
            .ok_or(Error::Budget(
                "bibliographic version aggregate source bytes",
            ))?;
        self.files.insert(reference.into(), member.raw.clone());
        Ok(Some(member.raw))
    }
    fn required(
        &mut self,
        reference: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Vec<u8>> {
        self.optional(reference, validator, l)?
            .ok_or(Error::Invalid("bibliographic exact version source missing"))
    }
    pub(crate) fn ground(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        claim: &Value,
        validator: &SourceCatalogValidator<'_>,
        entities: &Value,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Value> {
        let value = &claim["object"];
        let collection =
            self.resolve_record(stage, &value["collection_version"], validator, entities, l)?;
        if collection.record["record_type"] != "collection"
            || collection.record["record_id"] != claim["subject_ref"]
        {
            return Err(Error::Invalid(
                "bibliographic exact Collection basis identity/type",
            ));
        }
        let declared = array(&collection.record, "membership_claim_refs")?;
        let expected = array(value, "members")?;
        let mut members = BTreeSet::new();
        let mut bound = Vec::new();
        let mut inputs = BTreeMap::new();
        bind_files(&collection.provenance, &mut inputs)?;
        for reference in array(value, "membership_versions")? {
            let resolved = self.resolve_claim(stage, reference, validator, forms, l)?;
            let record = &resolved.record;
            let legacy = record["schema_version"] == "tos_claim_packet_v1"
                && record["claim_type"] == "bibliographic";
            let native = record["schema_version"] == "tos_source_relation_claim_v1"
                && record["claim_type"] == "relation";
            let member = record["object"]
                .as_str()
                .ok_or(Error::Invalid("bibliographic exact membership Work"))?;
            let polarity = record
                .get("polarity")
                .and_then(Value::as_str)
                .or(if legacy { Some("positive") } else { None });
            if !(legacy || native)
                || !matches!(
                    record["assertion_layer"].as_str(),
                    Some("bibliographic_assertion" | "scholarly_report")
                )
                || !declared.contains(&reference["id"])
                || record["subject_ref"] != claim["subject_ref"]
                || record["predicate"] != "contains_work"
                || polarity != Some("positive")
                || !expected.contains(&record["object"])
                || !members.insert(member.to_owned())
            {
                return Err(Error::Invalid(
                    "bibliographic distinct positive exact Collection membership",
                ));
            }
            bind_files(&resolved.provenance, &mut inputs)?;
            bound.push(json!({"ref":reference,"provenance":resolved.provenance,"version_status":resolved.version_status}));
        }
        let expected = expected
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or(Error::Invalid("bibliographic Collection member ref"))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if members.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(Error::Invalid("bibliographic exact membership set closure"));
        }
        check(validator, l)?;
        let basis = json!({"collection":{"ref":value["collection_version"],
            "provenance":collection.provenance,"version_status":collection.version_status},
            "memberships":bound,"input_digests":inputs,
            "establishes_membership":false,"grants_admission":false});
        encode(&basis, l.max_claim_cohort_bytes)?;
        Ok(basis)
    }
    /// Ordered exact retained metadata references. A successful result has already
    /// checked the entire selected record chain; it does not assess current use.
    pub(crate) fn exact_record_refs(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        id: &str,
        validator: &SourceCatalogValidator<'_>,
        entities: &Value,
        l: BibliographicLimits,
    ) -> Result<Version> {
        let row = catalog::catalog_row(stage, "records", id, l.catalog)?
            .ok_or(Error::Invalid("metadata version identity not catalogued"))?;
        self.resolve_record(stage, &row["source"]["record_ref"], validator, entities, l)
    }
    pub(crate) fn resolve_record(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        validator: &SourceCatalogValidator<'_>,
        entities: &Value,
        l: BibliographicLimits,
    ) -> Result<Version> {
        exact_ref(exact, false)?;
        let id = text(exact, "id")?;
        let row = catalog::catalog_row(stage, "records", id, l.catalog)?.ok_or(Error::Invalid(
            "bibliographic exact metadata identity not catalogued",
        ))?;
        let entry = &row["entry"];
        let reference = text(entry, "source_record_ref")?;
        let raw = self.required(reference, validator, l)?;
        let record = owned(&raw, l.catalog.max_row_bytes)?;
        let route = RecordRoute::derive(entry, &record, entities)?;
        let current = metadata_ref(&record, &route, l)?;
        route.validate_record(&record, &current, &record["schema_version"])?;
        if entry["record_sha256"] != text(&current, "digest")?.trim_start_matches("sha256:") {
            return Err(Error::Invalid(
                "bibliographic metadata version current catalog binding",
            ));
        }
        let schema = route.schema.as_str();
        catalog::check_catalog_schema(stage, validator, l.catalog, schema, "", &raw)?;
        let history_ref = adjacent(reference, "source-revision-history.json")?;
        let history_raw = self.optional(&history_ref, validator, l)?;
        let history = if let Some(raw) = &history_raw {
            owned(raw, l.catalog.max_row_bytes)?
        } else {
            json!({"schema_version":"tos_source_revision_history_v1","record_id":id,"receipts":[]})
        };
        exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
        let receipts = array(&history, "receipts")?;
        if !matches!(
            history["schema_version"].as_str(),
            Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
        ) || history["record_id"] != id
            || receipts.len() > 128
            || history_raw.is_some() && receipts.is_empty()
        {
            return Err(Error::Invalid(
                "bibliographic retained Collection history identity/bounds",
            ));
        }
        let mut commands = BTreeSet::new();
        let mut head = None;
        let mut baseline = current.clone();
        let mut selected_record = record.clone();
        let mut selected_source = json!({"source_ref":reference,"record_bytes":raw.len(),
            "record_sha256":format!("sha256:{}",Digest256::of_bytes(&raw).to_hex()),
            "archive_blob_ref":null,"archive_manifest_ref":null,"archive_manifest_sha256":null,"package_revision":null});
        let mut transition = Value::Null;
        let mut found = current == *exact;
        let mut refs = Vec::new();
        for (index, receipt) in receipts.iter().enumerate() {
            check(validator, l)?;
            metadata_receipt(
                receipt,
                &history,
                current.get("id").unwrap(),
                head.as_ref(),
                &mut commands,
                &route,
                l,
            )?;
            refs.push(receipt["previous_source"].clone());
            let (previous, source) = self.archive_record(
                reference,
                receipt,
                validator,
                stage,
                &route,
                &record["schema_version"],
                l,
            )?;
            if index == 0 {
                baseline = receipt["previous_source"].clone();
            }
            let fields = receipt["request"]["fields"]
                .as_object()
                .ok_or(Error::Invalid("bibliographic retained metadata patch"))?;
            let allowed = if let Some(compound) =
                compound_profile(receipt["request"]["operation"].as_str().unwrap_or(""))
            {
                vec![compound.field]
            } else {
                route.revision_fields()
            };
            if fields.is_empty() || fields.keys().any(|key| !allowed.contains(&key.as_str())) {
                return Err(Error::Invalid(
                    "bibliographic retained Collection metadata field scope",
                ));
            }
            let mut revised = previous
                .as_object()
                .ok_or(Error::Invalid("bibliographic archived Collection object"))?
                .clone();
            for (key, value) in fields {
                revised.insert(key.clone(), value.clone());
            }
            revised.insert(
                "record_version".into(),
                receipt["source"]["version"].clone(),
            );
            let revised = Value::Object(revised);
            route.validate_descriptive_delta(&previous, &revised)?;
            route.validate_record(&revised, &receipt["source"], &record["schema_version"])?;
            if metadata_ref(&revised, &route, l)? != receipt["source"] {
                return Err(Error::Invalid(
                    "bibliographic Collection successor reconstruction",
                ));
            }
            if receipt["previous_source"] == *exact {
                if found {
                    return Err(Error::Invalid(
                        "bibliographic duplicated exact Collection version",
                    ));
                }
                found = true;
                selected_record = previous;
                selected_source = source;
                transition = receipt_transition(receipt)?;
            }
            head = Some(receipt["source"].clone());
        }
        refs.push(current.clone());
        if head.is_some_and(|head| head != current) {
            return Err(Error::Invalid(
                "bibliographic Collection current metadata is not retained history head",
            ));
        }
        if !found {
            return Err(Error::Invalid(
                "bibliographic Collection exact version/digest not retained",
            ));
        }
        let catalog = legacy_catalog(stage, "records", Some(&route.kind), id, l)?;
        let provenance = json!({"verification_scope":"selected-record-chain","all_package_bytes_verified":false,
            "catalog":{"source_ref":format!("ToS/source-witnesses/catalog/{}",route.catalog_filename),"line":catalog.0,
                "sha256":catalog.1,"source_record_ref":reference,"current_record_ref":current},
            "descriptor":route.descriptor(&record["schema_version"]),
            "history":{"source_ref":history_raw.as_ref().map(|_|history_ref),
                "sha256":history_raw.as_ref().map(|raw|format!("sha256:{}",Digest256::of_bytes(raw).to_hex())),"receipt_count":receipts.len(),
                "retained_record_chain_verified":true,"retained_baseline_ref":baseline},
            "source":selected_source,"transition":transition});
        Ok(Version {
            record: selected_record,
            provenance,
            refs,
            current_ref: current.clone(),
            version_status: if current == *exact {
                "current"
            } else {
                "historical"
            },
        })
    }
    fn archive_record(
        &mut self,
        reference: &str,
        receipt: &Value,
        validator: &SourceCatalogValidator<'_>,
        stage: &KnowledgeStage<'_>,
        route: &RecordRoute,
        schema_version: &Value,
        l: BibliographicLimits,
    ) -> Result<(Value, Value)> {
        let previous = &receipt["previous_source"];
        exact_ref(previous, false)?;
        let revision = text(receipt, "previous_revision")?;
        let revision_sha = hash(revision)?;
        let archive = format!(
            "ToS/source-witnesses/.record-revisions/{}-{}",
            Digest256::of_bytes(text(previous, "id")?.as_bytes()).to_hex(),
            revision_sha
        );
        if receipt["archive_path"] != archive {
            return Err(Error::Invalid(
                "bibliographic metadata archive source-derived locator",
            ));
        }
        let manifest_ref = format!("{archive}/manifest.json");
        let manifest_raw = self.required(&manifest_ref, validator, l)?;
        let manifest = owned(&manifest_raw, l.catalog.max_row_bytes)?;
        let selected = manifest["schema_version"] == "tos_source_package_archive_v2";
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
        exact_keys(&manifest, &keys)?;
        if !matches!(
            manifest["schema_version"].as_str(),
            Some("tos_source_package_archive_v1" | "tos_source_package_archive_v2")
        ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
            || manifest["source_path"] != reference
            || manifest["source"] != *previous
            || manifest["revision"] != revision
        {
            return Err(Error::Invalid(
                "bibliographic metadata archive exact manifest binding",
            ));
        }
        let bindings = manifest["files"]
            .as_object()
            .ok_or(Error::Invalid("bibliographic metadata archive file map"))?;
        if bindings.is_empty() || bindings.len() > 64 {
            return Err(Error::Budget("bibliographic metadata archive member count"));
        }
        let basename = reference.rsplit('/').next().unwrap();
        let forms_name = format!("{}.human-forms.json", basename.trim_end_matches(".json"));
        let selected_names = [
            basename,
            forms_name.as_str(),
            "source-revision-history.json",
        ];
        let mut package = serde_json::Map::new();
        let mut declared_bytes = 0u64;
        for (name, binding) in bindings {
            exact_keys(binding, &["blob", "sha256", "bytes"])?;
            if name.is_empty()
                || name.contains(['/', '\\', '\0'])
                || matches!(name.as_str(), "." | "..")
                || selected && !selected_names.contains(&name.as_str())
            {
                return Err(Error::Invalid(
                    "bibliographic metadata archive member locator/scope",
                ));
            }
            let sha = hash(text(binding, "sha256")?)?;
            if binding["blob"] != format!("{sha}.blob") {
                return Err(Error::Invalid(
                    "bibliographic metadata archive blob locator",
                ));
            }
            let bytes = binding["bytes"]
                .as_u64()
                .filter(|n| *n <= 2 * 1024 * 1024)
                .ok_or(Error::Budget(
                    "bibliographic metadata archive declared member bytes",
                ))?;
            declared_bytes = declared_bytes
                .checked_add(bytes)
                .filter(|n| *n <= 8 * 1024 * 1024)
                .ok_or(Error::Budget(
                    "bibliographic metadata archive declared package bytes",
                ))?;
            package.insert(
                name.clone(),
                json!({"sha256":binding["sha256"],"bytes":binding["bytes"]}),
            );
        }
        if digest(&Value::Object(package), l.max_claim_cohort_bytes)? != revision_sha {
            return Err(Error::Invalid(
                "bibliographic metadata archive package revision",
            ));
        }
        let binding = bindings.get(basename).ok_or(Error::Invalid(
            "bibliographic archived Collection record absent",
        ))?;
        let blob_ref = format!("{archive}/{}", text(binding, "blob")?);
        let raw = self.required(&blob_ref, validator, l)?;
        if raw.len() as u64 != binding["bytes"].as_u64().unwrap_or(u64::MAX)
            || format!("sha256:{}", Digest256::of_bytes(&raw).to_hex()) != text(binding, "sha256")?
        {
            return Err(Error::Invalid(
                "bibliographic archived Collection record raw binding",
            ));
        }
        let record = owned(&raw, l.catalog.max_row_bytes)?;
        catalog::check_catalog_schema(stage, validator, l.catalog, &route.schema, "", &raw)?;
        route.validate_record(&record, previous, schema_version)?;
        if metadata_ref(&record, route, l)? != *previous {
            return Err(Error::Invalid(
                "bibliographic retained Collection exact record/profile",
            ));
        }
        Ok((
            record,
            json!({"source_ref":reference,"record_bytes":raw.len(),"record_sha256":binding["sha256"],
            "archive_blob_ref":blob_ref,"package_revision":revision,"archive_manifest_ref":manifest_ref,
            "archive_manifest_sha256":format!("sha256:{}",Digest256::of_bytes(&manifest_raw).to_hex())}),
        ))
    }
    fn package_names(
        &self,
        base: &str,
        max: usize,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Vec<String>> {
        let prefix = format!("{base}/");
        let mut names = Vec::new();
        for member in self.input.cut.current().members() {
            check(validator, l)?;
            if let Some(name) = member.path.as_str().strip_prefix(&prefix) {
                if names.len() >= max {
                    return Err(Error::Budget(
                        "bibliographic selected package member enumeration",
                    ));
                }
                names.push(name.to_owned());
            }
        }
        Ok(names)
    }
    fn package(
        &mut self,
        base: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let prefix = format!("{base}/");
        let names = self.package_names(base, 64, validator, l)?;
        if names.is_empty() || names.len() > 64 {
            return Err(Error::Budget(
                "bibliographic exact Claim package file count",
            ));
        }
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        for name in names {
            let lock = public_form_lock(&name);
            if name.contains(['/', '\\', '\0'])
                || matches!(
                    name.as_str(),
                    "catalog" | "payload" | "private" | "local-content" | "owner-local"
                )
                || name.starts_with('.') && !lock
            {
                return Err(Error::Invalid(
                    "bibliographic exact Claim package non-flat/public member",
                ));
            }
            let raw = self.required(&format!("{prefix}{name}"), validator, l)?;
            if raw.len() > 2 * 1024 * 1024 {
                return Err(Error::Budget("bibliographic public package member bytes"));
            }
            if lock && !raw.is_empty() {
                return Err(Error::Invalid(
                    "bibliographic public writer lock has contents",
                ));
            }
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= 8 * 1024 * 1024)
                .ok_or(Error::Budget("bibliographic exact Claim package bytes"))?;
            files.insert(name, raw);
        }
        Ok(files)
    }
    fn archive_package(
        &mut self,
        reference: &str,
        receipt: &Value,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<(BTreeMap<String, Vec<u8>>, BTreeMap<String, Value>)> {
        let previous = &receipt["previous_source"];
        exact_ref(previous, true)?;
        let revision = text(receipt, "previous_revision")?;
        let archive = format!(
            "ToS/source-witnesses/.record-revisions/{}-{}",
            Digest256::of_bytes(text(previous, "id")?.as_bytes()).to_hex(),
            hash(revision)?
        );
        if receipt["archive_path"] != archive {
            return Err(Error::Invalid(
                "bibliographic Claim archive source-derived locator",
            ));
        }
        let prefix = format!("{archive}/");
        let names = self.package_names(&archive, 65, validator, l)?;
        if names.is_empty() || names.len() > 65 {
            return Err(Error::Budget(
                "bibliographic retained Claim archive member count",
            ));
        }
        let mut blobs = BTreeMap::new();
        let mut total = 0usize;
        for name in names {
            if name != "manifest.json"
                && (name.len() != 69
                    || !name.ends_with(".blob")
                    || !name[..64]
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
            {
                return Err(Error::Invalid(
                    "bibliographic Claim archive non-blob member",
                ));
            }
            let raw = self.required(&format!("{prefix}{name}"), validator, l)?;
            if raw.len() > 2 * 1024 * 1024 {
                return Err(Error::Budget("bibliographic archive package member bytes"));
            }
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= 10 * 1024 * 1024)
                .ok_or(Error::Budget("bibliographic retained Claim archive bytes"))?;
            blobs.insert(name, raw);
        }
        let manifest_raw = blobs.remove("manifest.json").ok_or(Error::Invalid(
            "bibliographic Claim archive manifest absent",
        ))?;
        let manifest = owned(&manifest_raw, l.catalog.max_row_bytes)?;
        let selected = manifest["schema_version"] == "tos_source_package_archive_v2";
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
        exact_keys(&manifest, &keys)?;
        if !matches!(
            manifest["schema_version"].as_str(),
            Some("tos_source_package_archive_v1" | "tos_source_package_archive_v2")
        ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
            || manifest["source_path"] != reference
            || manifest["source"] != *previous
            || manifest["revision"] != revision
        {
            return Err(Error::Invalid(
                "bibliographic Claim archive exact manifest binding",
            ));
        }
        let bindings = manifest["files"]
            .as_object()
            .filter(|files| !files.is_empty() && files.len() <= 64)
            .ok_or(Error::Invalid("bibliographic Claim archive file bindings"))?;
        let basename = reference.rsplit('/').next().unwrap();
        let selected_names = [
            basename,
            "source-claims.human-forms.json",
            "source-revision-history.json",
        ];
        let mut files = BTreeMap::new();
        let mut locations = BTreeMap::new();
        let mut used = BTreeSet::new();
        for (name, binding) in bindings {
            exact_keys(binding, &["blob", "sha256", "bytes"])?;
            if name.is_empty()
                || name.contains(['/', '\\', '\0'])
                || matches!(
                    name.as_str(),
                    "." | ".."
                        | "payload"
                        | "owner-local"
                        | "private"
                        | "local-content"
                        | "catalog"
                )
                || name.starts_with('.') && !public_form_lock(name)
                || selected && !selected_names.contains(&name.as_str())
            {
                return Err(Error::Invalid(
                    "bibliographic Claim archive public member scope",
                ));
            }
            let sha = hash(text(binding, "sha256")?)?;
            let blob = format!("{sha}.blob");
            if binding["blob"] != blob {
                return Err(Error::Invalid("bibliographic Claim archive blob binding"));
            }
            let raw = blobs
                .get(&blob)
                .ok_or(Error::Invalid("bibliographic Claim archive blob absent"))?;
            if raw.len() as u64 != binding["bytes"].as_u64().unwrap_or(u64::MAX)
                || format!("sha256:{}", Digest256::of_bytes(raw).to_hex())
                    != text(binding, "sha256")?
            {
                return Err(Error::Invalid(
                    "bibliographic Claim archive raw byte binding",
                ));
            }
            if public_form_lock(name) && !raw.is_empty() {
                return Err(Error::Invalid(
                    "bibliographic archived writer lock contains bytes",
                ));
            }
            files.insert(name.clone(), raw.clone());
            used.insert(blob.clone());
            locations.insert(name.clone(),json!({"archive_path":format!("{prefix}{blob}"),"sha256":binding["sha256"],"bytes":binding["bytes"]}));
        }
        if used != blobs.keys().cloned().collect::<BTreeSet<_>>()
            || package_revision(&files, l)? != revision
        {
            return Err(Error::Invalid(
                "bibliographic Claim archive complete package binding",
            ));
        }
        Ok((files, locations))
    }
    fn native_membership(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        entry: &Value,
        current: &Value,
        reference: &str,
        files: &BTreeMap<String, Vec<u8>>,
        validator: &SourceCatalogValidator<'_>,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Version> {
        let basename = reference.rsplit('/').next().unwrap();
        let stream = files.get(basename).ok_or(Error::Invalid(
            "bibliographic native Claim package stream absent",
        ))?;
        let current_records = claim_records(stream, l)?;
        let id = text(exact, "id")?;
        let current_record = current_records
            .get(id)
            .ok_or(Error::Invalid("bibliographic current membership missing"))?;
        if record_ref(current_record, true, l)? != *current {
            return Err(Error::Invalid(
                "bibliographic current membership catalog binding",
            ));
        }
        let retained = files.get("claim-revision-history.json");
        let history = if let Some(raw) = retained {
            owned(raw, l.catalog.max_row_bytes)?
        } else {
            json!({"schema_version":"tos_claim_revision_history_v1","source_path":reference,"receipts":[]})
        };
        exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
        let receipts = array(&history, "receipts")?;
        if history["schema_version"] != "tos_claim_revision_history_v1"
            || history["source_path"] != reference
            || receipts.len() > 128
        {
            return Err(Error::Invalid(
                "bibliographic native Claim correction history profile/bounds",
            ));
        }
        if receipts.is_empty()
            && current_records
                .values()
                .any(|record| record["claim_version"] != 1)
        {
            return Err(Error::Invalid(
                "bibliographic noninitial Claim stream missing correction history",
            ));
        }
        let revision = package_revision(files, l)?;
        let current_source = json!({"source_ref":reference,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(stream).to_hex()),
            "stream_bytes":stream.len(),"package_revision":revision,"archive_blob_ref":null,"line":entry["source_claim_line"]});
        let mut chosen = if current == exact {
            Some((
                current_record.clone(),
                current_source,
                Value::Null,
                "current",
            ))
        } else {
            None
        };
        let mut expected: Option<Vec<u8>> = None;
        let mut commands = BTreeSet::new();
        let mut historical = BTreeSet::new();
        for receipt in receipts {
            check(validator, l)?;
            claim_receipt(receipt, &mut commands, l)?;
            let (archived, locations) = self.archive_package(reference, receipt, validator, l)?;
            let before = archived
                .get(basename)
                .ok_or(Error::Invalid("bibliographic archived Claim stream absent"))?;
            let previous_records = claim_records(before, l)?;
            if expected.as_ref().is_some_and(|expected| expected != before)
                || expected.is_none()
                    && previous_records
                        .values()
                        .any(|record| record["claim_version"] != 1)
            {
                return Err(Error::Invalid(
                    "bibliographic complete shared Claim correction sequence",
                ));
            }
            let before_id = text(&receipt["previous_source"], "id")?;
            let previous = previous_records.get(before_id).ok_or(Error::Invalid(
                "bibliographic corrected Claim predecessor absent",
            ))?;
            if record_ref(previous, true, l)? != receipt["previous_source"]
                || !historical.insert(encode(
                    &receipt["previous_source"],
                    l.catalog.max_row_bytes,
                )?)
            {
                return Err(Error::Invalid(
                    "bibliographic exact Claim predecessor binding/uniqueness",
                ));
            }
            let revised = advance_claim(previous, &receipt["request"], l)?;
            if record_ref(&revised, true, l)? != receipt["source"] {
                return Err(Error::Invalid(
                    "bibliographic retained Claim successor reconstruction",
                ));
            }
            let formname = claim_form_name(reference, before_id)?;
            let prior = archived
                .get(&formname)
                .map(|raw| owned(raw, l.catalog.max_row_bytes))
                .transpose()?;
            let current_forms = files.get(&formname).ok_or(Error::Invalid(
                "bibliographic corrected Claim current form set absent",
            ))?;
            let current_forms = owned(current_forms, l.catalog.max_row_bytes)?;
            let current_source = current_records.get(before_id).ok_or(Error::Invalid(
                "bibliographic corrected sibling absent from current stream",
            ))?;
            let current_materializations =
                forms.materialize(current_source, &current_forms, 262_144)?;
            encode(&json!(current_materializations), 262_144)?;
            reconstruct_forms(
                stage,
                &revised,
                prior.as_ref(),
                &current_forms,
                receipt,
                validator,
                forms,
                l,
            )?;
            if receipt["previous_source"] == *exact {
                if chosen.is_some() {
                    return Err(Error::Invalid(
                        "bibliographic duplicated exact retained membership",
                    ));
                }
                let line = claim_line(before, before_id, l)?;
                let location = locations.get(basename).ok_or(Error::Invalid(
                    "bibliographic archived Claim source location",
                ))?;
                let source = json!({"source_ref":reference,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(before).to_hex()),
                    "stream_bytes":before.len(),"package_revision":receipt["previous_revision"],"archive_blob_ref":location["archive_path"],"line":line});
                chosen = Some((
                    previous.clone(),
                    source,
                    receipt_transition(receipt)?,
                    "historical",
                ));
            }
            expected = Some(replace_claim(before, &revised, l)?);
        }
        if expected.is_some_and(|expected| expected != *stream) {
            return Err(Error::Invalid(
                "bibliographic current shared Claim stream not history head",
            ));
        }
        let (record, source, transition, version_status) = chosen.ok_or(Error::Invalid(
            "bibliographic exact membership version/digest not retained",
        ))?;
        let catalog = legacy_catalog(stage, "claims", None, id, l)?;
        let provenance = json!({"catalog":{"source_ref":"ToS/source-witnesses/catalog/claims.jsonl","line":catalog.0,"sha256":catalog.1,
            "source_claim_file_ref":reference,"source_claim_line":entry["source_claim_line"],"current_record_ref":current,"visibility":entry["visibility"]},
            "source":source,"history":{"source_ref":adjacent(reference,"claim-revision-history.json")?,
                "sha256":retained.map(|raw|format!("sha256:{}",Digest256::of_bytes(raw).to_hex())),"receipt_count":receipts.len(),"correction_chain_verified":true},
            "transition":transition});
        Ok(Version {
            record,
            provenance,
            refs: Vec::new(),
            current_ref: current.clone(),
            version_status,
        })
    }
    pub(crate) fn resolve_claim(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        validator: &SourceCatalogValidator<'_>,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Version> {
        exact_ref(exact, true)?;
        let id = text(exact, "id")?;
        let row = catalog::catalog_row(stage, "claims", id, l.catalog)?.ok_or(Error::Invalid(
            "bibliographic exact membership not catalogued",
        ))?;
        let entry = &row["entry"];
        let (record, location) = graph::slot(stage, "claim", id, l)?
            .ok_or(Error::Invalid("bibliographic membership exact source slot"))?;
        let current = record_ref(&record, true, l)?;
        let reference = text(entry, "source_claim_file_ref")?;
        let raw = self.required(reference, validator, l)?;
        if Digest256::of_bytes(&raw).to_hex() != text(&location, "file_sha256")?
            || raw.len() as u64 != location["file_bytes"].as_u64().unwrap_or(u64::MAX)
        {
            return Err(Error::Invalid(
                "bibliographic membership exact stream binding",
            ));
        }
        // Legacy membership has no native revision package. Every sibling is
        // checked to keep one selected row from hiding a restricted carrier.
        let parts = reference.split('/').collect::<Vec<_>>();
        let legacy = parts.len() == 6
            && parts[..3] == ["ToS", "source-witnesses", "collections"]
            && parts[5] == "membership-claims.jsonl";
        let history = json!({"source_ref":null,"sha256":null,"receipt_count":0,
            "correction_chain_verified":false,"adapter":"retained-collection-membership-v1"});
        let revision = Value::Null;
        if !legacy {
            if parts.len() < 4
                || parts[5.min(parts.len() - 1)] == "membership-claims.jsonl"
                || !reference.ends_with("/source-claims.jsonl")
            {
                return Err(Error::Invalid(
                    "bibliographic membership source-family route",
                ));
            }
            let base = reference.rsplit_once('/').unwrap().0;
            let files = self.package(base, validator, l)?;
            return self.native_membership(
                stage, exact, entry, &current, reference, &files, validator, forms, l,
            );
        }
        if current != *exact {
            return Err(Error::Invalid(
                "bibliographic legacy membership exact version not retained",
            ));
        }
        let mut siblings = BTreeSet::new();
        if raw.len() > 1_048_576 {
            return Err(Error::Budget(
                "bibliographic retained membership stream bytes",
            ));
        }
        for bytes in raw
            .split(|b| *b == b'\n')
            .filter(|row| !row.iter().all(u8::is_ascii_whitespace))
        {
            check(validator, l)?;
            let sibling = owned(bytes, l.catalog.max_row_bytes)?;
            exact_ref(&record_ref(&sibling, true, l)?, true)?;
            if !siblings.insert(text(&sibling, "claim_id")?.to_owned())
                || !matches!(
                    sibling["visibility"].as_str(),
                    Some("public" | "public_metadata_only")
                )
                || legacy
                    && (sibling["schema_version"] != "tos_claim_packet_v1"
                        || sibling["claim_type"] != "bibliographic"
                        || sibling["predicate"] != "contains_work"
                        || sibling["assertion_layer"] != "bibliographic_assertion"
                        || sibling.get("polarity").is_some_and(|v| v != "positive")
                        || !text(&sibling, "subject_ref")?.starts_with("tos.collection.")
                        || !text(&sibling, "object")?.starts_with("tos.work."))
                || !legacy && (sibling["claim_type"] != "relation" || sibling["claim_version"] != 1)
            {
                return Err(Error::Invalid(
                    "bibliographic public exact membership stream contract",
                ));
            }
        }
        let catalog = legacy_catalog(stage, "claims", None, id, l)?;
        let provenance = json!({"catalog":{"source_ref":"ToS/source-witnesses/catalog/claims.jsonl",
            "line":catalog.0,"sha256":catalog.1,"source_claim_file_ref":reference,
            "source_claim_line":entry["source_claim_line"],"current_record_ref":current,"visibility":entry["visibility"]},
            "source":{"source_ref":reference,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(&raw).to_hex()),
                "stream_bytes":raw.len(),"package_revision":revision,"archive_blob_ref":null,"line":entry["source_claim_line"]},
            "history":history,"transition":null});
        Ok(Version {
            record,
            provenance,
            refs: Vec::new(),
            current_ref: current.clone(),
            version_status: "current",
        })
    }
}
/// Owner descriptors selected from the already sealed registry and catalog.
/// This is the maintained MetadataVersionReader dispatch, not an admission API.
struct RecordRoute {
    kind: String,
    adapter: &'static str,
    identity: &'static str,
    basename: String,
    catalog_filename: String,
    type_id: String,
    schema: String,
}
impl RecordRoute {
    fn derive(entry: &Value, record: &Value, entities: &Value) -> Result<Self> {
        let kind = text(entry, "record_type")?;
        let reference = text(entry, "source_record_ref")?;
        let native_witness = kind == "artifact"
            || kind == "composite" && reference.ends_with("/composite-witness.json");
        let native_corpus = [
            "agent",
            "place",
            "organization",
            "work",
            "expression",
            "edition",
            "collection",
            "item",
        ]
        .contains(&kind);
        let types = array(entities, "types")?;
        let matches = types
            .iter()
            .filter(|e| {
                e.get("source_mappings")
                    .and_then(Value::as_array)
                    .is_some_and(|ms| {
                        ms.iter().any(|m| {
                            m["source_graph"] == "source-navigation" && m["source_kind_id"] == kind
                        })
                    })
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::Invalid(
                "bibliographic metadata owner registry mapping",
            ));
        }
        let owner = matches[0];
        let type_id = text(owner, "type_id")?.to_owned();
        let (adapter, identity, basename, filename, schema) = if native_witness {
            let (identity, basename, filename) = if kind == "artifact" {
                ("artifact_id", "artifact-witness.json", "artifacts.jsonl")
            } else {
                ("composite_id", "composite-witness.json", "composites.jsonl")
            };
            let schema = match (kind, record["schema_version"].as_str()) {
                ("artifact", Some("tos_artifact_source_witness_v1")) => {
                    "ToS/contracts/artifact-source-witness.schema.json"
                }
                ("artifact", Some("tos_artifact_source_witness_v2")) => {
                    "ToS/contracts/artifact-source-witness-v2.schema.json"
                }
                ("composite", Some("tos_scholarly_composite_witness_v1")) => {
                    "ToS/contracts/scholarly-composite-witness.schema.json"
                }
                _ => {
                    return Err(Error::Invalid(
                        "bibliographic native witness exact schema descriptor",
                    ));
                }
            };
            if entry["source_schema_ref"] != schema {
                return Err(Error::Invalid(
                    "bibliographic native witness catalog schema binding",
                ));
            }
            (
                "native-witness",
                identity,
                basename.to_owned(),
                filename.to_owned(),
                schema.to_owned(),
            )
        } else if kind == "link" {
            (
                "native-link",
                "record_id",
                "link.json".into(),
                "links.jsonl".into(),
                "ToS/contracts/source-link.schema.json".into(),
            )
        } else if native_corpus {
            let filename = match kind {
                "agent" => "agents.jsonl",
                "place" => "places.jsonl",
                "organization" => "organizations.jsonl",
                "work" => "works.jsonl",
                "expression" => "expressions.jsonl",
                "edition" => "editions.jsonl",
                "collection" => "collections.jsonl",
                "item" => "items.jsonl",
                _ => unreachable!(),
            };
            (
                "native-corpus",
                "record_id",
                format!("{kind}.json"),
                filename.into(),
                "ToS/contracts/corpus-record.schema.json".into(),
            )
        } else {
            let profile = &owner["source_record_profile"];
            if profile["record_type"] != kind || profile["id_prefix"] != format!("tos.{kind}.") {
                return Err(Error::Invalid(
                    "bibliographic declared metadata owner profile",
                ));
            }
            let schemas = array(profile, "schemas")?
                .iter()
                .filter(|s| s["schema_version"] == record["schema_version"])
                .collect::<Vec<_>>();
            if schemas.len() != 1 {
                return Err(Error::Invalid(
                    "bibliographic metadata selected schema profile",
                ));
            }
            let schema = text(schemas[0], "schema_ref")?;
            if entry["source_schema_ref"] != schema {
                return Err(Error::Invalid(
                    "bibliographic metadata catalog schema binding",
                ));
            }
            (
                "declared-profile",
                "record_id",
                text(profile, "source_basename")?.into(),
                text(profile, "catalog_filename")?.into(),
                schema.into(),
            )
        };
        let parts = reference.split('/').collect::<Vec<_>>();
        if parts.len() < 5
            || reference.contains(['\\', '\0'])
            || parts[..2] != ["ToS", "source-witnesses"]
            || parts.last() != Some(&basename.as_str())
            || parts.iter().any(|p| {
                p.is_empty()
                    || *p == ".."
                    || p.starts_with('.')
                    || [
                        "catalog",
                        "owner-local",
                        "payload",
                        "local-content",
                        "private",
                    ]
                    .contains(p)
            })
            || adapter == "native-link" && parts[2] != "links"
            || adapter == "native-witness"
                && parts[2]
                    != if kind == "artifact" {
                        "artifacts"
                    } else {
                        "scholarly-composites"
                    }
            || adapter == "declared-profile"
                && kind == "composite"
                && (parts.len() < 7 || parts[2] != "scholarly-composites")
        {
            return Err(Error::Invalid(
                "bibliographic metadata exact owner source home",
            ));
        }
        Ok(Self {
            kind: kind.into(),
            adapter,
            identity,
            basename,
            catalog_filename: filename,
            type_id,
            schema,
        })
    }
    fn validate_record(
        &self,
        record: &Value,
        reference: &Value,
        schema_version: &Value,
    ) -> Result<()> {
        if record[self.identity] != reference["id"]
            || !text(reference, "id")?.starts_with(&format!("tos.{}.", self.kind))
            || record["schema_version"] != *schema_version
            || !schema_version.is_string()
            || self.adapter != "native-witness" && record["record_type"] != self.kind
        {
            return Err(Error::Invalid(
                "bibliographic metadata descriptor changed within history",
            ));
        }
        let public = |v: &Value| matches!(v.as_str(), Some("public" | "public_metadata_only"));
        let valid = match self.adapter {
            "native-corpus" => {
                record["schema_version"] == "tos_corpus_record_v1"
                    && record.get("visibility").is_none()
            }
            "native-link" => {
                record["schema_version"] == "tos_source_link_v1"
                    && record.get("visibility").is_none()
            }
            "native-witness" => public(&record["authority"]["visibility"]),
            _ => public(&record["visibility"]),
        };
        if !valid {
            return Err(Error::Invalid(
                "bibliographic metadata public owner profile",
            ));
        }
        Ok(())
    }
    fn revision_fields(&self) -> Vec<&'static str> {
        match self.adapter {
            "native-corpus" => vec!["preferred_label", "notes", "field_languages", "source_refs"],
            "declared-profile" => vec![
                "preferred_label",
                "variant_labels",
                "notes",
                "field_languages",
                "source_refs",
                "extensions",
                "semantic_content",
                "semantic_scope",
            ],
            _ => match self.kind.as_str() {
                "artifact" => vec![
                    "path_identity",
                    "physical_description",
                    "find_context",
                    "bibliography",
                ],
                "composite" => vec!["preferred_label", "editorial_object"],
                "link" => vec![
                    "preferred_label",
                    "variant_labels",
                    "notes",
                    "source_refs",
                    "provider_label",
                ],
                _ => Vec::new(),
            },
        }
    }
    fn validate_descriptive_delta(&self, previous: &Value, revised: &Value) -> Result<()> {
        if self.adapter == "native-witness"
            && self.kind == "artifact"
            && ["basis", "provider_independent"]
                .iter()
                .any(|k| previous["path_identity"][*k] != revised["path_identity"][*k])
        {
            return Err(Error::Invalid(
                "bibliographic artifact physical identity path changed",
            ));
        }
        Ok(())
    }
    fn descriptor(&self, schema_version: &Value) -> Value {
        json!({"adapter":self.adapter,"record_type":self.kind,"profile_type_id":self.type_id,"source_schema_ref":self.schema,
            "source_schema_version":schema_version,"source_scope":"public_metadata_only","record_kind":"subject","identity_field":self.identity,
            "source_basename":self.basename,"schema_version":schema_version,"schema_ref":self.schema,"type_id":self.type_id})
    }
}
fn metadata_ref(record: &Value, route: &RecordRoute, l: BibliographicLimits) -> Result<Value> {
    let reference = json!({"id":record[route.identity],"version":record["record_version"],"digest":format!("sha256:{}",digest(record,l.catalog.max_row_bytes)?)});
    exact_ref(&reference, false)?;
    Ok(reference)
}
struct CompoundProfile {
    parent: &'static str,
    field: &'static str,
    child: &'static str,
    predicate: &'static str,
    schema: &'static str,
    extra: &'static [&'static str],
}
fn compound_profile(operation: &str) -> Option<CompoundProfile> {
    let (parent, field, child, predicate, schema, extra) = match operation {
        "collection.work.attach" => (
            "collection",
            "membership_claim_refs",
            "work",
            "contains_work",
            "tos_local_collection_membership_command_v1",
            &["work", "claim", "forms", "claim_forms", "reason"][..],
        ),
        "work.expression.create" => (
            "work",
            "expression_claim_refs",
            "record",
            "has_expression",
            "tos_local_work_expression_command_v1",
            &[
                "record",
                "claim",
                "forms",
                "expression_forms",
                "claim_forms",
                "reason",
            ][..],
        ),
        "expression.responsibility.attach" => (
            "expression",
            "responsibility_claim_refs",
            "agent",
            "translated_by",
            "tos_local_expression_responsibility_command_v1",
            &["agent", "claim", "forms", "claim_forms", "reason"][..],
        ),
        "expression.edition.create" => (
            "expression",
            "embodiment_claim_refs",
            "record",
            "embodied_by",
            "tos_local_expression_edition_command_v1",
            &[
                "record",
                "claim",
                "forms",
                "edition_forms",
                "claim_forms",
                "reason",
            ][..],
        ),
        "item.adopt" => (
            "edition",
            "exemplar_claim_refs",
            "record",
            "exemplified_by",
            "tos_local_item_adoption_command_v1",
            &[
                "record",
                "claim",
                "forms",
                "item_forms",
                "claim_forms",
                "reason",
                "rights",
                "item_kind",
                "inventory",
                "inventory_limitation",
                "fixity_verified_at",
            ][..],
        ),
        _ => return None,
    };
    Some(CompoundProfile {
        parent,
        field,
        child,
        predicate,
        schema,
        extra,
    })
}

pub(crate) struct Version {
    pub(crate) record: Value,
    pub(crate) provenance: Value,
    pub(crate) version_status: &'static str,
    pub(crate) refs: Vec<Value>,
    pub(crate) current_ref: Value,
}
/// Retained JSON uses the same exact scalar transport as current catalog rows.
/// Compare owner canonical bytes before any digest-based lineage reconstruction.
fn owned(raw: &[u8], cap: usize) -> Result<Value> {
    let value = SourceRow::parse(raw, cap)?.value().clone();
    let original = canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("bibliographic retained scalar limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if original != encode(&value, cap)? {
        return Err(Error::Invalid(
            "bibliographic retained exact scalar transport",
        ));
    }
    Ok(value)
}
fn check(validator: &SourceCatalogValidator<'_>, l: BibliographicLimits) -> Result<()> {
    if std::time::Instant::now() >= l.deadline
        || validator
            .cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    {
        Err(Error::Budget(
            "bibliographic selected version deadline/cancel",
        ))
    } else {
        Ok(())
    }
}
fn adjacent(reference: &str, name: &str) -> Result<String> {
    Ok(format!(
        "{}/{}",
        reference
            .rsplit_once('/')
            .ok_or(Error::Invalid("bibliographic version source locator"))?
            .0,
        name
    ))
}
fn exact_ref(reference: &Value, claim: bool) -> Result<()> {
    let value = reference
        .as_object()
        .ok_or(Error::Invalid("bibliographic exact version reference"))?;
    let id = text(reference, "id")?;
    let sha = text(reference, "digest")?
        .strip_prefix("sha256:")
        .ok_or(Error::Invalid("bibliographic exact version digest"))?;
    let parts = id
        .strip_prefix("tos.")
        .unwrap_or("")
        .splitn(2, '.')
        .collect::<Vec<_>>();
    let identity_valid = parts.len() == 2
        && !parts[0].is_empty()
        && parts[0]
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
        && parts[0]
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && (parts[0] == "claim") == claim
        && !parts[1].is_empty()
        && parts[1].split(['.', '-']).all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if value.len() != 3
        || !identity_valid
        || reference["version"]
            .as_u64()
            .is_none_or(|n| !(1..=9_007_199_254_740_991).contains(&n))
        || sha.len() != 64
        || !sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::Invalid(
            "bibliographic exact version reference grammar",
        ));
    }
    Ok(())
}
fn record_ref(record: &Value, claim: bool, l: BibliographicLimits) -> Result<Value> {
    let reference = if claim {
        json!({"id":record["claim_id"],"version":record["claim_version"],
        "digest":format!("sha256:{}",digest(record,l.catalog.max_row_bytes)?)})
    } else {
        json!({"id":record["record_id"],"version":record["record_version"],
        "digest":format!("sha256:{}",digest(record,l.catalog.max_row_bytes)?)})
    };
    exact_ref(&reference, claim)?;
    Ok(reference)
}
fn legacy_catalog(
    stage: &mut KnowledgeStage<'_>,
    category: &str,
    kind: Option<&str>,
    id: &str,
    l: BibliographicLimits,
) -> Result<(u64, String)> {
    let mut hash = Digest256Hasher::new();
    let mut after = None;
    let mut line = 0u64;
    let mut selected = None;
    while let Some(key) = catalog::catalog_next(stage, category, after.as_deref())? {
        if std::time::Instant::now() >= l.deadline {
            return Err(Error::Budget("bibliographic exact catalog render deadline"));
        }
        let row = catalog::catalog_row(stage, category, &key, l.catalog)?
            .ok_or(Error::Invalid("bibliographic version catalog disappeared"))?;
        if kind.is_none_or(|kind| row["entry"]["record_type"] == kind) {
            line += 1;
            hash.update(&encode(&row["entry"], l.catalog.max_output_row_bytes)?);
            hash.update(b"\n");
            if key == id {
                selected = Some(line);
            }
        }
        after = Some(key);
    }
    Ok((
        selected.ok_or(Error::Invalid("bibliographic exact legacy catalog address"))?,
        format!("sha256:{}", hash.finalize().to_hex()),
    ))
}
fn bind_files(provenance: &Value, inputs: &mut BTreeMap<String, String>) -> Result<()> {
    let mut bind = |reference: &Value, sha: &Value| -> Result<()> {
        if reference.is_null() || sha.is_null() {
            return Ok(());
        }
        let reference = reference
            .as_str()
            .ok_or(Error::Invalid("bibliographic version provenance ref"))?;
        let sha = sha
            .as_str()
            .ok_or(Error::Invalid("bibliographic version provenance digest"))?
            .strip_prefix("sha256:")
            .ok_or(Error::Invalid("bibliographic version digest prefix"))?;
        if let Some(previous) = inputs.insert(reference.into(), sha.into()) {
            if previous != sha {
                return Err(Error::Invalid(
                    "bibliographic conflicting exact version input",
                ));
            }
        }
        Ok(())
    };
    for section in ["catalog", "history"] {
        bind(
            &provenance[section]["source_ref"],
            &provenance[section]["sha256"],
        )?;
    }
    let source = &provenance["source"];
    let location = source
        .get("archive_blob_ref")
        .filter(|v| !v.is_null())
        .unwrap_or(&source["source_ref"]);
    let sha = source
        .get("record_sha256")
        .filter(|v| !v.is_null())
        .unwrap_or(&source["stream_sha256"]);
    bind(location, sha)?;
    bind(
        &source["archive_manifest_ref"],
        &source["archive_manifest_sha256"],
    )
}

fn exact_keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or(Error::Invalid("bibliographic exact retained object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Invalid("bibliographic exact retained object fields"));
    }
    Ok(())
}

fn hash(value: &str) -> Result<&str> {
    let sha = value
        .strip_prefix("sha256:")
        .ok_or(Error::Invalid("bibliographic retained digest prefix"))?;
    if sha.len() != 64
        || !sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::Invalid("bibliographic retained digest grammar"));
    }
    Ok(sha)
}
fn receipt_transition(receipt: &Value) -> Result<Value> {
    let mut transition = serde_json::Map::new();
    for key in [
        "command_id",
        "recorded_at",
        "previous_source",
        "source",
        "request_digest",
    ] {
        transition.insert(
            key.into(),
            receipt
                .get(key)
                .ok_or(Error::Invalid("bibliographic retained transition field"))?
                .clone(),
        );
    }
    Ok(Value::Object(transition))
}
fn metadata_receipt(
    receipt: &Value,
    history: &Value,
    id: &Value,
    head: Option<&Value>,
    commands: &mut BTreeSet<String>,
    route: &RecordRoute,
    l: BibliographicLimits,
) -> Result<()> {
    let selected = receipt.get("publication").is_some();
    let mut keys = vec![
        "command_id",
        "request_digest",
        "principal_id",
        "authority_ref",
        "owner_configuration",
        "recorded_at",
        "reason",
        "previous_source",
        "source",
        "previous_revision",
        "archive_path",
        "dependencies",
        "changed_fields",
        "forms",
        "grants_admission",
        "request",
    ];
    if selected {
        keys.push("publication");
    }
    exact_keys(receipt, &keys)?;
    let request = &receipt["request"];
    let operation = text(request, "operation")?;
    let compound = compound_profile(operation);
    if let Some(profile) = compound {
        let mut request_keys = vec![
            "schema_version",
            "operation",
            "command_id",
            "fields",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "expected_publication",
        ];
        request_keys.extend_from_slice(profile.extra);
        exact_keys(request, &request_keys)?;
        if request["schema_version"] != profile.schema
            || route.adapter != "native-corpus"
            || route.kind != profile.parent
            || !selected
        {
            return Err(Error::Invalid(
                "bibliographic compound retained metadata parent profile",
            ));
        }
        encode(request, 1_048_576)?;
        for key in [
            "expected_configuration",
            "expected_revision",
            "expected_dependencies",
        ] {
            hash(text(request, key)?)?;
        }
        if !request["expected_publication"].is_null() {
            hash(text(request, "expected_publication")?)?;
        }
        let command = text(request, "command_id")?;
        let reason = text(request, "reason")?;
        if !(1..=256).contains(&command.chars().count())
            || !(1..=4096).contains(&reason.trim().chars().count())
            || request["claim"]["predicate"] != profile.predicate
            || request["claim"]["subject_ref"] != *id
            || request["claim"]["object"] != request[profile.child]["record_id"]
        {
            return Err(Error::Invalid(
                "bibliographic compound retained exact endpoints",
            ));
        }
        exact_keys(&request["fields"], &[profile.field])?;
        if array(&request["fields"], profile.field)?.last() != Some(&request["claim"]["claim_id"])
            || operation == "work.expression.create" && request["record"]["work_ref"] != *id
            || operation == "expression.edition.create"
                && request["record"]["embodies_expression_refs"] != json!([id])
        {
            return Err(Error::Invalid(
                "bibliographic compound retained parent field append",
            ));
        }
        if operation == "item.adopt" {
            let at = text(request, "fixity_verified_at")?;
            tos_validation::retirement_rules::observed_instant_order(at, at)
                .map_err(|_| Error::Invalid("bibliographic retained Item fixity instant"))?;
            if request["inventory"].is_null() != !request["inventory_limitation"].is_null() {
                return Err(Error::Invalid(
                    "bibliographic retained Item inventory completeness",
                ));
            }
        }
        let transaction = json!({"operation":operation,"command_id":request["command_id"],"owner_configuration":request["expected_configuration"],"request_digest":format!("sha256:{}",digest(request,l.catalog.max_row_bytes)?)});
        if receipt["publication"]["transaction_id"]
            != format!("sha256:{}", digest(&transaction, l.catalog.max_row_bytes)?)
        {
            return Err(Error::Invalid(
                "bibliographic compound retained transaction binding",
            ));
        }
    } else {
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
        if selected {
            request_keys.push("expected_publication");
        }
        exact_keys(request, &request_keys)?;
        if request["schema_version"] != "tos_local_source_command_v1"
            || request["operation"] != "record.revise"
        {
            return Err(Error::Invalid(
                "bibliographic Collection retained correction request",
            ));
        }
    }
    if selected {
        exact_keys(
            &receipt["publication"],
            &["protocol", "transaction_id", "selected_files"],
        )?;
        let mut selected_files = vec![
            route.basename.clone(),
            format!(
                "{}.human-forms.json",
                route.basename.trim_end_matches(".json")
            ),
            "source-revision-history.json".into(),
        ];
        selected_files.sort();
        if history["schema_version"] != "tos_source_revision_history_v2"
            || receipt["publication"]["protocol"] != "tos_selected_source_metadata_v1"
            || receipt["publication"]["selected_files"] != json!(selected_files)
            || !receipt["publication"]["transaction_id"].is_string()
        {
            return Err(Error::Invalid(
                "bibliographic selected metadata retained publication binding",
            ));
        }
    }
    let instant = text(receipt, "recorded_at")?;
    tos_validation::retirement_rules::observed_instant_order(instant, instant)
        .map_err(|_| Error::Invalid("bibliographic retained correction aware instant"))?;
    exact_ref(&receipt["previous_source"], false)?;
    exact_ref(&receipt["source"], false)?;
    let fields = request["fields"]
        .as_object()
        .ok_or(Error::Invalid("bibliographic retained correction fields"))?;
    let mut changed = fields
        .keys()
        .map(|key| Value::String(key.clone()))
        .collect::<Vec<_>>();
    changed.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    if !commands.insert(text(receipt, "command_id")?.to_owned())
        || receipt["request_digest"]
            != format!("sha256:{}", digest(request, l.catalog.max_row_bytes)?)
        || receipt["changed_fields"] != Value::Array(changed)
        || receipt["grants_admission"] != false
        || receipt["source"]["id"] != *id
        || receipt["previous_source"]["id"] != *id
        || receipt["source"]["version"].as_u64()
            != receipt["previous_source"]["version"]
                .as_u64()
                .and_then(|n| n.checked_add(1))
        || head.is_some_and(|head| receipt["previous_source"] != *head)
    {
        return Err(Error::Invalid(
            "bibliographic retained Collection continuous lineage",
        ));
    }
    for (left, right) in [
        ("command_id", "command_id"),
        ("previous_source", "expected_source"),
        ("previous_revision", "expected_revision"),
        ("owner_configuration", "expected_configuration"),
        ("dependencies", "expected_dependencies"),
        ("reason", "reason"),
    ] {
        if receipt[left] != request[right] {
            return Err(Error::Invalid(
                "bibliographic retained correction exact request binding",
            ));
        }
    }
    Ok(())
}

fn package_revision(files: &BTreeMap<String, Vec<u8>>, l: BibliographicLimits) -> Result<String> {
    let bindings = files
        .iter()
        .map(|(name, raw)| {
            (
                name.clone(),
                json!({
        "sha256":format!("sha256:{}",Digest256::of_bytes(raw).to_hex()),"bytes":raw.len()}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Ok(format!(
        "sha256:{}",
        digest(&Value::Object(bindings), l.max_claim_cohort_bytes)?
    ))
}
fn claim_records(raw: &[u8], l: BibliographicLimits) -> Result<BTreeMap<String, Value>> {
    if raw.len() > 1_048_576 {
        return Err(Error::Budget(
            "bibliographic exact version Claim stream bytes",
        ));
    }
    let mut records = BTreeMap::new();
    for bytes in raw
        .split(|b| *b == b'\n')
        .filter(|row| !row.iter().all(u8::is_ascii_whitespace))
    {
        if std::time::Instant::now() >= l.deadline {
            return Err(Error::Budget("bibliographic Claim version parse deadline"));
        }
        let record = owned(bytes, l.catalog.max_row_bytes)?;
        exact_ref(&record_ref(&record, true, l)?, true)?;
        if record["claim_type"] != "relation"
            || !matches!(
                record["visibility"].as_str(),
                Some("public" | "public_metadata_only")
            )
        {
            return Err(Error::Invalid(
                "bibliographic public versioned Claim carrier",
            ));
        }
        if records
            .insert(text(&record, "claim_id")?.into(), record)
            .is_some()
        {
            return Err(Error::Invalid(
                "bibliographic duplicate versioned Claim stream identity",
            ));
        }
    }
    Ok(records)
}
fn claim_line(raw: &[u8], id: &str, l: BibliographicLimits) -> Result<u64> {
    for (index, bytes) in raw.split(|b| *b == b'\n').enumerate() {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let row = SourceRow::parse(bytes, l.catalog.max_row_bytes)?;
        if row.value()["claim_id"] == id {
            return Ok(index as u64 + 1);
        }
    }
    Err(Error::Invalid(
        "bibliographic selected retained Claim source line",
    ))
}
fn claim_form_name(reference: &str, id: &str) -> Result<String> {
    let basename = reference
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("bibliographic Claim forms source locator"))?;
    let stem = basename
        .strip_suffix(".jsonl")
        .ok_or(Error::Invalid("bibliographic Claim forms stream suffix"))?;
    Ok(format!(
        "{stem}.{}.human-forms.json",
        Digest256::of_bytes(id.as_bytes()).to_hex()
    ))
}
fn advance_claim(previous: &Value, request: &Value, l: BibliographicLimits) -> Result<Value> {
    let fields = request["fields"]
        .as_object()
        .filter(|fields| !fields.is_empty())
        .ok_or(Error::Invalid(
            "bibliographic retained Claim correction fields",
        ))?;
    let transition = request.get("layer_transition");
    let allowed = if transition.is_some() {
        &["assertion_layer"][..]
    } else {
        &[
            "qualifiers",
            "evidence_refs",
            "counterevidence_refs",
            "alternative_claim_refs",
            "supporting_quotes",
            "epistemic_status",
            "confidence",
            "object",
        ][..]
    };
    if fields.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(Error::Invalid(
            "bibliographic Claim correction changed immutable identity/admission field",
        ));
    }
    if let Some(transition) = transition {
        exact_keys(transition, &["from", "to"])?;
        for key in ["from", "to"] {
            let name = text(transition, key)?;
            if name.is_empty()
                || name.len() > 64
                || !name.as_bytes()[0].is_ascii_lowercase()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            {
                return Err(Error::Invalid(
                    "bibliographic exact retained Claim layer transition",
                ));
            }
        }
        if transition["from"] == transition["to"]
            || transition["from"] != previous["assertion_layer"]
            || fields.get("assertion_layer") != transition.get("to")
        {
            return Err(Error::Invalid(
                "bibliographic retained Claim predecessor layer transition",
            ));
        }
    }
    if fields.contains_key("object")
        && (!previous["object"].is_object() || !fields["object"].is_object())
    {
        return Err(Error::Invalid(
            "bibliographic Claim correction cannot change identity endpoint",
        ));
    }
    let mut revised = previous
        .as_object()
        .ok_or(Error::Invalid("bibliographic Claim predecessor object"))?
        .clone();
    for (key, value) in fields {
        let value = if key == "qualifiers" {
            let mut merged = previous
                .get("qualifiers")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, value) in value
                .as_object()
                .ok_or(Error::Invalid("bibliographic Claim qualifier patch"))?
            {
                merged.insert(key.clone(), value.clone());
            }
            Value::Object(merged)
        } else {
            value.clone()
        };
        revised.insert(key.clone(), value);
    }
    if Value::Object(revised.clone()) == *previous {
        return Err(Error::Invalid(
            "bibliographic retained Claim correction is unchanged",
        ));
    }
    let version = previous["claim_version"]
        .as_u64()
        .and_then(|n| n.checked_add(1))
        .ok_or(Error::Invalid("bibliographic Claim version arithmetic"))?;
    revised.insert("claim_version".into(), json!(version));
    let revised = Value::Object(revised);
    encode(&revised, l.catalog.max_row_bytes)?;
    if matches!(
        previous["predicate"].as_str(),
        Some("identity_transition_proposal" | "subject_identity_transition_proposal")
    ) {
        for field in [
            "kind",
            "operation",
            "members",
            "predecessors",
            "successors",
            "mapping",
            "supersedes_proposal",
        ] {
            if revised["object"].get(field).is_none()
                || previous["object"].get(field) != revised["object"].get(field)
            {
                return Err(Error::Invalid(
                    "bibliographic retained proposal correction changed frozen topology",
                ));
            }
        }
    }
    Ok(revised)
}
fn replace_claim(raw: &[u8], revised: &Value, l: BibliographicLimits) -> Result<Vec<u8>> {
    claim_records(raw, l)?;
    let id = text(revised, "claim_id")?;
    let mut output = Vec::new();
    let mut found = false;
    for physical in raw.split_inclusive(|b| *b == b'\n') {
        let ending = if physical.ends_with(b"\r\n") {
            &b"\r\n"[..]
        } else if physical.ends_with(b"\n") {
            &b"\n"[..]
        } else {
            &b""[..]
        };
        let bytes = &physical[..physical.len() - ending.len()];
        if !bytes.iter().all(u8::is_ascii_whitespace)
            && SourceRow::parse(bytes, l.catalog.max_row_bytes)?.value()["claim_id"] == id
        {
            if found {
                return Err(Error::Invalid(
                    "bibliographic corrected Claim appears twice",
                ));
            }
            output.extend_from_slice(&encode(revised, l.catalog.max_row_bytes)?);
            output.extend_from_slice(ending);
            found = true;
        } else {
            output.extend_from_slice(physical);
        }
        if output.len() > 1_048_576 {
            return Err(Error::Budget("bibliographic corrected Claim stream bytes"));
        }
    }
    if !found {
        return Err(Error::Invalid(
            "bibliographic corrected Claim missing from predecessor stream",
        ));
    }
    Ok(output)
}
fn claim_receipt(
    receipt: &Value,
    commands: &mut BTreeSet<String>,
    l: BibliographicLimits,
) -> Result<()> {
    exact_keys(
        receipt,
        &[
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "source_bindings",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
        ],
    )?;
    let request = &receipt["request"];
    let instant = text(receipt, "recorded_at")?;
    tos_validation::retirement_rules::observed_instant_order(instant, instant)
        .map_err(|_| Error::Invalid("bibliographic retained Claim aware instant"))?;
    exact_ref(&receipt["previous_source"], true)?;
    exact_ref(&receipt["source"], true)?;
    let fields = request["fields"].as_object().ok_or(Error::Invalid(
        "bibliographic retained Claim request fields",
    ))?;
    let mut changed = fields.keys().map(|key| json!(key)).collect::<Vec<_>>();
    changed.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    if !commands.insert(text(receipt, "command_id")?.into())
        || request["operation"] != "claim.revise"
        || receipt["request_digest"]
            != format!("sha256:{}", digest(request, l.catalog.max_row_bytes)?)
        || receipt["changed_fields"] != Value::Array(changed)
        || receipt["grants_admission"] != false
        || receipt["source"]["id"] != receipt["previous_source"]["id"]
        || receipt["source"]["version"].as_u64()
            != receipt["previous_source"]["version"]
                .as_u64()
                .and_then(|n| n.checked_add(1))
    {
        return Err(Error::Invalid(
            "bibliographic native Claim correction receipt",
        ));
    }
    for (left, right) in [
        ("command_id", "command_id"),
        ("previous_source", "expected_source"),
        ("previous_revision", "expected_revision"),
        ("owner_configuration", "expected_configuration"),
        ("dependencies", "expected_dependencies"),
        ("source_bindings", "expected_inputs"),
        ("reason", "reason"),
    ] {
        if receipt[left] != request[right] {
            return Err(Error::Invalid(
                "bibliographic retained Claim request binding",
            ));
        }
    }
    Ok(())
}
fn form_ref(form: &Value, l: BibliographicLimits) -> Result<Value> {
    Ok(
        json!({"id":text(form,"form_id")?,"version":form["form_version"],"digest":format!("sha256:{}",digest(form,l.catalog.max_row_bytes)?)}),
    )
}
fn reconstruct_forms(
    stage: &KnowledgeStage<'_>,
    revised: &Value,
    prior: Option<&Value>,
    current: &Value,
    receipt: &Value,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<()> {
    for set in prior.into_iter().chain(std::iter::once(current)) {
        catalog::check_catalog_schema(
            stage,
            validator,
            l.catalog,
            "ToS/contracts/human-form-set.schema.json",
            "",
            &encode(set, l.catalog.max_row_bytes)?,
        )?;
        if set["subject"]["id"] != revised["claim_id"] {
            return Err(Error::Invalid("bibliographic retained form set subject"));
        }
    }
    let selections = &receipt["request"]["forms"];
    let selections_array = selections
        .as_array()
        .filter(|values| (1..=32).contains(&values.len()))
        .ok_or(Error::Invalid("bibliographic retained form selector count"))?;
    let mut selected = BTreeSet::new();
    let mut statement = false;
    for selection in selections_array {
        exact_keys(selection, &["form_id", "field_id"])?;
        if !selected.insert(text(selection, "form_id")?) {
            return Err(Error::Invalid(
                "bibliographic duplicate retained form selector",
            ));
        }
        statement |= selection["field_id"] == "claim.statement";
    }
    if !statement
        || prior.is_some_and(|prior| {
            prior["forms"].as_array().is_some_and(|values| {
                values
                    .iter()
                    .any(|form| !selected.contains(form["form_id"].as_str().unwrap_or("")))
            })
        })
    {
        return Err(Error::Invalid(
            "bibliographic retained correction omitted complete statement/prior form",
        ));
    }
    let result = forms.reconstruct_revision_forms(
        revised,
        prior,
        text(receipt, "principal_id")?,
        selections,
        262_144,
    )?;
    let raw = encode(&result, 262_144)?;
    catalog::check_catalog_schema(
        stage,
        validator,
        l.catalog,
        "ToS/contracts/human-form-set.schema.json",
        "",
        &raw,
    )?;
    let subject = record_ref(revised, true, l)?;
    if result["subject"] != subject {
        return Err(Error::Invalid(
            "bibliographic reconstructed form set exact source subject",
        ));
    }
    let returned = array(&result, "forms")?;
    if returned.len() != selected.len() {
        return Err(Error::Invalid(
            "bibliographic reconstructed complete form set",
        ));
    }
    let mut references = Vec::new();
    for selection in selections_array {
        let matching = returned
            .iter()
            .filter(|form| form["form_id"] == selection["form_id"])
            .collect::<Vec<_>>();
        if matching.len() != 1 || matching[0]["subject"] != subject {
            return Err(Error::Invalid(
                "bibliographic reconstructed form identity/source binding",
            ));
        }
        references.push(form_ref(matching[0], l)?);
    }
    if receipt["forms"] != Value::Array(references.clone()) {
        return Err(Error::Invalid(
            "bibliographic retained correction reconstructed form refs",
        ));
    }
    let retained = array(current, "forms")?
        .iter()
        .chain(array(current, "prior_forms")?.iter())
        .map(|form| form_ref(form, l))
        .collect::<Result<Vec<_>>>()?;
    if references
        .iter()
        .any(|reference| !retained.contains(reference))
    {
        return Err(Error::Invalid(
            "bibliographic reconstructed correction forms not retained in current set",
        ));
    }
    Ok(())
}

fn public_form_lock(name: &str) -> bool {
    if matches!(
        name,
        ".historical-event.human-forms.json.writer.lock"
            | ".historical-process.human-forms.json.writer.lock"
            | ".historical-state.human-forms.json.writer.lock"
    ) {
        return true;
    }
    let tail = name
        .strip_prefix(".source-claims.")
        .or_else(|| name.strip_prefix(".historical-claims."));
    tail.and_then(|tail| tail.strip_suffix(".human-forms.json.writer.lock"))
        .is_some_and(|sha| {
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        })
}

#[cfg(test)]
mod oracle {
    use super::*;
    #[test]
    fn retained_python_successor_preserves_unselected_sibling_bytes() {
        // Independent maintained claim_revisions._advance/_replace oracle,
        // using the existing transport fixture plus exact physical whitespace.
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
        ))
        .unwrap();
        let previous = fixture["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["node_kind"] == "claim")
            .unwrap()["properties"]["source_claim"]
            .clone();
        let mut sibling = previous.clone();
        sibling["claim_id"] = json!("tos.claim.fixture.sibling");
        sibling["extensions"] = json!({"unknown":[false,0,null,"Ω"]});
        let mut raw = b"  ".to_vec();
        raw.extend(encode(&previous, 1_048_576).unwrap());
        raw.extend(b"\r\n\n\t");
        let mut suffix = b"\t".to_vec();
        suffix.extend(encode(&sibling, 1_048_576).unwrap());
        suffix.extend(b" \n");
        raw.extend(&suffix[1..]);
        let limits = BibliographicLimits {
            catalog: catalog::SourceCatalogLimits {
                max_files: 128,
                max_rows: 4096,
                max_file_bytes: 2_097_152,
                max_row_bytes: 1_048_576,
                max_contract_bytes: 2_097_152,
                max_output_row_bytes: 1_048_576,
            },
            max_claim_cohort_rows: 1,
            max_claim_cohort_bytes: 1_048_576,
            max_output_rows: 4096,
            max_output_bytes: 8_388_608,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        };
        let revised = advance_claim(
            &previous,
            &json!({"fields":{"qualifiers":{"statement":"Исправленная условная датировка."}}}),
            limits,
        )
        .unwrap();
        let result = replace_claim(&raw, &revised, limits).unwrap();
        assert_eq!(
            Digest256::of_bytes(&result).to_hex(),
            "3fe1ab6a4691cd9eee084af315a8b078ca16869323da0e9232df13aa639440f7"
        );
        assert!(result.ends_with(&suffix));
        let mut identity = previous;
        identity["object"] = json!("tos.work.fixture.endpoint");
        assert!(
            advance_claim(
                &identity,
                &json!({"fields":{"object":{"kind":"invented"}}}),
                limits
            )
            .is_err()
        );
    }
}
