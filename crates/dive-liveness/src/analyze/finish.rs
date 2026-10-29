//! Result assembly independent of retained behavior graph storage.
//!
//! The reference adapters keep the existing public graph-owning results. A
//! later checked compact producer can consume the same result assembly with
//! graph-independent producer evidence; no graph reconstruction occurs here.

use dive_check::capture::{self, CompleteEvidence, PrefixEvidence};

use super::periodic::PrefixCandidate;
use super::{Analysis, AnalysisError, Candidate, CandidateWord};

/// A final decision paired with actual completed exploration, never a prefix.
pub(super) enum Completed {
    Holds(CompleteEvidence),
    Candidate(CandidateWord, CompleteEvidence),
}

/// A selected word paired with a real committed prefix, never completion.
pub(super) struct Prefix {
    word: CandidateWord,
    exploration: PrefixEvidence,
}

pub(super) fn complete(
    word: Result<Option<CandidateWord>, AnalysisError>,
    exploration: CompleteEvidence,
) -> Result<Completed, AnalysisError> {
    match word? {
        Some(word) => Ok(Completed::Candidate(word, exploration)),
        None => Ok(Completed::Holds(exploration)),
    }
}

pub(super) fn prefix(
    word: Result<CandidateWord, AnalysisError>,
    exploration: PrefixEvidence,
) -> Result<Prefix, AnalysisError> {
    Ok(Prefix {
        word: word?,
        exploration,
    })
}

impl Completed {
    /// The legacy adapter retains the original graph object and its public
    /// fields. Its original sealed facts already contain the neutral result's
    /// producer evidence; only that redundant metadata copy is released.
    pub(super) fn reference(self, captured: capture::Complete) -> Analysis {
        match self {
            Self::Holds(exploration) => {
                drop(exploration);
                Analysis::Holds(captured)
            }
            Self::Candidate(word, exploration) => {
                drop(exploration);
                Analysis::Candidate(Candidate {
                    target_property: word.target_property,
                    properties: word.properties,
                    states: word.states,
                    back_to: word.back_to,
                    edges: word.edges,
                    exploration: captured,
                })
            }
        }
    }
}

impl Prefix {
    /// Retains the original public prefix and its graph without copying rows.
    pub(super) fn reference(self, captured: capture::Prefix) -> PrefixCandidate {
        let Self { word, exploration } = self;
        drop(exploration);
        PrefixCandidate {
            target_property: word.target_property,
            properties: word.properties,
            states: word.states,
            back_to: word.back_to,
            edges: word.edges,
            exploration: captured,
        }
    }
}

#[cfg(test)]
mod tests;
