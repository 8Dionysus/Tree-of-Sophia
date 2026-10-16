//! Bounded rarest-gram admission for the selected indexed-v2 search profile.
//! The complete posting/stat scope must already be cold-verified by CMP.

use crate::search_v2::{
    NormalizedIndexedSearchV2Request, SearchKind, SearchV2Error, SearchV2ErrorCode,
};

#[derive(Clone, Copy, Debug)]
pub struct GramSeekBudget {
    pub max_lookups: usize,
    pub max_candidates: u64,
    pub max_vm_steps: u64,
    pub max_rows: u64,
    pub max_decoded_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GramSeekCharge {
    pub lookups: u64,
    pub vm_steps: u64,
    pub rows: u64,
    /// Gram stats charge their selected scalar field. Posting seeks charge
    /// selected fields, encoded block bytes, and every decoded position,
    /// including positions skipped by a continuation. This is not file/page
    /// I/O or peak heap usage.
    pub decoded_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct GramStat {
    /// None is absent only under the independently verified complete index.
    pub postings: Option<u64>,
    pub charged: GramSeekCharge,
}

/// One indexed scalar lookup on the exact pinned selected model. It must
/// interrupt SQL at the supplied VM limit and report actual work, including
/// an absent-row lookup. An untrusted host needs a separate proof profile.
pub trait SearchGramModel {
    fn gram_stat(
        &mut self,
        kind: SearchKind,
        gram: &str,
        max_vm_steps: u64,
        max_rows: u64,
        max_decoded_bytes: u64,
    ) -> Result<GramStat, SearchV2Error>;
}

#[derive(Clone, Copy, Debug)]
pub struct PostingSeekBudget {
    pub max_probes: u64,
    pub max_rows: u64,
    pub max_decoded_bytes: u64,
    pub max_vm_steps: u64,
    pub page_rows: usize,
}

#[derive(Clone, Debug)]
pub struct PostingPage {
    pub positions: Vec<u64>,
    /// True only after the exact seek reaches its end. A page filled at the
    /// final posting still requires the ordinary empty completion probe.
    pub exhausted: bool,
    /// `rows` counts returned positions; `decoded_bytes` also counts every
    /// selected block field and position decoded before an `after` cursor.
    pub charged: GramSeekCharge,
}

pub trait SearchPostingModel {
    fn seek_postings(
        &mut self,
        kind: SearchKind,
        gram: &str,
        after: Option<u64>,
        max_rows: usize,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<PostingPage, SearchV2Error>;
}

pub const INDEXED_SEARCH_GRAM_CODEPOINTS_V1: u8 = 3;

/// Distinct three-code-point grams in first-query-occurrence order. Worker
/// SQL uses this exact owner rule for bounded D1 candidate admission.
pub fn unique_search_grams(query: &str) -> Vec<String> {
    let code_points: Vec<char> = query.chars().collect();
    let mut grams = Vec::new();
    for window in code_points.windows(INDEXED_SEARCH_GRAM_CODEPOINTS_V1 as usize) {
        let gram: String = window.iter().collect();
        if !grams.contains(&gram) {
            grams.push(gram);
        }
    }
    grams
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GramSeed {
    pub gram: Option<String>,
    pub postings: u64,
    pub charged: GramSeekCharge,
}

/// An absent selected gram proves an exact empty candidate set. Otherwise
/// choose the rarest global gram, breaking ties by the query's first occurrence.
/// Selective source/kind filters do not lower this global admission count.
pub fn choose_rarest_gram<M: SearchGramModel>(
    model: &mut M,
    kind: SearchKind,
    request: &NormalizedIndexedSearchV2Request,
    budget: GramSeekBudget,
) -> Result<GramSeed, SearchV2Error> {
    if request.query().chars().count() < INDEXED_SEARCH_GRAM_CODEPOINTS_V1 as usize {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::InvalidRequest,
            "indexed search query has no three-code-point gram",
        ));
    }
    let grams = unique_search_grams(request.query());
    let lookup_count = grams.len() as u64;
    let worst_decoded = lookup_count.checked_mul(8).ok_or_else(|| {
        SearchV2Error::new(
            SearchV2ErrorCode::BudgetExceeded,
            "gram stat byte cap overflow",
        )
    })?;
    if budget.max_lookups < grams.len()
        || budget.max_candidates == 0
        || budget.max_vm_steps == 0
        || budget.max_rows < lookup_count
        || budget.max_decoded_bytes < worst_decoded
    {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::BudgetExceeded,
            "gram stat lookup admission unavailable",
        ));
    }
    let mut charged = GramSeekCharge::default();
    let mut rarest: Option<(u64, String)> = None;
    for gram in grams {
        let remaining_vm = budget
            .max_vm_steps
            .checked_sub(charged.vm_steps)
            .ok_or_else(|| {
                SearchV2Error::new(
                    SearchV2ErrorCode::BudgetExceeded,
                    "gram VM budget exhausted",
                )
            })?;
        if remaining_vm == 0 {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::BudgetExceeded,
                "gram VM budget exhausted",
            ));
        }
        let stat = model.gram_stat(
            kind,
            &gram,
            remaining_vm,
            budget.max_rows - charged.rows,
            budget.max_decoded_bytes - charged.decoded_bytes,
        )?;
        charged.lookups = charged
            .lookups
            .checked_add(stat.charged.lookups)
            .ok_or_else(|| {
                SearchV2Error::new(
                    SearchV2ErrorCode::BudgetExceeded,
                    "gram lookup charge overflow",
                )
            })?;
        charged.vm_steps = charged
            .vm_steps
            .checked_add(stat.charged.vm_steps)
            .ok_or_else(|| {
                SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "gram VM charge overflow")
            })?;
        charged.rows = charged.rows.checked_add(stat.charged.rows).ok_or_else(|| {
            SearchV2Error::new(
                SearchV2ErrorCode::BudgetExceeded,
                "gram row charge overflow",
            )
        })?;
        charged.decoded_bytes = charged
            .decoded_bytes
            .checked_add(stat.charged.decoded_bytes)
            .ok_or_else(|| {
                SearchV2Error::new(
                    SearchV2ErrorCode::BudgetExceeded,
                    "gram byte charge overflow",
                )
            })?;
        if stat.charged.lookups != 1
            || stat.charged.vm_steps > remaining_vm
            || stat.charged.rows != u64::from(stat.postings.is_some())
            || stat.charged.decoded_bytes != stat.charged.rows * 8
            || charged.rows > budget.max_rows
            || charged.decoded_bytes > budget.max_decoded_bytes
            || charged.vm_steps > budget.max_vm_steps
        {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::IndexIncomplete,
                "gram stat host charge or row shape invalid",
            ));
        }
        let postings = stat.postings.unwrap_or(0);
        if rarest.as_ref().is_none_or(|(count, _)| postings < *count) {
            rarest = Some((postings, gram));
        }
    }
    let (postings, gram) = rarest.expect("normalized query has at least one gram");
    if postings > budget.max_candidates {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::BudgetExceeded,
            "global gram candidate cap exceeded",
        ));
    }
    Ok(GramSeed {
        gram: (postings != 0).then_some(gram),
        postings,
        charged,
    })
}

/// Drain one *bounded* selected posting list in source position order. The
/// complete stat count is checked against every visited row and a final empty
/// seek. `visit` may verify/hydrate a candidate, but the engine never builds
/// the whole graph or an unbounded posting closure.
pub fn visit_complete_postings<M: SearchPostingModel>(
    model: &mut M,
    kind: SearchKind,
    seed: &GramSeed,
    budget: PostingSeekBudget,
    mut visit: impl FnMut(&mut M, u64) -> Result<(), SearchV2Error>,
) -> Result<GramSeekCharge, SearchV2Error> {
    let gram = seed.gram.as_deref().ok_or_else(|| {
        SearchV2Error::new(
            SearchV2ErrorCode::InvalidRequest,
            "empty gram has no posting list",
        )
    })?;
    if budget.page_rows == 0 || budget.page_rows > 1024 || budget.max_vm_steps == 0 {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::BudgetExceeded,
            "posting seek budget is invalid",
        ));
    }
    let row_ceiling = seed.postings.checked_add(1).ok_or_else(|| {
        SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "posting count overflow")
    })?;
    // Logical output is only a lower bound: compressed blocks may be decoded
    // repeatedly by small pages, and their encoded/fixed fields are charged.
    let byte_floor = row_ceiling.checked_mul(8).ok_or_else(|| {
        SearchV2Error::new(
            SearchV2ErrorCode::BudgetExceeded,
            "posting byte cap overflow",
        )
    })?;
    let probe_ceiling = seed.postings / budget.page_rows as u64 + 1;
    if seed.postings == 0
        || budget.max_rows < row_ceiling
        || budget.max_decoded_bytes < byte_floor
        || budget.max_probes < probe_ceiling
    {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::BudgetExceeded,
            "posting list cannot be preadmitted",
        ));
    }
    let mut charged = GramSeekCharge::default();
    let mut after = None;
    let mut visited = 0u64;
    loop {
        let remaining_vm = budget.max_vm_steps.saturating_sub(charged.vm_steps);
        let remaining_rows = budget.max_rows.saturating_sub(charged.rows);
        let remaining_bytes = budget
            .max_decoded_bytes
            .saturating_sub(charged.decoded_bytes);
        let max_rows = budget
            .page_rows
            .min(remaining_rows.min(usize::MAX as u64) as usize)
            .min((remaining_bytes / 8).min(usize::MAX as u64) as usize);
        if remaining_vm == 0
            || max_rows == 0
            || remaining_bytes < 8
            || charged.lookups >= budget.max_probes
        {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::BudgetExceeded,
                "posting seek work budget exhausted",
            ));
        }
        let page =
            model.seek_postings(kind, gram, after, max_rows, remaining_vm, remaining_bytes)?;
        if page.positions.len() > max_rows
            || page.charged.lookups != 1
            || page.charged.rows != page.positions.len() as u64
            || page.charged.decoded_bytes < page.charged.rows * 8
            || page.charged.decoded_bytes > remaining_bytes
            || page.charged.vm_steps > remaining_vm
            || page.exhausted != (page.positions.len() < max_rows)
        {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::IndexIncomplete,
                "posting page or work receipt invalid",
            ));
        }
        charged.lookups = charged.lookups.checked_add(1).ok_or_else(|| {
            SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "posting probe overflow")
        })?;
        charged.rows = charged.rows.checked_add(page.charged.rows).ok_or_else(|| {
            SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "posting row overflow")
        })?;
        charged.decoded_bytes = charged
            .decoded_bytes
            .checked_add(page.charged.decoded_bytes)
            .ok_or_else(|| {
                SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "posting byte overflow")
            })?;
        charged.vm_steps = charged
            .vm_steps
            .checked_add(page.charged.vm_steps)
            .ok_or_else(|| {
                SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "posting VM overflow")
            })?;
        for position in page.positions {
            if after.is_some_and(|previous| position <= previous) || position > i64::MAX as u64 {
                return Err(SearchV2Error::new(
                    SearchV2ErrorCode::IndexIncomplete,
                    "posting position order is invalid",
                ));
            }
            after = Some(position);
            visited = visited.checked_add(1).ok_or_else(|| {
                SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded, "posting visit overflow")
            })?;
            if visited > seed.postings {
                return Err(SearchV2Error::new(
                    SearchV2ErrorCode::IndexIncomplete,
                    "posting list exceeds selected stat count",
                ));
            }
            visit(model, position)?;
        }
        if page.exhausted {
            if visited != seed.postings {
                return Err(SearchV2Error::new(
                    SearchV2ErrorCode::IndexIncomplete,
                    "posting list omits selected stat rows",
                ));
            }
            return Ok(charged);
        }
    }
}
