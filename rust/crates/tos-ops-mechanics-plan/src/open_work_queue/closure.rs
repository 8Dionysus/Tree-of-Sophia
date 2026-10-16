use super::*;
fn acquisitions(receipt: &Value) -> Result<Vec<Value>> {
    let mut v = vec![receipt["acquisition"].clone()];
    if !receipt["additional_acquisitions"].is_null() {
        v.extend_from_slice(arr(&receipt["additional_acquisitions"])?);
    }
    Ok(v)
}
fn identity_field<'a>(v: &Value, fields: &'a [&'a str]) -> Result<&'a str> {
    let found: Vec<_> = fields
        .iter()
        .filter(|f| v[**f].as_str().is_some_and(|s| !s.is_empty()))
        .collect();
    require(
        found.len() == 1,
        "acquisition/witness needs exactly one source identity",
    )?;
    Ok(found[0])
}
fn event<'a>(events: &'a Records, id: &str) -> Result<&'a Value> {
    events
        .get(id)
        .map(|e| &e.0)
        .ok_or_else(|| invalid(format!("provenance event does not resolve: {id}")))
}
fn output_digest(outputs: &Value, reference: &str, expected: &str) -> Result<()> {
    let matched: Vec<_> = arr(outputs)?
        .iter()
        .filter(|v| v["ref"] == reference)
        .collect();
    require(
        matched.len() == 1 && matched[0]["sha256"] == expected,
        format!("provenance output does not bind exact bytes: {reference}"),
    )
}
fn item_identity(repo: &mut Repo<'_>, item: &Value, path: &str) -> Result<Set> {
    let id = req(item, "item_id")?;
    let edition = req(item, "embodiment_ref")?;
    let mut refs = Set::from([id.into(), path.into(), edition.into()]);
    let e = repo.catalog("editions.jsonl", edition)?;
    let expressions = strings(&e["links"]["embodies_expression_refs"])?;
    require(
        !expressions.is_empty(),
        "canonical edition must bind an expression",
    )?;
    for expression in expressions {
        refs.insert(expression.clone());
        let e = repo.catalog("expressions.jsonl", &expression)?;
        refs.insert(req(&e["links"], "work_ref")?.into());
    }
    Ok(refs)
}
fn lineage<'a>(events: &'a Records, descendant: &str, ancestor: &str) -> Vec<&'a Value> {
    let mut seen = Set::new();
    let mut pending = vec![(descendant.to_owned(), Vec::new())];
    while let Some((id, mut path)) = pending.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some((v, _)) = events.get(&id) else {
            continue;
        };
        path.push(v);
        if id == ancestor {
            return path;
        }
        for parent in list(&v["inputs"]).iter().rev() {
            if let Some(parent) = parent["ref"].as_str() {
                if events.contains_key(parent) {
                    pending.push((parent.into(), path.clone()));
                }
            }
        }
    }
    vec![]
}
fn event_time(v: &Value, issued: Option<i64>, require_end: bool) -> Result<()> {
    let start = optional_stamp(&v["started_at"])?;
    let end = if require_end {
        Some(stamp(&v["ended_at"])?)
    } else {
        optional_stamp(&v["ended_at"])?
    };
    if let (Some(a), Some(b)) = (start, end) {
        require(b >= a, "provenance event interval is reversed")?;
    }
    if let Some(i) = issued {
        require(
            end.is_some_and(|e| e <= i),
            "provenance event ended after receipt or lacks completion",
        )?;
    }
    Ok(())
}
pub(super) fn acquisition_closure(
    repo: &mut Repo<'_>,
    acq: &Value,
    candidate_id: &str,
    candidate: &Value,
    discovery: &Value,
    discoveries: &Records,
    route: &Set,
    discovery_id: Option<&str>,
    discovery_ref: Option<&str>,
    events: &Records,
    issued: Option<i64>,
    lineage_chronology: bool,
) -> Result<()> {
    obj(acq)?;
    let ids = ["item_ref", "artifact_ref", "composite_ref"];
    if acq["downloaded"] != true {
        for key in [
            "item_ref",
            "artifact_ref",
            "composite_ref",
            "representation_ref",
            "file_ref",
            "provenance_event_ref",
        ] {
            require(
                acq[key].is_null(),
                format!("non-downloaded acquisition must null {key}"),
            )?;
        }
        return Ok(());
    }
    let field = identity_field(acq, &ids)?;
    let identity = req(acq, field)?;
    let file = req(acq, "file_ref")?;
    let event_id = req(acq, "provenance_event_ref")?;
    let e = event(events, event_id)?;
    require(
        e["event_type"] == "acquisition",
        "acquisition ref resolves wrong event type",
    )?;
    event_time(e, issued, issued.is_some())?;
    let config = &e["method"]["configuration"];
    let configured = if config.is_object() {
        config.get("candidate_id")
    } else {
        None
    };
    require(
        configured.is_none_or(|v| v == candidate_id),
        "acquisition event binds a different candidate",
    )?;
    arr(&e["outputs"])?;
    let outputs = io_refs(&e["outputs"]);
    let file_digest = file.strip_prefix("tos.file.sha256.").unwrap_or(file);
    require(valid_sha(file_digest), "file_ref lacks lowercase digest")?;
    if field == "item_ref" {
        require(
            acq["representation_ref"].is_null(),
            "item acquisition cannot carry representation_ref",
        )?;
        require(
            route.contains(event_id),
            "item acquisition event not bound to receipt route",
        )?;
        let (item, path) = repo.unique_record(SOURCE, "item.manifest.json", "item_id", identity)?;
        require(
            route.contains(identity) || route.contains(&path),
            "item identity not bound to receipt route",
        )?;
        let manifest_event = req(&item, "acquisition_event_ref")?;
        let mut chain = lineage(events, manifest_event, event_id);
        require(
            manifest_event == event_id || !chain.is_empty(),
            "item manifest event lacks acquisition lineage",
        )?;
        if chain.is_empty() {
            chain.push(e);
        }
        let identity_refs = item_identity(repo, &item, &path)?;
        if issued.is_some() && lineage_chronology {
            for event in &chain {
                event_time(event, issued, true)?;
            }
        }
        let event_inputs: Set = chain.iter().flat_map(|e| io_refs(&e["inputs"])).collect();
        let mut target = union([
            &references(&candidate["target"]["known_tos_refs"]),
            &references(&discovery["target"]["known_tos_refs"]),
            &references(&discovery["provenance_event_refs"]),
        ]);
        for s in [discovery_id, discovery_ref].into_iter().flatten() {
            target.insert(s.into());
        }
        require(
            intersects(&event_inputs, &target),
            "acquisition event does not bind candidate/discovery",
        )?;
        require(
            intersects(&event_inputs, &identity_refs),
            "acquisition event does not bind canonical item identity ladder",
        )?;
        output_digest(&e["outputs"], file, file_digest)?;
        let matched: Vec<_> = list(&item["payload_files"])
            .iter()
            .filter(|f| f["file_id"] == file)
            .collect();
        require(
            matched.len() == 1 && matched[0]["sha256"] == file_digest,
            "item manifest payload fixity differs",
        )?;
        require(outputs.contains(file), "acquisition does not output file")?;
        return Ok(());
    }
    let representation_ref = req(acq, "representation_ref")?;
    let representation = repo.json(representation_ref)?;
    require(
        representation["file_id"] == file
            && representation["payload"].is_object()
            && representation["payload"]["sha256"] == file_digest,
        "representation file/fixity differs",
    )?;
    require(
        outputs.contains(representation_ref),
        "acquisition does not output representation",
    )?;
    let (root, name, key, path_key) = if field == "artifact_ref" {
        (
            "ToS/source-witnesses/artifacts",
            "artifact-witness.json",
            "artifact_id",
            "artifact_ref",
        )
    } else {
        (
            "ToS/source-witnesses/scholarly-composites",
            "composite-witness.json",
            "composite_id",
            "composite_ref",
        )
    };
    let (record, path) = repo.unique_record(root, name, key, identity)?;
    require(
        representation[key] == record[key] && representation[path_key] == path,
        "representation disagrees with canonical source identity/path",
    )?;
    let rep_discovery = req(&representation, "discovery_ref")?;
    let rep_id = discoveries
        .iter()
        .find(|(_, (_, loc))| loc == rep_discovery)
        .map(|(id, _)| id.as_str())
        .ok_or_else(|| invalid("representation discovery does not resolve"))?;
    require(
        representation["provenance_event_ref"] == event_id,
        "representation acquisition event differs",
    )?;
    require(
        discovery_ref == Some(rep_discovery) && discovery_id == Some(rep_id)
            || configured.is_some_and(|v| v == candidate_id),
        "representation discovery is not bound to receipt or candidate",
    )?;
    output_digest(
        &e["outputs"],
        representation_ref,
        &repo.hash(representation_ref)?,
    )?;
    output_digest(&e["outputs"], file, file_digest)
}
struct Rights {
    value: Value,
    reference: String,
    file: String,
}
fn rights_contexts(repo: &mut Repo<'_>, acquisitions: &[Value]) -> Result<Vec<Rights>> {
    let mut contexts = vec![];
    for acq in acquisitions {
        if !acq.is_object() || acq["downloaded"] != true {
            continue;
        }
        let file = req(acq, "file_ref")?;
        let fields = ["item_ref", "artifact_ref", "composite_ref"];
        let field = identity_field(acq, &fields)?;
        let id = req(acq, field)?;
        let object = if field == "item_ref" {
            repo.unique_record(SOURCE, "item.manifest.json", "item_id", id)?
                .0
        } else {
            repo.json(req(acq, "representation_ref")?)?
        };
        let reference = req(&object, "rights_ref")?;
        require(
            scheme(reference).is_none(),
            "canonical acquisition rights must be local",
        )?;
        let rights = repo.json(reference)?;
        let scopes = references(&rights["scope_refs"]);
        require(
            rights["scope_refs"].is_array() && scopes.contains(id) && scopes.contains(file),
            "canonical rights scope must bind identity and file",
        )?;
        let layers: Vec<_> = list(&rights["layer_assessments"])
            .iter()
            .filter(|v| v.is_object() && references(&v["scope_refs"]).contains(file))
            .collect();
        require(
            !layers.is_empty(),
            "canonical rights lack applicable file layer",
        )?;
        require(
            rights.get("assessment_status").is_none()
                || positive_rights(&rights["assessment_status"]),
            "canonical rights assessment is not positive",
        )?;
        require(
            layers
                .iter()
                .all(|v| positive_rights(&v["assessment_status"])),
            "canonical rights layer is not positive",
        )?;
        contexts.push(Rights {
            value: rights,
            reference: reference.into(),
            file: file.into(),
        });
    }
    Ok(contexts)
}
fn rights_evidence(repo: &mut Repo<'_>, result: &Value) -> Result<()> {
    let refs = strings(&result["evidence_refs"])?;
    require(!refs.is_empty(), "rights evidence refs must be non-empty")?;
    for r in refs {
        if scheme(&r).is_some() {
            require(
                http_url(&r, false),
                "rights evidence URI must be HTTP(S) with host",
            )?;
        } else {
            require(
                repo.is_file(&r)?,
                format!("rights evidence does not resolve: {r}"),
            )?;
        }
    }
    Ok(())
}
fn rights_scope(candidate: &Value, result: &Value, contexts: &[Rights]) -> Result<()> {
    let scope = &candidate["rights_review_scope"];
    let requested_j = strings(&scope["jurisdictions"])?;
    let requested_l = strings(&scope["layers"])?;
    let reviewed_j = strings(&result["reviewed_jurisdictions"])?;
    let reviewed_l = strings(&result["reviewed_layers"])?;
    require(
        !requested_j.is_empty()
            && !requested_l.is_empty()
            && !reviewed_j.is_empty()
            && !reviewed_l.is_empty(),
        "positive rights requires jurisdiction and layer scope",
    )?;
    require(
        intersects(&requested_j, &reviewed_j),
        "reviewed rights jurisdictions do not cover candidate",
    )?;
    let req_tokens = tokens(&scope["layers"])?;
    let rev_tokens = tokens(&result["reviewed_layers"])?;
    require(
        intersects(&req_tokens, &rev_tokens),
        "reviewed rights layers do not cover candidate",
    )?;
    let evidence = references(&result["evidence_refs"]);
    for c in contexts {
        require(
            evidence.contains(&c.reference),
            "rights result omits canonical acquisition rights ref",
        )?;
        require(
            c.value["jurisdictions_reviewed"].is_array()
                && intersects(&references(&c.value["jurisdictions_reviewed"]), &reviewed_j),
            "canonical rights jurisdiction not reviewed",
        )?;
        let layers: Vec<_> = list(&c.value["layer_assessments"])
            .iter()
            .filter(|v| v.is_object())
            .map(|v| v["layer_role"].clone())
            .collect();
        let layer_tokens = tokens(&json!(layers))?;
        require(
            intersects(&req_tokens, &layer_tokens) && intersects(&rev_tokens, &layer_tokens),
            format!(
                "reviewed rights scope does not identify acquired layer {}",
                c.file
            ),
        )?;
    }
    Ok(())
}
pub(super) fn planting_refs(
    repo: &mut Repo<'_>,
    refs: &Value,
    candidate: &Value,
    receipt: &Value,
    discovery: &Value,
    discoveries: &Records,
    acquisitions: &[Value],
    events: &Records,
    issued: Option<i64>,
) -> Result<()> {
    let refs = arr(refs)?;
    let atlas = req(&candidate["selection"], "atlas_row_id")?;
    let relation = strings(&receipt["operational_relation_refs"])?;
    let mut route = relation.clone();
    for key in ["discovery_ref", "discovery_id"] {
        if let Some(s) = receipt[key].as_str() {
            route.insert(s.into());
        }
    }
    let mut acquired = Set::new();
    let mut acq_context = Set::new();
    for acq in acquisitions {
        if !acq.is_object() || acq["downloaded"] != true {
            continue;
        }
        for key in [
            "item_ref",
            "artifact_ref",
            "composite_ref",
            "representation_ref",
            "file_ref",
            "provenance_event_ref",
        ] {
            if let Some(s) = acq[key].as_str().filter(|s| !s.is_empty()) {
                acq_context.insert(s.into());
                if ["item_ref", "artifact_ref", "composite_ref"].contains(&key) {
                    acquired.insert(s.into());
                }
            }
        }
        if let Some((e, _)) = events.get(text(&acq["provenance_event_ref"])) {
            acq_context.extend(io_refs(&e["inputs"]));
            acq_context.extend(io_refs(&e["outputs"]));
        }
    }
    let mut supported = union([
        &references(&discovery["target"]["known_tos_refs"]),
        &acquired,
    ]);
    supported.extend(relation.iter().filter(|s| s.starts_with("tos.")).cloned());
    for reference in refs {
        let reference = txt(reference)?;
        let p = repo.json(reference)?;
        req(&p, "planting_id")?;
        require(
            p["atlas_row_id"] == atlas && p["dossier_id"] == atlas,
            "planting atlas/dossier scope differs",
        )?;
        let witness = &p["source_witness"];
        obj(witness)?;
        let fields = ["artifact_id", "composite_id", "work_id"];
        let field = identity_field(witness, &fields)?;
        let id = req(witness, field)?;
        let record_ref = req(witness, "record_ref")?;
        let record = repo.json(record_ref)?;
        let key = if field == "work_id" {
            "record_id"
        } else {
            field
        };
        require(
            record[key] == id,
            "planting source identity disagrees with record_ref",
        )?;
        let prefix = match field {
            "artifact_id" => "ToS/source-witnesses/artifacts/",
            "composite_id" => "ToS/source-witnesses/scholarly-composites/",
            _ => "ToS/source-witnesses/works/",
        };
        require(
            record_ref.starts_with(prefix),
            "planting source ref outside identity owner",
        )?;
        require(
            supported.contains(id)
                || relation.contains(record_ref)
                || acq_context.contains(record_ref),
            "planting source not bound to receipt/acquisition",
        )?;
        let discovery_ref = req(&p, "discovery_ref")?;
        let (discovery_id, (planting_discovery, _)) = discoveries
            .iter()
            .find(|(_, (_, loc))| loc == discovery_ref)
            .ok_or_else(|| invalid("planting discovery does not resolve"))?;
        let source_bound = route.contains(record_ref)
            || route.contains(id)
            || acq_context.contains(record_ref)
            || acq_context.contains(id);
        require(
            route.contains(discovery_ref) || route.contains(discovery_id) || source_bound,
            "planting discovery not bound to receipt records",
        )?;
        let target_refs = references(&planting_discovery["target"]["known_tos_refs"]);
        require(
            target_refs.is_empty() || target_refs.contains(id),
            "planting discovery target identifies another witness",
        )?;
        let eid = req(&p, "provenance_event_ref")?;
        let e = event(events, eid)?;
        if let Some(i) = issued {
            require(
                stamp(&e["ended_at"])? <= i,
                "planting event ended after receipt",
            )?;
        }
        arr(&e["inputs"])?;
        arr(&e["outputs"])?;
        let input = io_refs(&e["inputs"]);
        let output = io_refs(&e["outputs"]);
        require(
            output.contains(reference),
            "planting event does not output planting",
        )?;
        output_digest(&e["outputs"], reference, &repo.hash(reference)?)?;
        require(
            input.contains(record_ref)
                || output.contains(record_ref)
                || input.contains(id)
                || output.contains(id),
            "planting event does not bind source witness",
        )?;
        require(
            route.contains(eid) || source_bound,
            "planting event not bound to receipt records",
        )?;
    }
    Ok(())
}
pub(super) fn receipt_closure(
    repo: &mut Repo<'_>,
    receipt: &Value,
    candidate: &Value,
    discovery: &Value,
    discoveries: &Records,
    events: &Records,
    planting_chronology: bool,
    lineage_chronology: bool,
) -> Result<()> {
    let candidate_id = req(receipt, "candidate_id")?;
    let issued = optional_stamp(&receipt["issued_at"])?;
    let mut route = strings(&receipt["operational_relation_refs"])?;
    for key in ["discovery_ref", "discovery_id"] {
        if let Some(s) = receipt[key].as_str() {
            route.insert(s.into());
        }
    }
    let acqs = acquisitions(receipt)?;
    let downloaded = acqs
        .iter()
        .any(|v| v.is_object() && v["downloaded"] == true);
    let rights = &receipt["rights_result"];
    require(
        !downloaded || rights["status"] == "positive-for-acquisition",
        "download requires positive rights result",
    )?;
    rights_evidence(repo, rights)?;
    for a in &acqs {
        acquisition_closure(
            repo,
            a,
            candidate_id,
            candidate,
            discovery,
            discoveries,
            &route,
            receipt["discovery_id"].as_str(),
            receipt["discovery_ref"].as_str(),
            events,
            issued,
            lineage_chronology,
        )?;
    }
    if downloaded {
        rights_scope(candidate, rights, &rights_contexts(repo, &acqs)?)?;
    }
    planting_refs(
        repo,
        &receipt["planting_refs"],
        candidate,
        receipt,
        discovery,
        discoveries,
        &acqs,
        events,
        if planting_chronology { issued } else { None },
    )?;
    require(
        receipt["terminal_status"] != "held_source_witness"
            || downloaded
            || !list(&receipt["planting_refs"]).is_empty(),
        "held witness lacks acquired or planted source",
    )
}
