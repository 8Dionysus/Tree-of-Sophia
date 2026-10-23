use tos_foundation::ExactRecordRef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    ResourceLimit,
    NumericRange,
    Invalid(&'static str),
    NeedsBinding(&'static str),
    Incompatible(&'static str),
    Unsupported(&'static str),
}
impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DomainError {}
pub(crate) type Result<T> = std::result::Result<T, DomainError>;

/// Per-call cumulative work/requested-payload admission. Existing input memory,
/// allocator excess capacity/overhead/RSS and external resolver work are excluded.
/// Fixed-width rational comparison has a constant bound of 256 loop iterations
/// (at most 1024 division/remainder operations), independent of input list size.
#[derive(Clone, Copy, Debug)]
pub struct DomainLimits {
    pub max_work: usize,
    pub max_requested_bytes: usize,
    pub max_items: usize,
}
pub(crate) struct Budget {
    limits: DomainLimits,
    work: usize,
    bytes: usize,
    items: usize,
}
fn charge(used: &mut usize, amount: usize, cap: usize) -> Result<()> {
    let next = used.checked_add(amount).ok_or(DomainError::ResourceLimit)?;
    if next > cap {
        return Err(DomainError::ResourceLimit);
    }
    *used = next;
    Ok(())
}
impl Budget {
    pub fn new(limits: DomainLimits) -> Result<Self> {
        if limits.max_work == 0 || limits.max_requested_bytes == 0 || limits.max_items == 0 {
            return Err(DomainError::ResourceLimit);
        }
        Ok(Self {
            limits,
            work: 0,
            bytes: 0,
            items: 0,
        })
    }
    pub fn work(&mut self, n: usize) -> Result<()> {
        charge(&mut self.work, n, self.limits.max_work)
    }
    pub fn item(&mut self) -> Result<()> {
        charge(&mut self.items, 1, self.limits.max_items)
    }
    pub fn reserve<T>(&mut self, n: usize) -> Result<()> {
        let size = n
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(DomainError::ResourceLimit)?;
        charge(&mut self.bytes, size, self.limits.max_requested_bytes)
    }
    pub fn exact(&mut self, r: &ExactRecordRef) -> Result<()> {
        self.work(
            r.id.as_str()
                .len()
                .checked_add(40)
                .ok_or(DomainError::ResourceLimit)?,
        )
    }
    pub fn eq_ref(&mut self, a: &ExactRecordRef, b: &ExactRecordRef) -> Result<bool> {
        self.exact(a)?;
        self.exact(b)?;
        Ok(a == b)
    }
}
pub(crate) fn push<T>(budget: &mut Budget, into: &mut Vec<T>, value: T) -> Result<()> {
    budget.item()?;
    // Charge every requested logical element, including reused allocator capacity.
    // Excess allocator capacity is explicitly outside this logical payload cap.
    budget.reserve::<T>(1)?;
    if into.len() == into.capacity() {
        // A reallocation may copy all existing elements; charge that work too.
        budget.work(
            into.len()
                .checked_add(1)
                .ok_or(DomainError::ResourceLimit)?,
        )?;
        into.try_reserve_exact(1)
            .map_err(|_| DomainError::ResourceLimit)?;
    }
    into.push(value);
    Ok(())
}
