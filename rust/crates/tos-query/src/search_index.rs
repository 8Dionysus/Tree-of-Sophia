//! Shared Worker search gram rules.

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
