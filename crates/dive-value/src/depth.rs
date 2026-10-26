//! A nesting limit for the recursive algorithms.
//!
//! Dropping, `==`, [`Value::cmp_with`](crate::Value::cmp_with), hashing and
//! fingerprinting are iterative and handle any nesting. TLC's checked operations
//! ([`crate::tlc`]), printing and JSON recurse. They refuse values nested more than
//! [`MAX_DEPTH`] levels deep with [`ErrorKind::DiveLimit`] rather than overflow the
//! stack. TLC itself is limited only by the JVM stack (`-Xss`).

use std::cell::Cell;

use crate::error::{ErrorKind, Result, ValueError};

/// The deepest nesting the recursive algorithms accept: safe on a 2 MiB thread stack
/// even in debug builds (measured margin: about 1.5x).
pub const MAX_DEPTH: u32 = 500;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Current checked-operation nesting on this thread, without entering a level.
/// This snapshot changes no depth policy and grants no authority to skip work.
#[must_use]
pub fn checked_operation_depth() -> u32 {
    DEPTH.with(Cell::get)
}

/// One level of recursion; released on drop.
pub(crate) struct Guard(());

impl Guard {
    /// Enters one level.
    pub(crate) fn enter() -> Result<Self> {
        let d = DEPTH.with(|d| {
            let v = d.get() + 1;
            d.set(v);
            v
        });
        let guard = Self(());
        if d > MAX_DEPTH {
            return Err(too_deep());
        }
        Ok(guard)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

pub(crate) fn too_deep() -> ValueError {
    ValueError::new(
        ErrorKind::DiveLimit,
        format!(
            "The value is nested more than {MAX_DEPTH} levels deep, more than dive's checked operations, printing and JSON support."
        ),
    )
}
