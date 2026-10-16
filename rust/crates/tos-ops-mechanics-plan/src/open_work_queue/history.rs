use super::*;
fn snapshot_ref(repo: &Repo<'_>, v: &Value) -> Result<Option<String>> {
    let Some(s) = v.as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if scheme(s).is_some() {
        return Ok(None);
    }
    require(
        repo.is_file(s)?,
        format!("snapshot input does not resolve: {s}"),
    )?;
    Ok(Some(s.into()))
}
fn closure_inputs(
    repo: &mut Repo<'_>,
    receipts: &[Value],
    discoveries: &Records,
    events: &Records,
) -> Result<Set> {
    let mut paths: Set = repo
        .files(SOURCE, "provenance.jsonl", true)?
        .into_iter()
        .collect();
    for (d, _) in discoveries.values() {
        for eid in references(&d["provenance_event_refs"]) {
            if let Some((e, _)) = events.get(&eid) {
                if let Some(p) = snapshot_ref(repo, &e["method"]["prompt_or_instruction_ref"])? {
                    if p.starts_with("ToS/research-packets/") {
                        paths.insert(p);
                    }
                }
                for o in list(&e["outputs"]) {
                    if text(&o["ref"]).starts_with("ToS/research-packets/") {
                        if let Some(p) = snapshot_ref(repo, &o["ref"])? {
                            paths.insert(p);
                        }
                    }
                }
            }
        }
    }
    for r in receipts {
        for reference in list(&r["rights_result"]["evidence_refs"]) {
            if let Some(p) = snapshot_ref(repo, reference)? {
                paths.insert(p);
            }
        }
        for reference in list(&r["planting_refs"]) {
            if let Some(p) = snapshot_ref(repo, reference)? {
                let planting = repo.json(&p)?;
                paths.insert(p);
                for r in [
                    &planting["source_witness"]["record_ref"],
                    &planting["discovery_ref"],
                ] {
                    if let Some(p) = snapshot_ref(repo, r)? {
                        paths.insert(p);
                    }
                }
            }
        }
        let mut acqs = vec![&r["acquisition"]];
        acqs.extend(list(&r["additional_acquisitions"]));
        for acq in acqs {
            if !acq.is_object() || acq["downloaded"] != true {
                continue;
            }
            if let Some(p) = snapshot_ref(repo, &acq["representation_ref"])? {
                paths.insert(p);
            }
            for (field, root, filename, key) in [
                ("item_ref", SOURCE, "item.manifest.json", "item_id"),
                (
                    "artifact_ref",
                    "ToS/source-witnesses/artifacts",
                    "artifact-witness.json",
                    "artifact_id",
                ),
                (
                    "composite_ref",
                    "ToS/source-witnesses/scholarly-composites",
                    "composite-witness.json",
                    "composite_id",
                ),
            ] {
                if let Some(id) = acq[field].as_str().filter(|s| !s.is_empty()) {
                    paths.insert(repo.unique_record(root, filename, key, id)?.1);
                }
            }
        }
    }
    Ok(paths)
}
fn historical_hashes(
    repo: &mut Repo<'_>,
    cutoff: i64,
    events: &Records,
) -> Result<BTreeMap<String, String>> {
    let indexed: BTreeMap<_, _> = events.values().map(|(v, loc)| (loc.as_str(), v)).collect();
    let mut hashes = BTreeMap::new();
    for path in repo.files(SOURCE, "provenance.jsonl", true)? {
        let raw = repo.read(&path)?;
        let mut retained = vec![];
        let mut event = false;
        for (index, line) in physical_lines(&raw).enumerate() {
            if line.iter().all(|c| c.is_ascii_whitespace()) {
                retained.extend_from_slice(line);
                continue;
            }
            let key = format!("{path}:{}", index + 1);
            let v = indexed
                .get(key.as_str())
                .ok_or_else(|| invalid("provenance line is not indexed"))?;
            if stamp(&v["ended_at"])? <= cutoff {
                retained.extend_from_slice(line);
                event = true;
            }
        }
        if event {
            hashes.insert(path, codec::digest(&retained));
        }
    }
    Ok(hashes)
}
fn snapshot(
    repo: &mut Repo<'_>,
    candidates: &[Located],
    receipts: &[Value],
    discoveries: &Records,
    events: &Records,
    cutoff: Option<i64>,
    excluded: &Set,
    overrides: &BTreeMap<String, String>,
) -> Result<Value> {
    let mut master = 0;
    for p in MASTER {
        master += repo.lines(p)?.len();
    }
    let dossiers = repo.lines(DOSSIERS)?.len();
    let backlog = repo.lines(BACKLOG)?.len();
    let works = repo.lines(WORKS)?.len();
    let mut paths: Set = MASTER
        .into_iter()
        .chain([DOSSIERS, BACKLOG, WORKS, LEDGER])
        .map(str::to_owned)
        .collect();
    paths.extend(discoveries.values().map(|(_, loc)| loc.clone()));
    for (candidate, _) in candidates {
        for reference in arr(&candidate["source_refs"])? {
            paths.insert(req(reference, "source_path")?.into());
        }
    }
    paths.extend(closure_inputs(repo, receipts, discoveries, events)?);
    paths.extend(repo.files(RECEIPTS, "*.json", false)?);
    paths.extend(repo.files(TIMINGS, "*.json", false)?);
    let historical = if let Some(cut) = cutoff {
        historical_hashes(repo, cut, events)?
    } else {
        BTreeMap::new()
    };
    let mut inputs = vec![];
    let mut fingerprint = String::new();
    for path in paths {
        if excluded.contains(&path)
            || cutoff.is_some()
                && Path::new(&path)
                    .file_name()
                    .is_some_and(|s| s == "provenance.jsonl")
                && !historical.contains_key(&path)
        {
            continue;
        }
        let hash = if let Some(h) = overrides.get(&path).or_else(|| historical.get(&path)) {
            h.clone()
        } else {
            repo.hash(&path)?
        };
        fingerprint.push_str(&path);
        fingerprint.push('\0');
        fingerprint.push_str(&hash);
        fingerprint.push('\n');
        inputs.push(json!({"path":path,"sha256":hash}));
    }
    Ok(
        json!({"input_sha256":codec::digest(fingerprint.as_bytes()),"inputs":inputs,"counts":{"master_rows":master,"accepted_dossiers":dossiers,"source_anchor_backlog_rows":backlog,"catalog_works":works,"discovery_runs":discoveries.len(),"reviewed_candidates":candidates.len(),"terminal_receipts":receipts.len()}}),
    )
}
pub(super) fn ledger_boundary(
    repo: &mut Repo<'_>,
    receipt: &Value,
    candidates: &[Located],
) -> Result<(Vec<Located>, BTreeMap<String, String>)> {
    let Some(expected) = receipt.get("candidate_ledger_sha256") else {
        return Ok((candidates.to_vec(), BTreeMap::new()));
    };
    let expected = sha(expected)?;
    let raw = repo.read(LEDGER)?;
    let mut count = 0;
    let mut length = 0;
    for line in physical_lines(&raw) {
        count += 1;
        length += line.len();
        if codec::digest(&raw[..length]) == expected {
            let mut historical = vec![];
            for (v, loc) in candidates {
                if candidate_line(loc)? <= count {
                    historical.push((v.clone(), loc.clone()));
                }
            }
            return Ok((
                historical,
                BTreeMap::from([(LEDGER.into(), expected.into())]),
            ));
        }
    }
    Ok((candidates.to_vec(), BTreeMap::new()))
}
pub(super) fn queue_payload(
    repo: &mut Repo<'_>,
    candidates: &[Located],
    receipts: &[Value],
    discoveries: &Records,
    events: &Records,
    cutoff: Option<i64>,
    excluded: &Set,
    overrides: &BTreeMap<String, String>,
    producer: &str,
) -> Result<Value> {
    let mut latest = BTreeMap::<String, &Value>::new();
    for r in receipts {
        let id = req(r, "candidate_id")?;
        let version = positive_version(r)?;
        if latest
            .get(id)
            .is_none_or(|old| positive_version(old).is_ok_and(|n| version > n))
        {
            latest.insert(id.into(), r);
        }
    }
    let snapshot = snapshot(
        repo,
        candidates,
        receipts,
        discoveries,
        events,
        cutoff,
        excluded,
        overrides,
    )?;
    let mut ordered = Vec::new();
    for (v, loc) in candidates {
        ordered.push((candidate_key(v, loc)?, v, loc));
    }
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let mut entries = vec![];
    let mut next = Value::Null;
    let mut counts = BTreeMap::<String, usize>::new();
    for (_, v, loc) in ordered {
        let id = req(v, "candidate_id")?;
        let receipt = latest.get(id).copied();
        let status = receipt.map_or(&v["queue_status"], |r| &r["terminal_status"]);
        let status = txt(status)?;
        if next.is_null() && queueable(&v["candidate_kind"]) && status == READY {
            next = json!(id);
        }
        *counts.entry(status.into()).or_default() += 1;
        entries.push(json!({"candidate_id":id,"candidate_kind":v["candidate_kind"],"preferred_label":v["preferred_label"],"selection":v["selection"],"effective_status":status,"candidate_source_ref":loc,"candidate_sha256":digest(v)?,"terminal_receipt_id":receipt.map(|r|r["receipt_id"].clone()),"discovery_id":receipt.map(|r|r["discovery_id"].clone())}));
    }
    let mut out = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/open-work-candidate-queue.schema.json","schema_version":"tos_open_work_candidate_queue_v1","owner_surface":LEDGER,"receipt_root":RECEIPTS,"timing_root":TIMINGS,"generated_by":producer,"source_snapshot":snapshot,"counts":{"candidates":entries.len(),"by_effective_status":counts},"next_candidate_id":next,"candidates":entries,"authority_boundary":"This generated queue provides navigation to reviewed candidate records and terminal receipts."});
    out["queue_sha256"] = json!(queue_digest(&out)?);
    Ok(out)
}
pub(super) fn reconstruct(
    repo: &mut Repo<'_>,
    receipt: &Value,
    ordered: &[Value],
    candidates: &[Located],
    discoveries: &Records,
    events: &Records,
    producer: &str,
) -> Result<String> {
    let current = receipt_key(receipt)?;
    let mut prior = vec![];
    let mut future = vec![];
    for r in ordered {
        if receipt_key(r)? < current {
            prior.push(r.clone())
        } else {
            future.push(r.clone())
        }
    }
    let future_ids: Set = future
        .iter()
        .map(|r| text(&r["discovery_id"]).to_string())
        .collect();
    let mut before = Records::new();
    for (id, (d, loc)) in discoveries {
        if future_ids.contains(id) {
            continue;
        }
        if stamp(&d["started_at"])? <= current.0
            && optional_stamp(&d["ended_at"])?.is_some_and(|end| end <= current.0)
        {
            before.insert(id.clone(), (d.clone(), loc.clone()));
        }
    }
    let receipt_ids: Set = future
        .iter()
        .map(|r| text(&r["receipt_id"]).to_owned())
        .collect();
    let mut excluded = Set::new();
    for path in repo.files(RECEIPTS, "*.json", false)? {
        let r = repo.json(&path)?;
        if receipt_ids.contains(text(&r["receipt_id"])) {
            excluded.insert(path);
        }
    }
    for r in &future {
        for key in ["discovery_ref", "timing_ref"] {
            if let Some(s) = r[key].as_str().filter(|s| !s.is_empty()) {
                excluded.insert(s.into());
            }
        }
    }
    let mut owned_timings: Set = prior
        .iter()
        .filter_map(|r| {
            r["timing_ref"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .collect();
    for (d, _) in before.values() {
        for eid in references(&d["provenance_event_refs"]) {
            if let Some((event, _)) = events.get(&eid) {
                for o in list(&event["outputs"]) {
                    if let Some(s) = o["ref"].as_str() {
                        if s.starts_with(&format!("{TIMINGS}/")) && repo.is_file(s)? {
                            owned_timings.insert(s.into());
                        }
                    }
                }
            }
        }
    }
    for path in repo.files(TIMINGS, "*.json", false)? {
        if !owned_timings.contains(&path) {
            excluded.insert(path);
            continue;
        }
        if stamp(&repo.json(&path)?["measured_at"])? > current.0 {
            excluded.insert(path);
        }
    }
    let (historical, overrides) = ledger_boundary(repo, receipt, candidates)?;
    let p = queue_payload(
        repo,
        &historical,
        &prior,
        &before,
        events,
        Some(current.0),
        &excluded,
        &overrides,
        producer,
    )?;
    Ok(req(&p, "queue_sha256")?.into())
}
pub(super) fn snapshot_witness(
    repo: &mut Repo<'_>,
    receipt: &Value,
    candidate_id: &str,
    label: &str,
    discoveries: &Records,
    events: &Records,
) -> Result<bool> {
    let discovery = &discoveries
        .get(req(receipt, "discovery_id")?)
        .ok_or_else(|| invalid("snapshot discovery absent"))?
        .0;
    let refs = references(&discovery["provenance_event_refs"]);
    let snapshot = text(&receipt["queue_snapshot_sha256"]);
    let ledger = text(&receipt["candidate_ledger_sha256"]);
    if !valid_sha(ledger) {
        return Ok(false);
    }
    let issued = stamp(&receipt["issued_at"])?;
    let ledger_bound = |e: &Value| {
        list(&e["inputs"])
            .iter()
            .find(|v| v["ref"] == LEDGER)
            .is_some_and(|v| v["sha256"] == ledger)
    };
    for eid in &refs {
        let Some((e, _)) = events.get(eid) else {
            continue;
        };
        if !list(&receipt["operational_relation_refs"])
            .iter()
            .any(|v| v == eid)
            || !stamp(&e["ended_at"]).is_ok_and(|end| end <= issued)
            || !ledger_bound(e)
        {
            continue;
        }
        let cfg = &e["method"]["configuration"];
        if cfg.is_object()
            && cfg["candidate_id"] == candidate_id
            && cfg["frozen_queue_snapshot_sha256"] == snapshot
        {
            return Ok(true);
        }
    }
    for eid in &refs {
        let Some((e, _)) = events.get(eid) else {
            continue;
        };
        if e["method"]["configuration"]["candidate_id"] != candidate_id
            || !stamp(&e["ended_at"]).is_ok_and(|end| end <= issued)
            || !ledger_bound(e)
        {
            continue;
        }
        let reference = text(&e["method"]["prompt_or_instruction_ref"]);
        if !reference.starts_with("ToS/research-packets/")
            || !repo.is_file(reference).unwrap_or(false)
        {
            continue;
        }
        let output = list(&e["outputs"]).iter().find(|v| v["ref"] == reference);
        let output_hash = output.map(|v| text(&v["sha256"])).unwrap_or("");
        if !valid_sha(output_hash) || output_hash != repo.hash(reference)? {
            continue;
        }
        let raw = repo.read(reference)?;
        let packet = std::str::from_utf8(&raw).map_err(io::Error::other)?;
        let candidate_pattern = format!(
            r"(?im)^\s*Candidate:\s*`{}`\s*$",
            regex::escape(candidate_id)
        );
        let queue_pattern = format!(
            r"(?im)^\s*(?:Frozen\s+)?Queue snapshot SHA-256:\s*`{}`\s*$",
            regex::escape(snapshot)
        );
        if regex::Regex::new(&candidate_pattern)
            .map_err(io::Error::other)?
            .is_match(packet)
            && regex::Regex::new(&queue_pattern)
                .map_err(io::Error::other)?
                .is_match(packet)
            && phrase(packet, label)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn validate_history(
    repo: &mut Repo<'_>,
    candidates: &[Located],
    receipts: &[Value],
    discoveries: &Records,
    events: &Records,
) -> Result<()> {
    version_order(receipts)?;
    let by_id: BTreeMap<_, _> = candidates
        .iter()
        .map(|(v, _)| (text(&v["candidate_id"]), v))
        .collect();
    let mut ordered = Vec::new();
    for receipt in receipts {
        ordered.push((receipt_key(receipt)?, receipt.clone()));
    }
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let ordered: Vec<Value> = ordered.into_iter().map(|(_, v)| v).collect();
    let mut statuses: BTreeMap<String, Value> = candidates
        .iter()
        .map(|(v, _)| {
            (
                text(&v["candidate_id"]).to_owned(),
                v["queue_status"].clone(),
            )
        })
        .collect();
    let mut previous = BTreeMap::<String, &Value>::new();
    let mut validated = Set::new();
    for receipt in &ordered {
        let id = req(receipt, "receipt_id")?;
        let cid = req(receipt, "candidate_id")?;
        let candidate = by_id
            .get(cid)
            .ok_or_else(|| invalid("unknown history candidate"))?;
        let (historical, _) = ledger_boundary(repo, receipt, candidates)?;
        let snapshot = sha(&receipt["queue_snapshot_sha256"])?;
        let prior = previous.get(cid).copied();
        if let Some(prior) = prior {
            require(
                receipt_key(receipt)? >= receipt_key(prior)?,
                "superseding receipt precedes predecessor",
            )?;
            require(
                receipt["queue_snapshot_sha256"] == prior["queue_snapshot_sha256"],
                "superseding receipt changes frozen queue",
            )?;
        } else {
            require(
                queueable(&candidate["candidate_kind"]),
                "receipt cannot advance non-queueable candidate",
            )?;
            let mut frontier = Vec::new();
            for (c, loc) in &historical {
                frontier.push((candidate_key(c, loc)?, c));
            }
            frontier.sort_by(|a, b| a.0.cmp(&b.0));
            let expected = frontier
                .iter()
                .find(|(_, c)| {
                    queueable(&c["candidate_kind"])
                        && statuses
                            .get(text(&c["candidate_id"]))
                            .is_some_and(|v| v == READY)
                })
                .map(|(_, c)| text(&c["candidate_id"]));
            require(
                expected == Some(cid),
                "receipt does not advance historical next candidate",
            )?;
            statuses.insert(cid.into(), receipt["terminal_status"].clone());
        }
        // Both producer identities are explicit. Historic snapshots retain their
        // original Python identity; new queues identify the actual Rust producer.
        let current = reconstruct(
            repo,
            receipt,
            &ordered,
            candidates,
            discoveries,
            events,
            PRODUCER,
        )?;
        let accepted = current == snapshot
            || reconstruct(
                repo,
                receipt,
                &ordered,
                candidates,
                discoveries,
                events,
                LEGACY_PRODUCER,
            )? == snapshot
            || prior.is_some_and(|p| validated.contains(text(&p["receipt_id"])))
            || snapshot_witness(
                repo,
                receipt,
                cid,
                req(candidate, "preferred_label")?,
                discoveries,
                events,
            )?;
        require(
            accepted,
            "queue snapshot lacks exact pre-run reconstruction or independent witness",
        )?;
        validated.insert(id.into());
        previous.insert(cid.into(), receipt);
    }
    Ok(())
}
