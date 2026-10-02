//! Frozen v1 morphology/theme candidate producer. Dictionary stems remain proposals.
use crate::research_execution::ResearchExecution;
use crate::research_parallel_lexical::{
    arr, base_key, h, hash, lines, load, load_parallel, n, pretty, read, round, s, write,
};
use serde_json::{Value as V, json};
use std::{
    collections::{BTreeMap as Map, BTreeSet as Set},
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
};
type R<T> = Result<T, String>;
const ROUTE: &str = "ToS/candidate-intake/zarathustra/dta-antonovsky-morphology-themes-v1";
const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
const GENERATOR: &str = "scripts/build_zarathustra_morphology_theme_candidates_v1.py";
// Selected maintained rendering-recipe identity, separate from native execution.
const RECIPE_SHA256: &str = "8eebdf809bd5989d6c3aae60796a0be57bc886bf3f0441a6109b9b879954ac49";
fn route(x: &str) -> String {
    format!("{ROUTE}/{x}")
}
fn previous_ref(x: &str) -> String {
    format!("{WORK}/lexical-indexes/dta-antonovsky-parallel-candidates-v1/{x}")
}
fn private_previous() -> String {
    format!(
        "{WORK}/gold-sets/foundation-pilot-v1/local-content/parallel-lexical-candidates-v1/parallel-candidate-analysis.v1.json"
    )
}
fn private() -> String {
    format!(
        "{WORK}/gold-sets/foundation-pilot-v1/local-content/morphology-themes-v1/morphology-theme-analysis.v1.json"
    )
}
fn field(v: &V, k: &str) -> R<String> {
    s(&v[k]).map(str::to_owned)
}
fn threshold(plan: &V, k: &str) -> usize {
    n(&plan["thresholds"][k])
}
fn signatures(value: &str, lang: &str) -> R<Vec<(String, String, usize)>> {
    let mut base = base_key(value);
    if lang == "de" {
        base = base
            .replace('ä', "a")
            .replace('ö', "o")
            .replace('ü', "u")
            .replace('ß', "ss");
    }
    let table: V = serde_json::from_str(include_str!("research_morphology_theme_suffixes.json"))
        .map_err(|e| e.to_string())?;
    let mut rows = vec![(base.clone(), "orthographic_base".into(), 3)];
    for row in arr(&table[if lang == "de" {
        "DE_SUFFIXES"
    } else {
        "RU_SUFFIXES"
    }])? {
        let suffix = s(&row[0])?;
        let method = s(&row[1])?;
        if let Some(stem) = base.strip_suffix(suffix) {
            if stem.chars().count() >= 4 {
                let x = (
                    stem.to_owned(),
                    method.to_owned(),
                    if method == "inflection" { 1 } else { 2 },
                );
                if !rows.contains(&x) {
                    rows.push(x);
                }
            }
        }
    }
    Ok(rows)
}
fn case(form: &V, previous: &V) -> String {
    let variants = &previous["surface_variants"]["de"][s(&form["form_sha256"]).unwrap_or("")];
    let total = variants
        .as_object()
        .map(|v| v.values().map(n).sum::<usize>())
        .unwrap_or(0);
    if total == 0 {
        return "mixed_or_sparse".into();
    }
    let upper = variants
        .as_object()
        .unwrap()
        .iter()
        .filter(|(surface, _)| surface.chars().next().is_some_and(char::is_uppercase))
        .map(|(_, v)| n(v))
        .sum::<usize>();
    let ratio = upper as f64 / total as f64;
    if total >= 3 && ratio >= 0.8 {
        "noun_like_initial_upper"
    } else if total >= 3 && ratio <= 0.2 {
        "lowercase_like"
    } else {
        "mixed_or_sparse"
    }
    .into()
}
fn dictionary(root: &ResearchExecution, path: &str) -> R<Vec<u8>> {
    const CAP: u64 = 16 * 1024 * 1024;
    root.check()?;
    let mut file =
        tos_fd_open::open_absolute_regular(Path::new(path), CAP).map_err(|e| e.to_string())?;
    root.read_file(&mut file, CAP)
}
fn hunspell(root: &ResearchExecution, forms: &[String]) -> R<(Map<String, Vec<String>>, V)> {
    root.check()?;
    use crate::owned_native_child::{CaptureLimits, capture};
    const MAX_STDIN: usize = 16 * 1024 * 1024;
    let input_bytes = forms.iter().try_fold(0usize, |total, form| {
        root.tick(1)?;
        total
            .checked_add(form.len())
            .and_then(|v| v.checked_add(1))
            .filter(|v| *v <= MAX_STDIN)
            .ok_or_else(|| "Hunspell stdin cap exceeded".to_owned())
    })?;
    let mut input = Vec::with_capacity(input_bytes.max(1));
    for form in forms {
        root.tick(1)?;
        input.extend(form.as_bytes());
        input.push(b'\n');
    }
    if forms.is_empty() {
        input.push(b'\n');
    }
    let output = capture(
        Command::new("hunspell").args(["-d", "ru_RU", "-m"]),
        Some(&input),
        CaptureLimits {
            max_stdin_bytes: MAX_STDIN,
            max_stdout_bytes: 32 * 1024 * 1024,
            max_stderr_bytes: 128 * 1024,
        },
        root.deadline(),
    )?;
    root.check()?;
    if !output.status.success() {
        return Err(format!("Hunspell exited {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let blocks: Vec<&str> = stdout
        .split("\n\n")
        .filter(|b| !b.trim().is_empty())
        .collect();
    if blocks.len() != forms.len() {
        return Err(format!(
            "Hunspell block mismatch: {} != {}",
            blocks.len(),
            forms.len()
        ));
    }
    let re = regex::Regex::new(r"(?:^|\s)st:([^\s]+)").map_err(|e| e.to_string())?;
    let mut stems = Map::new();
    for (form, block) in forms.iter().zip(blocks) {
        root.tick(1)?;
        stems.insert(
            form.clone(),
            re.captures_iter(block)
                .map(|c| base_key(&c[1]).replace('ё', "е"))
                .collect::<Set<_>>()
                .into_iter()
                .collect(),
        );
    }
    let version = capture(
        Command::new("hunspell").arg("-v"),
        None,
        CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 16 * 1024,
            max_stderr_bytes: 16 * 1024,
        },
        root.deadline(),
    )?;
    root.check()?;
    if !version.status.success() {
        return Err(format!("Hunspell version exited {}", version.status));
    }
    let aff = dictionary(root, "/usr/share/hunspell/ru_RU.aff")
        .map_err(|_| "fixed ru_RU Hunspell dictionary is unavailable")?;
    let dic = dictionary(root, "/usr/share/hunspell/ru_RU.dic")
        .map_err(|_| "fixed ru_RU Hunspell dictionary is unavailable")?;
    Ok((
        stems,
        json!({"command":["hunspell","-d","ru_RU","-m"],"version":String::from_utf8_lossy(&version.stdout).trim(),"dictionary_aff_sha256":hash(&aff),"dictionary_dic_sha256":hash(&dic),"stderr_sha256":hash(String::from_utf8_lossy(&output.stderr).as_bytes())}),
    ))
}
#[derive(Default)]
struct Group {
    language: String,
    members: Set<String>,
    methods: Vec<(String, usize)>,
    strength: usize,
}
fn family_row(
    binding: String,
    kind: &str,
    lang: &str,
    method: &str,
    status: &str,
    members: &[String],
    forms: &Map<String, V>,
) -> V {
    let mut hashes: Vec<String> = members
        .iter()
        .map(|k| forms[k]["form_sha256"].as_str().unwrap().into())
        .collect();
    hashes.sort();
    json!({"binding":binding,"candidate_kind":kind,"language":lang,"method":method,"status":status,"member_keys":members,"member_form_sha256s":hashes,"member_count":members.len(),"occurrence_count":members.iter().map(|k|n(&forms[k]["occurrence_count"])).sum::<usize>(),"part_range":members.iter().map(|k|n(&forms[k]["part_range"])).max().unwrap_or(0),"accepted":false,"review_refs":[],"graph_effect":false})
}
fn surface_families(
    root: &ResearchExecution,
    previous: &V,
    plan: &V,
) -> R<(Vec<V>, Map<String, V>, V)> {
    root.check()?;
    let mut forms = Map::new();
    let mut order = vec![];
    for row in arr(&previous["keywords"])? {
        root.tick(1)?;
        if n(&row["occurrence_count"]) < threshold(plan, "minimum_form_frequency") {
            continue;
        }
        let key = format!("{}:{}", s(&row["language"])?, s(&row["analysis_key"])?);
        let mut form = json!({"language":row["language"],"form":row["analysis_key"],"form_sha256":row["form_key_sha256"],"occurrence_count":row["occurrence_count"],"part_range":row["part_range"],"reading_range":row["range_count"]});
        if row["language"] == "de" {
            form["case_class"] = json!(case(&form, previous));
        }
        if !forms.contains_key(&key) {
            order.push(key.clone());
        }
        forms.insert(key, form);
    }
    let mut groups: Vec<Group> = vec![];
    let mut group_indices: Map<(String, String), usize> = Map::new();
    for key in order {
        root.tick(1)?;
        let form = &forms[&key];
        let lang = s(&form["language"])?;
        for (signature, method, strength) in signatures(s(&form["form"])?, lang)? {
            root.tick(1)?;
            let keygroup = (lang.to_owned(), signature);
            let i = if let Some(i) = group_indices.get(&keygroup) {
                *i
            } else {
                let i = groups.len();
                groups.push(Group {
                    language: lang.into(),
                    ..Default::default()
                });
                group_indices.insert(keygroup, i);
                i
            };
            let g = &mut groups[i];
            g.members.insert(key.clone());
            if let Some((_, count)) = g.methods.iter_mut().find(|(m, _)| m == &method) {
                *count += 1
            } else {
                g.methods.push((method, 1));
            }
            g.strength = g.strength.max(strength);
        }
    }
    let maximum = threshold(plan, "maximum_surface_family_size");
    let mut candidates = vec![];
    let mut seen = Set::new();
    let mut member_multi = Set::new();
    for group in groups {
        root.tick(1)?;
        let members: Vec<String> = group.members.into_iter().collect();
        if members.len() < 2 || !seen.insert((group.language.clone(), members.clone())) {
            continue;
        }
        let total = members
            .iter()
            .map(|k| n(&forms[k]["occurrence_count"]))
            .sum::<usize>();
        if total < threshold(plan, "minimum_multi_form_family_frequency") {
            continue;
        }
        let method = &group
            .methods
            .iter()
            .enumerate()
            .max_by(|(ia, a), (ib, b)| a.1.cmp(&b.1).then_with(|| ib.cmp(ia)))
            .unwrap()
            .1
            .0;
        let mut status = if members.len() > maximum {
            "deferred"
        } else {
            "proposed"
        };
        if group.language == "ru" {
            status = if members.len() <= maximum {
                "ambiguous"
            } else {
                "deferred"
            }
        } else if method == "orthographic_base"
            || members
                .iter()
                .any(|k| forms[k]["case_class"] != "noun_like_initial_upper")
        {
            status = "ambiguous"
        }
        let mut hashes: Vec<String> = members
            .iter()
            .map(|k| s(&forms[k]["form_sha256"]).unwrap().into())
            .collect();
        hashes.sort();
        candidates.push(family_row(
            format!("surface|{}|{}", group.language, hashes.join("|")),
            "surface_morphology_family_candidate",
            &group.language,
            method,
            status,
            &members,
            &forms,
        ));
        member_multi.extend(members);
    }
    let ru_keys: Vec<String> = forms
        .keys()
        .filter(|k| {
            k.starts_with("ru:")
                && forms[*k]["form"].as_str().is_some_and(|x| {
                    !x.is_empty() && x.chars().all(|c| ('а'..='я').contains(&c) || c == 'ё')
                })
        })
        .cloned()
        .collect();
    let inputs = ru_keys
        .iter()
        .map(|k| s(&forms[k]["form"]).unwrap().into())
        .collect::<Vec<_>>();
    let (provider_stems, provider) = hunspell(root, &inputs)?;
    let mut provider_groups: Map<String, Set<String>> = Map::new();
    let mut provider_count: Map<String, usize> = Map::new();
    for key in ru_keys {
        root.tick(1)?;
        for stem in &provider_stems[s(&forms[&key]["form"])?] {
            root.tick(1)?;
            if stem.chars().count() >= 3 {
                provider_groups
                    .entry(stem.clone())
                    .or_default()
                    .insert(key.clone());
                *provider_count.entry(key.clone()).or_default() += 1
            }
        }
    }
    for (stem, members) in provider_groups {
        root.tick(1)?;
        let members: Vec<_> = members.into_iter().collect();
        let total = members
            .iter()
            .map(|k| n(&forms[k]["occurrence_count"]))
            .sum::<usize>();
        if members.len() < 2 || total < threshold(plan, "minimum_multi_form_family_frequency") {
            continue;
        }
        let status = if members.len() <= 20 && members.iter().all(|x| provider_count[x] == 1) {
            "proposed"
        } else {
            "ambiguous"
        };
        let mut hashes: Vec<String> = members
            .iter()
            .map(|k| s(&forms[k]["form_sha256"]).unwrap().into())
            .collect();
        hashes.sort();
        let mut row = family_row(
            format!("provider|ru|{}", hashes.join("|")),
            "provider_morphology_family_candidate",
            "ru",
            "hunspell_dictionary_stem",
            status,
            &members,
            &forms,
        );
        row["provider_stem_sha256"] = json!(h(&stem));
        candidates.push(row);
        member_multi.extend(members);
    }
    let mut strong = Set::new();
    for a in arr(&previous["translation_surface_associations"])? {
        root.tick(1)?;
        if a["status"] == "proposed" {
            strong.insert(format!("de:{}", s(&a["source_form"])?));
            strong.insert(format!("ru:{}", s(&a["target_form"])?));
        }
    }
    for (k, form) in &forms {
        root.tick(1)?;
        if member_multi.contains(k) || (n(&form["occurrence_count"]) < 12 && !strong.contains(k)) {
            continue;
        }
        candidates.push(family_row(
            format!(
                "singleton|{}|{}",
                s(&form["language"])?,
                s(&form["form_sha256"])?
            ),
            "singleton_surface_probe_candidate",
            s(&form["language"])?,
            "singleton_recurrence_probe",
            "proposed",
            &[k.clone()],
            &forms,
        ));
    }
    let mut dedup: Map<String, V> = Map::new();
    for mut row in candidates {
        root.tick(1)?;
        let binding = field(&row, "binding")?;
        if let Some(current) = dedup.get_mut(&binding) {
            current["status"] = json!("ambiguous");
            if let Some(hash) = row.get("provider_stem_sha256") {
                let mut hashes = arr(&current["provider_stem_sha256s"])?.clone();
                hashes.push(hash.clone());
                hashes.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                hashes.dedup();
                current["provider_stem_sha256s"] = json!(hashes);
            }
        } else {
            if let Some(hash) = row.as_object_mut().unwrap().remove("provider_stem_sha256") {
                row["provider_stem_sha256s"] = json!([hash]);
            }
            dedup.insert(binding, row);
        }
    }
    let mut families: Vec<V> = dedup.into_values().collect();
    families.sort_by(|a, b| {
        a["language"]
            .as_str()
            .cmp(&b["language"].as_str())
            .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
    });
    Ok((families, forms, provider))
}
fn memberships(root: &ResearchExecution, families: &[V]) -> R<Map<String, Vec<String>>> {
    root.check()?;
    let mut by_form: Map<String, Vec<&V>> = Map::new();
    for f in families {
        root.tick(1)?;
        for key in arr(&f["member_keys"])? {
            root.tick(1)?;
            by_form.entry(s(key)?.into()).or_default().push(f);
        }
    }
    let rank = |f: &&V| {
        if f["status"] == "proposed" {
            0
        } else if f["status"] == "ambiguous" {
            1
        } else {
            2
        }
    };
    let mut out = Map::new();
    for (k, mut rows) in by_form {
        root.tick(1)?;
        rows.sort_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then_with(|| (n(&a["member_count"]) <= 1).cmp(&(n(&b["member_count"]) <= 1)))
                .then_with(|| n(&a["member_count"]).cmp(&n(&b["member_count"])))
                .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
        });
        out.insert(
            k,
            rows.into_iter()
                .take(2)
                .map(|f| field(f, "binding"))
                .collect::<R<_>>()?,
        );
    }
    Ok(out)
}
fn challengers(
    root: &ResearchExecution,
    mut families: Vec<V>,
    forms: &Map<String, V>,
    previous: &V,
    plan: &V,
) -> R<Vec<V>> {
    root.check()?;
    let members = memberships(root, &families)?;
    let score = |x: &V| {
        (
            (x["status"] == "proposed") as usize,
            n(&x["strict_support"]),
            n(&x["dice_millionths"]),
            -(n(&x["source_candidate_rank"]) as i64),
            -(n(&x["target_candidate_rank"]) as i64),
        )
    };
    let mut best_de: Map<String, &V> = Map::new();
    let mut best_ru: Map<String, &V> = Map::new();
    for row in arr(&previous["translation_surface_associations"])? {
        root.tick(1)?;
        if row["status"] == "deferred"
            || n(&row["strict_support"]) < 5
            || n(&row["dice_millionths"]) < 300_000
        {
            continue;
        }
        for (lang, best, field) in [
            ("de", &mut best_de, "source_form"),
            ("ru", &mut best_ru, "target_form"),
        ] {
            let key = format!("{lang}:{}", s(&row[field])?);
            if forms.contains_key(&key) && best.get(&key).is_none_or(|old| score(row) > score(old))
            {
                best.insert(key, row);
            }
        }
    }
    let mut evidence: Map<(String, String), Vec<(String, usize)>> = Map::new();
    for (lang, best, opposite, field) in [
        ("de", best_de, "ru", "target_form"),
        ("ru", best_ru, "de", "source_form"),
    ] {
        for (key, row) in best {
            root.tick(1)?;
            let opp = format!("{opposite}:{}", s(&row[field])?);
            for f in members.get(&opp).into_iter().flatten() {
                root.tick(1)?;
                evidence
                    .entry((lang.into(), f.clone()))
                    .or_default()
                    .push((key.clone(), n(&row["support"])));
            }
        }
    }
    let mut existing: Set<(String, Vec<String>)> = families
        .iter()
        .map(|f| {
            Ok((
                field(f, "language")?,
                arr(&f["member_form_sha256s"])?
                    .iter()
                    .map(|x| s(x).map(str::to_owned))
                    .collect::<R<_>>()?,
            ))
        })
        .collect::<R<_>>()?;
    for ((lang, opposite), rows) in evidence {
        root.tick(1)?;
        let unique: Vec<String> = rows
            .iter()
            .filter(|(k, _)| forms.contains_key(k))
            .map(|(k, _)| k.clone())
            .collect::<Set<_>>()
            .into_iter()
            .collect();
        if unique.len() < 2 {
            continue;
        }
        let support: usize = rows
            .iter()
            .filter(|(k, _)| unique.contains(k))
            .map(|(_, s)| s)
            .sum();
        let mut hashes: Vec<String> = unique
            .iter()
            .map(|k| s(&forms[k]["form_sha256"]).unwrap().into())
            .collect();
        hashes.sort();
        if support < threshold(plan, "alignment_challenger_minimum_combined_support")
            || !existing.insert((lang.clone(), hashes.clone()))
        {
            continue;
        }
        let mut row = family_row(
            format!("alignment-challenger|{lang}|{}", hashes.join("|")),
            "alignment_neighborhood_family_challenger",
            &lang,
            "shared_opposite_language_family_neighborhood",
            "deferred",
            &unique,
            forms,
        );
        row["alignment_support"] = json!(support);
        row["opposite_family_binding"] = json!(opposite);
        families.push(row);
    }
    families.sort_by(|a, b| {
        a["language"]
            .as_str()
            .cmp(&b["language"].as_str())
            .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
    });
    Ok(families)
}
#[derive(Default)]
struct Bridge {
    support: usize,
    strict: usize,
    proposed: usize,
    weighted: usize,
    ids: Set<String>,
}
fn bridges(root: &ResearchExecution, families: &[V], previous: &V, plan: &V) -> R<Vec<V>> {
    root.check()?;
    let members = memberships(root, families)?;
    let mut accum: Map<(String, String), Bridge> = Map::new();
    for row in arr(&previous["translation_surface_associations"])? {
        root.tick(1)?;
        let de = format!("de:{}", s(&row["source_form"])?);
        let ru = format!("ru:{}", s(&row["target_form"])?);
        for a in members.get(&de).into_iter().flatten() {
            root.tick(1)?;
            for b in members.get(&ru).into_iter().flatten() {
                root.tick(1)?;
                let e = accum.entry((a.clone(), b.clone())).or_default();
                e.support += n(&row["support"]);
                e.strict += n(&row["strict_support"]);
                e.weighted += n(&row["dice_millionths"]) * n(&row["support"]).max(1);
                e.proposed += (row["status"] == "proposed") as usize;
                e.ids.insert(field(row, "candidate_id")?);
            }
        }
    }
    let mut out = vec![];
    for ((a, b), e) in accum {
        root.tick(1)?;
        if e.support < threshold(plan, "family_bridge_minimum_support") {
            continue;
        }
        let dice = e.weighted / e.support.max(1);
        if dice < threshold(plan, "family_bridge_minimum_dice_millionths") {
            continue;
        }
        out.push(json!({"binding":format!("family-bridge|{a}|{b}"),"relation_type":"candidate_translation_neighborhood_bridge","subject_binding":a,"object_binding":b,"status":if e.proposed>0&&e.strict>=4{"proposed"}else{"ambiguous"},"support":e.support,"strict_support":e.strict,"dice_millionths":dice,"source_candidate_refs":e.ids,"accepted":false,"review_refs":[],"graph_effect":false}));
    }
    out.sort_by(|a, b| {
        n(&b["support"])
            .cmp(&n(&a["support"]))
            .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
    });
    Ok(out)
}
fn co_relations(root: &ResearchExecution, families: &[V], plan: &V) -> R<Vec<V>> {
    root.check()?;
    let members = memberships(root, families)?;
    let primary: Map<String, String> = members
        .into_iter()
        .map(|(k, v)| (k, v[0].clone()))
        .collect();
    let units = load_parallel(root)?;
    let positive: Vec<&V> = units
        .iter()
        .filter(|x| x["status"] == "proposed" && x["positive_evidence_eligible"] == true)
        .collect();
    let freq: Map<String, usize> = families
        .iter()
        .map(|x| Ok((field(x, "binding")?, n(&x["occurrence_count"]))))
        .collect::<R<_>>()?;
    let mut out = vec![];
    for lang in ["de", "ru"] {
        root.tick(1)?;
        let mut df: Map<String, usize> = Map::new();
        let mut pairs: Map<(String, String), usize> = Map::new();
        for unit in &positive {
            root.tick(1)?;
            let candidates: Set<String> = arr(&unit[lang])?
                .iter()
                .filter_map(|token| {
                    token
                        .as_str()
                        .and_then(|t| primary.get(&format!("{lang}:{t}")))
                        .cloned()
                })
                .collect();
            let mut ranked: Vec<_> = candidates.into_iter().collect();
            ranked.sort_by(|a, b| freq[b].cmp(&freq[a]).then_with(|| a.cmp(b)));
            ranked.truncate(18);
            for k in &ranked {
                root.tick(1)?;
                *df.entry(k.clone()).or_default() += 1
            }
            ranked.sort();
            for i in 0..ranked.len() {
                root.tick(1)?;
                for j in i + 1..ranked.len() {
                    root.tick(1)?;
                    *pairs
                        .entry((ranked[i].clone(), ranked[j].clone()))
                        .or_default() += 1
                }
            }
        }
        let mut rows = vec![];
        for ((a, b), support) in pairs {
            root.tick(1)?;
            if support < threshold(plan, "co_recurrence_minimum_support") {
                continue;
            }
            let dice = 2. * support as f64 / (df[&a] + df[&b]) as f64;
            let pmi = ((support * positive.len()) as f64 / (df[&a] * df[&b]) as f64).log2();
            if round(dice * 1_000_000.)
                < threshold(plan, "co_recurrence_minimum_dice_millionths") as i64
                || round(pmi * 1000.)
                    < threshold(plan, "co_recurrence_minimum_pmi_millibits") as i64
            {
                continue;
            }
            rows.push(json!({"binding":format!("co-recurrence|{lang}|{a}|{b}"),"relation_type":"candidate_within_language_co_recurrence","language":lang,"subject_binding":a,"object_binding":b,"status":if support>=12&&dice>=0.30&&pmi>=1.{"proposed"}else{"ambiguous"},"support":support,"unit_count":positive.len(),"dice_millionths":round(dice*1_000_000.),"pmi_millibits":round(pmi*1000.),"accepted":false,"review_refs":[],"graph_effect":false}));
        }
        rows.sort_by(|a, b| {
            n(&b["support"])
                .cmp(&n(&a["support"]))
                .then_with(|| n(&b["dice_millionths"]).cmp(&n(&a["dice_millionths"])))
                .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
        });
        out.extend(rows.into_iter().take(threshold(
            plan,
            "maximum_co_recurrence_relations_per_language",
        )));
    }
    Ok(out)
}
fn find(root: &ResearchExecution, parent: &mut Map<String, String>, value: &str) -> R<String> {
    root.check()?;
    let mut current = value.to_owned();
    let mut chain = Vec::new();
    loop {
        root.tick(1)?;
        let next = parent
            .entry(current.clone())
            .or_insert_with(|| current.clone())
            .clone();
        if next == current {
            break;
        }
        chain.push(current);
        current = next;
    }
    for member in chain {
        root.tick(1)?;
        parent.insert(member, current.clone());
    }
    Ok(current)
}
fn clusters(root: &ResearchExecution, families: &[V], bridges: &[V]) -> R<Vec<V>> {
    root.check()?;
    let by_binding: Map<String, &V> = families
        .iter()
        .map(|f| Ok((field(f, "binding")?, f)))
        .collect::<R<_>>()?;
    let mut parent = Map::new();
    for bridge in bridges {
        root.tick(1)?;
        if bridge["status"] == "proposed" {
            let a = find(root, &mut parent, s(&bridge["subject_binding"])?)?;
            let b = find(root, &mut parent, s(&bridge["object_binding"])?)?;
            if a != b {
                parent.insert(a.clone().max(b.clone()), a.min(b));
            }
        }
    }
    let mut components: Map<String, Set<String>> = Map::new();
    for value in parent.keys().cloned().collect::<Vec<_>>() {
        root.tick(1)?;
        components
            .entry(find(root, &mut parent, &value)?)
            .or_default()
            .insert(value);
    }
    let mut out = vec![];
    for members in components.into_values() {
        root.tick(1)?;
        let mut languages: Map<String, usize> = Map::new();
        for k in &members {
            root.tick(1)?;
            *languages
                .entry(field(by_binding[k], "language")?)
                .or_default() += 1
        }
        if !languages.contains_key("de") || !languages.contains_key("ru") {
            continue;
        }
        let member_bridges: Vec<&V> = bridges
            .iter()
            .filter(|b| {
                members.contains(b["subject_binding"].as_str().unwrap_or(""))
                    && members.contains(b["object_binding"].as_str().unwrap_or(""))
            })
            .collect();
        out.push(json!({"binding":format!("cluster|{}",members.iter().cloned().collect::<Vec<_>>().join("|")),"candidate_kind":"bilingual_recurrence_neighborhood_cluster_candidate","status":if members.len()==2&&!member_bridges.is_empty(){"proposed"}else{"ambiguous"},"member_bindings":members,"member_count":members.len(),"language_member_counts":languages,"bridge_count":member_bridges.len(),"aggregate_bridge_support":member_bridges.iter().map(|x|n(&x["support"])).sum::<usize>(),"accepted":false,"review_refs":[],"graph_effect":false,"concept_identity_asserted":false}));
    }
    out.sort_by(|a, b| {
        n(&b["aggregate_bridge_support"])
            .cmp(&n(&a["aggregate_bridge_support"]))
            .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
    });
    Ok(out)
}
fn relations(
    root: &ResearchExecution,
    families: &[V],
    bridges: &[V],
    co: &[V],
    clusters: &[V],
) -> R<Vec<V>> {
    root.check()?;
    let mut rows = bridges.to_vec();
    rows.extend_from_slice(co);
    let mut by_form: Map<String, Set<String>> = Map::new();
    for f in families {
        root.tick(1)?;
        let binding = s(&f["binding"])?;
        for fh in arr(&f["member_form_sha256s"])? {
            root.tick(1)?;
            let hash = s(fh)?;
            by_form
                .entry(hash.into())
                .or_default()
                .insert(binding.into());
            rows.push(json!({"binding":format!("membership|{hash}|{binding}"),"relation_type":"candidate_form_membership","subject_form_sha256":hash,"object_binding":binding,"status":f["status"],"support":f["occurrence_count"],"accepted":false,"review_refs":[],"graph_effect":false}));
        }
    }
    let mut seen = Set::new();
    for bindings in by_form.values() {
        root.tick(1)?;
        let keys: Vec<_> = bindings.iter().collect();
        for i in 0..keys.len() {
            root.tick(1)?;
            for j in i + 1..keys.len() {
                root.tick(1)?;
                let a = keys[i];
                let b = keys[j];
                if !seen.insert((a, b)) {
                    continue;
                }
                rows.push(json!({"binding":format!("competition|{a}|{b}"),"relation_type":"candidate_competes_with","subject_binding":a,"object_binding":b,"status":"ambiguous","support":1,"accepted":false,"review_refs":[],"graph_effect":false}));
            }
        }
    }
    let mut family_cluster: Map<String, String> = Map::new();
    for c in clusters {
        root.tick(1)?;
        let binding = s(&c["binding"])?;
        for member in arr(&c["member_bindings"])? {
            root.tick(1)?;
            let member = s(member)?;
            family_cluster.insert(member.into(), binding.into());
            rows.push(json!({"binding":format!("cluster-membership|{member}|{binding}"),"relation_type":"candidate_cluster_membership","subject_binding":member,"object_binding":binding,"status":c["status"],"support":c["aggregate_bridge_support"],"accepted":false,"review_refs":[],"graph_effect":false}));
        }
    }
    let mut pairs: Map<(String, String), (usize, usize)> = Map::new();
    for row in co {
        root.tick(1)?;
        let a = family_cluster.get(s(&row["subject_binding"])?);
        let b = family_cluster.get(s(&row["object_binding"])?);
        if let (Some(a), Some(b)) = (a, b) {
            if a == b {
                continue;
            }
            let pair = if a < b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            let e = pairs.entry(pair).or_default();
            e.0 += n(&row["support"]);
            e.1 += 1;
        }
    }
    for ((a, b), (support, count)) in pairs {
        root.tick(1)?;
        rows.push(json!({"binding":format!("cluster-co-recurrence|{a}|{b}"),"relation_type":"candidate_cluster_co_recurrence","subject_binding":a,"object_binding":b,"status":"ambiguous","support":support,"family_relation_count":count,"accepted":false,"review_refs":[],"graph_effect":false}));
    }
    rows.sort_by(|a, b| {
        a["relation_type"]
            .as_str()
            .cmp(&b["relation_type"].as_str())
            .then_with(|| a["binding"].as_str().cmp(&b["binding"].as_str()))
    });
    Ok(rows)
}
type Identity = (String, String);
fn bindings(
    root: &ResearchExecution,
    families: &[V],
    clusters: &[V],
    relations: &[V],
) -> R<Vec<Identity>> {
    root.check()?;
    let mut out = Set::new();
    for (kind, rows) in [
        ("family", families),
        ("cluster", clusters),
        ("relation", relations),
    ] {
        for row in rows {
            root.tick(1)?;
            out.insert((kind.into(), field(row, "binding")?));
        }
    }
    Ok(out.into_iter().collect())
}
fn identities(root: &ResearchExecution, bindings: &[Identity]) -> R<Map<Identity, String>> {
    root.check()?;
    let data = load(root, &route("identity-issuance.v1.json"))?;
    let mut found = Map::new();
    for row in arr(&data["identities"])? {
        root.tick(1)?;
        found.insert(
            (field(row, "kind")?, field(row, "binding")?),
            field(row, "id")?,
        );
    }
    if found.keys().collect::<Set<_>>() != bindings.iter().collect::<Set<_>>()
        || found.len() != arr(&data["identities"])?.len()
    {
        return Err("candidate identity binding drift".into());
    }
    if found.values().collect::<Set<_>>().len() != found.len() {
        return Err("candidate identity collision".into());
    }
    Ok(found)
}
fn issue(root: &ResearchExecution, bindings: &[Identity]) -> R<()> {
    root.check()?;
    let p = route("identity-issuance.v1.json");
    if root.join(&p).exists() {
        return Err("identity issuance exists; refusing remint".into());
    }
    let mut random = fs::File::open("/dev/urandom").map_err(|e| e.to_string())?;
    let mut rows = vec![];
    for (kind, binding) in bindings {
        root.tick(1)?;
        let prefix = match kind.as_str() {
            "family" => "tos.annotation.morph-family-candidate.sid-",
            "cluster" => "tos.annotation.theme-cluster-candidate.sid-",
            "relation" => "tos.claim.typed-relation-candidate.sid-",
            _ => return Err("invalid identity kind".into()),
        };
        let mut bytes = [0u8; 16];
        root.read_exact(&mut random, &mut bytes)?;
        rows.push(json!({"kind":kind,"binding":binding,"id":format!("{prefix}{}",bytes.iter().map(|x|format!("{x:02x}")).collect::<String>())}));
    }
    crate::research_parallel_lexical::write_exclusive(
        root,
        &p,
        &pretty(
            &json!({"schema_version":"tos_zarathustra_morphology_theme_candidate_identity_issuance_v1","issuance_id":"tos.identity-issuance.zarathustra-morphology-themes-v1","issued_on":"2026-09-02","opaque_identity":true,"binding_is_not_identity_or_linguistic_judgment":true,"candidate_count":rows.len(),"identities":rows}),
        )?,
        0o644,
    )
}
fn publicize(
    root: &ResearchExecution,
    rows: &[V],
    kind: &str,
    ids: &Map<Identity, String>,
) -> R<Vec<V>> {
    root.check()?;
    let mut out = vec![];
    for raw in rows {
        root.tick(1)?;
        let mut row = raw.clone();
        let binding = field(&row, "binding")?;
        let obj = row.as_object_mut().unwrap();
        obj.remove("binding");
        obj.insert(
            format!("{kind}_id"),
            json!(ids.get(&(kind.into(), binding)).ok_or("missing identity")?),
        );
        obj.remove("member_keys");
        obj.remove("opposite_family_binding");
        for key in ["subject_binding", "object_binding"] {
            root.tick(1)?;
            if let Some(value) = obj.remove(key) {
                let ref_ = s(&value)?;
                let target = if ref_.starts_with("cluster|") {
                    "cluster"
                } else {
                    "family"
                };
                obj.insert(
                    key.replace("binding", "ref"),
                    json!(
                        ids.get(&(target.into(), ref_.into()))
                            .ok_or("missing reference identity")?
                    ),
                );
            }
        }
        if let Some(members) = obj.remove("member_bindings") {
            let refs = arr(&members)?
                .iter()
                .map(|x| {
                    ids.get(&("family".into(), s(x)?.into()))
                        .cloned()
                        .ok_or("missing family identity".into())
                })
                .collect::<R<Vec<_>>>()?;
            obj.insert("member_refs".into(), json!(refs));
        }
        out.push(row);
    }
    Ok(out)
}
fn readable(
    root: &ResearchExecution,
    families: &[V],
    forms: &Map<String, V>,
    clusters: &[V],
    relations: &[V],
    ids: &Map<Identity, String>,
    summary: &V,
) -> R<V> {
    root.check()?;
    let mut labels: Map<String, Vec<String>> = Map::new();
    let mut languages = Map::new();
    let mut private_families = vec![];
    for f in families {
        root.tick(1)?;
        let forms: Vec<String> = arr(&f["member_keys"])?
            .iter()
            .map(|x| field(&forms[s(x)?], "form"))
            .collect::<R<_>>()?;
        let binding = field(f, "binding")?;
        labels.insert(binding.clone(), forms.clone());
        languages.insert(binding.clone(), field(f, "language")?);
        let mut row = f.clone();
        row["family_id"] = json!(
            ids.get(&("family".into(), binding))
                .ok_or("missing family id")?
        );
        row["forms"] = json!(forms);
        private_families.push(row);
    }
    let mut private_clusters = vec![];
    for c in clusters {
        root.tick(1)?;
        let mut display = json!({});
        for lang in ["de", "ru"] {
            root.tick(1)?;
            let mut terms = vec![];
            for member in arr(&c["member_bindings"])? {
                root.tick(1)?;
                let member = s(member)?;
                if languages[member] == lang {
                    terms.extend(labels[member].clone());
                }
            }
            terms.truncate(8);
            display[lang] = json!(terms);
        }
        let mut row = c.clone();
        row["cluster_id"] = json!(
            ids.get(&("cluster".into(), field(c, "binding")?))
                .ok_or("missing cluster id")?
        );
        row["display_hint"] = display;
        private_clusters.push(row);
    }
    let mut examples = json!({});
    for (name, forms) in [
        ("de_mensch", vec!["mensch", "menschen"]),
        ("de_liebe", vec!["liebe", "lieben", "liebt"]),
        ("ru_human", vec!["человек", "людей", "люди"]),
        ("ru_love", vec!["любовь", "люблю"]),
    ] {
        examples[name] = json!(
            private_families
                .iter()
                .filter(|x| x["forms"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| forms.contains(&v.as_str().unwrap())))
                .cloned()
                .collect::<Vec<_>>()
        );
    }
    let rels: Vec<V> = relations
        .iter()
        .map(|x| {
            let mut row = x.clone();
            row["relation_id"] =
                json!(ids[&("relation".into(), x["binding"].as_str().unwrap().into())]);
            row
        })
        .collect();
    Ok(
        json!({"schema_version":"tos_zarathustra_morphology_theme_private_analysis_v1","source_bearing":true,"required_mode":"0600","summary":summary,"families":private_families,"clusters":private_clusters,"typed_relations":rels,"named_probe_examples":examples,"authority_boundary":"Private readable agent proposals for surface families, theme clusters and typed relations, retaining their source bindings and proposal status."}),
    )
}
fn census(root: &ResearchExecution, rows: &[V], field: &str) -> R<Map<String, usize>> {
    root.check()?;
    let mut out = Map::new();
    for row in rows {
        root.tick(1)?;
        *out.entry(row[field].as_str().unwrap_or("").into())
            .or_default() += 1
    }
    Ok(out)
}
struct Generated {
    outputs: Map<String, Vec<u8>>,
    private: Option<Vec<u8>>,
    bindings: Vec<Identity>,
    analysis: V,
}
fn generate(root: &ResearchExecution, with_ids: bool) -> R<Generated> {
    root.check()?;
    let plan = load(root, &route("plan.v1.json"))?;
    for (name, record) in plan["inputs"]
        .as_object()
        .ok_or("plan inputs object required")?
    {
        root.tick(1)?;
        let raw = read(root, s(&record["ref"])?)?;
        if hash(&raw) != record["sha256"].as_str().unwrap_or("") {
            return Err(format!("input drift: {name}"));
        }
    }
    let p = private_previous();
    if root
        .source_file(&p, 64 * 1024 * 1024)
        .and_then(|f| f.metadata().map_err(|e| e.to_string()))
        .map(|m| m.permissions().mode() & 0o777 != 0o600)
        .unwrap_or(true)
    {
        return Err("previous private lexical analysis missing or not 0600".into());
    }
    let previous = load(root, &p)?;
    let (families, forms, provider) = surface_families(root, &previous, &plan)?;
    let families = challengers(root, families, &forms, &previous, &plan)?;
    let bridges = bridges(root, &families, &previous, &plan)?;
    let co = co_relations(root, &families, &plan)?;
    let clusters = clusters(root, &families, &bridges)?;
    let relations = relations(root, &families, &bridges, &co, &clusters)?;
    let bindings = bindings(root, &families, &clusters, &relations)?;
    if !with_ids {
        return Ok(Generated {
            outputs: Map::new(),
            private: None,
            bindings,
            analysis: json!({"families":families,"clusters":clusters,"relations":relations}),
        });
    }
    let ids = identities(root, &bindings)?;
    let summary = json!({"schema_version":"tos_zarathustra_morphology_theme_candidate_summary_v1","status":"completed-agent-candidate-pass-no-promotion","parts":4,"input_form_candidate_count":forms.len(),"morphological_family_candidate_count":families.len(),"families_by_language":census(root, &families,"language")?,"family_status_counts":census(root, &families,"status")?,"alignment_challenger_family_count":families.iter().filter(|x|x["candidate_kind"]=="alignment_neighborhood_family_challenger").count(),"thematic_cluster_candidate_count":clusters.len(),"cluster_status_counts":census(root, &clusters,"status")?,"typed_relation_candidate_count":relations.len(),"relation_type_counts":census(root, &relations,"relation_type")?,"accepted_candidate_count":0,"human_review_count":0,"semantic_relation_asserted":false,"concept_identity_asserted":false,"graph_effect":false,"canon_effect":false});
    let mut analysis = readable(
        root, &families, &forms, &clusters, &relations, &ids, &summary,
    )?;
    analysis["russian_morphology_provider"] = provider.clone();
    let mut outputs = Map::new();
    outputs.insert(
        route("morphological-family-candidates.v1.jsonl"),
        lines(&publicize(root, &families, "family", &ids)?)?,
    );
    outputs.insert(
        route("typed-relation-candidates.v1.jsonl"),
        lines(&publicize(root, &relations, "relation", &ids)?)?,
    );
    outputs.insert(
        route("thematic-cluster-candidates.v1.jsonl"),
        lines(&publicize(root, &clusters, "cluster", &ids)?)?,
    );
    outputs.insert(route("summary.v1.json"), pretty(&summary)?);
    let forms_with_membership = families
        .iter()
        .flat_map(|f| {
            f["member_keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
        })
        .collect::<Set<_>>()
        .len();
    let coverage = json!({"schema_version":"tos_zarathustra_morphology_theme_candidate_coverage_v1","parts_complete":4,"languages":["de","ru"],"input_keyword_form_candidates":forms.len(),"forms_with_family_membership":forms_with_membership,"positive_alignment_units_used_for_co_recurrence":load(root,&previous_ref("coverage-receipt.v1.json"))?["proposed_positive_evidence_units"],"german_provider_census_token_coverage":load(root,s(&plan["inputs"]["german_morphology_census_receipt"]["ref"] )?)?["coverage"]["token_weighted_coverage"],"provider_output_used_as_accepted_morphology":false,"russian_provider":provider,"tracked_source_strings":false,"private_output_mode":"0600","competing_memberships_preserved":true,"zero_review_refs":true,"accepted_candidate_count":0,"graph_effect":false});
    outputs.insert(route("coverage-receipt.v1.json"), pretty(&coverage)?);
    outputs.insert(route("provenance.jsonl"),lines(&[json!({"schema_version":"tos_provenance_event_v1","event_id":"tos.event.zarathustra-morphology-theme-candidates-v1.build","event_type":"agent_candidate_morphology_theme_materialization","occurred_at":"2026-09-02T03:00:00-06:00","agent_ref":"codex-internal-agents.morphology-theme-candidate-v1","software_ref":GENERATOR,"software_sha256":RECIPE_SHA256,"plan_ref":route("plan.v1.json"),"plan_sha256":hash(&read(root,&route("plan.v1.json"))?),"authority_boundary":plan["authority_boundary"]})])?);
    let private = pretty(&analysis)?;
    let output_meta: Map<String, V> = outputs
        .iter()
        .map(|(p, raw)| (p.clone(), json!({"sha256":hash(raw),"byte_size":raw.len()})))
        .collect();
    let manifest = json!({"schema_version":"tos_zarathustra_morphology_theme_candidate_manifest_v1","plan_ref":route("plan.v1.json"),"plan_sha256":hash(&read(root,&route("plan.v1.json"))?),"identity_issuance_ref":route("identity-issuance.v1.json"),"identity_issuance_sha256":hash(&read(root,&route("identity-issuance.v1.json"))?),"generated_outputs":output_meta,"private_outputs":{(self::private()):{"sha256":hash(&private),"byte_size":private.len(),"required_mode":"0600"}},"source_text_included":false,"accepted_candidate_count":0,"semantic_relation_asserted":false,"concept_identity_asserted":false,"graph_effect":false,"canon_effect":false});
    outputs.insert(route("manifest.v1.json"), pretty(&manifest)?);
    Ok(Generated {
        outputs,
        private: Some(private),
        bindings,
        analysis,
    })
}
pub fn run(root: &Path, args: &[String]) -> R<V> {
    let execution = ResearchExecution::new(root, 180)?;
    run_scoped(&execution, args)
}
pub fn run_scoped(root: &ResearchExecution, args: &[String]) -> R<V> {
    root.check()?;
    let mut mode = None;
    let mut issue_ids = false;
    for arg in args {
        root.tick(1)?;
        match arg.as_str() {
            "--build" | "--check" | "--preview" => {
                if mode.replace(arg.as_str()).is_some() {
                    return Err("exactly one of --build, --check, --preview required".into());
                }
            }
            "--issue-identities" => issue_ids = true,
            _ => return Err(format!("unrecognized argument: {arg}")),
        }
    }
    let mode = mode.ok_or("exactly one of --build, --check, --preview required")?;
    if issue_ids && mode != "--build" {
        return Err("--issue-identities is valid only with --build".into());
    }
    if mode == "--preview" {
        let g = generate(root, false)?;
        return Ok(
            json!({"identity_count":g.bindings.len(),"family_count":arr(&g.analysis["families"] )?.len(),"cluster_count":arr(&g.analysis["clusters"] )?.len(),"relation_count":arr(&g.analysis["relations"] )?.len()}),
        );
    }
    if mode == "--build" && issue_ids {
        let g = generate(root, false)?;
        issue(root, &g.bindings)?;
    }
    let g = generate(root, true)?;
    if mode == "--build" {
        for (p, raw) in &g.outputs {
            root.tick(1)?;
            write(root, p, raw, 0o644)?
        }
        write(root, &private(), g.private.as_ref().unwrap(), 0o600)?;
    } else {
        for (p, raw) in &g.outputs {
            root.tick(1)?;
            if read(root, p)? != *raw {
                return Err(format!("tracked parity mismatch: {p}"));
            }
        }
        if read(root, &private())? != *g.private.as_ref().unwrap() {
            return Err(format!("private parity mismatch: {}", private()));
        }
        if root
            .source_file(&private(), 64 * 1024 * 1024)?
            .metadata()
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777
            != 0o600
        {
            return Err(format!("private mode mismatch: {}", private()));
        }
    }
    Ok(g.analysis["summary"].clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hunspell_input_cap_fails_before_external_process() {
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        let forms = vec!["а".repeat(8 * 1024 * 1024 + 1)];
        assert_eq!(
            hunspell(&execution, &forms).unwrap_err(),
            "Hunspell stdin cap exceeded"
        );
    }
    #[test]
    fn signatures_preserve_competing_methods() {
        assert_eq!(signatures("ſchickſal", "de").unwrap()[0].0, "schicksal");
        assert_eq!(signatures("ﬃ", "de").unwrap()[0].0, "ffi");
        let ru = signatures("человѣк", "ru").unwrap();
        assert_eq!(ru[0].0, "человѣк");
        let ru = signatures("сильного", "ru").unwrap();
        assert!(ru.contains(&("сильн".into(), "adjectival_inflection".into(), 2)));
        assert!(ru.contains(&("сильн".into(), "nominal_inflection".into(), 2)));
        let de = signatures("Häusern", "de").unwrap();
        assert!(de.contains(&("haus".into(), "nominal_inflection".into(), 2)));
    }
    #[test]
    fn memberships_preserve_two_competing_candidates() {
        let families = vec![
            json!({"binding":"a","status":"ambiguous","member_count":2,"member_keys":["ru:свет"]}),
            json!({"binding":"b","status":"proposed","member_count":1,"member_keys":["ru:свет"]}),
            json!({"binding":"c","status":"proposed","member_count":2,"member_keys":["ru:свет"]}),
        ];
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        assert_eq!(
            memberships(&execution, &families).unwrap()["ru:свет"],
            vec!["c", "b"]
        );
    }
    #[test]
    fn public_projection_withholds_strings_and_bindings() {
        let rows = vec![
            json!({"binding":"surface|de|x","member_keys":["de:licht"],"accepted":false,"review_refs":[],"graph_effect":false}),
        ];
        let ids = Map::from([(("family".into(), "surface|de|x".into()), "opaque".into())]);
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        let p = publicize(&execution, &rows, "family", &ids).unwrap();
        assert!(p[0].get("binding").is_none());
        assert!(p[0].get("member_keys").is_none());
        assert_eq!(p[0]["family_id"], "opaque");
        assert_eq!(p[0]["accepted"], false);
    }
}
