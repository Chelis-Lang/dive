//! Fallible inline seen-state admission, separate from Boolean candidate stores.

use std::path::PathBuf;

use dive_store::{
    StoreMode, collision,
    seen::{DiskSeen, InsertOutcome, Key, Limits, Operation, SeenError},
};

use crate::{Collision, Verdict, engine::Outcome};

/// Explicit opt-in transient disk seen-state policy (ADR 0068).
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

impl SeenUsage {
    pub(crate) fn creation_failure(request: Option<&SeenOptions>, error: &SeenError) -> Self {
        Self {
            limits: request.map_or_else(Limits::default, |r| r.limits),
            admitted_keys: 0,
            before_cleanup: None,
            after_cleanup: None,
            first_failure: Some(SeenFailure {
                phase: SeenPhase::Creation,
                error: error.clone(),
                generated: None,
                predecessor: None,
                successor_ordinal: None,
            }),
            deciding_event_before_failure: None,
            replay: None,
        }
    }
}

pub(crate) fn print_replay_refusal() -> Verdict {
    Verdict::Unsupported {
        feature: "disk seen-state invariant-error replay with Print/PrintT output".into(),
    }
}

pub(crate) fn verdict(error: &SeenError) -> Verdict {
    Verdict::ResourceLimit {
        message: format!("disk seen-state storage failed: {error}"),
    }
}

pub(crate) struct Run {
    #[cfg(test)]
    test_parent: PathBuf,
    set: DiskSeen,
    usage: SeenUsage,
}

impl Run {
    pub(crate) fn open(mode: StoreMode, options: &SeenOptions) -> Result<Self, SeenError> {
        let parent = options.directory.clone().unwrap_or_else(std::env::temp_dir);
        let set = DiskSeen::create(mode, &parent, options.limits)?;
        Ok(Self {
            #[cfg(test)]
            test_parent: parent,
            set,
            usage: SeenUsage {
                limits: options.limits,
                admitted_keys: 0,
                before_cleanup: None,
                after_cleanup: None,
                first_failure: None,
                deciding_event_before_failure: None,
                replay: None,
            },
        })
    }

    pub(crate) fn semantic_entries(&self) -> u64 {
        self.usage.admitted_keys
    }

    pub(crate) fn admission_contains(
        &mut self,
        key: Key<'_>,
        generated: u64,
        predecessor: Option<u64>,
        ordinal: u64,
    ) -> Result<bool, SeenError> {
        self.set.contains(key).inspect_err(|error| {
            self.usage.first_failure.get_or_insert(SeenFailure {
                phase: SeenPhase::AdmissionMembership,
                error: error.clone(),
                generated: Some(generated),
                predecessor,
                successor_ordinal: Some(ordinal),
            });
        })
    }

    pub(crate) fn replay_evidence(&self) -> Option<&SeenReplay> {
        self.usage.replay.as_ref()
    }

    pub(crate) fn start_replay(&mut self, evidence: SeenReplay) {
        self.usage.replay = Some(evidence);
    }

    pub(crate) fn finish_replay(&mut self, admitted: u64, queued: u64, error: Option<Verdict>) {
        if let Some(replay) = &mut self.usage.replay {
            replay.admitted_keys = admitted;
            replay.queued = queued;
            replay.halted_by = error;
        }
    }

    pub(crate) fn refuse_print_replay(&mut self, current: &Verdict) -> bool {
        let Some(replay) = &mut self.usage.replay else {
            return false;
        };
        replay.unproved_print_output = true;
        // A prior storage/resource/unsupported stop keeps its first cause.
        if !current.is_verdict() {
            return false;
        }
        replay.halted_by = Some(print_replay_refusal());
        true
    }

    pub(crate) fn replay_contains(
        &mut self,
        key: Key<'_>,
        generated: u64,
        predecessor: u64,
        ordinal: u64,
    ) -> Result<bool, SeenError> {
        #[cfg(test)]
        tests::before_replay(&self.test_parent, ordinal, false);
        self.set.contains(key).inspect_err(|error| {
            self.usage.first_failure.get_or_insert(SeenFailure {
                phase: SeenPhase::ReplayMembership,
                error: error.clone(),
                generated: Some(generated),
                predecessor: Some(predecessor),
                successor_ordinal: Some(ordinal),
            });
        })
    }

    pub(crate) fn replay_insert(
        &mut self,
        key: Key<'_>,
        generated: u64,
        predecessor: u64,
        ordinal: u64,
    ) -> Result<InsertOutcome, SeenError> {
        #[cfg(test)]
        tests::before_replay(&self.test_parent, ordinal, true);
        match self.set.insert(key) {
            Ok(InsertOutcome::Inserted) => {
                self.usage.admitted_keys += 1;
                Ok(InsertOutcome::Inserted)
            }
            Ok(InsertOutcome::Present) => {
                let error = SeenError {
                    operation: Operation::Corruption,
                    message: "disk replay identity changed between positioned query and insertion"
                        .into(),
                    cleanup_failure: None,
                };
                self.usage.first_failure.get_or_insert(SeenFailure {
                    phase: SeenPhase::ReplayInsertion,
                    error: error.clone(),
                    generated: Some(generated),
                    predecessor: Some(predecessor),
                    successor_ordinal: Some(ordinal),
                });
                Err(error)
            }
            Err(error) => {
                self.usage.first_failure.get_or_insert(SeenFailure {
                    phase: SeenPhase::ReplayInsertion,
                    error: error.clone(),
                    generated: Some(generated),
                    predecessor: Some(predecessor),
                    successor_ordinal: Some(ordinal),
                });
                Err(error)
            }
        }
    }

    pub(crate) fn insert(
        &mut self,
        key: Key<'_>,
        generated: u64,
        predecessor: Option<u64>,
        successor_ordinal: u64,
    ) -> Result<bool, SeenError> {
        #[cfg(test)]
        tests::before_insert(&self.test_parent, generated);
        match self.set.insert(key) {
            Ok(InsertOutcome::Present) => Ok(false),
            Ok(InsertOutcome::Inserted) => {
                // The library's u32 entry limit precludes overflow. This count
                // records key admission, not subsequent log/frontier publication.
                self.usage.admitted_keys += 1;
                Ok(true)
            }
            Err(error) => {
                self.usage.first_failure.get_or_insert(SeenFailure {
                    phase: SeenPhase::Insertion,
                    error: error.clone(),
                    generated: Some(generated),
                    predecessor,
                    successor_ordinal: Some(successor_ordinal),
                });
                Err(error)
            }
        }
    }

    pub(crate) fn collision(
        &mut self,
        mode: StoreMode,
        distinct: u64,
        generated: u64,
    ) -> Result<Option<Collision>, SeenError> {
        if mode == StoreMode::Exact {
            return Ok(None);
        }
        let gap = if collision::optimistic_probability(distinct, generated)
            >= collision::TLC_ACTUAL_THRESHOLD
        {
            match self.set.tlc_min_gap(collision::TLC_DEFAULT_FP_BITS) {
                Ok(gap) => gap,
                Err(error) => {
                    self.usage.first_failure.get_or_insert(SeenFailure {
                        phase: SeenPhase::Summary,
                        error: error.clone(),
                        generated: None,
                        predecessor: None,
                        successor_ordinal: None,
                    });
                    return Err(error);
                }
            }
        } else {
            collision::NO_GAP
        };
        let figures = collision::tlc_success_figures(distinct, generated, || gap);
        Ok(Some(Collision {
            optimistic: figures.optimistic,
            actual: figures.actual,
        }))
    }

    fn close(mut self, outcome: &mut Outcome) -> SeenUsage {
        #[cfg(test)]
        tests::before_close(&self.test_parent);
        self.usage.before_cleanup = Some(self.set.stats());
        if let Err(error) = self.set.close() {
            let failure = self.usage.first_failure.get_or_insert_with(|| {
                self.usage.deciding_event_before_failure = Some(outcome.verdict.clone());
                SeenFailure {
                    phase: SeenPhase::Cleanup,
                    error: error.clone(),
                    generated: None,
                    predecessor: None,
                    successor_ordinal: None,
                }
            });
            // A synthetic engine identity failure does not poison DiskSeen.
            // Its later audit/cleanup error must still retain that first cause.
            if failure.error.operation == error.operation && failure.error.message == error.message
            {
                failure.error = error;
            } else {
                failure.error.cleanup_failure = Some(error.to_string());
            }
            outcome.verdict = verdict(&failure.error);
            outcome.collision = None;
            // Preserve the original concrete trace as diagnostic evidence; this
            // resource-limit result must never become full conformance agreement.
            outcome.graph = None;
        }
        self.usage.after_cleanup = Some(self.set.stats());
        self.usage
    }
}

pub(crate) fn finish(disk: &mut Option<Run>, mut outcome: Outcome) -> Outcome {
    if let Some(run) = disk.take() {
        outcome.seen = Some(run.close(&mut outcome));
    }
    outcome
}

#[cfg(test)]
mod tests;
