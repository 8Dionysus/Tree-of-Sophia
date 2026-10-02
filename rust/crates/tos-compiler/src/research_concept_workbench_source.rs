use super::*;
const ALIGN: &str = "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1";
const DE: &str = "technical-markup/dta-first-editions-parts-1-4-v1";
const RU: &str = "technical-markup/antonovsky-1911-structural-paragraph-v2";
pub(super) struct Maps {
    pub de: BTreeMap<String, (u64, String)>,
    pub ru: BTreeMap<String, (u64, String)>,
    pub links: BTreeMap<String, Rows>,
    pub count: u64,
    pub statuses: Value,
    pub shapes: Value,
}
pub(super) fn source_maps(root: &ResearchExecution) -> Result<Maps> {
    let spine = lines(root, &format!("{WORK}/{ALIGN}/alignment-spine.v1.jsonl"))?;
    let mut de = BTreeMap::new();
    let mut ru = BTreeMap::new();
    for row in &spine {
        root.tick(1)?;
        let part = n(row, "part_order");
        let reading = if row["reading_ordinal_within_part"].is_null() {
            format!("p{part}.unscoped-technical")
        } else {
            format!("p{part}.r{}", n(row, "reading_ordinal_within_part"))
        };
        for (field, map) in [
            ("source_paragraph_unit_refs", &mut de),
            ("target_paragraph_unit_refs", &mut ru),
        ] {
            root.tick(1)?;
            for r in arr(&row[field]) {
                root.tick(1)?;
                map.insert(r.as_str().unwrap().into(), (part, reading.clone()));
            }
        }
    }
    for row in lines(root, &format!("{WORK}/{ALIGN}/scope-exclusions.v1.jsonl"))? {
        let part = n(&row, "part_order");
        let reading = format!("p{part}.r{}", n(&row, "reading_ordinal_within_part"));
        let target = if s(&row, "side") == "source" {
            &mut de
        } else {
            &mut ru
        };
        for r in arr(&row["ordered_excluded_verse_line_refs"]) {
            root.tick(1)?;
            target.insert(r.as_str().unwrap().into(), (part, reading.clone()));
        }
    }
    let by_id: BTreeMap<_, _> = spine.iter().map(|r| (s(r, "alignment_id"), r)).collect();
    let mut links: BTreeMap<String, Rows> = BTreeMap::new();
    let mut all = vec![];
    for part in 1..=4 {
        root.tick(1)?;
        let packet = load(
            root,
            &format!("{WORK}/{ALIGN}/part-{part}.translation-alignment-packet.v1.json"),
        )?;
        for row in arr(&packet["alignments"]) {
            root.tick(1)?;
            let aid = s(row, "alignment_id");
            let link = json!({"alignment_ref":aid,"status":row["status"],"shape":row["correspondence_shape"]});
            let original = by_id.get(aid).ok_or("alignment missing")?;
            for r in arr(&original["source_paragraph_unit_refs"])
                .iter()
                .chain(arr(&original["target_paragraph_unit_refs"]))
            {
                root.tick(1)?;
                links
                    .entry(r.as_str().unwrap().into())
                    .or_default()
                    .push(link.clone());
            }
            all.push(link);
        }
    }
    if all.len() != 3423 {
        return Err(format!("alignment census drift: {}", all.len()));
    }
    Ok(Maps {
        de,
        ru,
        links,
        count: all.len() as u64,
        statuses: count(&all, "status"),
        shapes: count(&all, "shape"),
    })
}
fn private_text(root: &ResearchExecution, path: &str) -> Result<String> {
    let mut file = root.source_file(path, 32 * 1024 * 1024)?;
    if file
        .metadata()
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o777
        != 0o600
    {
        return Err(format!("private text not mode0600: {path}"));
    }
    let raw = root.read_file(&mut file, 32 * 1024 * 1024)?;
    String::from_utf8(raw).map_err(|e| e.to_string())
}
pub(super) fn de_units(root: &ResearchExecution, m: &Maps) -> Result<Rows> {
    let citations = lines(root, &format!("{WORK}/{DE}/citation-spine.v1.jsonl"))?;
    let mut packets = BTreeMap::new();
    for part in 1..=4 {
        root.tick(1)?;
        packets.insert(
            part,
            load(
                root,
                &format!("{WORK}/{DE}/part-{part}.source-text-unit.v1.json"),
            )?,
        );
    }
    let mut cache = BTreeMap::new();
    let mut result = vec![];
    for citation in citations
        .iter()
        .filter(|r| ["paragraph", "verse_line"].contains(&s(r, "unit_kind")))
    {
        root.tick(1)?;
        let part = n(citation, "part_order");
        let packet = &packets[&part];
        let id = s(citation, "unit_id");
        let unit = arr(&packet["units"])
            .iter()
            .find(|r| s(r, "unit_id") == id)
            .ok_or("German unit missing")?;
        let mut fragments = vec![];
        for ar in arr(&unit["ordered_anchor_refs"]) {
            root.tick(1)?;
            let anchor = arr(&packet["anchors"])
                .iter()
                .find(|r| r["anchor_ref"] == *ar)
                .ok_or("German anchor missing")?;
            let reference = s(anchor, "text_layer_ref");
            if !cache.contains_key(reference) {
                let text = private_text(root, reference)?;
                if hash(&text) != s(anchor, "text_layer_sha256") {
                    return Err(format!("German text layer drift: {reference}"));
                }
                cache.insert(reference.to_string(), text.chars().collect::<Vec<char>>());
            }
            let chars: &Vec<char> = &cache[reference];
            let start = n(&anchor["selector"], "start") as usize;
            let end = n(&anchor["selector"], "end") as usize;
            if start > end || end > chars.len() {
                return Err("German selector bounds".into());
            }
            let exact: String = chars[start..end].iter().collect();
            if hash(&exact) != s(anchor, "exact_sha256") {
                return Err(format!("German anchor return mismatch: {ar}"));
            }
            fragments.push(exact);
        }
        let (mp, reading) = m.de.get(id).ok_or("German reading map missing")?;
        if *mp != part {
            return Err("German reading part mismatch".into());
        }
        let text = fragments.join("\n");
        result.push(json!({"context_unit_ref":id,"language":"de","part":part,"reading_ref":reading,"unit_kind":citation["unit_kind"],"witness_order":result.len()+1,"text":text,"exact_sha256":hash(&text),"anchor_refs":unit["ordered_anchor_refs"],"source_locator":citation["source_locator"],"alignment_links":m.links.get(id).cloned().unwrap_or_default(),"analysis_tokens":crate::research_parallel_lexical::tokens(&text,"de")}));
    }
    if count(&result, "unit_kind") != json!({"paragraph":3447,"verse_line":368}) {
        return Err("German witness-unit census drift".into());
    }
    Ok(result)
}
pub(super) fn ru_units(root: &ResearchExecution, m: &Maps) -> Result<(Rows, Rows)> {
    root.reserve_structural_reads()?;
    let model = crate::antonovsky_structural::reconstruct_from_directory(
        root.root(),
        root.root_directory(),
        root.deadline(),
    )?;
    let mut charged = 0;
    root.charge_structural(&model, &mut charged)?;
    let ids = crate::antonovsky_structural::load_identities(root.root(), &model)?;
    root.charge_structural(&model, &mut charged)?;
    let mut texts = BTreeMap::new();
    let mut orders = BTreeMap::new();
    for (i, row) in model.rows.iter().enumerate() {
        root.tick(1)?;
        let id =
            &ids["logical_rows"][&crate::antonovsky_structural::row_binding(row, &model.lines)];
        texts.insert(id.clone(), row.text.clone());
        orders.insert(id.clone(), i + 1);
    }
    let mut owner = BTreeMap::<String, String>::new();
    let mut result = vec![];
    for row in lines(root, &format!("{WORK}/{RU}/paragraph-spine.v2.jsonl"))? {
        let id = s(&row, "paragraph_unit_id");
        let refs = arr(&row["ordered_logical_row_refs"]);
        let mut text = vec![];
        let mut order = usize::MAX;
        for r in refs {
            root.tick(1)?;
            let reference = r.as_str().ok_or("logical ref")?;
            owner.insert(reference.into(), id.into());
            text.push(
                texts
                    .get(reference)
                    .ok_or("Russian logical text missing")?
                    .clone(),
            );
            order = order.min(
                *orders
                    .get(reference)
                    .ok_or("Russian logical order missing")?,
            );
        }
        let (part, reading) = m.ru.get(id).ok_or("Russian paragraph reading missing")?;
        let text = text.join("\n");
        result.push(json!({"context_unit_ref":id,"language":"ru","part":part,"reading_ref":reading,"unit_kind":"paragraph","witness_order":order,"text":text,"exact_sha256":hash(&text),"anchor_refs":refs,"source_locator":row["display_citation"],"alignment_links":m.links.get(id).cloned().unwrap_or_default(),"analysis_tokens":crate::research_parallel_lexical::tokens(&text,"ru")}));
    }
    for row in lines(root, &format!("{WORK}/{RU}/verse-line-spine.v2.jsonl"))? {
        let id = s(&row, "verse_line_unit_id");
        let logical = s(&row, "logical_row_unit_ref");
        owner.insert(logical.into(), id.into());
        let (part, reading) = m.ru.get(id).ok_or("Russian verse reading missing")?;
        let text = texts
            .get(logical)
            .ok_or("Russian verse logical text missing")?;
        if hash(text) != s(&row, "exact_sha256") {
            return Err(format!("Russian verse return mismatch: {id}"));
        }
        result.push(json!({"context_unit_ref":id,"language":"ru","part":part,"reading_ref":reading,"unit_kind":"verse_line","witness_order":orders[logical],"text":text,"exact_sha256":hash(text),"anchor_refs":[logical],"source_locator":logical,"alignment_links":[],"analysis_tokens":crate::research_parallel_lexical::tokens(text,"ru")}));
    }
    result.sort_by_key(|r| n(r, "witness_order"));
    if count(&result, "unit_kind") != json!({"paragraph":3569,"verse_line":359}) {
        return Err("Russian witness-unit census drift".into());
    }
    let mut raw = crate::research_parallel_lexical::build_ru_observations_from_model(
        root,
        &model,
        &mut charged,
    )?;
    for r in &mut raw {
        root.tick(1)?;
        r["context_unit_ref"] = owner
            .get(s(r, "unit_id"))
            .map(|x| json!(x))
            .unwrap_or(Value::Null);
    }
    Ok((result, raw))
}
pub(super) fn occurrences(
    root: &ResearchExecution,
    de: &[Value],
    ru: &[Value],
    raw: &[Value],
) -> Result<Rows> {
    let mut out = vec![];
    let locators: BTreeMap<_, _> = de
        .iter()
        .map(|r| ((n(r, "part"), s(r, "source_locator")), r))
        .collect();
    let dbref = format!(
        "{WORK}/gold-sets/foundation-pilot-v1/local-content/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.sqlite3"
    );
    let mut dbfile = root.source_file(&dbref, 128 * 1024 * 1024)?;
    let meta = dbfile.metadata().map_err(|e| e.to_string())?;
    if !meta.file_type().is_file() || meta.permissions().mode() & 0o777 != 0o600 {
        return Err("German exact-occurrence database must be regular mode 0600".into());
    }
    if root.hash_file(&mut dbfile, 128 * 1024 * 1024)?
        != "c2912a9f481205f0de9a1a0242b26a3419e3d82d44163ab91a2a2ab16ced5736"
    {
        return Err("German exact-occurrence database fixity drift".into());
    }
    let db = root.open_sqlite_readonly(&dbfile)?;
    let deadline = root.deadline();
    db.progress_handler(1000, Some(move || std::time::Instant::now() >= deadline));
    // The ordered readonly scan may sort. Keep temporary sorting in RAM;
    // strict exact-FD source access must never open filesystem temp objects.
    // The enclosing finite execution envelope owns the full RAM limit.
    root.check()?;
    db.execute_batch("PRAGMA temp_store=MEMORY")
        .map_err(|e| format!("concept German readonly sorter policy: {e}"))?;
    let temp_store: i64 = db
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .map_err(|e| format!("concept German readonly sorter policy check: {e}"))?;
    if temp_store != 2 {
        return Err("concept German readonly sorter requires memory temp storage".into());
    }
    root.check()?;
    let mut stmt=db.prepare("SELECT o.occurrence_id,s.part_order,o.token_ordinal,o.exact_form,o.normalized_form,o.exact_form_sha256,o.normalized_form_sha256,o.text_node_path,o.start_offset,o.end_offset FROM occurrences o JOIN source_items s USING(item_ref) ORDER BY s.part_order,o.token_ordinal").map_err(|e|format!("concept German ordered scan prepare: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("concept German ordered scan start: {e}"))?;
    while let Some(r) = rows
        .next()
        .map_err(|e| format!("concept German ordered scan row: {e}"))?
    {
        root.tick(1)?;
        let oid: String = r.get(0).map_err(|e| e.to_string())?;
        let part: u64 = r.get(1).map_err(|e| e.to_string())?;
        let ordinal: u64 = r.get(2).map_err(|e| e.to_string())?;
        let surface: String = r.get(3).map_err(|e| e.to_string())?;
        let normalized: String = r.get(4).map_err(|e| e.to_string())?;
        let path: String = r.get(7).map_err(|e| e.to_string())?;
        let mut cursor = path.as_str();
        let mut unit = None;
        while cursor.contains('/') {
            if let Some(u) = locators.get(&(part, cursor)) {
                unit = Some(*u);
                break;
            }
            cursor = cursor.rsplit_once('/').unwrap().0;
        }
        if unit.is_none() && path.contains("/lg[") {
            if let Some(first) = path.find("/div[") {
                if let Some(next) = path[first + 1..].find("/div[") {
                    let suffix = &path[first + 1 + next..];
                    let candidates: Vec<_> = de
                        .iter()
                        .filter(|u| {
                            n(u, "part") == part
                                && s(u, "unit_kind") == "verse_line"
                                && s(u, "source_locator").ends_with(suffix)
                        })
                        .collect();
                    if candidates.len() == 1 {
                        unit = Some(candidates[0]);
                    }
                }
            }
        }
        let key = crate::research_parallel_lexical::base_key(&normalized);
        out.push(json!({"existing_occurrence_ref":oid,"language":"de","part":part,"token_ordinal":ordinal,"surface":surface,"exact_sha256":r.get::<_,String>(5).map_err(|e|e.to_string())?,"normalized":normalized,"normalized_sha256":r.get::<_,String>(6).map_err(|e|e.to_string())?,"analysis_key":key,"analysis_key_sha256":hash(&key),"context_unit_ref":unit.map(|u|u["context_unit_ref"].clone()),"reading_ref":unit.map(|u|u["reading_ref"].clone()),"unit_kind":unit.map(|u|u["unit_kind"].clone()).unwrap_or(json!("structural_or_unmapped")),"witness_order":unit.map(|u|n(u,"witness_order")).unwrap_or(ordinal),"source_locator_sha256":hash(&path),"start":r.get::<_,u64>(8).map_err(|e|e.to_string())?,"end":r.get::<_,u64>(9).map_err(|e|e.to_string())?,"in_work_scope":!(part==4&&path.starts_with("TEI/text[1]/body[1]/div[21]/"))}));
    }
    drop(rows);
    drop(stmt);
    db.close().map_err(|(_retained, error)| error.to_string())?;
    root.verify_file_unchanged(&dbfile, &meta)?;
    let by: BTreeMap<_, _> = ru.iter().map(|r| (s(r, "context_unit_ref"), r)).collect();
    for r in raw {
        root.tick(1)?;
        let unit = by.get(s(r, "context_unit_ref"));
        out.push(json!({"existing_occurrence_ref":r["occurrence_id"],"language":"ru","part":r["part"],"token_ordinal":r["ordinal"],"surface":r["surface"],"exact_sha256":r["exact_sha256"],"normalized":r["normalized"],"normalized_sha256":r["normalized_sha256"],"analysis_key":r["analysis_key"],"analysis_key_sha256":r["analysis_key_sha256"],"context_unit_ref":r["context_unit_ref"],"reading_ref":unit.map(|u|u["reading_ref"].clone()).unwrap_or(r["reading"].clone()),"unit_kind":unit.map(|u|u["unit_kind"].clone()).unwrap_or(r["role"].clone()),"witness_order":unit.map(|u|n(u,"witness_order")).unwrap_or(1_000_000+n(r,"ordinal")),"source_locator_sha256":hash(s(r,"unit_id")),"start":r["start"],"end":r["end"],"in_work_scope":true}));
    }
    Ok(out)
}
pub(super) fn census(root: &ResearchExecution, rows: &[Value]) -> Result<Value> {
    root.tick(1)?;
    let de: Vec<_> = rows.iter().filter(|r| s(r, "language") == "de").collect();
    let ru: Vec<_> = rows.iter().filter(|r| s(r, "language") == "ru").collect();
    let scope: Vec<_> = de
        .iter()
        .copied()
        .filter(|r| r["in_work_scope"] == true)
        .collect();
    let sizes = |rs: &[&Value], k: &str| rs.iter().map(|r| s(r, k)).collect::<BTreeSet<_>>().len();
    let c = json!({"de_exact_occurrences":de.len(),"ru_exact_occurrences":ru.len(),"de_exact_forms":sizes(&de,"surface"),"de_analysis_forms":sizes(&de,"analysis_key"),"de_work_scope_occurrences":scope.len(),"de_work_scope_exact_forms":sizes(&scope,"surface"),"de_work_scope_analysis_forms":sizes(&scope,"analysis_key"),"ru_exact_forms":sizes(&ru,"surface"),"ru_normalized_forms":sizes(&ru,"normalized"),"ru_analysis_forms":sizes(&ru,"analysis_key")});
    if c != json!({"de_exact_occurrences":86287,"ru_exact_occurrences":93643,"de_exact_forms":11352,"de_analysis_forms":10113,"de_work_scope_occurrences":84491,"de_work_scope_exact_forms":11118,"de_work_scope_analysis_forms":9909,"ru_exact_forms":17443,"ru_normalized_forms":16240,"ru_analysis_forms":15152})
    {
        return Err(format!("exact occurrence/form census drift: {c}"));
    }
    root.tick(1)?;
    Ok(c)
}
