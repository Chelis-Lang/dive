//! Fallible inline seen-state admission, separate from Boolean candidate stores.

#[cfg(test)]
use std::path::PathBuf;

use dive_store::{
    StoreMode, collision,
    seen::{DiskSeen, InsertOutcome, Key, Operation, SeenError},
};

use crate::{Collision, Verdict, engine::Outcome};

pub use dive_runtime::usage::{SeenFailure, SeenOptions, SeenPhase, SeenReplay, SeenUsage};

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
