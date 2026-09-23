//! Portable Unicode 16 primitives derived from the owner-pinned native Worker
//! table. These operate on valid UTF-8 scalars; WTF-16 JSON surrogates must be
//! refused by the caller before invoking them.

use crate::error::{FoundationError, FoundationErrorCode as Code, Result};

include!("unicode_generated.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnicodeProfile {
    PythonNativeUnicodeV1,
}

impl UnicodeProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PythonNativeUnicodeV1 => "tos-python-native-unicode-v1",
        }
    }

    pub const fn ucd_version(self) -> &'static str { "16.0.0" }

    pub fn from_profile(profile: &str) -> Result<Self> {
        match profile {
            "tos-python-native-unicode-v1" => Ok(Self::PythonNativeUnicodeV1),
            _ => Err(FoundationError::new(Code::UnsupportedFormat, "unknown Unicode profile")),
        }
    }
}

/// Python 3.14/Unicode 16 `str.strip()` whitespace, matching the existing
/// native Worker `nativeStrip` definition. No normalization or case conversion.
pub fn python_strip_unicode16_v1(input: &str, max_input_code_points: usize) -> Result<&str> {
    check_input(input, max_input_code_points)?;
    Ok(input.trim_matches(is_python_whitespace))
}

/// Unicode 16 Default Lowercase with contextual Final_Sigma, matching the
/// owner-pinned Worker table and Python 3.14 `str.lower()`. Budgets count exact
/// input scalars, output scalars and output UTF-8 bytes. No host Unicode tables.
pub fn python_lower_unicode16_v1(
    input: &str,
    max_input_code_points: usize,
    max_output_code_points: usize,
    max_output_bytes: usize,
) -> Result<String> {
    check_input(input, max_input_code_points)?;
    let chars: Vec<char> = input.chars().collect();
    let mut following_cased = vec![false; chars.len()];
    let mut next = false;
    for index in (0..chars.len()).rev() {
        let point = chars[index] as u32;
        following_cased[index] = next;
        if !in_ranges(point, CASE_IGNORABLE) {
            next = in_ranges(point, CASED);
        }
    }
    let mut previous = false;
    let mut output = String::with_capacity(input.len().min(max_output_bytes));
    let mut output_points = 0usize;
    for (index, ch) in chars.iter().copied().enumerate() {
        let point = ch as u32;
        let mapped = if point == 0x3a3 && previous && !following_cased[index] {
            Some("ς")
        } else {
            LOWER.binary_search_by_key(&point, |(key, _)| *key)
                .ok().map(|position| LOWER[position].1)
        };
        let (new_points, new_bytes) = match mapped {
            Some(text) => (text.chars().count(), text.len()),
            None => (1, ch.len_utf8()),
        };
        output_points = output_points.checked_add(new_points)
            .ok_or_else(|| budget_error("Unicode output code-point budget exceeded"))?;
        if output_points > max_output_code_points
            || new_bytes > max_output_bytes.saturating_sub(output.len())
        {
            return Err(budget_error("Unicode output budget exceeded"));
        }
        match mapped {
            Some(text) => output.push_str(text),
            None => output.push(ch),
        }
        if !in_ranges(point, CASE_IGNORABLE) {
            previous = in_ranges(point, CASED);
        }
    }
    Ok(output)
}

fn check_input(input: &str, max_code_points: usize) -> Result<()> {
    if input.chars().count() > max_code_points {
        return Err(budget_error("Unicode input code-point budget exceeded"));
    }
    Ok(())
}

fn budget_error(detail: &'static str) -> FoundationError {
    FoundationError::new(Code::BudgetExceeded, detail)
}

fn in_ranges(point: u32, ranges: &[(u32, u32)]) -> bool {
    ranges.binary_search_by(|(start, end)| {
        if point < *start { std::cmp::Ordering::Greater }
        else if point > *end { std::cmp::Ordering::Less }
        else { std::cmp::Ordering::Equal }
    }).is_ok()
}

fn is_python_whitespace(ch: char) -> bool {
    matches!(ch,
        '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{0085}' |
        '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' |
        '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}
