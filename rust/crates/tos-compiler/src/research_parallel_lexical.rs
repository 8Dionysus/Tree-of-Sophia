//! Whole-work mechanical lexical candidates. Source strings stay in private artifacts.
use crate::research_execution::ResearchExecution;
#[cfg(test)]
use rusqlite::Connection;
use rusqlite::params;
use serde_json::{Value as V, json};
use std::{
    collections::{BTreeMap as Map, BTreeSet as Set},
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tos_foundation::{Digest256, JsonLimits, JsonMode, emit_python_compact_json, parse_json};
use unicode_normalization::UnicodeNormalization;
type R<T> = Result<T, String>;
const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
const ROUTE: &str = "lexical-indexes/dta-antonovsky-parallel-candidates-v1";
const ALIGN: &str = "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1";
const RU_TECH: &str = "technical-markup/antonovsky-1911-structural-paragraph-v2";
const DE_TECH: &str = "technical-markup/dta-first-editions-parts-1-4-v1";
const PRIVATE: &str = "gold-sets/foundation-pilot-v1/local-content/parallel-lexical-candidates-v1";
// The v1 compatibility projection identifies the selected maintained rendering recipe.
// Native execution identity belongs to the independent execution receipt.
const GENERATOR: &str = "scripts/build_zarathustra_parallel_lexical_candidates_v1.py";
// Selected maintained rendering-recipe identity, separate from native execution.
const RECIPE_SHA256: &str = "5eeabab845a141f7ee7c34bc7ad4141694f238fde0531c4c0fa282a1ec659f84";
fn path(s: &str) -> String {
    format!("{WORK}/{s}")
}
fn route(s: &str) -> String {
    path(&format!("{ROUTE}/{s}"))
}
fn private(s: &str) -> String {
    path(&format!("{PRIVATE}/{s}"))
}
pub(crate) fn h(s: &str) -> String {
    hash(s.as_bytes())
}
pub(crate) fn hash(b: &[u8]) -> String {
    Digest256::of_bytes(b).to_hex()
}
pub(crate) fn read(root: &ResearchExecution, p: &str) -> R<Vec<u8>> {
    root.read(p)
}
pub(crate) fn load(root: &ResearchExecution, p: &str) -> R<V> {
    let bytes = read(root, p)?;
    root.check()?;
    let result = serde_json::from_slice(&bytes).map_err(|e| format!("{p}: {e}"));
    root.check()?;
    result
}
/// Exact authenticated technical profile; algorithms never branch on input hashes.
#[derive(Debug)]
pub(crate) struct SelectedPlan {
    pub reference: String,
    pub digest: String,
    pub value: V,
    pub custom: bool,
}
pub(crate) fn bound_json(
    root: &ResearchExecution,
    reference: &str,
    digest: &str,
    size: Option<u64>,
    mode: Option<u32>,
    max_bytes: u64,
) -> R<V> {
    let mut file = root.source_file(reference, max_bytes)?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if size.is_some_and(|n| n != before.len())
        || mode.is_some_and(|m| before.permissions().mode() & 0o7777 != m)
    {
        return Err(format!("bound input size/mode drift: {reference}"));
    }
    let bytes = root.read_file(&mut file, max_bytes)?;
    root.verify_file_unchanged(&file, &before)?;
    if hash(&bytes) != digest {
        return Err(format!("bound input digest drift: {reference}"));
    }
    root.check()?;
    let result = serde_json::from_slice(&bytes).map_err(|e| format!("{reference}: {e}"));
    root.check()?;
    result
}
pub(crate) fn select_plan(
    root: &ResearchExecution,
    reference: &str,
    default_ref: &str,
    default_digest: &str,
) -> R<SelectedPlan> {
    let original = bound_json(root, default_ref, default_digest, None, None, 64 * 1024)?;
    let custom = reference != default_ref;
    if !custom {
        return Ok(SelectedPlan {
            reference: reference.into(),
            digest: default_digest.into(),
            value: original,
            custom,
        });
    }
    use std::os::unix::fs::MetadataExt;
    let root_metadata = root
        .root_directory()
        .metadata()
        .map_err(|e| e.to_string())?;
    let mut profile_file = root.source_file(reference, 64 * 1024)?;
    let profile_metadata = profile_file.metadata().map_err(|e| e.to_string())?;
    let uid = unsafe { libc::geteuid() };
    if root_metadata.permissions().mode() & 0o7777 != 0o700
        || root_metadata.uid() != uid
        || profile_metadata.permissions().mode() & 0o7777 != 0o600
        || profile_metadata.uid() != uid
    {
        return Err("custom profile requires owned private 0700 carrier and 0600 plan".into());
    }
    let bytes = root.read_file(&mut profile_file, 64 * 1024)?;
    root.verify_file_unchanged(&profile_file, &profile_metadata)?;
    root.check()?;
    let value: V = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    root.check()?;
    let lineage = &value["input_profile_lineage"];
    if !lineage.is_object()
        || lineage["profile_version"].as_u64().is_none_or(|n| n < 2)
        || lineage["supersedes_plan_ref"] != default_ref
        || lineage["supersedes_plan_sha256"] != default_digest
        || value["plan_id"].as_str().is_none_or(|id| id.is_empty())
        || value["plan_id"] == original["plan_id"]
    {
        return Err(
            "technical profile requires distinct identity and exact predecessor lineage".into(),
        );
    }
    if value["status"] != "proposed-technical-input-profile-successor"
        || !value
            .as_object()
            .is_some_and(|o| o.contains_key("frozen_at"))
        || !value["frozen_at"].is_null()
    {
        return Err("technical profile requires explicit proposal status".into());
    }
    let mut comparable = value.clone();
    let object = comparable.as_object_mut().ok_or("plan object required")?;
    object.remove("input_profile_lineage");
    for key in ["plan_id", "status", "frozen_at"] {
        object.insert(key.into(), original[key].clone());
    }
    let inputs = object
        .get_mut("inputs")
        .and_then(V::as_object_mut)
        .ok_or("plan inputs object required")?;
    let old_inputs = original["inputs"]
        .as_object()
        .ok_or("default inputs object required")?;
    if inputs.len() != old_inputs.len() {
        return Err("technical profile input membership differs".into());
    }
    for (key, record) in inputs {
        root.tick(1)?;
        let old = old_inputs.get(key).ok_or("technical profile added input")?;
        let digest = s(&record["sha256"])?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("technical profile input SHA256 required".into());
        }
        record
            .as_object_mut()
            .ok_or("input record object required")?
            .insert("sha256".into(), old["sha256"].clone());
    }
    root.check()?;
    if comparable != original {
        return Err("technical profile changes semantic fields or input references".into());
    }
    let digest = hash(&bytes);
    root.check()?;
    Ok(SelectedPlan {
        reference: reference.into(),
        digest,
        value,
        custom,
    })
}
pub(crate) fn loadl(root: &ResearchExecution, p: &str) -> R<Vec<V>> {
    String::from_utf8(read(root, p)?)
        .map_err(|e| e.to_string())?
        .lines()
        .map(|l| {
            root.tick(1)?;
            serde_json::from_str(l).map_err(|e| e.to_string())
        })
        .collect()
}
pub(crate) fn arr(v: &V) -> R<&Vec<V>> {
    v.as_array().ok_or_else(|| "array required".into())
}
pub(crate) fn s(v: &V) -> R<&str> {
    v.as_str().ok_or_else(|| "string required".into())
}
pub(crate) fn n(v: &V) -> usize {
    v.as_u64().unwrap_or(0) as usize
}
pub(crate) fn b(v: &V) -> bool {
    v.as_bool().unwrap_or(false)
}
fn sorted(v: &V) -> V {
    match v {
        V::Object(m) => V::Object(
            m.iter()
                .collect::<Map<_, _>>()
                .into_iter()
                .map(|(k, v)| (k.clone(), sorted(v)))
                .collect(),
        ),
        V::Array(v) => V::Array(v.iter().map(sorted).collect()),
        _ => v.clone(),
    }
}
/// Foundation owns CPython float spelling. Sorting is explicit, independent of
/// serde feature unification. Pretty framing changes whitespace only.
fn encode(v: &V, pretty: bool) -> R<Vec<u8>> {
    let raw = serde_json::to_vec(&sorted(v)).map_err(|e| e.to_string())?;
    let limits =
        JsonLimits::new(64 * 1024 * 1024, 128, 4_000_000, 4300).map_err(|e| e.to_string())?;
    let doc = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(|e| e.to_string())?;
    let compact = emit_python_compact_json(doc.root(), limits).map_err(|e| e.to_string())?;
    if !pretty {
        return Ok(compact);
    }
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut string = false;
    let mut escape = false;
    for (i, &b) in compact.iter().enumerate() {
        if string {
            out.push(b);
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                string = false;
            }
            continue;
        }
        match b {
            b'"' => {
                string = true;
                out.push(b);
            }
            b'{' | b'[' => {
                out.push(b);
                depth += 1;
                if compact.get(i + 1) != Some(&(if b == b'{' { b'}' } else { b']' })) {
                    out.push(b'\n');
                    out.extend(vec![b' '; depth * 2]);
                }
            }
            b'}' | b']' => {
                depth -= 1;
                if compact.get(i.wrapping_sub(1)) != Some(&(if b == b'}' { b'{' } else { b'[' })) {
                    out.push(b'\n');
                    out.extend(vec![b' '; depth * 2]);
                }
                out.push(b);
            }
            b',' => {
                out.extend(b",\n");
                out.extend(vec![b' '; depth * 2]);
            }
            b':' => out.extend(b": "),
            _ => out.push(b),
        }
    }
    out.push(b'\n');
    Ok(out)
}
pub(crate) fn pretty(v: &V) -> R<Vec<u8>> {
    encode(v, true)
}
pub(crate) fn lines(v: &[V]) -> R<Vec<u8>> {
    let mut o = vec![];
    for x in v {
        o.extend(encode(x, false)?);
        o.push(b'\n');
    }
    Ok(o)
}
pub(crate) fn write(root: &ResearchExecution, p: &str, bytes: &[u8], mode: u32) -> R<()> {
    root.write(p, bytes, mode, false)
}
pub(crate) fn write_exclusive(root: &ResearchExecution, p: &str, bytes: &[u8], mode: u32) -> R<()> {
    root.write(p, bytes, mode, true)
}
fn letter(c: char) -> bool {
    c.is_ascii_alphabetic() || "ÄÖÜäöüßẞЁёІіЇїѢѣѲѳѴѵ".contains(c) || ('А'..='я').contains(&c)
}
fn base(x: &str) -> String {
    let normalized: String = x.nfc().collect();
    let points = normalized.chars().count();
    // Unicode 16 full casefold expands one scalar to at most three. UTF-8
    // output likewise fits three times the input bytes; budgets are exact
    // input-scalars plus that normative expansion bound.
    tos_foundation::python_casefold_unicode16_v1(
        &normalized,
        points,
        points.saturating_mul(3),
        normalized.len().saturating_mul(3),
    )
    .expect("Unicode 16 casefold stays within its three-scalar expansion bound")
}
fn key(x: &str, lang: &str) -> String {
    let mut x = base(x);
    if lang == "ru" {
        x = x
            .chars()
            .map(|c| match c {
                'ѣ' => 'е',
                'і' | 'ї' | 'ѵ' => 'и',
                'ѳ' => 'ф',
                _ => c,
            })
            .collect();
        if x.chars().count() > 2 && x.ends_with('ъ') {
            x.pop();
        }
    }
    x
}
fn clean(x: &str, lang: &str) -> bool {
    x.chars().all(|c| {
        if lang == "de" {
            !('А'..='я').contains(&c) && !"ЁёІіЇїѢѣѲѳѴѵ".contains(c)
        } else {
            !c.is_ascii_alphabetic() && !"ÄÖÜäöüßẞ".contains(c)
        }
    })
}
fn exact(x: &str) -> Vec<(String, usize, usize)> {
    let c: Vec<char> = x.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        if !letter(c[i]) {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        loop {
            while i < c.len() && letter(c[i]) {
                i += 1
            }
            if i + 1 < c.len() && "’'".contains(c[i]) && letter(c[i + 1]) {
                i += 2
            } else {
                break;
            }
        }
        out.push((c[start..i].iter().collect(), start, i));
    }
    out
}
const LETTER: &str = "A-Za-zÄÖÜäöüßẞА-Яа-яЁёІіЇїѢѣѲѳѴѵ";
fn join_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(&format!(
            r"([{LETTER}]{{2,}})[-¬][\s\x1c-\x1f]*\n[\s\x1c-\x1f]*([{LETTER}]{{2,}})"
        ))
        .unwrap()
    })
}
fn preprocess(x: &str) -> String {
    let x = x.replace("\r\n", "\n");
    join_regex()
        .replace_all(&x, |c: &regex::Captures| format!("{}{}", &c[1], &c[2]))
        .into_owned()
}
pub fn tokens(x: &str, lang: &str) -> Vec<String> {
    exact(&preprocess(x))
        .iter()
        .map(|(v, _, _)| key(v, lang))
        .collect()
}
fn spaced(x: &str) -> usize {
    let c: Vec<char> = x.chars().collect();
    let mut i = 0;
    let mut count = 0;
    while i < c.len() {
        if !letter(c[i]) || (i > 0 && letter(c[i - 1])) {
            i += 1;
            continue;
        }
        let mut positions = vec![i];
        let mut q = i;
        loop {
            let mut next = q + 1;
            if next >= c.len()
                || !(c[next].is_whitespace() || matches!(c[next], '\u{1c}'..='\u{1f}'))
            {
                break;
            }
            while next < c.len()
                && (c[next].is_whitespace() || matches!(c[next], '\u{1c}'..='\u{1f}'))
            {
                next += 1
            }
            if next >= c.len() || !letter(c[next]) {
                break;
            }
            positions.push(next);
            q = next;
        }
        let end = positions
            .iter()
            .enumerate()
            .rev()
            .find(|(j, p)| *j >= 2 && (**p + 1 == c.len() || !letter(c[**p + 1])))
            .map(|(_, p)| *p);
        if let Some(end) = end {
            count += 1;
            i = end + 1
        } else {
            i += 1
        }
    }
    count
}
pub fn base_key(x: &str) -> String {
    base(x)
}
pub fn ru_key(x: &str) -> String {
    key(x, "ru")
}
pub fn quality_profile(x: &str, lang: &str) -> V {
    let raw = exact(x);
    let one = raw
        .iter()
        .filter(|(x, _, _)| base(x).chars().count() == 1)
        .count();
    let mixed = raw.iter().filter(|(x, _, _)| !clean(x, lang)).count();
    let content = raw
        .iter()
        .filter(|(x, _, _)| key(x, lang).chars().count() >= 3 && clean(x, lang))
        .count();
    let share = if raw.is_empty() {
        1.
    } else {
        one as f64 / raw.len() as f64
    };
    json!({"exact_token_count":raw.len(),"one_letter_token_count":one,"one_letter_share":share,"spaced_letter_run_count":spaced(x),"line_join_candidate_count":join_regex().find_iter(&x.replace("\r\n","\n")).count(),"mixed_script_token_count":mixed,"content_token_count":content,"positive_evidence_eligible":!raw.is_empty()&&share<=0.25&&content>0&&mixed as f64/raw.len() as f64<=0.1})
}
fn stop(lang: &str, keyword: bool) -> Set<String> {
    let de = "aber alle allem allen aller alles als also am an auch auf aus bei bin bis bist da damit dann das dass dein deine dem den denn der des die dies diese doch dort du durch ein eine einem einen einer eines er es etwas für gegen gehabt ganz hat haben hier ich im in ist ja jede jedem jeden jeder jedes kann kein keine man mein meine mich mit muß nach nicht noch nun nur ob oder ohne sein seine sich sie sind so über um und uns unser unter vom von vor war waren was weil wenn werde werden wie wieder wir wird wo zu zum zur";
    let ru = "а без бы был была были было быть в во вот все всего всех вы где да для до его ее если есть еще же за и из или им их к как ко когда кто ли мне мой моя мы на над не него нее нет ни но ну о об он она они оно от по под при про с со так также там те тем то того тоже тот ты у уж уже хотя чем что чтобы эта эти это я въ къ съ онъ она они его ея ею ему ихъ мой моя мое моею мы ты вы не былъ была было были бы иль надъ подъ при чемъ что-бы";
    let extra = if !keyword {
        ""
    } else if lang == "de" {
        "ihr mir euch ihm dir wer dich ihn ihnen sei schon soll muss dieser diese diess jene selber"
    } else {
        "меня себя мои тебя себе вас вам них свою этого ибо только чтоб теперь даже тогда должен своей много слишком больше более"
    };
    format!("{} {extra}", if lang == "de" { de } else { ru })
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}
pub(crate) fn round(x: f64) -> i64 {
    x.round_ties_even() as i64
}
fn million(x: f64) -> i64 {
    round(x.clamp(-1., 1.) * 1_000_000.)
}
#[derive(Clone)]
struct Unit {
    part: usize,
    id: String,
    reading: String,
    tokens: Vec<String>,
}
#[derive(Clone)]
struct Parallel {
    alignment: String,
    part: usize,
    reading: String,
    status: String,
    shape: String,
    strict: bool,
    de: Vec<String>,
    ru: Vec<String>,
    dq: V,
    rq: V,
    universe: bool,
    positive: bool,
}
fn anchor(
    root: &ResearchExecution,
    side: &V,
    reference: &str,
    cache: &mut Map<String, String>,
) -> R<String> {
    root.check()?;
    let a = arr(&side["anchors"])?
        .iter()
        .find(|a| a["anchor_ref"] == reference)
        .ok_or("missing anchor")?;
    let r = s(&a["text_layer_ref"])?;
    if !cache.contains_key(r) {
        if root
            .source_file(r, 64 * 1024 * 1024)?
            .metadata()
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777
            != 0o600
        {
            return Err(format!("private text layer is not 0600: {r}"));
        }
        let text = String::from_utf8(read(root, r)?).map_err(|e| e.to_string())?;
        if h(&text) != s(&a["text_layer_sha256"])? {
            return Err(format!("private text layer drift: {r}"));
        }
        cache.insert(r.into(), text);
    }
    let text: String = cache[r]
        .chars()
        .skip(n(&a["selector"]["start"]))
        .take(n(&a["selector"]["end"]) - n(&a["selector"]["start"]))
        .collect();
    root.tick(text.len() as u64)?;
    if h(&text) != s(&a["exact_sha256"])? {
        return Err(format!("anchor return mismatch: {reference}"));
    }
    Ok(text)
}
fn parallel(root: &ResearchExecution) -> R<(Vec<Parallel>, Vec<Unit>, Vec<String>)> {
    root.check()?;
    let spine: Map<String, V> = loadl(root, &path(&format!("{ALIGN}/alignment-spine.v1.jsonl")))?
        .into_iter()
        .map(|x| (x["alignment_id"].as_str().unwrap_or("").into(), x))
        .collect();
    let mut cache = Map::new();
    let mut rows = vec![];
    let mut units = vec![];
    for part in 1..=4 {
        root.tick(1)?;
        let p = load(
            root,
            &path(&format!(
                "{ALIGN}/part-{part}.translation-alignment-packet.v1.json"
            )),
        )?;
        for a in arr(&p["alignments"])? {
            root.tick(1)?;
            let de: Vec<String> = arr(&a["ordered_source_anchor_refs"])?
                .iter()
                .map(|x| anchor(root, &p["source_side"], s(x)?, &mut cache))
                .collect::<R<_>>()?;
            let ru: Vec<String> = arr(&a["ordered_target_anchor_refs"])?
                .iter()
                .map(|x| anchor(root, &p["target_side"], s(x)?, &mut cache))
                .collect::<R<_>>()?;
            let dq = quality_profile(&de.join("\n"), "de");
            let rq = quality_profile(&ru.join("\n"), "ru");
            let positive = de
                .iter()
                .all(|x| b(&quality_profile(x, "de")["positive_evidence_eligible"]))
                && ru
                    .iter()
                    .all(|x| b(&quality_profile(x, "ru")["positive_evidence_eligible"]));
            let id = s(&a["alignment_id"])?;
            let row = Parallel {
                alignment: id.into(),
                part,
                reading: format!(
                    "p{part}.r{}",
                    n(
                        &spine.get(id).ok_or("missing alignment spine")?["reading_ordinal_within_part"]
                    )
                ),
                status: s(&a["status"])?.into(),
                shape: s(&a["correspondence_shape"])?.into(),
                strict: a["status"] == "proposed"
                    && a["correspondence_shape"] == "one_to_one"
                    && s(&a["status_reason"])?.contains("agrees across"),
                de: tokens(&de.join("\n"), "de"),
                ru: tokens(&ru.join("\n"), "ru"),
                universe: dq["one_letter_share"].as_f64().unwrap_or(1.) <= 0.5
                    && rq["one_letter_share"].as_f64().unwrap_or(1.) <= 0.5
                    && n(&dq["content_token_count"]) > 0
                    && n(&rq["content_token_count"]) > 0,
                dq,
                rq,
                positive,
            };
            units.push(Unit {
                part,
                id: id.into(),
                reading: String::new(),
                tokens: row.de.clone(),
            });
            rows.push(row);
        }
    }
    Ok((rows, units, cache.into_keys().collect()))
}
#[derive(Clone)]
pub struct Occ {
    pub id: String,
    pub unit: String,
    pub reading: String,
    pub part: usize,
    pub role: String,
    pub ordinal: usize,
    pub start: usize,
    pub end: usize,
    pub surface: String,
    pub normalized: String,
    pub analysis: String,
}
fn ru_observations(root: &ResearchExecution) -> R<(Vec<Occ>, Vec<Unit>, V, Vec<Unit>)> {
    root.check()?;
    root.reserve_structural_reads()?;
    let model = crate::antonovsky_structural::reconstruct_from_directory(
        root.root(),
        root.root_directory(),
        root.deadline(),
    )
    .map_err(|e| e.to_string())?;
    let mut charged = 0;
    root.charge_structural(&model, &mut charged)?;
    ru_observations_from_model(root, &model, &mut charged)
}

fn ru_observations_from_model(
    root: &ResearchExecution,
    model: &crate::antonovsky_structural::Model,
    charged: &mut u64,
) -> R<(Vec<Occ>, Vec<Unit>, V, Vec<Unit>)> {
    root.check()?;
    root.charge_structural(model, charged)?;
    let bound = loadl(
        root,
        &path(&format!("{RU_TECH}/logical-row-spine.v2.jsonl")),
    )?;
    if bound.len() != model.rows.len() {
        return Err("Russian logical-row parity drift".into());
    }
    let mut occurrences = vec![];
    let mut units = vec![];
    let mut texts = Map::new();
    let mut roles: Map<String, usize> = Map::new();
    let included = Set::from(["reading_unit_heading", "cycle_heading", "prose", "verse"]);
    for (raw, bd) in model.rows.iter().zip(&bound) {
        root.tick(1)?;
        if h(&raw.text) != s(&bd["exact_sha256"])? {
            return Err("Russian row text return mismatch".into());
        }
        texts.insert(raw.reference.clone(), raw.text.clone());
        let role = raw.role.clone().unwrap_or_default();
        *roles.entry(role.clone()).or_default() += 1;
        if !included.contains(role.as_str()) {
            continue;
        }
        let Some(reading) = raw.reading.as_ref().filter(|x| !x.is_empty()) else {
            continue;
        };
        let part = reading_part(reading)?;
        let id = s(&bd["logical_row_unit_id"])?;
        units.push(Unit {
            part,
            reading: reading.clone(),
            id: id.into(),
            tokens: tokens(&raw.text, "ru"),
        });
        for (ordinal, (surface, start, end)) in exact(&raw.text).into_iter().enumerate() {
            root.tick(1)?;
            occurrences.push(Occ {
                id: format!(
                    "tos.occurrence.zarathustra-ru-ant1911.sid-{}",
                    &h(&format!("{id}\n{start}\n{end}\n{}", h(&surface)))[..32]
                ),
                unit: id.into(),
                reading: reading.clone(),
                part,
                role: role.clone(),
                ordinal: ordinal + 1,
                start,
                end,
                normalized: base(&surface),
                analysis: key(&surface, "ru"),
                surface,
            });
        }
    }
    if occurrences.iter().map(|o| &o.id).collect::<Set<_>>().len() != occurrences.len() {
        return Err("Russian occurrence identity collision".into());
    }
    let mut phrases = vec![];
    for (name, raws, idkey) in [
        (
            "paragraph-spine.v2.jsonl",
            &model.paragraphs,
            "paragraph_unit_id",
        ),
        (
            "verse-line-spine.v2.jsonl",
            &model.verse_lines,
            "verse_line_unit_id",
        ),
    ] {
        let bound = loadl(root, &path(&format!("{RU_TECH}/{name}")))?;
        if bound.len() != raws.len() {
            return Err(format!("Russian {name} parity drift"));
        }
        for (raw, bd) in raws.iter().zip(bound) {
            root.tick(1)?;
            let reading = s(&raw["reading_ref"])?;
            let text = if idkey == "paragraph_unit_id" {
                arr(&raw["row_refs"])?
                    .iter()
                    .map(|r| {
                        texts
                            .get(s(r)?)
                            .cloned()
                            .ok_or("missing Russian row".into())
                    })
                    .collect::<R<Vec<_>>>()?
                    .join("\n")
            } else {
                texts
                    .get(s(&raw["row_ref"])?)
                    .cloned()
                    .ok_or("missing verse row")?
            };
            phrases.push(Unit {
                part: reading_part(reading)?,
                id: s(&bd[idkey])?.into(),
                reading: reading.into(),
                tokens: tokens(&text, "ru"),
            });
        }
    }
    let role_ids: Map<&str, &str> = bound
        .iter()
        .map(|x| Ok((s(&x["logical_row_unit_id"])?, s(&x["technical_role"])?)))
        .collect::<R<_>>()?;
    phrases.extend(
        units
            .iter()
            .filter(|x| {
                matches!(
                    role_ids.get(x.id.as_str()),
                    Some(&"reading_unit_heading") | Some(&"cycle_heading")
                )
            })
            .cloned(),
    );
    let meta = json!({"role_counts":roles,"included_unit_count":units.len()});
    root.check()?;
    root.charge_structural(model, charged)?;
    Ok((occurrences, units, meta, phrases))
}
fn reading_part(r: &str) -> R<usize> {
    r.split('.')
        .next()
        .and_then(|x| x.split('_').nth(1))
        .and_then(|x| x.parse().ok())
        .ok_or_else(|| format!("invalid reading: {r}"))
}
type Counts = Map<String, Map<usize, usize>>;
type Ranges = Map<String, Set<String>>;
type Surfaces = Map<String, Map<String, usize>>;
fn german(root: &ResearchExecution, plan: &V) -> R<(V, Counts, Ranges, Surfaces)> {
    root.check()?;
    let p = path(
        "gold-sets/foundation-pilot-v1/local-content/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.sqlite3",
    );
    let input = &plan["inputs"]["german_lexical_projection"];
    let projection_ref = s(&input["ref"])?;
    let projection_bytes = read(root, projection_ref)?;
    if hash(&projection_bytes) != s(&input["sha256"])? {
        return Err("input drift: german_lexical_projection".into());
    }
    let projection: V =
        serde_json::from_slice(&projection_bytes).map_err(|e| format!("{projection_ref}: {e}"))?;
    root.check()?;
    let expected_digest = s(&projection["local_projection_receipt"]["database_sha256"])?;
    let mut held_db = root.source_file(&p, 128 * 1024 * 1024)?;
    let metadata = held_db.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o7777 != 0o600 {
        return Err("German lexical database requires private regular mode 0600".into());
    }
    if root.hash_file(&mut held_db, 128 * 1024 * 1024)? != expected_digest {
        return Err("German lexical database digest drift".into());
    }
    let db = root.open_sqlite_readonly(&held_db)?;
    let deadline = root.deadline();
    db.progress_handler(10_000, Some(move || std::time::Instant::now() >= deadline));
    // This grouped readonly scan may sort. The exact-FD VFS deliberately
    // refuses filesystem temp objects; keep its sorter in caller-owned RAM.
    // The enclosing finite execution envelope owns the full RAM limit.
    root.check()?;
    db.execute_batch("PRAGMA temp_store=MEMORY")
        .map_err(|e| format!("parallel German readonly sorter policy: {e}"))?;
    let temp_store: i64 = db
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .map_err(|e| format!("parallel German readonly sorter policy check: {e}"))?;
    if temp_store != 2 {
        return Err("parallel German readonly sorter requires memory temp storage".into());
    }
    root.check()?;
    let mut stmt=db.prepare("SELECT o.normalized_form,o.exact_form,s.part_order,o.section_resource_id,count(*) FROM occurrences o JOIN source_items s USING(item_ref) GROUP BY o.normalized_form,o.exact_form,s.part_order,o.section_resource_id").map_err(|e|format!("parallel German grouped scan prepare: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, usize>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, usize>(4)?,
            ))
        })
        .map_err(|e| format!("parallel German grouped scan start: {e}"))?;
    let mut counts: Counts = Map::new();
    let mut ranges: Ranges = Map::new();
    let mut surfaces: Surfaces = Map::new();
    for row in rows {
        root.tick(1)?;
        let (k, surface, part, section, count) =
            row.map_err(|e| format!("parallel German grouped scan row: {e}"))?;
        *counts
            .entry(k.clone())
            .or_default()
            .entry(part)
            .or_default() += count;
        *surfaces
            .entry(k.clone())
            .or_default()
            .entry(surface)
            .or_default() += count;
        if let Some(section) = section.filter(|s| !s.is_empty()) {
            ranges
                .entry(k)
                .or_default()
                .insert(format!("{part}:{section}"));
        }
    }
    drop(stmt);
    db.close().map_err(|(_retained, error)| error.to_string())?;
    root.verify_file_unchanged(&held_db, &metadata)?;
    Ok((
        json!({"token_count":counts.values().map(|x|x.values().sum::<usize>()).sum::<usize>(),"form_count":counts.len()}),
        counts,
        ranges,
        surfaces,
    ))
}
fn ru_statistics(
    root: &ResearchExecution,
    occs: &[Occ],
) -> R<(Counts, Ranges, Surfaces, Map<String, usize>)> {
    root.check()?;
    let mut c: Counts = Map::new();
    let mut r: Ranges = Map::new();
    let mut v: Surfaces = Map::new();
    let mut folds = Map::new();
    for o in occs {
        root.tick(1)?;
        *c.entry(o.normalized.clone())
            .or_default()
            .entry(o.part)
            .or_default() += 1;
        r.entry(o.normalized.clone())
            .or_default()
            .insert(o.reading.clone());
        *v.entry(o.normalized.clone())
            .or_default()
            .entry(o.surface.clone())
            .or_default() += 1;
        if o.normalized != o.analysis {
            *folds.entry(o.normalized.clone()).or_default() += 1;
        }
    }
    Ok((c, r, v, folds))
}
fn unit_statistics(root: &ResearchExecution, units: &[Unit], lang: &str) -> R<(Counts, Ranges)> {
    root.check()?;
    let mut c: Counts = Map::new();
    let mut r: Ranges = Map::new();
    for u in units {
        root.tick(1)?;
        for k in &u.tokens {
            root.tick(1)?;
            if k.chars().count() < 3 || !clean(k, lang) {
                continue;
            }
            *c.entry(k.clone()).or_default().entry(u.part).or_default() += 1;
            r.entry(k.clone())
                .or_default()
                .insert(if u.reading.is_empty() {
                    u.id.clone()
                } else {
                    u.reading.clone()
                });
        }
    }
    Ok((c, r))
}
fn keywords(
    root: &ResearchExecution,
    lang: &str,
    counts: &Counts,
    ranges: &Ranges,
) -> R<(Vec<V>, Vec<V>)> {
    root.check()?;
    let stop = stop(lang, true);
    let mut totals: Map<usize, usize> = Map::new();
    for parts in counts.values() {
        root.tick(1)?;
        for (p, c) in parts {
            root.tick(1)?;
            *totals.entry(*p).or_default() += c;
        }
    }
    let all: usize = totals.values().sum();
    let mut tracked = vec![];
    let mut private = vec![];
    for (k, parts) in counts {
        root.tick(1)?;
        let total: usize = parts.values().sum();
        if total < 5 || k.chars().count() < 3 || stop.contains(k) {
            continue;
        }
        let ent = if parts.len() < 2 {
            0.
        } else {
            -parts
                .values()
                .map(|n| {
                    let p = *n as f64 / total as f64;
                    p * p.ln()
                })
                .sum::<f64>()
                / 4_f64.ln()
        };
        let prominence = (total as f64).ln_1p() * (0.35 + 0.65 * ent);
        let row = json!({"schema_version":"tos_lexical_keyword_candidate_v1","language":lang,"form_key_sha256":h(k),"occurrence_count":total,"part_range":parts.len(),"range_count":ranges.get(k).map_or(0,Set::len),"part_entropy_millionths":million(ent),"global_recurrence_prominence_millionths":round(prominence*1_000_000.),"part_occurrence_counts":parts.iter().map(|(p,c)|json!({"part_order":p,"occurrence_count":c})).collect::<Vec<_>>(),"status":"proposed","semantic_sufficiency":false});
        tracked.push(row.clone());
        let mut row = row;
        row["analysis_key"] = json!(k);
        row["part_counts"] = json!(parts);
        for p in 1..=4 {
            root.tick(1)?;
            let c = *parts.get(&p).unwrap_or(&0);
            if c < 3 {
                continue;
            }
            let tp = *totals.get(&p).unwrap_or(&0);
            let rest_c = total - c;
            let rest_n = all - tp;
            let odds = ((c as f64 + 0.5) / (tp as f64 - c as f64 + 0.5)).ln()
                - ((rest_c as f64 + 0.5) / (rest_n as f64 - rest_c as f64 + 0.5)).ln();
            if odds > 0. {
                if row.get("part_salience").is_none() {
                    row["part_salience"] = json!({});
                }
                row["part_salience"][p.to_string()] = json!(
                    format!("{:.6}", odds * (c as f64).ln_1p())
                        .parse::<f64>()
                        .map_err(|e| e.to_string())
                        .unwrap()
                );
            }
        }
        private.push(row);
    }
    tracked.sort_by(|a, b| {
        n(&b["global_recurrence_prominence_millionths"])
            .cmp(&n(&a["global_recurrence_prominence_millionths"]))
            .then_with(|| {
                a["form_key_sha256"]
                    .as_str()
                    .cmp(&b["form_key_sha256"].as_str())
            })
    });
    private.sort_by(|a, b| {
        n(&b["global_recurrence_prominence_millionths"])
            .cmp(&n(&a["global_recurrence_prominence_millionths"]))
            .then_with(|| a["analysis_key"].as_str().cmp(&b["analysis_key"].as_str()))
    });
    Ok((tracked, private))
}
fn phrases(root: &ResearchExecution, lang: &str, units: &[Unit]) -> R<(Vec<V>, Vec<V>)> {
    root.check()?;
    let stop = stop(lang, false);
    let mut grams: Map<Vec<String>, (usize, Set<usize>, Set<String>)> = Map::new();
    for u in units {
        root.tick(1)?;
        for len in [2, 3] {
            root.tick(1)?;
            for gram in u.tokens.windows(len) {
                root.tick(1)?;
                if gram
                    .iter()
                    .any(|x| x.chars().count() < 2 || !clean(x, lang))
                    || gram.iter().all(|x| stop.contains(x))
                {
                    continue;
                }
                let entry = grams.entry(gram.to_vec()).or_default();
                entry.0 += 1;
                entry.1.insert(u.part);
                entry.2.insert(u.id.clone());
            }
        }
    }
    let mut tracked = vec![];
    let mut private = vec![];
    for (gram, (count, parts, units)) in grams {
        root.tick(1)?;
        if count < 3 {
            continue;
        }
        let row = json!({"schema_version":"tos_lexical_repeated_sequence_candidate_v1","language":lang,"sequence_sha256":h(&gram.join("\n")),"token_length":gram.len(),"occurrence_count":count,"unit_range":units.len(),"part_range":parts.len(),"status":if count>=5&&units.len()>=3{"proposed"}else{"deferred"},"semantic_sufficiency":false});
        tracked.push(row.clone());
        let mut row = row;
        row["sequence"] = json!(gram);
        private.push(row);
    }
    tracked.sort_by(|a, b| {
        n(&b["occurrence_count"])
            .cmp(&n(&a["occurrence_count"]))
            .then_with(|| {
                a["sequence_sha256"]
                    .as_str()
                    .cmp(&b["sequence_sha256"].as_str())
            })
    });
    private.sort_by(|a, b| {
        n(&b["occurrence_count"])
            .cmp(&n(&a["occurrence_count"]))
            .then_with(|| {
                a["sequence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap())
                    .cmp(
                        b["sequence"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|x| x.as_str().unwrap()),
                    )
            })
    });
    Ok((tracked, private))
}
pub fn build_ru_observations(root: &ResearchExecution) -> R<Vec<V>> {
    root.check()?;
    let (occs, _, _, _) = ru_observations(root)?;
    render_ru_observations(root, &occs)
}

pub(crate) fn build_ru_observations_from_model(
    root: &ResearchExecution,
    model: &crate::antonovsky_structural::Model,
    charged: &mut u64,
) -> R<Vec<V>> {
    let (occs, _, _, _) = ru_observations_from_model(root, model, charged)?;
    render_ru_observations(root, &occs)
}

fn render_ru_observations(root: &ResearchExecution, occs: &[Occ]) -> R<Vec<V>> {
    occs.iter().map(|o|{root.tick(1)?; Ok(json!({"occurrence_id":o.id,"unit_id":o.unit,"reading":o.reading,"part":o.part,"role":o.role,"ordinal":o.ordinal,"start":o.start,"end":o.end,"surface":o.surface,"exact_sha256":h(&o.surface),"normalized":o.normalized,"normalized_sha256":h(&o.normalized),"analysis_key":o.analysis,"analysis_key_sha256":h(&o.analysis)}))}).collect()
}
type Pair = (String, String);
#[derive(Default)]
struct Evidence {
    universe: Vec<String>,
    positive: Vec<String>,
    strict: usize,
    risk: usize,
    readings: Map<String, usize>,
    shapes: Map<String, usize>,
}
fn content(tokens: &[String], lang: &str) -> Set<String> {
    let stop = stop(lang, false);
    tokens
        .iter()
        .filter(|x| x.chars().count() >= 3 && clean(x, lang) && !stop.contains(*x))
        .cloned()
        .collect()
}
fn associations(
    root: &ResearchExecution,
    units: &[Parallel],
    issuance: Option<&Map<String, String>>,
) -> R<(Vec<V>, Vec<V>, Vec<String>)> {
    root.check()?;
    let mut pairs: Map<Pair, Evidence> = Map::new();
    let mut de_df: Map<String, usize> = Map::new();
    let mut ru_df: Map<String, usize> = Map::new();
    let mut positives = 0;
    for row in units {
        root.tick(1)?;
        let de = content(&row.de, "de");
        let ru = content(&row.ru, "ru");
        let positive = row.status == "proposed" && row.positive;
        let universe = row.status == "proposed" && row.universe;
        let risk = row.status != "proposed";
        if positive {
            positives += 1;
            for k in &de {
                root.tick(1)?;
                *de_df.entry(k.clone()).or_default() += 1
            }
            for k in &ru {
                root.tick(1)?;
                *ru_df.entry(k.clone()).or_default() += 1
            }
        }
        if !positive && !universe && !risk {
            continue;
        }
        for a in &de {
            root.tick(1)?;
            for b in &ru {
                root.tick(1)?;
                let entry = pairs.entry((a.clone(), b.clone())).or_default();
                if universe {
                    entry.universe.push(row.alignment.clone())
                }
                if positive {
                    entry.positive.push(row.alignment.clone());
                    *entry.readings.entry(row.reading.clone()).or_default() += 1;
                    *entry.shapes.entry(row.shape.clone()).or_default() += 1;
                    if row.strict {
                        entry.strict += 1
                    }
                }
                if risk {
                    entry.risk += 1
                }
            }
        }
    }
    let mut raw: Vec<(String, String, V, f64)> = vec![];
    for ((de, ru), e) in &pairs {
        root.tick(1)?;
        if e.universe.len() < 4 {
            continue;
        }
        let support = e.positive.len();
        let df_de = *de_df.get(de).unwrap_or(&0);
        let df_ru = *ru_df.get(ru).unwrap_or(&0);
        let (pdr, prd, dice, pmi, npmi, maxshare) = if support > 0 && df_de > 0 && df_ru > 0 {
            let pmi = ((support * positives) as f64 / (df_de * df_ru) as f64).ln();
            let den = -(support as f64 / positives as f64).ln();
            (
                support as f64 / df_de as f64,
                support as f64 / df_ru as f64,
                2. * support as f64 / (df_de + df_ru) as f64,
                pmi / 2_f64.ln(),
                if den == 0. { 1. } else { pmi / den },
                *e.readings.values().max().unwrap() as f64 / support as f64,
            )
        } else {
            (0., 0., 0., -1000., -1., 1.)
        };
        let riskshare = if support + e.risk > 0 {
            e.risk as f64 / (support + e.risk) as f64
        } else {
            1.
        };
        let m = json!({"identity_universe_support":e.universe.len(),"support":support,"source_unit_frequency":df_de,"target_unit_frequency":df_ru,"strict_support":e.strict,"risk_support":e.risk,"source_to_target_precision_millionths":million(pdr),"target_to_source_precision_millionths":million(prd),"dice_millionths":million(dice),"npmi_millionths":million(npmi),"pmi_millibits":round(pmi*1000.),"risk_share_millionths":million(riskshare),"reading_range":e.readings.len(),"maximum_single_reading_share_millionths":million(maxshare),"correspondence_shapes":e.shapes});
        let score =
            -(million(dice) as f64) * (support as f64).ln_1p() * (round(pmi * 1000.).max(0) as f64);
        raw.push((de.clone(), ru.clone(), m, score));
    }
    let mut by_de: Map<String, Vec<usize>> = Map::new();
    let mut by_ru: Map<String, Vec<usize>> = Map::new();
    for (i, (de, ru, _, _)) in raw.iter().enumerate() {
        root.tick(1)?;
        by_de.entry(de.clone()).or_default().push(i);
        by_ru.entry(ru.clone()).or_default().push(i);
    }
    let cmp = |a: &usize, b: &usize| {
        let a = &raw[*a];
        let b = &raw[*b];
        a.3.total_cmp(&b.3)
            .then_with(|| n(&b.2["support"]).cmp(&n(&a.2["support"])))
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    };
    let mut ranks_de = Map::new();
    let mut ranks_ru = Map::new();
    for (group, ranks) in [(&mut by_de, &mut ranks_de), (&mut by_ru, &mut ranks_ru)] {
        root.tick(1)?;
        for values in group.values_mut() {
            root.tick(1)?;
            values.sort_by(cmp);
            for (rank, i) in values.iter().enumerate() {
                root.tick(1)?;
                ranks.insert(*i, rank + 1);
            }
        }
    }
    for (i, (_, _, m, _)) in raw.iter_mut().enumerate() {
        root.tick(1)?;
        let dr = ranks_de[&i];
        let rr = ranks_ru[&i];
        m["source_candidate_rank"] = json!(dr);
        m["target_candidate_rank"] = json!(rr);
        let proposed = n(&m["support"]) >= 8
            && n(&m["dice_millionths"]) >= 200_000
            && m["pmi_millibits"].as_i64().unwrap_or(0) >= 2000
            && dr <= 3
            && rr <= 3
            && n(&m["strict_support"]) >= 4
            && n(&m["risk_share_millionths"]) <= 200_000
            && n(&m["maximum_single_reading_share_millionths"]) <= 500_000
            && m["correspondence_shapes"].get("many_to_many").is_none();
        let ambiguous = n(&m["dice_millionths"]) >= 100_000
            && m["pmi_millibits"].as_i64().unwrap_or(0) >= 1000
            && dr <= 5
            && rr <= 5;
        m["status"] = json!(if proposed {
            "proposed"
        } else if ambiguous {
            "ambiguous"
        } else {
            "deferred"
        });
    }
    raw.sort_by(|a, b| {
        n(&b.2["support"])
            .cmp(&n(&a.2["support"]))
            .then_with(|| {
                n(&b.2["identity_universe_support"]).cmp(&n(&a.2["identity_universe_support"]))
            })
            .then_with(|| n(&b.2["dice_millionths"]).cmp(&n(&a.2["dice_millionths"])))
            .then_with(|| h(&a.0).cmp(&h(&b.0)))
            .then_with(|| h(&a.1).cmp(&h(&b.1)))
    });
    let mut tracked = vec![];
    let mut private = vec![];
    let mut bindings = vec![];
    for (de, ru, m, _) in raw {
        root.tick(1)?;
        let binding = format!("{}|{}", h(&de), h(&ru));
        let id = match issuance {
            Some(i) => json!(i.get(&binding).ok_or("candidate identity binding drift")?),
            None => V::Null,
        };
        bindings.push(binding);
        let e = &pairs[&(de.clone(), ru.clone())];
        let mut row = json!({"schema_version":"tos_translation_surface_association_candidate_v1","candidate_id":id,"source_language":"de","target_language":"ru","source_form_key_sha256":h(&de),"target_form_key_sha256":h(&ru),"supporting_alignment_refs":e.positive,"identity_universe_alignment_refs":e.universe,"candidate_kind":"translation_surface_association_candidate","semantic_probe_posture":"proposal_not_lexical_equivalence_sign_or_concept","accepted":false,"review_refs":[],"graph_effect":false});
        row.as_object_mut()
            .unwrap()
            .extend(m.as_object().unwrap().clone());
        tracked.push(row.clone());
        row["source_form"] = json!(de);
        row["target_form"] = json!(ru);
        private.push(row);
    }
    Ok((tracked, private, bindings))
}
fn ru_db(root: &ResearchExecution, occs: &[Occ], plan_digest: &str) -> R<Vec<u8>> {
    root.check()?;
    const MIB: u64 = 1024 * 1024;
    let mut workspace = root.sqlite_scope(tos_source_store::PinnedSqliteAuxLimits {
        main_logical_bytes: 80 * MIB,
        main_allocated_bytes: 80 * MIB,
        temp_db_logical_bytes: 80 * MIB,
        temp_db_allocated_bytes: 80 * MIB,
        main_journal_logical_bytes: 84 * MIB,
        main_journal_allocated_bytes: 84 * MIB,
        temp_journal_logical_bytes: 4 * MIB,
        temp_journal_allocated_bytes: 4 * MIB,
        other_aux_aggregate_logical_bytes: 8 * MIB,
        other_aux_aggregate_allocated_bytes: 8 * MIB,
        max_live_aux: 8,
    })?;
    let result: R<()> = (|| {
        let mut db = workspace
            .scope_mut()
            .open_connection()
            .map_err(|e| format!("parallel Russian database open: {e}"))?;
        let deadline = root.deadline();
        db.progress_handler(10_000, Some(move || std::time::Instant::now() >= deadline));
        root.check()?;
        // Use a 96 MiB suggested pager target during random indexed inserts.
        // The bounded main database remains 80 MiB; this is not a total heap
        // cap, and the shared read meter and outer memory envelope still apply.
        db.execute_batch("PRAGMA main.cache_size=-98304")
            .map_err(|e| format!("parallel Russian cache policy: {e}"))?;
        let cache_kib: i64 = db
            .query_row("PRAGMA main.cache_size", [], |row| row.get(0))
            .map_err(|e| format!("parallel Russian cache readback: {e}"))?;
        if cache_kib != -98304 {
            return Err("parallel Russian cache policy readback drift".into());
        }
        root.check()?;
        db.execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID; CREATE TABLE occurrences(occurrence_id TEXT PRIMARY KEY,unit_id TEXT NOT NULL,reading_ref TEXT NOT NULL,part_order INTEGER NOT NULL,role TEXT NOT NULL,token_ordinal INTEGER NOT NULL,start_offset INTEGER NOT NULL,end_offset INTEGER NOT NULL,exact_form TEXT NOT NULL,exact_form_sha256 TEXT NOT NULL,normalized_form TEXT NOT NULL,normalized_form_sha256 TEXT NOT NULL,analysis_key TEXT NOT NULL,analysis_key_sha256 TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX occurrence_analysis_idx ON occurrences(analysis_key); CREATE INDEX occurrence_unit_idx ON occurrences(unit_id,token_ordinal); CREATE VIRTUAL TABLE unit_fts USING fts5(unit_id UNINDEXED, exact_text, normalized_text, tokenize='unicode61 remove_diacritics 0');").map_err(|e|format!("parallel Russian database schema: {e}"))?;
        let tx = db
            .transaction()
            .map_err(|e| format!("parallel Russian database transaction: {e}"))?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO occurrences VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
                .map_err(|e| {
                    format!(
                        "parallel Russian occurrence prepare: {e}; sqlite_extended_code={:?}",
                        e.sqlite_error().map(|error| error.extended_code)
                    )
                })?;
            for o in occs {
                root.tick(1)?;
                stmt.execute(params![
                    o.id,
                    o.unit,
                    o.reading,
                    o.part,
                    o.role,
                    o.ordinal,
                    o.start,
                    o.end,
                    o.surface,
                    h(&o.surface),
                    o.normalized,
                    h(&o.normalized),
                    o.analysis,
                    h(&o.analysis)
                ])
                .map_err(|e| {
                    format!(
                        "parallel Russian occurrence insert: {e}; sqlite_extended_code={:?}",
                        e.sqlite_error().map(|error| error.extended_code)
                    )
                })?;
            }
        }
        let mut grouped: Map<&str, Vec<&Occ>> = Map::new();
        for o in occs {
            root.tick(1)?;
            grouped.entry(&o.unit).or_default().push(o);
        }
        for (unit, rows) in grouped {
            root.tick(1)?;
            tx.execute(
                "INSERT INTO unit_fts VALUES(?,?,?)",
                params![
                    unit,
                    rows.iter()
                        .map(|o| o.surface.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    rows.iter()
                        .map(|o| o.analysis.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                ],
            )
            .map_err(|e| {
                format!(
                    "parallel Russian FTS insert: {e}; sqlite_extended_code={:?}",
                    e.sqlite_error().map(|error| error.extended_code)
                )
            })?;
        }
        for(k,v)in[("authority_boundary","Private mechanical occurrence search over the selected source layers, preserving the recorded textual and semantic assessment status.".to_owned()),("plan_sha256",plan_digest.to_owned()),("occurrence_count",occs.len().to_string())]{tx.execute("INSERT INTO metadata VALUES(?,?)",params![k,v]).map_err(|e|format!("parallel Russian database metadata: {e}"))?;}
        tx.commit()
            .map_err(|e| format!("parallel Russian database commit: {e}"))?;
        db.execute_batch("VACUUM")
            .map_err(|e| format!("parallel Russian database vacuum: {e}"))?;
        db.close()
            .map_err(|(_retained, error)| format!("parallel Russian database close: {error}"))?;
        Ok(())
    })();
    workspace.complete(result, 80 * MIB)
}
struct Generated {
    outputs: Map<String, Vec<u8>>,
    private: Map<String, Vec<u8>>,
    bindings: Vec<String>,
    analysis: V,
}
fn issuance(root: &ResearchExecution, bindings: &[String]) -> R<Map<String, String>> {
    root.check()?;
    let data = load(root, &route("identity-issuance.v1.json"))?;
    let rows = arr(&data["identities"])?;
    let mut out = Map::new();
    let mut ids = Set::new();
    for x in rows {
        root.tick(1)?;
        let id = s(&x["id"])?;
        if !ids.insert(id) {
            return Err("candidate identity collision".into());
        }
        out.insert(s(&x["binding"])?.to_owned(), id.to_owned());
    }
    if rows.len() != bindings.len()
        || out.keys().collect::<Set<_>>() != bindings.iter().collect::<Set<_>>()
    {
        return Err("candidate identity binding drift".into());
    }
    Ok(out)
}
fn issue(root: &ResearchExecution, bindings: &[String]) -> R<()> {
    root.check()?;
    let p = route("identity-issuance.v1.json");
    if root.join(&p).exists() {
        return Err("identity issuance exists; refusing remint".into());
    }
    let mut random = fs::File::open("/dev/urandom").map_err(|e| e.to_string())?;
    let mut rows = vec![];
    for binding in bindings {
        root.tick(1)?;
        let mut bytes = [0u8; 16];
        root.read_exact(&mut random, &mut bytes)?;
        let hex: String = bytes.iter().map(|x| format!("{x:02x}")).collect();
        rows.push(
            json!({"binding":binding,"id":format!("tos.lexical-association-candidate.sid-{hex}")}),
        );
    }
    write_exclusive(
        root,
        &p,
        &pretty(
            &json!({"schema_version":"tos_zarathustra_parallel_lexical_candidate_identity_issuance_v1","issuance_id":"tos.identity-issuance.zarathustra-parallel-lexical-candidates-v1","issued_on":"2026-09-02","opaque_identity":true,"binding_is_not_identity":true,"candidate_count":rows.len(),"identities":rows}),
        )?,
        0o644,
    )
}
fn generate(
    root: &ResearchExecution,
    with_issuance: bool,
    selected: &SelectedPlan,
) -> R<Generated> {
    root.check()?;
    let plan = &selected.value;
    for (label, r) in plan["inputs"]
        .as_object()
        .ok_or("plan inputs object required")?
    {
        root.tick(1)?;
        let raw = read(root, s(&r["ref"])?).map_err(|_| format!("input drift: {label}"))?;
        if hash(&raw) != s(&r["sha256"])? {
            return Err(format!("input drift: {label}"));
        }
    }
    let (parallel, mut de_units, private_layers) = parallel(root)?;
    let (occ, ru_units, ru_meta, ru_phrase) = ru_observations(root)?;
    let (de_meta, de_counts, de_ranges, de_surfaces) = german(root, &plan)?;
    let (ru_exact, ru_ranges, ru_surfaces, folds) = ru_statistics(root, &occ)?;
    let (ru_counts, ru_readings) = unit_statistics(root, &ru_units, "ru")?;
    let (de_kw, de_kw_private) = keywords(root, "de", &de_counts, &de_ranges)?;
    let (ru_kw, ru_kw_private) = keywords(root, "ru", &ru_counts, &ru_readings)?;
    let part4 = load(
        root,
        &path(&format!("{DE_TECH}/part-4.source-text-unit.v1.json")),
    )?;
    let layer = String::from_utf8(read(root, s(&part4["source_layer"]["text_layer_ref"])?)?)
        .map_err(|e| e.to_string())?;
    for u in arr(&part4["units"])? {
        root.tick(1)?;
        if u["unit_kind"] != "verse_line" {
            continue;
        }
        let a = arr(&part4["anchors"])?
            .iter()
            .find(|a| a["anchor_ref"] == u["ordered_anchor_refs"][0])
            .ok_or("missing German verse anchor")?;
        let text: String = layer
            .chars()
            .skip(n(&a["selector"]["start"]))
            .take(n(&a["selector"]["end"]) - n(&a["selector"]["start"]))
            .collect();
        root.tick(text.len() as u64)?;
        if h(&text) != s(&a["exact_sha256"])? {
            return Err("German verse-line anchor drift".into());
        }
        de_units.push(Unit {
            part: 4,
            id: s(&u["unit_id"])?.into(),
            reading: String::new(),
            tokens: tokens(&text, "de"),
        });
    }
    let (de_ph, de_ph_private) = phrases(root, "de", &de_units)?;
    let (ru_ph, ru_ph_private) = phrases(root, "ru", &ru_phrase)?;
    let (_, _, bindings) = associations(root, &parallel, None)?;
    let issuance = if with_issuance || selected.custom {
        Some(issuance(root, &bindings)?)
    } else {
        None
    };
    let (assoc, assoc_private, _) = associations(root, &parallel, issuance.as_ref())?;
    let mut ru_rows:Vec<V>=ru_exact.iter().map(|(key,parts)|json!({"schema_version":"tos_russian_exact_form_recurrence_projection_v1","normalized_form_sha256":h(key),"analysis_form_sha256":h(&self::key(key,"ru")),"occurrence_count":parts.values().sum::<usize>(),"part_range":parts.len(),"reading_range":ru_ranges.get(key).map_or(0,Set::len),"part_counts":parts.iter().map(|(p,c)|json!({"part_order":p,"occurrence_count":c})).collect::<Vec<_>>(),"orthographic_folded_occurrence_count":folds.get(key).unwrap_or(&0),"source_string_tracked":false,"semantic_sufficiency":false})).collect();
    ru_rows.sort_by(|a, b| {
        a["normalized_form_sha256"]
            .as_str()
            .cmp(&b["normalized_form_sha256"].as_str())
    });
    let mut status_counts: Map<String, usize> = Map::new();
    for a in &assoc {
        root.tick(1)?;
        *status_counts.entry(s(&a["status"])?.into()).or_default() += 1
    }
    let mut parallel_status: Map<String, usize> = Map::new();
    for p in &parallel {
        root.tick(1)?;
        *parallel_status.entry(p.status.clone()).or_default() += 1
    }
    let quality_deferred = parallel.iter().filter(|p| !p.positive).count();
    let summary = json!({"schema_version":"tos_zarathustra_parallel_lexical_candidate_summary_v1","status":"completed-mechanical-candidate-observation-no-promotion","parts":4,"german_token_occurrences":de_meta["token_count"],"german_normalized_form_count":de_meta["form_count"],"russian_exact_token_occurrences":occ.len(),"russian_analysis_form_count":ru_counts.len(),"russian_included_unit_count":ru_meta["included_unit_count"],"keyword_candidate_count":de_kw.len()+ru_kw.len(),"keyword_candidates_by_language":{"de":de_kw.len(),"ru":ru_kw.len()},"repeated_sequence_count":de_ph.len()+ru_ph.len(),"repeated_sequences_by_language":{"de":de_ph.len(),"ru":ru_ph.len()},"parallel_alignment_units":parallel.len(),"parallel_status_counts":parallel_status,"quality_deferred_alignment_units":quality_deferred,"association_candidate_count":assoc.len(),"association_status_counts":status_counts,"accepted_candidate_count":0,"review_count":0,"graph_effect":false,"semantic_equivalence_asserted":false});
    let mut quality_census = json!({});
    for lang in ["de", "ru"] {
        root.tick(1)?;
        let mut fields = Map::new();
        for field in [
            "exact_token_count",
            "one_letter_token_count",
            "spaced_letter_run_count",
            "line_join_candidate_count",
            "mixed_script_token_count",
        ] {
            fields.insert(
                field,
                parallel
                    .iter()
                    .map(|p| n(&if lang == "de" { &p.dq } else { &p.rq }[field]))
                    .sum::<usize>(),
            );
        }
        quality_census[lang] = json!(fields);
    }
    let mut all_kw_private = de_kw_private.clone();
    all_kw_private.extend(ru_kw_private.clone());
    let mut all_ph_private = de_ph_private;
    all_ph_private.extend(ru_ph_private);
    let de_keys: Set<&str> = de_kw_private
        .iter()
        .filter_map(|x| x["analysis_key"].as_str())
        .collect();
    let ru_keys: Set<&str> = ru_kw_private
        .iter()
        .filter_map(|x| x["analysis_key"].as_str())
        .collect();
    let mut surface_variants = json!({"de":{},"ru":{}});
    for (lang, surfaces) in [("de", de_surfaces), ("ru", ru_surfaces)] {
        root.tick(1)?;
        for (k, v) in surfaces {
            root.tick(1)?;
            if (lang == "de" && de_keys.contains(k.as_str()))
                || (lang == "ru" && ru_keys.contains(key(&k, "ru").as_str()))
            {
                surface_variants[lang][h(&k)] = json!(v);
            }
        }
    }
    let analysis = json!({"schema_version":"tos_zarathustra_parallel_lexical_private_analysis_v1","source_bearing":true,"mode":"0600","summary":summary,"russian_role_census":ru_meta["role_counts"],"alignment_quality_census":quality_census,"keywords":all_kw_private,"repeated_sequences":all_ph_private,"translation_surface_associations":assoc_private,"surface_variants":surface_variants,"authority_boundary":plan["authority_boundary"]});
    let mut outputs = Map::new();
    outputs.insert(route("russian-recurrence.v1.jsonl"), lines(&ru_rows)?);
    let mut kw = de_kw;
    kw.extend(ru_kw);
    outputs.insert(route("keyword-candidates.v1.jsonl"), lines(&kw)?);
    let mut ph = de_ph;
    ph.extend(ru_ph);
    outputs.insert(route("repeated-sequences.v1.jsonl"), lines(&ph)?);
    outputs.insert(
        route("translation-surface-association-candidates.v1.jsonl"),
        lines(&assoc)?,
    );
    outputs.insert(route("summary.v1.json"), pretty(&summary)?);
    let mut private_outputs = Map::new();
    private_outputs.insert(
        private("antonovsky-1911-lexical-observation-v1.sqlite3"),
        ru_db(root, &occ, &selected.digest)?,
    );
    private_outputs.insert(
        private("parallel-candidate-analysis.v1.json"),
        pretty(&analysis)?,
    );
    let coverage = json!({"schema_version":"tos_zarathustra_parallel_lexical_candidate_coverage_v1","parts_complete":4,"russian_logical_rows_reconstructed":ru_meta["role_counts"].as_object().unwrap().values().map(n).sum::<usize>(),"russian_included_roles":plan["scope"]["russian_included_roles"],"russian_exact_occurrence_count":occ.len(),"russian_occurrence_ids_unique":true,"german_existing_lexical_occurrence_count":de_meta["token_count"],"paragraph_alignment_units_consumed":parallel.len(),"proposed_positive_evidence_units":parallel.iter().filter(|x|x.status=="proposed"&&x.positive).count(),"quality_deferred_alignment_units":quality_deferred,"quality_deferred_proposed_units":parallel.iter().filter(|x|x.status=="proposed"&&!x.positive).count(),"ambiguous_risk_units":parallel.iter().filter(|x|x.status=="ambiguous").count(),"deferred_risk_units":parallel.iter().filter(|x|x.status=="deferred").count(),"private_layer_refs_read":private_layers,"private_outputs_mode":"0600","tracked_source_strings":false,"accepted_candidate_count":0,"semantic_equivalence_asserted":false});
    outputs.insert(route("coverage-receipt.v1.json"), pretty(&coverage)?);
    let provenance = json!({"schema_version":"tos_provenance_event_v1","event_id":"tos.event.zarathustra-parallel-lexical-candidates-v1.build","event_type":"mechanical_lexical_candidate_materialization","occurred_at":"2026-09-02T00:15:00-06:00","ended_at":"2026-09-02T00:15:00-06:00","agent_ref":"codex-internal-agents.lexical-candidate-v1","software_ref":GENERATOR,"software_sha256":RECIPE_SHA256,"plan_ref":selected.reference,"plan_sha256":selected.digest,"authority_boundary":plan["authority_boundary"]});
    outputs.insert(route("provenance.jsonl"), lines(&[provenance])?);
    let output_meta: Map<&str, V> = outputs
        .iter()
        .map(|(p, v)| (p.as_str(), json!({"sha256":hash(v),"byte_size":v.len()})))
        .collect();
    let private_meta: Map<&str, V> = private_outputs
        .iter()
        .map(|(p, v)| {
            (
                p.as_str(),
                json!({"sha256":hash(v),"byte_size":v.len(),"required_mode":"0600"}),
            )
        })
        .collect();
    let issuance_digest = read(root, &route("identity-issuance.v1.json"))
        .ok()
        .map(|b| hash(&b));
    let manifest = json!({"schema_version":"tos_zarathustra_parallel_lexical_candidate_manifest_v1","plan_ref":selected.reference,"plan_sha256":selected.digest,"identity_issuance_ref":route("identity-issuance.v1.json"),"identity_issuance_sha256":issuance_digest,"generated_outputs":output_meta,"private_outputs":private_meta,"source_text_included":false,"semantic_equivalence_asserted":false,"accepted_candidate_count":0,"canon_effect":false});
    outputs.insert(route("manifest.v1.json"), pretty(&manifest)?);
    Ok(Generated {
        outputs,
        private: private_outputs,
        bindings,
        analysis,
    })
}
/// Preserves the frozen build/check/preview contract; invocation owns only mechanical artifacts.
pub fn run(root: &Path, args: &[String]) -> R<V> {
    let execution = ResearchExecution::new(root, 180)?;
    run_scoped(&execution, args)
}
pub fn run_scoped(root: &ResearchExecution, args: &[String]) -> R<V> {
    root.check()?;
    let mut mode = None;
    let mut issue_ids = false;
    let mut plan_ref = route("plan.v1.json");
    let mut selected_flag = false;
    let mut arguments = args.iter();
    while let Some(arg) = arguments.next() {
        root.tick(1)?;
        match arg.as_str() {
            "--build" | "--check" | "--preview" => {
                if mode.replace(arg.as_str()).is_some() {
                    return Err("exactly one of --build, --check, --preview required".into());
                }
            }
            "--issue-identities" => issue_ids = true,
            "--plan-ref" => {
                if selected_flag {
                    return Err("duplicate --plan-ref".into());
                }
                selected_flag = true;
                root.tick(1)?;
                plan_ref = arguments
                    .next()
                    .ok_or("--plan-ref requires ROOT_RELATIVE ref")?
                    .clone();
            }
            _ => return Err(format!("unrecognized argument: {arg}")),
        }
    }
    let mode = mode.ok_or("exactly one of --build, --check, --preview required")?;
    if issue_ids && mode != "--build" {
        return Err("--issue-identities is valid only with --build".into());
    }
    let selected = select_plan(
        root,
        &plan_ref,
        &route("plan.v1.json"),
        "09f29b6442ce0634dde77c0945f8c559d1b4dad3f0563aab2868e87591f8c3a7",
    )?;
    if selected.custom && issue_ids {
        return Err("custom technical profile cannot remint v1 identities".into());
    }
    if mode == "--preview" {
        let generated = generate(root, false, &selected)?;
        let mut top = arr(&generated.analysis["translation_surface_associations"])?.clone();
        top.sort_by(|a, b| {
            n(&b["support"])
                .cmp(&n(&a["support"]))
                .then_with(|| n(&b["dice_millionths"]).cmp(&n(&a["dice_millionths"])))
        });
        let top: Vec<V> = top
            .into_iter()
            .take(25)
            .map(|x| {
                let mut row = json!({});
                for k in [
                    "source_form",
                    "target_form",
                    "support",
                    "dice_millionths",
                    "npmi_millionths",
                    "status",
                ] {
                    row[k] = x[k].clone()
                }
                row
            })
            .collect();
        return Ok(json!({"summary":generated.analysis["summary"],"top_associations":top}));
    }
    if mode == "--build" && issue_ids {
        let generated = generate(root, false, &selected)?;
        issue(root, &generated.bindings)?;
    }
    let generated = generate(root, true, &selected)?;
    issuance(root, &generated.bindings)?;
    if mode == "--build" {
        for (p, v) in &generated.outputs {
            root.tick(1)?;
            write(root, p, v, 0o644)?
        }
        for (p, v) in &generated.private {
            root.tick(1)?;
            write(root, p, v, 0o600)?
        }
    } else {
        for (p, v) in &generated.outputs {
            root.tick(1)?;
            if read(root, p)? != *v {
                return Err(format!("tracked parity mismatch: {p}"));
            }
        }
        for (p, v) in &generated.private {
            root.tick(1)?;
            if read(root, p)? != *v {
                return Err(format!("private parity mismatch: {p}"));
            }
            if root
                .source_file(p, 256 * 1024 * 1024)?
                .metadata()
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o777
                != 0o600
            {
                return Err(format!("private mode mismatch: {p}"));
            }
        }
    }
    Ok(generated.analysis["summary"].clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn technical_profile_preserves_semantics_and_exact_lineage() {
        let directory = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 4 * 1024 * 1024).unwrap();
        let original = json!({"schema_version":"fixture_v1","plan_id":"original","status":"frozen-before-output","frozen_at":"date","inputs":{"a":{"ref":"input.json","sha256":"0".repeat(64)}},"methods":{"gate":8},"authority_boundary":"candidate-only"});
        let raw = pretty(&original).unwrap();
        let digest = hash(&raw);
        execution.write("plan.v1.json", &raw, 0o644, false).unwrap();
        let mut profile = original.clone();
        profile["plan_id"] = json!("successor");
        profile["status"] = json!("proposed-technical-input-profile-successor");
        profile["frozen_at"] = V::Null;
        profile["inputs"]["a"]["sha256"] = json!("1".repeat(64));
        profile["input_profile_lineage"] = json!({"profile_version":2,"supersedes_plan_ref":"plan.v1.json","supersedes_plan_sha256":digest});
        execution
            .write("profile.json", &pretty(&profile).unwrap(), 0o600, false)
            .unwrap();
        select_plan(&execution, "profile.json", "plan.v1.json", &digest)
            .expect("owned private fixture accepts the exact technical lineage");
        profile["methods"]["gate"] = json!(9);
        execution
            .write("profile.json", &pretty(&profile).unwrap(), 0o600, false)
            .unwrap();
        assert!(
            select_plan(&execution, "profile.json", "plan.v1.json", &digest)
                .unwrap_err()
                .contains("semantic fields")
        );
        profile["methods"]["gate"] = json!(8);
        profile["input_profile_lineage"]["supersedes_plan_sha256"] = json!("2".repeat(64));
        execution
            .write("profile.json", &pretty(&profile).unwrap(), 0o600, false)
            .unwrap();
        assert!(
            select_plan(&execution, "profile.json", "plan.v1.json", &digest)
                .unwrap_err()
                .contains("predecessor lineage")
        );
    }
    #[test]
    fn scoped_deadline_fails_before_source_reads_or_outputs() {
        let directory = tempfile::tempdir().unwrap();
        let execution = ResearchExecution::new(directory.path(), 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1050));
        assert_eq!(
            run_scoped(&execution, &["--build".into()]).unwrap_err(),
            "research operation deadline exceeded"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }
    #[test]
    fn private_sqlite_temp_is_searchable_and_cleans_custody() {
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 300 * 1024 * 1024).unwrap();
        execution
            .write(&route("plan.v1.json"), b"{}\n", 0o644, false)
            .unwrap();
        let occ = Occ {
            id: "o1".into(),
            unit: "u1".into(),
            reading: "part_1.r1".into(),
            part: 1,
            role: "prose".into(),
            ordinal: 1,
            start: 0,
            end: 4,
            surface: "Свет".into(),
            normalized: "свет".into(),
            analysis: "свет".into(),
        };
        let bytes = ru_db(&execution, &[occ], &hash(b"{}\n")).unwrap();
        assert!(fs::read_dir(directory.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".research-sqlite-")
        }));
        execution
            .write(&private("test.sqlite3"), &bytes, 0o600, true)
            .unwrap();
        let database = Connection::open_with_flags(
            directory.path().join(private("test.sqlite3")),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        assert_eq!(
            database
                .query_row(
                    "SELECT count(*) FROM unit_fts WHERE normalized_text MATCH 'свет'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }
    #[test]
    fn unicode_offsets_and_historical_folds() {
        assert_eq!(
            exact("Ѣсть Groß’wort."),
            vec![("Ѣсть".into(), 0, 4), ("Groß’wort".into(), 5, 14)]
        );
        assert_eq!(ru_key("ѢстьЪ"), "есть");
        assert_eq!(base_key("GROẞ"), "gross");
        assert_eq!(base_key("ſchickſal"), "schicksal");
        assert_eq!(base_key("ﬀﬁﬂﬃﬄﬅﬆ"), "fffiflffifflstst");
        assert_eq!(base_key("ΟΣς"), "οσσ");
        assert_eq!(base_key("A\u{308}"), "ä");
        assert_eq!(tokens("за¬\nрату-\nстра", "ru"), vec!["зарату", "стра"]);
        assert_eq!(tokens("з а р а т у с т р а", "ru").len(), 10);
    }
    #[test]
    fn quality_constituents_and_scripts() {
        assert!(!b(
            &quality_profile("з а р слово", "ru")["positive_evidence_eligible"]
        ));
        assert!(!b(
            &quality_profile("слово Latin", "ru")["positive_evidence_eligible"]
        ));
        assert_eq!(
            n(&quality_profile("з а р", "ru")["spaced_letter_run_count"]),
            1
        );
        assert_eq!(
            n(&quality_profile("за-\nратустра", "ru")["line_join_candidate_count"]),
            1
        );
    }
    #[test]
    fn negative_association_retains_universe_identity() {
        let mut units = vec![];
        for i in 0..4 {
            units.push(Parallel {
                alignment: format!("a{i}"),
                part: 1,
                reading: format!("r{i}"),
                status: "proposed".into(),
                shape: "one_to_one".into(),
                strict: true,
                de: vec!["licht".into()],
                ru: vec!["свет".into()],
                dq: V::Null,
                rq: V::Null,
                universe: true,
                positive: false,
            });
        }
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        let (t, p, ids) = associations(&execution, &units, None).unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(n(&t[0]["support"]), 0);
        assert_eq!(t[0]["status"], "deferred");
        assert_eq!(t[0]["accepted"], false);
        assert!(t[0].get("source_form").is_none());
        assert_eq!(p[0]["source_form"], "licht");
    }
}

pub fn load_parallel(root: &ResearchExecution) -> R<Vec<V>> {
    let (rows, _, _) = parallel(root)?;
    rows.iter().map(|p|{root.tick(1)?; Ok(json!({"alignment_id":p.alignment,"part":p.part,"reading":p.reading,"status":p.status,"shape":p.shape,"strict":p.strict,"de":p.de,"ru":p.ru,"de_quality":p.dq,"ru_quality":p.rq,"candidate_universe_eligible":p.universe,"positive_evidence_eligible":p.positive}))}).collect()
}
