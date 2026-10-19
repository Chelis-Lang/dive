//! `dive-conformance`: the differential harness against pinned TLC and SANY.
//!
//! It fetches and verifies the pinned oracles and corpora, generates golden
//! results from TLC and SANY, runs an implementation (TLC itself, `dive`, or
//! an external tool) against those goldens, and emits per-model reports.
//! See `crates/dive-conformance/README.md` and ADRs 0002–0005.
//!
//! The harness never guesses. Every TLC run is classified from its `-tool`
//! message stream and cross-checked against its exit code; anything the
//! harness cannot classify is reported as `unclassified` or `oracle-error`
//! with the raw data, never folded into a verdict.
//!
//! ```
//! use dive_conformance::tlc::{messages, result};
//!
//! let stdout = "@!@!@STARTMSG 2110:1 @!@!@\nInvariant Inv is violated.\n@!@!@ENDMSG 2110 @!@!@\n";
//! let r = result::classify(
//!     &messages::parse(stdout),
//!     &result::RunFacts { exit_code: Some(12), ..Default::default() },
//! );
//! assert_eq!(r.verdict, result::Verdict::SafetyViolation);
//! assert_eq!(r.violated_property.as_deref(), Some("Inv"));
//! ```
#![forbid(unsafe_code)]

pub mod baseline;
pub mod cache;
pub mod compare;
pub mod config;
pub mod corpus;
pub mod dive;
pub mod divergence;
pub mod error;
pub mod fetch;
pub mod golden;
pub mod hash;
pub mod implementation;
pub mod oracle;
pub mod pipeline;
pub mod process;
pub mod report;
pub mod sany;
pub mod tlc;
pub mod trace_check;
pub mod upstream;
pub mod util;

pub use error::{Error, Result};

/// Non-gating observations of bounded final/short temporal checking.
pub mod final_study;
