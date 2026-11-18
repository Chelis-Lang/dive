//! Storage usage evidence and the disk seen-state policy carried by
//! [`crate::CheckOptions`] and [`crate::CheckResult`]. The stores
//! themselves belong to the engines.

use std::path::PathBuf;

use dive_store::seen::{Limits, SeenError};

use crate::Verdict;

/// Storage evidence for the frontier only; this does not bound process memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrontierUsage {
    /// Encoded frontier resident byte budget, independently for each of the
    /// current and next queues. `None` keeps both queues in memory.
    pub memory_bytes_per_queue: Option<usize>,
    /// Encoded record workspace bound, when spill is enabled.
    pub max_record_bytes: Option<usize>,
    /// Peak combined resident encoded data/index capacities of live queues.
    pub peak_retained_bytes: u64,
    /// Resident encoded bytes after explicit queue cleanup (normally zero).
    pub retained_bytes: u64,
    /// Confirmed complete writes including transient headers and indices.
    /// A failed filesystem operation marks accounting incomplete; partial writes
    /// may have left additional bytes and are never reported as exact.
    pub spilled_bytes: u64,
    /// Deciding search event before filesystem failure invalidated certification.
    pub deciding_event_before_failure: Option<String>,
    /// Filesystem failures, separately from any deciding safety event.
    pub failures: Vec<String>,
}

/// Explicit opt-in transient disk seen-state policy.
/// Default checking retains the original resident store. These bounds cover
/// on-disk membership and key workspaces, not total search/process memory.
#[derive(Clone, Debug, Default)]
pub struct SeenOptions {
    /// Full key/entry/probe/collision-summary admission limits. Zero entries or
    /// probes are malformed policies; zero summary pairs is a valid refusal bound.
    pub limits: Limits,
    /// Parent for a private owned directory. Omitted uses system temporary storage.
    /// Caller files are never overwritten or recursively removed.
    pub directory: Option<PathBuf>,
}

/// Ordered engine phase of the first observed storage failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SeenPhase {
    /// Before generating Init; binding and assumptions may already have run.
    Creation,
    /// At one generated state's original key insertion position.
    Insertion,
    /// Membership at an explicit search-admission ceiling, before insertion.
    AdmissionMembership,
    /// A replay membership query at the original source/successor ordinal.
    ReplayMembership,
    /// A new key inserted after the original invariant-evaluation stop.
    ReplayInsertion,
    /// After safety EOF, before postconditions; required collision metadata failed.
    Summary,
    /// Explicit audit/removal after search and witness reconstruction.
    Cleanup,
}

/// First positioned disk failure, never absence or a duplicate-state result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenFailure {
    /// Ordered engine phase.
    pub phase: SeenPhase,
    /// First typed library or engine protocol cause, including later cleanup failure.
    pub error: SeenError,
    /// Generated-state counter at insertion; not a physical record count.
    pub generated: Option<u64>,
    /// Concrete predecessor ID at insertion, absent for Init.
    pub predecessor: Option<u64>,
    /// Original emitted successor ordinal, absent outside insertion.
    pub successor_ordinal: Option<u64>,
}

/// Disk retention and cleanup facts, independent of full model certification.
#[derive(Clone, Debug)]
pub struct SeenUsage {
    /// Resolved policy.
    pub limits: Limits,
    /// Successful key admissions at their original ordered positions. A later
    /// predecessor/frontier failure can mean these are not published discoveries.
    pub admitted_keys: u64,
    /// Physical facts before explicit cleanup; expected bytes exclude partial
    /// writes/staging after failure, and inserts have no crash-durability contract.
    pub before_cleanup: Option<dive_store::seen::Stats>,
    /// Facts after explicit cleanup, even when the first error is still returned.
    pub after_cleanup: Option<dive_store::seen::Stats>,
    /// First storage failure and position; subsequent cleanup never replaces it.
    pub first_failure: Option<SeenFailure>,
    /// Outcome before a later audit/cleanup failure prevented completion. This
    /// is diagnostic evidence, not the final model verdict.
    pub deciding_event_before_failure: Option<Verdict>,
    /// Original stop and ordered replay facts; no generated-counter advance,
    /// frontier publication or completed witness is implied by key admission.
    pub replay: Option<SeenReplay>,
}

/// Positioned invariant-error replay evidence for an inline disk search.
#[derive(Clone, Debug)]
pub struct SeenReplay {
    /// Original invariant evaluation error, retained even after a storage error.
    pub original_error: Verdict,
    /// Original concrete predecessor identifier.
    pub predecessor: u64,
    /// Original stopping successor ordinal.
    pub stopped_successor_ordinal: u64,
    /// Frozen generated count; replay does not count generated successors.
    pub generated: u64,
    /// Keys admitted during replay, separate from predecessor/frontier writes.
    pub admitted_keys: u64,
    /// TLC's logical replay queue contribution; no native queue is published.
    pub queued: u64,
    /// Actual Print/PrintT execution has an unproved repeated-output contract.
    /// Original counts and error remain diagnostic evidence only.
    pub unproved_print_output: bool,
    /// Further error/refusal, when replay did not finish normally.
    pub halted_by: Option<Verdict>,
}
