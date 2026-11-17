//! Result assembly independent of retained behavior graph storage.
//!
//! The reference adapters keep the existing public graph-owning results. A
//! later checked compact producer can consume the same result assembly with
//! graph-independent producer evidence; no graph reconstruction occurs here.

use dive_runtime::capture::{self, CompleteEvidence, PrefixEvidence};

use super::periodic::PrefixCandidate;
use super::{Analysis, AnalysisError, Candidate, CandidateWord};

/// An owned candidate word, requiring fresh independent validation.
///
/// Constructing or editing a word establishes no temporal verdict. Its edge
/// ordinals and labels describe the original graph occurrences.
#[derive(Clone, Debug)]
pub struct NativeWord {
    /// Property whose negation selected the candidate.
    pub target_property: String,
    /// Complete attributed false-property set on this word.
    pub properties: Vec<String>,
    /// Concrete word positions, without repeating the closing target.
    pub states: Vec<dive_eval::next::StateVals>,
    /// Zero-based target of the final transition.
    pub back_to: usize,
    /// Original transitions, including the closing edge and stutters.
    pub edges: Vec<capture::Edge>,
}

/// A final decision paired with actual completed exploration, never a prefix.
/// A candidate still requires independent validation.
#[derive(Clone, Debug)]
pub enum Completed {
    /// No accepting word was found after completed exploration and analysis.
    Holds(CompleteEvidence),
    /// Owned candidate and actual completed producer facts.
    Candidate(NativeWord, CompleteEvidence),
}

/// A selected word paired with a real committed prefix, never completion.
#[derive(Clone, Debug)]
pub struct Prefix {
    /// Owned candidate, requiring fresh independent validation.
    pub word: NativeWord,
    /// Actual incomplete producer facts, never completion.
    pub exploration: PrefixEvidence,
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
    /// The compatibility adapter retains the original graph object and its public
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
