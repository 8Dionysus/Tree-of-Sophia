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
    /// Decoded SQLite result-column bytes, not file/page I/O or heap usage.
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
    let code_points: Vec<char> = request.query().chars().collect();
    if code_points.len() < 3 {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::InvalidRequest,
            "indexed search query has no three-code-point gram",
        ));
    }
    let mut grams = Vec::new();
    for window in code_points.windows(3) {
        let gram: String = window.iter().collect();
        if !grams.contains(&gram) {
            grams.push(gram);
        }
    }
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
