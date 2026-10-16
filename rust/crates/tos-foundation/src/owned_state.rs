//! Logical owned residency for finite state admission, separate from allocator
//! bookkeeping and the original OS RAM guard. Inline slots are charged by their
//! enclosing owner; each owned buffer is charged at its actual capacity. A
//! borrowed reference or shared handle does not own another copy of its target.
use crate::{FoundationError, FoundationErrorCode, Result};

pub trait OwnedState {
    fn owned_heap_bytes(&self) -> Result<usize>;
    fn retained_state_bytes(&self) -> Result<usize>
    where
        Self: Sized,
    {
        checked_state_add(std::mem::size_of::<Self>(), self.owned_heap_bytes()?)
    }
}
pub fn checked_state_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or_else(|| {
        FoundationError::new(
            FoundationErrorCode::BudgetExceeded,
            "owned state byte overflow",
        )
    })
}
fn slots<T>(capacity: usize) -> Result<usize> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| {
            FoundationError::new(
                FoundationErrorCode::BudgetExceeded,
                "owned state capacity overflow",
            )
        })
}
impl OwnedState for String {
    fn owned_heap_bytes(&self) -> Result<usize> {
        Ok(self.capacity())
    }
}
impl OwnedState for std::path::PathBuf {
    fn owned_heap_bytes(&self) -> Result<usize> {
        Ok(self.capacity())
    }
}
impl<T: OwnedState> OwnedState for Vec<T> {
    fn owned_heap_bytes(&self) -> Result<usize> {
        let mut bytes = slots::<T>(self.capacity())?;
        for value in self {
            bytes = checked_state_add(bytes, value.owned_heap_bytes()?)?;
        }
        Ok(bytes)
    }
}
impl<T: OwnedState> OwnedState for Option<T> {
    fn owned_heap_bytes(&self) -> Result<usize> {
        self.as_ref().map_or(Ok(0), OwnedState::owned_heap_bytes)
    }
}
impl<T: ?Sized> OwnedState for &T {
    fn owned_heap_bytes(&self) -> Result<usize> {
        Ok(0)
    }
}
impl OwnedState for crate::JsonValue {
    fn owned_heap_bytes(&self) -> Result<usize> {
        self.retained_storage_bytes()
    }
}
impl OwnedState for crate::Digest256 {
    fn owned_heap_bytes(&self) -> Result<usize> {
        Ok(0)
    }
}
macro_rules! inline {
    ($($ty:ty),*) => { $(impl OwnedState for $ty {
        fn owned_heap_bytes(&self) -> Result<usize> { Ok(0) }
    })* };
}
inline!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, bool, f32, f64
);

/// Exhaustive field destructuring makes an added owner field a compilation
/// error until its retained-state responsibility is explicitly incorporated.
#[macro_export]
macro_rules! impl_owned_state {
    ($ty:ty {$($field:ident),* $(,)?}) => {
        impl $crate::OwnedState for $ty {
            fn owned_heap_bytes(&self) -> $crate::Result<usize> {
                let Self { $($field),* } = self;
                let mut bytes = 0usize;
                $(bytes = $crate::checked_state_add(bytes,
                    $crate::OwnedState::owned_heap_bytes($field)?)?;)*
                Ok(bytes)
            }
        }
    };
}

impl<T: OwnedState, const N: usize> OwnedState for [T; N] {
    fn owned_heap_bytes(&self) -> Result<usize> {
        let mut bytes = 0;
        for value in self {
            bytes = checked_state_add(bytes, value.owned_heap_bytes()?)?;
        }
        Ok(bytes)
    }
}
impl<A: OwnedState, B: OwnedState> OwnedState for (A, B) {
    fn owned_heap_bytes(&self) -> Result<usize> {
        checked_state_add(self.0.owned_heap_bytes()?, self.1.owned_heap_bytes()?)
    }
}
