use crate::budget::{DomainError, Result};
use std::cmp::Ordering;

/// Exact normalized bounded rational. No binary float, saturation or rounding.
/// i128::MIN is outside this explicitly symmetric finite implementation profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Rational { numerator: i128, denominator: i128 }
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 { let r = a % b; a = b; b = r; } a
}
fn mul(a: i128, b: i128) -> Result<i128> { a.checked_mul(b).ok_or(DomainError::NumericRange) }
impl Rational {
    pub fn new(n: i128, d: i128) -> Result<Self> {
        if n == i128::MIN || d == i128::MIN { return Err(DomainError::NumericRange); }
        if d == 0 { return Err(DomainError::Invalid("zero rational denominator")); }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        let g = gcd(n.unsigned_abs(), d as u128) as i128;
        Ok(Self { numerator: n / g, denominator: d / g })
    }
    pub fn integer(n: i128) -> Result<Self> { Self::new(n, 1) }
    pub fn numerator(self) -> i128 { self.numerator }
    pub fn denominator(self) -> i128 { self.denominator }
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        let g = gcd(self.denominator as u128, rhs.denominator as u128) as i128;
        let a = mul(self.numerator, rhs.denominator / g)?;
        let b = mul(rhs.numerator, self.denominator / g)?;
        Self::new(a.checked_add(b).ok_or(DomainError::NumericRange)?,
                  mul(self.denominator, rhs.denominator / g)?)
    }
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        self.checked_add(Self::new(-rhs.numerator, rhs.denominator)?)
    }
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        let g = gcd(self.numerator.unsigned_abs(), rhs.denominator as u128) as i128;
        let h = gcd(rhs.numerator.unsigned_abs(), self.denominator as u128) as i128;
        Self::new(mul(self.numerator / g, rhs.numerator / h)?,
                  mul(self.denominator / h, rhs.denominator / g)?)
    }
    pub fn checked_div(self, rhs: Self) -> Result<Self> {
        if rhs.numerator == 0 { return Err(DomainError::Invalid("division by zero")); }
        self.checked_mul(Self::new(rhs.denominator, rhs.numerator)?)
    }
}
// Continued-fraction comparison avoids overflowing cross products. Every loop
// is a Euclidean step on fixed-width unsigned values; no allocation is used.
fn positive_cmp(mut a: u128, mut b: u128, mut c: u128, mut d: u128) -> Ordering {
    let mut reverse = false;
    loop {
        let order = (a / b).cmp(&(c / d));
        if order != Ordering::Equal { return if reverse { order.reverse() } else { order }; }
        let (r, s) = (a % b, c % d);
        if r == 0 || s == 0 {
            let order = r.cmp(&s);
            return if reverse { order.reverse() } else { order };
        }
        (a, b, c, d) = (b, r, d, s); reverse = !reverse;
    }
}
impl Ord for Rational {
    fn cmp(&self, rhs: &Self) -> Ordering {
        match self.numerator.signum().cmp(&rhs.numerator.signum()) {
            Ordering::Equal => {
                let o = positive_cmp(self.numerator.unsigned_abs(), self.denominator as u128,
                                     rhs.numerator.unsigned_abs(), rhs.denominator as u128);
                if self.numerator < 0 { o.reverse() } else { o }
            }
            other => other,
        }
    }
}
impl PartialOrd for Rational {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> { Some(self.cmp(rhs)) }
}
