//! Checked integer ratios with deterministic ties-to-even rounding.

/// Scale a rational number and round to the nearest integer, with ties to even.
pub fn checked_scaled_ratio_round_even(
    numerator: u128,
    denominator: u128,
    scale: u64,
) -> std::result::Result<u64, &'static str> {
    if denominator == 0 {
        return Err("zero recurrence denominator");
    }
    let n = numerator
        .checked_mul(scale as u128)
        .ok_or("recurrence scale overflow")?;
    let (q, r) = (n / denominator, n % denominator);
    let increment = r > denominator - r || (r == denominator - r && q % 2 == 1);
    u64::try_from(q + u128::from(increment)).map_err(|_| "recurrence rounded value overflow")
}
