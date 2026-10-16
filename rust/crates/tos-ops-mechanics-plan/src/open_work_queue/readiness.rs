use super::*;
const DIMENSIONS: [&str; 5] = ["version", "access", "rights", "file", "branch"];
fn evidence(repo: &mut Repo<'_>, reference: &Value) -> Result<String> {
    let path = req(reference, "path")?;
    require(repo.is_file(path)?, "readiness evidence does not resolve")?;
    require(
        repo.hash(path)? == sha(&reference["sha256"])?,
        "readiness evidence digest mismatch",
    )?;
    Ok(path.into())
}
fn owner_refs(repo: &mut Repo<'_>, observed: &Value) -> Result<Set> {
    arr(&observed["owner_refs"])?
        .iter()
        .map(|r| evidence(repo, r))
        .collect()
}
fn owner_path(path: &str, prefix: &str) -> bool {
    path.starts_with(prefix) && !path.starts_with("ToS/source-witnesses/discovery/candidates/")
}
fn execution(repo: &mut Repo<'_>, target: &Value, dimensions: &Value) -> Result<String> {
    let Some(execution) = target.get("execution").filter(|v| !v.is_null()) else {
        return Ok("pending".into());
    };
    let status = req(execution, "status")?;
    let refs = owner_refs(repo, execution)?;
    if status == "pending" {
        return Ok(status.into());
    }
    require(
        execution["evidence_posture"] == "owner-reviewed" && !refs.is_empty(),
        "execution requires owner-reviewed evidence",
    )?;
    require(
        refs.iter()
            .all(|p| owner_path(p, "ToS/source-witnesses/") || owner_path(p, "ToS/philosophy/")),
        "execution evidence must return to source or philosophy owner",
    )?;
    if status != "completed" {
        return Ok(status.into());
    }
    require(
        dimensions["file"]["status"] == "present" && dimensions["branch"]["status"] == "ready",
        "completed execution requires present files and ready branch",
    )?;
    let spec = &target["target"];
    let work_id = req(&spec["planned_ids"], "work")?;
    let new = spec["create_record_refs"].get("work");
    let existing = spec["existing_record_refs"]
        .get("work")
        .filter(|v| !v.is_null());
    if let Some(v) = existing {
        require(
            new.is_none_or(Value::is_null) && list(&spec["known_tos_refs"]).contains(v),
            "existing Work must be known and cannot also be declared new",
        )?;
    }
    let work_ref = txt(existing
        .or(new)
        .ok_or_else(|| invalid("completed execution lacks exact Work reference"))?)?;
    require(
        work_id.starts_with("tos.work.")
            && refs.contains(work_ref)
            && work_ref.starts_with("ToS/source-witnesses/"),
        "completed execution requires exact planned Work digest evidence",
    )?;
    let work = repo.json(work_ref)?;
    require(
        work["schema_version"] == "tos_corpus_record_v1"
            && work["record_type"] == "work"
            && work["record_id"] == work_id,
        "completed Work identity differs",
    )?;
    let mut matched = false;
    for path in refs {
        if !path.starts_with("ToS/philosophy/")
            || Path::new(&path)
                .file_name()
                .is_none_or(|v| v != "source-planting.json")
        {
            continue;
        }
        let planting = repo.json(&path)?;
        repo.schema("philosophy-source-planting", &planting)?;
        let branch = req(&planting, "branch_path")?;
        let source = &planting["source_witness"];
        let p = Path::new(&path);
        let parents: Vec<_> = p.ancestors().skip(1).collect();
        require(
            source["work_id"] == work_id
                && source["record_ref"] == work_ref
                && list(&spec["known_tos_refs"]).contains(&json!(branch))
                && parents.len() > 3
                && parents[1].file_name().is_some_and(|n| n == "plantings")
                && parents[2].file_name().is_some_and(|n| n == "sources")
                && parents[3] == Path::new(branch),
            "execution planting does not bind planned Work and exact branch",
        )?;
        matched = true;
    }
    require(
        matched,
        "completed execution lacks owner source-planting evidence",
    )?;
    Ok(status.into())
}
pub(super) fn project(
    repo: &mut Repo<'_>,
    mut payload: Value,
    plan: Option<&str>,
) -> Result<Value> {
    let base = payload["queue_sha256"].clone();
    let existing: BTreeMap<String, Value> = arr(&payload["candidates"])?
        .iter()
        .map(|v| Ok((req(v, "candidate_id")?.into(), v.clone())))
        .collect::<Result<_>>()?;
    let mut plan_ref = Value::Null;
    let targets = if let Some(path) = plan {
        require(
            path.starts_with("ToS/source-witnesses/discovery/"),
            "readiness plan must belong to source discovery",
        )?;
        let v = repo.json(path)?;
        repo.schema("open-work-readiness-plan", &v)?;
        plan_ref = json!({"path":path,"sha256":repo.hash(path)?});
        arr(&v["targets"])?.to_vec()
    } else {
        vec![]
    };
    let mut entries = vec![];
    let mut seen_targets = Set::new();
    let mut seen_candidates = Set::new();
    for mut target in targets {
        let id = req(&target, "target_id")?.to_owned();
        require(seen_targets.insert(id), "duplicate readiness target_id")?;
        let cid = target
            .get("candidate_id")
            .filter(|v| !v.is_null())
            .map(txt)
            .transpose()?
            .map(str::to_owned);
        if let Some(cid) = &cid {
            let candidate = existing
                .get(cid)
                .ok_or_else(|| invalid("unknown historical candidate_id"))?;
            require(
                seen_candidates.insert(cid.clone()),
                "duplicate readiness candidate_id",
            )?;
            require(
                target["candidate_sha256"] == candidate["candidate_sha256"],
                "candidate digest mismatch",
            )?;
        }
        for source in arr(&target["source_record_refs"])? {
            let path = evidence(repo, source)?;
            if path.ends_with(".json.gz")
                || path.starts_with("ToS/research-packets/source-registries/")
                    && Path::new(&path)
                        .components()
                        .any(|p| p.as_os_str() == "documents")
            {
                let doc =
                    codec::decoded(&repo.read(&path)?, path.ends_with(".gz")).map_err(invalid)?;
                require(
                    doc.is_object()
                        && ["corpus_id", "document_id"]
                            .iter()
                            .all(|k| doc[*k] == source[*k]),
                    "normalized source corpus/document mismatch",
                )?;
                let matches = list(&doc["records"])
                    .iter()
                    .filter(|r| {
                        r.is_object()
                            && ["corpus_id", "document_id"]
                                .iter()
                                .all(|k| r[*k] == source[*k])
                            && (r["record_id"] == source["record_id"]
                                || r["source_record_id"] == source["record_id"])
                    })
                    .count();
                require(
                    matches == 1,
                    "normalized source record must resolve exactly once",
                )?;
            }
        }
        let dimensions = &target["readiness"];
        for dimension in DIMENSIONS {
            let observed = &dimensions[dimension];
            let refs = owner_refs(repo, observed)?;
            if matches!(text(&observed["status"]), "ready" | "present" | "absent") {
                require(
                    observed["evidence_posture"] == "owner-reviewed" && !refs.is_empty(),
                    "positive readiness needs owner-reviewed evidence",
                )?;
                let prefix = if dimension == "branch" {
                    "ToS/philosophy/"
                } else {
                    "ToS/source-witnesses/"
                };
                require(
                    refs.iter().all(|p| owner_path(p, prefix)),
                    "imported or queue evidence is not owner review",
                )?;
            }
        }
        let acquisition = &target["acquisition"];
        if dimensions["rights"]["status"] == "ready" {
            require(
                references(&acquisition["intended_uses"])
                    .is_subset(&references(&dimensions["rights"]["intended_uses"])),
                "rights readiness does not cover every intended use",
            )?;
        }
        let urls = arr(&acquisition["source_urls"])?;
        for url in urls {
            require(
                http_url(txt(url)?, true),
                "source URL must be credential-free HTTP(S)",
            )?;
        }
        let destinations = arr(&acquisition["destination_paths"])?;
        for dest in destinations {
            let dest = txt(dest)?;
            repo.path(dest)?;
            require(
                dest.starts_with("ToS/source-witnesses/")
                    && Path::new(dest)
                        .components()
                        .any(|p| p.as_os_str() == "payload"),
                "destination must be an owner payload path",
            )?;
        }
        let file_status = text(&dimensions["file"]["status"]);
        if file_status == "present" {
            require(
                !destinations.is_empty(),
                "present file status lacks destination files",
            )?;
            let evidence: Set = arr(&dimensions["file"]["owner_refs"])?
                .iter()
                .map(|r| req(r, "path").map(str::to_owned))
                .collect::<Result<_>>()?;
            for dest in destinations {
                let dest = txt(dest)?;
                require(
                    repo.is_file(dest)? && evidence.contains(dest),
                    "present payloads need existing files and exact digest evidence",
                )?;
            }
        } else if file_status == "absent" {
            for dest in destinations {
                require(
                    !repo.exists(txt(dest)?)?,
                    "absent file status conflicts with existing destination",
                )?;
            }
        }
        let execution = execution(repo, &target, dimensions)?;
        let eligible = cid.as_ref().is_none_or(|id| {
            let v = &existing[id];
            queueable(&v["candidate_kind"]) && v["effective_status"] == READY
        });
        let ready = execution == "pending"
            && eligible
            && ["version", "access", "rights", "branch"]
                .iter()
                .all(|k| dimensions[*k]["status"] == "ready")
            && matches!(file_status, "absent" | "present")
            && !destinations.is_empty()
            && !urls.is_empty();
        let action = match execution.as_str() {
            "completed" => "completed-owner-evidenced",
            "deferred" => "await-owner-resumption",
            "blocked" => "resolve-execution-blocker",
            _ if ready && file_status == "present" => "verify-existing-witness",
            _ if ready => "acquire-after-owner-gates",
            _ if eligible => "resolve-owner-evidence",
            _ => "historical-terminal-or-excluded",
        };
        target["candidate_id"] = json!(cid);
        target["eligible"] = json!(eligible);
        target["ready_for_acquisition_review"] = json!(ready);
        target["next_action"] = json!(action);
        entries.push(target);
    }
    for (id, candidate) in &existing {
        if seen_candidates.contains(id) {
            continue;
        }
        require(
            !seen_targets.contains(id),
            "readiness target collides with historical candidate",
        )?;
        let dimensions:serde_json::Map<String,Value>=DIMENSIONS.into_iter().map(|name|(name.into(),json!({"status":"unknown","evidence_posture":"unreviewed","owner_refs":[],"rationale":"No explicit owner readiness plan; historical queue review is ordering-only."}))).collect();
        entries.push(json!({"target_id":id,"candidate_id":id,"candidate_sha256":candidate["candidate_sha256"],"preferred_label":candidate["preferred_label"],"source_record_refs":[],"target":null,"readiness":dimensions,"acquisition":{"source_urls":[],"destination_paths":[],"intended_uses":[]},"eligible":queueable(&candidate["candidate_kind"])&&candidate["effective_status"]==READY,"ready_for_acquisition_review":false,"next_action":"resolve-owner-evidence"}));
    }
    entries.sort_by_key(|e| {
        let candidate = existing.get(text(&e["candidate_id"]));
        let selection = candidate.map(|c| &c["selection"]);
        let known = DIMENSIONS
            .iter()
            .filter(|k| {
                matches!(
                    text(&e["readiness"][**k]["status"]),
                    "ready" | "present" | "absent"
                )
            })
            .count();
        (
            !e["eligible"].as_bool().unwrap_or(false),
            !e["ready_for_acquisition_review"].as_bool().unwrap_or(false),
            e.get("execution")
                .map(|v| &v["status"])
                .is_some_and(|v| v != "pending"),
            DIMENSIONS
                .iter()
                .any(|k| e["readiness"][*k]["status"] == "blocked"),
            std::cmp::Reverse(known),
            selection
                .and_then(|s| s["chronology_sort_year"].as_i64())
                .unwrap_or(1_000_000_000),
            selection
                .and_then(|s| s["atlas_row_order"].as_i64())
                .unwrap_or(1_000_000_000),
            text(&e["target_id"]).to_owned(),
        )
    });
    let selected = entries
        .iter()
        .find(|v| v["ready_for_acquisition_review"] == true);
    payload["selection_mode"] = json!("readiness");
    payload["chronological_queue_sha256"] = base;
    payload["readiness_plan_ref"] = plan_ref;
    payload["next_target_id"] = selected.map_or(Value::Null, |s| s["target_id"].clone());
    payload["next_candidate_id"] = selected.map_or(Value::Null, |s| s["candidate_id"].clone());
    payload["readiness_entries"] = json!(entries);
    payload["selection_boundary"] = json!(
        "read-only preparation; owner evidence assertions are not new verification; plan-only targets do not enter historical candidate receipt replay"
    );
    payload["queue_sha256"] = json!(queue_digest(&payload)?);
    Ok(payload)
}
