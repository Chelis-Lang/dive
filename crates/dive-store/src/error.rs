//! Store errors.

use std::fmt;

use dive_value::ValueError;

/// An error from state storage.
///
/// Apart from [`StoreError::Predecessor`] (native I/O/allocation/corruption) and
/// [`StoreError::Value`], which carries the value layer's TLC-format error
/// (a state that cannot be fingerprinted or encoded, as in TLC), every variant is an
/// internal invariant violation: a bug in the caller or in `dive-store`, reported with
/// enough context to reproduce it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StoreError {
    /// Native predecessor storage failed; never a model verdict.
    Predecessor {
        /// The operation that failed.
        operation: String,
        /// Underlying I/O, corruption or allocation detail.
        message: String,
    },
    /// A value could not be fingerprinted, encoded or decoded.
    Value(ValueError),
    /// Two candidates in one resolve had the same key and the same position. Positions
    /// must be unique within a level for the merge to be deterministic (ADR 0015 §2).
    DuplicatePosition {
        /// The shard in which the duplicate was found.
        shard: usize,
    },
    /// A candidate buffer built for a set with another number of shards.
    ShardMismatch {
        /// The set's shard count.
        expected: usize,
        /// The buffer's shard count.
        found: usize,
    },
    /// A trace-log parent that is not an earlier state.
    BadParent {
        /// The state being recorded.
        id: u64,
        /// Its claimed parent.
        parent: u64,
    },
    /// A state id that the trace log does not contain.
    UnknownState(u64),
    /// A frontier state larger than a frontier chunk can hold (1 GiB).
    StateTooLarge(usize),
    /// A state fingerprinted with a variable order over another number of
    /// variables.
    VarCount {
        /// The variable order's length.
        expected: usize,
        /// The state's number of values.
        found: usize,
    },
}

impl StoreError {
    /// The stable dive error code (`DIVE-Snnnn`).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Predecessor { .. } => "DIVE-S0300",
            Self::Value(_) => "DIVE-S0001",
            Self::DuplicatePosition { .. } => "DIVE-S0100",
            Self::ShardMismatch { .. } => "DIVE-S0104",
            Self::BadParent { .. } => "DIVE-S0101",
            Self::UnknownState(_) => "DIVE-S0102",
            Self::StateTooLarge(_) => "DIVE-S0103",
            Self::VarCount { .. } => "DIVE-S0105",
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Predecessor { operation, message } => {
                write!(f, "{}: predecessor {operation}: {message}", self.code())
            }
            Self::Value(e) => write!(f, "{e}"),
            Self::DuplicatePosition { shard } => write!(
                f,
                "{}: internal error: two candidates share a key and a position (shard {shard})",
                self.code()
            ),
            Self::ShardMismatch { expected, found } => write!(
                f,
                "{}: internal error: a candidate buffer for {found} shards was resolved into a set of {expected}",
                self.code()
            ),
            Self::BadParent { id, parent } => write!(
                f,
                "{}: internal error: state {id} recorded with parent {parent}, which is not an earlier state",
                self.code()
            ),
            Self::UnknownState(id) => write!(
                f,
                "{}: internal error: state {id} is not in the trace log",
                self.code()
            ),
            Self::StateTooLarge(n) => write!(
                f,
                "{}: internal error: an encoded state of {n} bytes exceeds the frontier chunk size",
                self.code()
            ),
            Self::VarCount { expected, found } => write!(
                f,
                "{}: internal error: a state of {found} values fingerprinted with a variable order over {expected}",
                self.code()
            ),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<ValueError> for StoreError {
    fn from(e: ValueError) -> Self {
        Self::Value(e)
    }
}
