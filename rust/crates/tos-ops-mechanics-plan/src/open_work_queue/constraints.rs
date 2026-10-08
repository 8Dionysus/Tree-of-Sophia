use super::*;
fn words(s: &str) -> Result<Vec<String>> {
    let folded = codec::values::fold(s).map_err(invalid)?.replace('’', "'");
    Ok(folded
        .split(|c| !tos_foundation::python_word_unicode16_v1(c))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect())
}
pub(super) fn phrase(haystack: &str, needle: &str) -> Result<bool> {
    let h = words(haystack)?;
    let n = words(needle)?;
    Ok(!n.is_empty() && h.windows(n.len()).any(|w| w == n))
}
pub(super) fn tokens(v: &Value) -> Result<Set> {
    let source = if let Some(s) = v.as_str() {
        s.to_owned()
    } else {
        String::from_utf8(canonical(v)?).map_err(io::Error::other)?
    };
    let base: Set = words(&source)?
        .into_iter()
        .filter(|s| !STOP.contains(&s.as_str()))
        .collect();
    let mut out = base.clone();
    for word in base {
        for part in
            std::iter::once(word.as_str()).chain(word.split(['-', '_']).filter(|s| !s.is_empty()))
        {
            out.insert(part.into());
            out.extend(aliases(part).iter().map(|s| s.to_string()));
        }
    }
    Ok(out)
}
fn refinement(candidate: &Value, discovery: &Value) -> Result<()> {
    let fields = ["required_properties", "acceptable_substitutions", "formats"];
    let mut surfaces = vec![discovery["description"].clone()];
    for key in [
        "required_properties",
        "acceptable_substitutions",
        "languages",
        "formats",
    ] {
        if discovery[key].is_array() {
            surfaces.push(discovery[key].clone());
        }
    }
    let surface = tokens(&json!(surfaces))?;
    let mut mapped = BTreeSet::new();
    let refinements = if discovery["constraint_refinements"].is_null() {
        &[][..]
    } else {
        arr(&discovery["constraint_refinements"])?
    };
    for r in refinements {
        let cf = req(r, "candidate_field")?;
        let df = req(r, "discovery_field")?;
        require(
            fields.contains(&cf) && fields.contains(&df),
            "refinement names unsupported constraint field",
        )?;
        let cv = strings(&r["candidate_values"])?;
        let dv = strings(&r["discovery_values"])?;
        require(
            !cv.is_empty() && !dv.is_empty(),
            "refinement values must be non-empty",
        )?;
        require(
            r["review_status"] == "reviewed",
            "refinement requires reviewed status",
        )?;
        req(r, "reviewed_at")?;
        req(r, "reviewer_ref")?;
        require(
            cv.is_subset(&strings(&candidate[cf])?) && dv.is_subset(&strings(&discovery[df])?),
            "refinement does not bind candidate/discovery values",
        )?;
        for value in cv {
            require(
                mapped.insert((cf.to_string(), value)),
                "duplicate reviewed refinement",
            )?;
        }
    }
    for field in fields {
        let cv = strings(&candidate[field])?;
        let dv = strings(&discovery[field])?;
        require(
            cv.is_empty() || !dv.is_empty(),
            "discovery drops all candidate constraints",
        )?;
        for value in cv {
            require(
                tokens(&json!(value))?.is_subset(&surface)
                    || mapped.contains(&(field.into(), value)),
                format!("discovery drops complete candidate {field} constraint"),
            )?;
        }
    }
    let cl = strings(&candidate["languages"])?;
    let dl = strings(&discovery["languages"])?;
    require(
        cl.is_empty() || intersects(&cl, &dl),
        "discovery drops all candidate languages",
    )
}
pub(super) fn target_binding(candidate: &Value, discovery: &Value, receipt: &Value) -> Result<()> {
    let c = &candidate["target"];
    let d = &discovery["target"];
    obj(c)?;
    obj(d)?;
    let dk = req(d, "target_kind")?;
    let compatible = [
        "work",
        "expression",
        "edition",
        "item",
        "artifact",
        "scholarly-composite",
        "translation",
        "critical-edition",
        "research-publication",
    ];
    require(
        matches!(text(&c["target_kind"]), "work" | "scholarly-composite")
            && (compatible.contains(&dk) || c["target_kind"] == "work" && dk == "lexical-resource"),
        "discovery target kind is incompatible",
    )?;
    let cr = strings(&c["known_tos_refs"])?;
    let dr = strings(&d["known_tos_refs"])?;
    require(
        cr.is_subset(&dr),
        "discovery drops candidate known_tos_refs",
    )?;
    if cr.is_empty() {
        require(
            phrase(req(d, "description")?, req(candidate, "preferred_label")?)?,
            "discovery description does not identify candidate",
        )?;
    }
    for (key, value) in [
        ("candidate_target_sha256", c),
        ("discovery_target_sha256", d),
    ] {
        require(
            receipt[key] == digest(value)?,
            format!("{key} does not freeze target"),
        )?;
    }
    refinement(c, d)
}
pub(super) fn target_resolution(
    repo: &mut Repo<'_>,
    v: &Value,
    candidate: Option<&Value>,
    discovery: Option<&Value>,
) -> Result<()> {
    obj(v)?;
    let status = req(v, "identity_status")?;
    require(
        ["unresolved", "provisional", "reconciled"].contains(&status),
        "unsupported target identity_status",
    )?;
    let fields = [
        ("work_ref", "works.jsonl", "work"),
        ("expression_ref", "expressions.jsonl", "expression"),
        ("edition_ref", "editions.jsonl", "edition"),
        ("item_ref", "items.jsonl", "item"),
    ];
    let present: Set = fields
        .iter()
        .filter_map(|(k, _, _)| v[*k].as_str().filter(|s| !s.is_empty()).map(str::to_owned))
        .collect();
    require(
        status != "unresolved" || present.is_empty(),
        "unresolved target declares identity refs",
    )?;
    require(
        status != "reconciled" || !present.is_empty(),
        "reconciled target lacks identity refs",
    )?;
    if candidate.is_some() || discovery.is_some() {
        let mut bound = Set::new();
        for source in [candidate, discovery].into_iter().flatten() {
            obj(&source["target"])?;
            bound.extend(strings(&source["target"]["known_tos_refs"])?);
        }
        require(
            present.is_subset(&bound),
            "target identity refs are not bound to candidate/discovery",
        )?;
    }
    for (field, filename, typ) in fields {
        if !v[field].is_null() {
            let entry = repo.catalog(filename, txt(&v[field])?)?;
            require(
                entry["record_type"] == typ,
                "target resolves wrong catalog record type",
            )?;
        }
    }
    Ok(())
}
fn unique_channels(v: &Value) -> Result<BTreeMap<String, &Value>> {
    let mut out = BTreeMap::new();
    for row in arr(v)? {
        let id = req(row, "channel_id")?;
        require(
            out.insert(id.to_string(), row).is_none(),
            "duplicate channel_id",
        )?;
    }
    Ok(out)
}
fn elapsed(v: &Value) -> Result<f64> {
    let n = v
        .as_f64()
        .ok_or_else(|| invalid("timing must be numeric"))?;
    require(n.is_finite() && n > 0., "active timing must be positive")?;
    Ok(n)
}
pub(super) fn timings(
    discovery: &Value,
    timing: &Value,
    reference: &str,
    issued: Option<i64>,
) -> Result<()> {
    let channels = unique_channels(&discovery["channels"])?;
    let comparisons = unique_channels(&discovery["channel_comparison"])?;
    let measured = unique_channels(&timing["measurements"])?;
    require(
        !channels.is_empty() && !measured.is_empty(),
        "active discovery needs measured channels",
    )?;
    require(
        timing["discovery_id"] == discovery["discovery_id"] && timing["discovery_ref"] == reference,
        "timing discovery binding differs",
    )?;
    let start = stamp(&discovery["started_at"])?;
    let end = stamp(&discovery["ended_at"])?;
    let observed = stamp(&timing["measured_at"])?;
    require(end >= start, "discovery interval reversed")?;
    require(
        issued.is_none_or(|i| observed <= i),
        "timing is later than receipt",
    )?;
    let mut last = i64::MIN;
    for (id, c) in &channels {
        let duration = elapsed(&c["elapsed_seconds"])?;
        let m = measured
            .get(id)
            .ok_or_else(|| invalid("missing external timing measurement"))?;
        let m = &m["measurement"];
        obj(m)?;
        require(
            m["probe_url"] == c["endpoint_url"],
            "timing probe URL differs",
        )?;
        require(
            m["method"] == "monotonic-http-request-v1",
            "unsupported timing method",
        )?;
        require(
            m["clock"] == "python.time.perf_counter_ns",
            "unsupported historical timing clock",
        )?;
        let begin = stamp(&m["started_at"])?;
        let stop = stamp(&m["ended_at"])?;
        require(
            begin == stamp(&c["queried_at"])? && stop >= begin && begin >= start && stop <= end,
            "timing interval differs from query/discovery",
        )?;
        last = last.max(stop);
        require(
            (elapsed(&m["elapsed_seconds"])? - duration).abs() <= 0.000001,
            "measured elapsed seconds differ",
        )?;
        let comparison = comparisons
            .get(id)
            .ok_or_else(|| invalid("comparison missing active channel"))?;
        require(
            (elapsed(&comparison["machine_seconds"])? - duration).abs() <= 0.000001,
            "comparison machine seconds differ",
        )?;
        let notes = text(&comparison["notes"]).to_lowercase();
        require(
            !["unknown sentinel", "timer sentinel", "not instrumented"]
                .iter()
                .any(|s| notes.contains(s)),
            "comparison retains unknown timing",
        )?;
    }
    require(
        channels.keys().eq(comparisons.keys()) && channels.keys().eq(measured.keys()),
        "timing/comparison active channel set differs",
    )?;
    require(
        observed >= last,
        "timing measured_at precedes channel completion",
    )
}
pub(super) fn version_order(receipts: &[Value]) -> Result<()> {
    let mut groups = BTreeMap::<String, BTreeMap<i64, i64>>::new();
    for receipt in receipts {
        let (issued, version, _) = receipt_key(receipt)?;
        let versions = groups
            .entry(req(receipt, "candidate_id")?.into())
            .or_default();
        require(
            versions.insert(version, issued).is_none(),
            "duplicate receipt version",
        )?;
    }
    for versions in groups.values() {
        let mut prior = None;
        for issued in versions.values() {
            require(
                prior.is_none_or(|p| *issued >= p),
                "superseding receipt is backdated",
            )?;
            prior = Some(*issued);
        }
    }
    Ok(())
}
pub(super) fn load_receipts(
    repo: &mut Repo<'_>,
    candidates: &[Located],
    discoveries: &Records,
    events: &Records,
) -> Result<Vec<Value>> {
    let by_id: BTreeMap<_, _> = candidates
        .iter()
        .map(|(v, _)| (text(&v["candidate_id"]), v))
        .collect();
    let mut receipts = vec![];
    let mut ids = Set::new();
    let mut versions = BTreeMap::<String, BTreeSet<i64>>::new();
    let mut latest = BTreeMap::<String, usize>::new();
    for path in repo.files(RECEIPTS, "*.json", false)? {
        let r = repo.json(&path)?;
        let id = req(&r, "receipt_id")?;
        require(ids.insert(id.into()), "duplicate receipt_id")?;
        let candidate_id = req(&r, "candidate_id")?;
        let candidate = by_id
            .get(candidate_id)
            .ok_or_else(|| invalid("receipt references unknown candidate"))?;
        require(
            r["candidate_record_sha256"] == digest(candidate)?,
            "receipt candidate digest differs",
        )?;
        sha(&r["candidate_ledger_sha256"])?;
        require(
            terminal(&r["terminal_status"]),
            "unsupported terminal status",
        )?;
        let (discovery, location) = discoveries
            .get(req(&r, "discovery_id")?)
            .ok_or_else(|| invalid("receipt references unknown discovery"))?;
        let reference = req(&r, "discovery_ref")?;
        require(reference == location, "receipt discovery_ref differs")?;
        sha(&r["discovery_sha256"])?;
        require(
            r["discovery_sha256"] == repo.hash(reference)?,
            "receipt discovery hash differs",
        )?;
        let issued = stamp(&r["issued_at"])?;
        require(
            issued >= stamp(&discovery["ended_at"])?,
            "receipt precedes discovery completion",
        )?;
        target_binding(candidate, discovery, &r)?;
        target_resolution(
            repo,
            &r["target_resolution"],
            Some(candidate),
            Some(discovery),
        )?;
        let version = positive_version(&r)?;
        require(
            versions
                .entry(candidate_id.into())
                .or_default()
                .insert(version),
            "duplicate terminal receipt version",
        )?;
        if latest
            .get(candidate_id)
            .is_none_or(|i| positive_version(&receipts[*i]).is_ok_and(|old| version > old))
        {
            latest.insert(candidate_id.into(), receipts.len());
        }
        receipts.push(r);
    }
    for (i, r) in receipts.iter().enumerate() {
        let id = req(r, "candidate_id")?;
        let current = latest[id] == i;
        receipt_closure(
            repo,
            r,
            by_id[id],
            &discoveries[req(r, "discovery_id")?].0,
            discoveries,
            events,
            current,
            current,
        )?;
    }
    for (candidate, index) in latest {
        let r = &receipts[index];
        let reference = req(r, "timing_ref")?;
        let timing = repo.json(reference)?;
        sha(&r["timing_sha256"])?;
        require(
            r["timing_sha256"] == repo.hash(reference)?,
            format!("latest timing digest differs for {candidate}"),
        )?;
        timings(
            &discoveries[req(r, "discovery_id")?].0,
            &timing,
            req(r, "discovery_ref")?,
            Some(stamp(&r["issued_at"])?),
        )?;
    }
    Ok(receipts)
}

const STOP: &[&str] = &[
    "a",
    "all",
    "an",
    "and",
    "any",
    "as",
    "at",
    "be",
    "by",
    "for",
    "from",
    "in",
    "into",
    "is",
    "kept",
    "must",
    "no",
    "of",
    "one",
    "only",
    "or",
    "preserve",
    "preserves",
    "rather",
    "remain",
    "remains",
    "retained",
    "retains",
    "same",
    "separate",
    "separates",
    "separation",
    "the",
    "this",
    "to",
    "with",
    "without",
];
fn aliases(word: &str) -> &'static [&'static str] {
    match word {
        "article" => &["scholarship"],
        "artifact" => &["physical-witness"],
        "authority" => &["metadata"],
        "bibliographic" => &["metadata"],
        "byte" => &["digital-object"],
        "bytes" => &["digital-object"],
        "catalog" => &["metadata"],
        "catalogue" => &["metadata"],
        "coffin" => &["physical-witness"],
        "commentary" => &["scholarship"],
        "composition" => &["work-identity"],
        "corpus" => &["work-identity"],
        "critical" => &["scholarship"],
        "download" => &["digital-object"],
        "downloadable" => &["digital-object"],
        "edition" => &["scholarship"],
        "edition_presentation" => &["scholarship"],
        "evidence" => &["rights"],
        "export" => &["digital-object"],
        "facsimile" => &["digital-object"],
        "file" => &["digital-object"],
        "files" => &["digital-object"],
        "fixity" => &["identity"],
        "holding" => &["metadata"],
        "html" => &["digital-object", "text-layer"],
        "identities" => &["identity"],
        "identity" => &["identity"],
        "iiif" => &["digital-object"],
        "image" => &["digital-object"],
        "inscription" => &["text-layer"],
        "institutional" => &["metadata"],
        "jpeg" => &["digital-object", "text-layer"],
        "json" => &["digital-object", "text-layer"],
        "license" => &["rights"],
        "licensed" => &["rights"],
        "line_art" => &["digital-object"],
        "member" => &["identity"],
        "members" => &["identity"],
        "monument" => &["physical-witness"],
        "original_work" => &["work-identity"],
        "papyrus" => &["physical-witness"],
        "payload" => &["digital-object"],
        "pdf" => &["digital-object"],
        "permission" => &["rights"],
        "photograph" => &["digital-object"],
        "physical" => &["physical-witness"],
        "plate" => &["digital-object"],
        "plates" => &["digital-object"],
        "project" => &["scholarship"],
        "provider" => &["metadata"],
        "publication" => &["scholarship"],
        "record" => &["metadata"],
        "records" => &["metadata"],
        "rights" => &["rights"],
        "scan" => &["digital-object"],
        "scholarly" => &["scholarship"],
        "spell" => &["text-layer"],
        "study" => &["scholarship"],
        "subcorpus" => &["work-identity"],
        "tablet" => &["physical-witness"],
        "terms" => &["rights"],
        "text" => &["text-layer"],
        "transcription" => &["text-layer"],
        "translation" => &["text-layer"],
        "transliteration" => &["text-layer"],
        "utterance" => &["text-layer"],
        "version" => &["identity"],
        "visual" => &["digital-object"],
        "witness" => &["physical-witness"],
        "witnesses" => &["physical-witness"],
        "work" => &["work-identity"],
        "xml" => &["digital-object", "text-layer"],
        _ => &[],
    }
}
