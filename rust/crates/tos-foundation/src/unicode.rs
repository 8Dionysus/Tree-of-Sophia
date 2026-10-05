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

    pub const fn ucd_version(self) -> &'static str {
        "16.0.0"
    }

    pub fn from_profile(profile: &str) -> Result<Self> {
        match profile {
            "tos-python-native-unicode-v1" => Ok(Self::PythonNativeUnicodeV1),
            _ => Err(FoundationError::new(
                Code::UnsupportedFormat,
                "unknown Unicode profile",
            )),
        }
    }
}

/// Python 3.14/Unicode 16 `str.strip()` whitespace, matching the existing
/// native Worker `nativeStrip` definition. No normalization or case conversion.
pub fn python_strip_unicode16_v1(input: &str, max_input_code_points: usize) -> Result<&str> {
    check_input(input, max_input_code_points)?;
    Ok(input.trim_matches(is_python_whitespace))
}

/// The same owner-pinned whitespace law, with one checked borrowed scalar
/// walk. Caller owns the typed CharIndices/offset controller and the exported
/// Unicode diagnostic floor under its original workspace; no String is made.
pub fn python_strip_unicode16_v1_with_check<'a>(
    input: &'a str,
    max_input_code_points: usize,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<&'a str> {
    let mut count = 0usize;
    let mut start = input.len();
    let mut end = 0usize;
    for (offset, scalar) in input.char_indices() {
        check()?;
        count = count
            .checked_add(1)
            .filter(|n| *n <= max_input_code_points)
            .ok_or_else(|| budget_error("Unicode input code-point budget exceeded"))?;
        if !is_python_whitespace(scalar) {
            if start == input.len() {
                start = offset;
            }
            end = offset + scalar.len_utf8();
        }
    }
    check()?;
    if end == 0 {
        Ok(&input[input.len()..])
    } else {
        Ok(&input[start..end])
    }
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
    let mut check = || Ok(());
    lower_following(&chars, &mut following_cased, &mut check)?;
    let mut output = String::with_capacity(input.len().min(max_output_bytes));
    lower_visit(
        &chars,
        &following_cased,
        max_output_code_points,
        max_output_bytes,
        &mut check,
        &mut |piece| {
            output.push_str(piece);
            Ok(())
        },
    )?;
    Ok(output)
}

/// The same Unicode16/Final_Sigma visitor under the caller's original remaining
/// workspace and cumulative work/cancellation callback. Every vector/string is
/// exact-reserved after its complete live overlap is admitted. The callback
/// runs on input/count/emit scalars and every actual table comparison; callers
/// must debit the same original meter, including terminal failures. Before
/// entry the caller must reserve python_lower_unicode16_v1_error_state_upper_bound
/// from that SAME original ledger, outside available_state_bytes, and keep it
/// through success/refusal. Even a zero-workspace refusal owns its diagnostic.
pub fn python_lower_unicode16_v1_with_state_budget_and_check(
    input: &str,
    max_input_code_points: usize,
    max_output_code_points: usize,
    max_output_bytes: usize,
    available_state_bytes: usize,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<String> {
    let fixed = lower_fixed_state();
    if fixed > available_state_bytes {
        return Err(budget_error("Unicode controller state budget exceeded"));
    }
    let mut count = 0usize;
    for _ in input.chars() {
        check()?;
        count = count
            .checked_add(1)
            .filter(|n| *n <= max_input_code_points)
            .ok_or_else(|| budget_error("Unicode input code-point budget exceeded"))?;
    }
    let vectors = count
        .checked_mul(std::mem::size_of::<char>() + std::mem::size_of::<bool>())
        .and_then(|n| n.checked_add(fixed))
        .filter(|n| *n <= available_state_bytes)
        .ok_or_else(|| budget_error("Unicode input workspace exceeded"))?;
    let mut chars = Vec::new();
    chars
        .try_reserve_exact(count)
        .map_err(|_| budget_error("Unicode exact scalar reserve failed"))?;
    let mut following_cased = Vec::new();
    following_cased
        .try_reserve_exact(count)
        .map_err(|_| budget_error("Unicode exact flags reserve failed"))?;
    if chars.capacity() != count || following_cased.capacity() != count {
        return Err(budget_error("Unicode exact vector capacity differs"));
    }
    for ch in input.chars() {
        check()?;
        chars.push(ch);
        following_cased.push(false);
    }
    lower_following(&chars, &mut following_cased, check)?;
    let mut counted_bytes = 0usize;
    lower_visit(
        &chars,
        &following_cased,
        max_output_code_points,
        max_output_bytes,
        check,
        &mut |piece| {
            counted_bytes = counted_bytes
                .checked_add(piece.len())
                .ok_or_else(|| budget_error("Unicode output count overflow"))?;
            Ok(())
        },
    )?;
    vectors
        .checked_add(counted_bytes)
        .filter(|n| *n <= available_state_bytes)
        .ok_or_else(|| budget_error("Unicode output workspace exceeded"))?;
    let mut output = String::new();
    output
        .try_reserve_exact(counted_bytes)
        .map_err(|_| budget_error("Unicode exact output reserve failed"))?;
    if output.capacity() != counted_bytes {
        return Err(budget_error("Unicode exact output capacity differs"));
    }
    lower_visit(
        &chars,
        &following_cased,
        max_output_code_points,
        max_output_bytes,
        check,
        &mut |piece| {
            output.push_str(piece);
            Ok(())
        },
    )?;
    if output.len() != counted_bytes || output.capacity() != counted_bytes {
        return Err(budget_error("Unicode counted/emitted output differs"));
    }
    check()?;
    Ok(output)
}

/// Caller-held failure state, admitted before the checked lower entry. These
/// are this owner's static diagnostics only; callback failures retain their
/// caller's separately admitted diagnostic owner. No input-dependent text is
/// copied, and this slot is not a new grant or released on terminal failure.
pub fn python_lower_unicode16_v1_error_state_upper_bound() -> usize {
    const MESSAGES: &[&str] = &[
        "Unicode controller state budget exceeded",
        "Unicode input code-point budget exceeded",
        "Unicode input workspace exceeded",
        "Unicode exact scalar reserve failed",
        "Unicode exact flags reserve failed",
        "Unicode exact vector capacity differs",
        "Unicode output count overflow",
        "Unicode output workspace exceeded",
        "Unicode exact output reserve failed",
        "Unicode exact output capacity differs",
        "Unicode counted/emitted output differs",
        "Unicode output code-point budget exceeded",
        "Unicode output budget exceeded",
    ];
    const fn maximum(messages: &[&str]) -> usize {
        let (mut index, mut bytes) = (0, 0);
        while index < messages.len() {
            if messages[index].len() > bytes {
                bytes = messages[index].len();
            }
            index += 1;
        }
        bytes
    }
    std::mem::size_of::<FoundationError>() + maximum(MESSAGES)
}

// Typed simultaneous controller geometry. No input-byte multiplier or native
// allocator pool is used for these Rust owners. Borrowed input/tables/callback
// targets stay with the caller and are not duplicated in this workspace.
fn lower_fixed_state() -> usize {
    // Every simultaneously active routine is represented separately. Tuple
    // temporaries are conservatively retained in addition to destructured
    // locals; shared heap/input/callback targets are not cloned here.
    let main = std::mem::size_of::<Vec<char>>() + std::mem::size_of::<Vec<bool>>()
        + std::mem::size_of::<String>() + std::mem::size_of::<std::str::Chars<'_>>()
        + std::mem::size_of::<&str>() + std::mem::size_of::<&mut dyn FnMut() -> Result<()>>()
        // Four limit/workspace parameters; count, vectors, counted_bytes.
        + std::mem::size_of::<(usize, usize, usize, usize, usize, usize, usize)>()
        + std::mem::size_of::<char>() + std::mem::size_of::<Result<()>>();
    let following = std::mem::size_of::<&[char]>()
        + std::mem::size_of::<&mut [bool]>()
        + std::mem::size_of::<&mut dyn FnMut() -> Result<()>>()
        + std::mem::size_of::<std::iter::Rev<std::ops::Range<usize>>>()
        + std::mem::size_of::<bool>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<u32>()
        + std::mem::size_of::<Result<()>>();
    let ranges = std::mem::size_of::<&[(u32, u32)]>()
        + std::mem::size_of::<&mut dyn FnMut() -> Result<()>>()
        // low/high/middle plus point/start/end and destructuring tuple.
        + std::mem::size_of::<(usize, usize, usize)>()
        + std::mem::size_of::<(u32, u32, u32)>() + std::mem::size_of::<(u32, u32)>()
        + std::mem::size_of::<Result<bool>>();
    let visit = std::mem::size_of::<&[char]>() + std::mem::size_of::<&[bool]>()
        + std::mem::size_of::<&mut dyn FnMut() -> Result<()>>()
        + std::mem::size_of::<&mut dyn FnMut(&str) -> Result<()>>()
        + std::mem::size_of::<std::iter::Enumerate<std::iter::Copied<std::slice::Iter<'_, char>>>>()
        // max_points/max_bytes, points/bytes, index, low/high/middle.
        + std::mem::size_of::<(usize, usize, usize, usize, usize, usize, usize, usize)>()
        + std::mem::size_of::<bool>() + std::mem::size_of::<char>() + std::mem::size_of::<u32>()
        + std::mem::size_of::<Option<&str>>() + std::mem::size_of::<&str>()
        + std::mem::size_of::<[u8; 4]>()
        + std::mem::size_of::<(usize, usize, Option<&str>)>()
        + std::mem::size_of::<(u32, &str)>() + std::mem::size_of::<u32>() + std::mem::size_of::<&str>()
        + std::mem::size_of::<std::str::Chars<'_>>() + std::mem::size_of::<Result<()>>();
    // Count and emission closures each capture exactly one mutable borrow.
    main + following
        + ranges
        + visit
        + std::mem::size_of::<&mut usize>()
        + std::mem::size_of::<&mut String>()
}

fn lower_following(
    chars: &[char],
    following: &mut [bool],
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let mut next = false;
    for index in (0..chars.len()).rev() {
        check()?;
        let point = chars[index] as u32;
        following[index] = next;
        if !lower_ranges(point, CASE_IGNORABLE, check)? {
            next = lower_ranges(point, CASED, check)?;
        }
    }
    Ok(())
}
fn lower_ranges(
    point: u32,
    ranges: &[(u32, u32)],
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let (mut low, mut high) = (0, ranges.len());
    while low < high {
        check()?;
        let middle = low + (high - low) / 2;
        let (start, end) = ranges[middle];
        if point < start {
            high = middle;
        } else if point > end {
            low = middle + 1;
        } else {
            return Ok(true);
        }
    }
    Ok(false)
}
fn lower_visit(
    chars: &[char],
    following: &[bool],
    max_points: usize,
    max_bytes: usize,
    check: &mut dyn FnMut() -> Result<()>,
    emit: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    let (mut previous, mut points, mut bytes) = (false, 0usize, 0usize);
    for (index, ch) in chars.iter().copied().enumerate() {
        check()?;
        let point = ch as u32;
        let mapped = if point == 0x3a3 && previous && !following[index] {
            Some("ς")
        } else {
            let (mut low, mut high, mut mapped) = (0, LOWER.len(), None);
            while low < high {
                check()?;
                let middle = low + (high - low) / 2;
                let (key, value) = LOWER[middle];
                if point < key {
                    high = middle;
                } else if point > key {
                    low = middle + 1;
                } else {
                    mapped = Some(value);
                    break;
                }
            }
            mapped
        };
        let mut scalar = [0u8; 4];
        let piece = match mapped {
            Some(text) => text,
            None => ch.encode_utf8(&mut scalar),
        };
        for _ in piece.chars() {
            check()?;
            points = points
                .checked_add(1)
                .ok_or_else(|| budget_error("Unicode output code-point budget exceeded"))?;
        }
        bytes = bytes
            .checked_add(piece.len())
            .ok_or_else(|| budget_error("Unicode output budget exceeded"))?;
        if points > max_points || bytes > max_bytes {
            return Err(budget_error("Unicode output budget exceeded"));
        }
        emit(piece)?;
        if !lower_ranges(point, CASE_IGNORABLE, check)? {
            previous = lower_ranges(point, CASED, check)?;
        }
    }
    Ok(())
}

/// Unicode 16 default full case folding (CaseFolding C + F; excludes Turkic T),
/// matching Python 3.14 `str.casefold()`. Unlike lowercase, this is context-free
/// and may expand one scalar into several. It does not normalize text or use
/// host Unicode tables. Budgets count input/output scalars and output UTF-8 bytes.
/// The caller owns cancellation between bounded invocations, as with lowercase.
pub fn python_casefold_unicode16_v1(
    input: &str,
    max_input_code_points: usize,
    max_output_code_points: usize,
    max_output_bytes: usize,
) -> Result<String> {
    check_input(input, max_input_code_points)?;
    let mut output = String::with_capacity(input.len().min(max_output_bytes));
    let mut output_points = 0usize;
    for ch in input.chars() {
        let mapped = CASEFOLD
            .binary_search_by_key(&(ch as u32), |(key, _)| *key)
            .ok()
            .map(|position| CASEFOLD[position].1);
        let (new_points, new_bytes) = match mapped {
            Some(text) => (text.chars().count(), text.len()),
            None => (1, ch.len_utf8()),
        };
        output_points = output_points
            .checked_add(new_points)
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
    }
    Ok(output)
}

/// The same context-free Unicode16 C+F mapping under the original caller
/// state/work/cutoff. Before entry caller holds the shared Unicode diagnostic
/// floor outside `available_state_bytes`, and its callback target/controller.
/// No input vector is allocated. Count and emit use the same table visitor;
/// each scalar/comparison/output-copy is checked before work, including empty.
pub fn python_casefold_unicode16_v1_with_state_budget_and_check(
    input: &str,
    max_input_code_points: usize,
    max_output_code_points: usize,
    max_output_bytes: usize,
    available_state_bytes: usize,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<String> {
    let fixed = casefold_fixed_state();
    if fixed > available_state_bytes {
        return Err(budget_error("Unicode controller state budget exceeded"));
    }
    let (_, counted_bytes) = casefold_visit(
        input,
        max_input_code_points,
        max_output_code_points,
        max_output_bytes,
        check,
        &mut |_| Ok(()),
    )?;
    fixed
        .checked_add(counted_bytes)
        .filter(|n| *n <= available_state_bytes)
        .ok_or_else(|| budget_error("Unicode output workspace exceeded"))?;
    check()?;
    let mut output = String::new();
    output
        .try_reserve_exact(counted_bytes)
        .map_err(|_| budget_error("Unicode exact output reserve failed"))?;
    if output.capacity() != counted_bytes {
        return Err(budget_error("Unicode exact output capacity differs"));
    }
    let (_, emitted_bytes) = casefold_visit(
        input,
        max_input_code_points,
        max_output_code_points,
        max_output_bytes,
        check,
        &mut |piece| {
            output.push_str(piece);
            Ok(())
        },
    )?;
    if emitted_bytes != counted_bytes || output.len() != counted_bytes {
        return Err(budget_error("Unicode counted/emitted output differs"));
    }
    check()?;
    Ok(output)
}

/// Same diagnostic owner as checked lowercase; all casefold static labels
/// are members of that finite list. It is caller-held even on zero workspace.
pub fn python_casefold_unicode16_v1_error_state_upper_bound() -> usize {
    python_lower_unicode16_v1_error_state_upper_bound()
}

/// Typed simultaneously live helper controllers plus exact UTF-8 output.
/// Caller input, check target, error floor and enclosing frames stay separate.
pub fn python_casefold_unicode16_v1_fixed_state_upper_bound() -> usize {
    casefold_fixed_state()
}
fn casefold_fixed_state() -> usize {
    let main = std::mem::size_of::<String>()
        + std::mem::size_of::<&str>()
        + std::mem::size_of::<&mut dyn FnMut() -> Result<()>>()
        + std::mem::size_of::<&mut String>()
        + std::mem::size_of::<(usize, usize, usize, usize, usize, usize, usize)>()
        + std::mem::size_of::<(usize, usize)>()
        + std::mem::size_of::<Result<String>>()
        + std::mem::size_of::<Result<(usize, usize)>>();
    let visit = std::mem::size_of::<&str>()
        + std::mem::size_of::<std::str::Chars<'_>>() * 2
        + std::mem::size_of::<&mut dyn FnMut() -> Result<()>>()
        + std::mem::size_of::<&mut dyn FnMut(&str) -> Result<()>>()
        + std::mem::size_of::<(
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
        )>()
        + std::mem::size_of::<std::ops::Range<usize>>()
        + std::mem::size_of::<(u32, &str)>()
        + std::mem::size_of::<char>()
        + std::mem::size_of::<u32>()
        + std::mem::size_of::<Option<&str>>()
        + std::mem::size_of::<&str>()
        + std::mem::size_of::<[u8; 4]>()
        + std::mem::size_of::<Result<()>>()
        + std::mem::size_of::<Result<(usize, usize)>>();
    main + visit
}
fn casefold_visit(
    input: &str,
    max_input: usize,
    max_points: usize,
    max_bytes: usize,
    check: &mut dyn FnMut() -> Result<()>,
    emit: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<(usize, usize)> {
    let (mut input_points, mut points, mut bytes) = (0usize, 0usize, 0usize);
    let mut input_scalars = input.chars();
    loop {
        check()?;
        let Some(ch) = input_scalars.next() else {
            break;
        };
        input_points = input_points
            .checked_add(1)
            .filter(|n| *n <= max_input)
            .ok_or_else(|| budget_error("Unicode input code-point budget exceeded"))?;
        let point = ch as u32;
        let (mut low, mut high, mut mapped) = (0, CASEFOLD.len(), None);
        while low < high {
            check()?;
            let middle = low + (high - low) / 2;
            let (key, value) = CASEFOLD[middle];
            if point < key {
                high = middle;
            } else if point > key {
                low = middle + 1;
            } else {
                mapped = Some(value);
                break;
            }
        }
        let mut scalar = [0u8; 4];
        let piece = match mapped {
            Some(value) => value,
            None => ch.encode_utf8(&mut scalar),
        };
        let mut output_scalars = piece.chars();
        loop {
            check()?;
            let Some(_) = output_scalars.next() else {
                break;
            };
            points = points
                .checked_add(1)
                .filter(|n| *n <= max_points)
                .ok_or_else(|| budget_error("Unicode output code-point budget exceeded"))?;
        }
        bytes = bytes
            .checked_add(piece.len())
            .filter(|n| *n <= max_bytes)
            .ok_or_else(|| budget_error("Unicode output budget exceeded"))?;
        // Copy work is UTF-8 bytes, distinct from mapped-scalar/table work.
        for _ in 0..piece.len() {
            check()?;
        }
        emit(piece)?;
    }
    check()?;
    Ok((points, bytes))
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
    ranges
        .binary_search_by(|(start, end)| {
            if point < *start {
                std::cmp::Ordering::Greater
            } else if point > *end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn is_python_whitespace(ch: char) -> bool {
    matches!(ch,
        '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{0085}' |
        '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' |
        '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// Python Unicode `\d`/`isdecimal` category, using the pinned Unicode 16 data.
/// Numeric letters and superscripts are not decimal digits.
pub fn python_decimal_unicode16_v1(ch: char) -> bool {
    use unicode_general_category::{GeneralCategory, get_general_category};
    matches!(get_general_category(ch), GeneralCategory::DecimalNumber)
}

/// Python 3.14/Unicode 16 Unicode-regex `\w`: all letter and number
/// categories plus ASCII underscore. Combining marks and join controls do not
/// count as word scalars. The caller selects ASCII mode and boundary adjacency.
pub fn python_word_unicode16_v1(ch: char) -> bool {
    use unicode_general_category::{GeneralCategory as Category, get_general_category};
    ch == '_'
        || matches!(
            get_general_category(ch),
            Category::UppercaseLetter
                | Category::LowercaseLetter
                | Category::TitlecaseLetter
                | Category::ModifierLetter
                | Category::OtherLetter
                | Category::DecimalNumber
                | Category::LetterNumber
                | Category::OtherNumber
        )
}

/// Python 3.14/Unicode 16 printable scalar predicate used by string repr.
/// CPython excludes separator and other categories except ASCII space.
pub fn python_printable_unicode16_v1(ch: char) -> bool {
    use unicode_general_category::{GeneralCategory as Category, get_general_category};
    ch == ' '
        || !matches!(
            get_general_category(ch),
            Category::Control
                | Category::Format
                | Category::Surrogate
                | Category::PrivateUse
                | Category::Unassigned
                | Category::LineSeparator
                | Category::ParagraphSeparator
                | Category::SpaceSeparator
        )
}
