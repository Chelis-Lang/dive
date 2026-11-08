//! Producer facts independent of retained state and adjacency storage.

use std::time::Duration;

use super::GraphCompletion;
use crate::{CheckOptions, CheckResult, Stats};

/// Actual completed exploration facts, without retaining its behavior graph.
///
/// Only a successful producer with the existing checked [`GraphCompletion`]
/// creates this value. It certifies exploration, never temporal acceptance.
/// Public graph edits, statistics and a zero queue cannot create these facts.
///
/// ```
/// use dive_runtime::capture::{Complete, CompleteEvidence};
/// fn release_graph(captured: Complete) -> CompleteEvidence {
///     let counters = captured.evidence().stats();
///     let evidence = captured.into_evidence();
///     assert_eq!(evidence.stats(), counters);
///     evidence
/// }
/// ```
///
/// ```compile_fail
/// use dive_runtime::capture::CompleteEvidence;
/// let forged = CompleteEvidence { /* public graph shape is insufficient */ };
/// ```
#[derive(Clone, Debug)]
pub struct CompleteEvidence {
    result: Box<CheckResult>,
    options: CheckOptions,
    stats: Stats,
    completion: GraphCompletion,
}

impl CompleteEvidence {
    #[doc(hidden)]
    #[must_use]
    pub fn new(
        result: CheckResult,
        options: CheckOptions,
        stats: Stats,
        completion: GraphCompletion,
    ) -> Self {
        Self {
            result: Box::new(result),
            options,
            stats,
            completion,
        }
    }

    /// The original producer's shape/counter receipt, unchanged by detachment.
    #[must_use]
    pub fn completion(&self) -> &GraphCompletion {
        &self.completion
    }

    /// The producer's complete ordinary safety result, including qualifications,
    /// worker metadata, output, warnings and elapsed time.
    #[must_use]
    pub fn safety_result(&self) -> &CheckResult {
        &self.result
    }

    /// Actual producer counters, independent of later public capture-field edits.
    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }

    /// Original producer settings, including profile, seed, store and run budget.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }

    #[doc(hidden)]
    pub fn set_elapsed(&mut self, elapsed: Duration) {
        self.result.elapsed = elapsed;
    }
}

/// Actual successful committed-prefix facts, explicitly without completion.
///
/// A prefix has an unexhausted frontier. It can support a candidate word that
/// still needs independent validation; it cannot establish a holding verdict.
/// A safety failure or resource stop does not create a successful prefix.
///
/// ```
/// use dive_runtime::capture::{Prefix, PrefixEvidence};
/// fn release_prefix(captured: Prefix) -> PrefixEvidence {
///     let level = captured.level();
///     let evidence = captured.into_evidence();
///     assert_eq!(evidence.level(), level);
///     assert!(evidence.stats().left_on_queue > 0);
///     evidence
/// }
/// ```
///
/// ```compile_fail
/// use dive_runtime::capture::{CompleteEvidence, PrefixEvidence};
/// fn promote(prefix: PrefixEvidence) -> CompleteEvidence { prefix.into() }
/// ```
#[derive(Clone, Debug)]
pub struct PrefixEvidence {
    result: Box<CheckResult>,
    options: CheckOptions,
    stats: Stats,
    level: u64,
}

impl PrefixEvidence {
    #[doc(hidden)]
    #[must_use]
    pub fn new(result: CheckResult, options: CheckOptions, stats: Stats, level: u64) -> Self {
        Self {
            result: Box::new(result),
            options,
            stats,
            level,
        }
    }

    /// Actual successful safety result at the committed boundary. Its nonzero
    /// queue describes unfinished exploration, not safety/resource failure.
    #[must_use]
    pub fn safety_result(&self) -> &CheckResult {
        &self.result
    }

    /// Actual counters at the committed boundary, including the nonzero queue.
    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }

    /// Original producer settings, including profile, seed, store and run budget.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }

    /// Last fully committed expansion level. Exploration remains incomplete.
    #[must_use]
    pub const fn level(&self) -> u64 {
        self.level
    }

    #[doc(hidden)]
    #[must_use]
    pub fn into_result(self) -> Box<CheckResult> {
        self.result
    }
}
