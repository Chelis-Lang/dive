//! `dive-api`: the embeddable library API and `cargo test` integration.
//!
//! Implemented so far: parsing ([`parse_source`], [`parse_file`]) with a
//! versioned JSON report ([`ParseReport::to_json`]); reading TLC model
//! configurations ([`parse_config_file`], [`ConfigReport`]); and
//! SANY-equivalent semantic analysis ([`analyze_file`], [`analyze_source`])
//! with its own report ([`AnalyzeReport::to_json`]); safety model checking
//! ([`check_file`], [`CheckReport`]); and trace validation against a
//! specification ([`trace`], ADR 0016); and bounded final temporal checking
//! for Rust callers ([`temporal`]); and finite seeded safety simulation
//! ([`simulate_file`]). Everything else in the
//! charter is still to come. Permitted internal dependencies are listed in
//! `xtask/src/layering.rs`.
//!
//! # Example
//!
//! ```
//! let report = dive_api::parse_source("Spec.tla", "---- MODULE Spec ----\nx == 1\n====\n");
//! assert!(report.accepted());
//! assert_eq!(report.modules(), ["Spec"]);
//! let json = report.to_json(false);
//! assert_eq!(json["schema_version"], dive_api::PARSE_SCHEMA_VERSION);
//! ```
#![forbid(unsafe_code)]

mod config;

pub mod temporal;
pub mod trace;

pub use config::{ConfigReport, parse_config_file, parse_config_source};

mod analyze;
mod check;
mod simulate;
pub use simulate::{SIMULATE_SCHEMA_VERSION, SimulateReport, SimulateRequest, simulate_file};
