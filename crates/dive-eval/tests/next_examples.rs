//! The first slice of milestone CM1 (`docs/design/m5-plan.md`): Examples
//! models through the successor generator, the test-only binding and the
//! single-worker search of `harness::model`, under `--compat=tlc`, against
//! the TLC goldens in `conformance/golden/tlc-2026.10.06.014338/examples/`.
//!
//! Verdict, violated property, states generated, distinct states, states
//! left on the queue, depth and counterexample length must equal the
//! golden's. For a model whose configuration checks a temporal property only
//! the counts are compared: the liveness checker is not part of M5, and the
//! search TLC runs before it is the same.
//!
//! The models' files are vendored in `tests/fixtures/examples/` (Examples
//! `6a7b26a`, MIT; see the `LICENSE.md` there), so the test needs no
//! corpus. `examples/next_corpus.rs` runs the whole small tier.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

mod harness;

use std::path::{Path, PathBuf};

use harness::model::{Checked, bfs, bind_model, diff, on_big_stack};

/// The CM1 candidate list. `Paxos/MCVoting` (`SYMMETRY`, refused in M5) is
/// replaced, and `TwoPhase/MCTwoPhase` and `locks_auxiliary_vars/Peterson`
/// (which need the TLAPS library, not vendored here; they agree in the
/// corpus run) are left out; `acp/ACP_NB_WRONG_TLC`, `HourClock2`,
/// `MCInternalMemory`, `Stones` and `ElevatorSafetySmall` are added.
const MODELS: &[&str] = &[
    "specifications/DieHard/DieHard.cfg",
    "specifications/DieHard/MCDieHarder.cfg",
    "specifications/MissionariesAndCannibals/MissionariesAndCannibals.cfg",
    "specifications/N-Queens/Queens.toolbox/FourQueens/MC.cfg",
    "specifications/spanning/MC_spanning.cfg",
    "specifications/tower_of_hanoi/Hanoi.toolbox/Model_1/MC.cfg",
    "specifications/DiningPhilosophers/DiningPhilosophers.cfg",
    "specifications/CigaretteSmokers/CigaretteSmokers.cfg",
    "specifications/CoffeeCan/CoffeeCan100Beans.cfg",
    "specifications/SpecifyingSystems/HourClock/HourClock.cfg",
    "specifications/SpecifyingSystems/HourClock/HourClock2.cfg",
    "specifications/SpecifyingSystems/AsynchronousInterface/AsynchInterface.cfg",
    "specifications/SpecifyingSystems/AsynchronousInterface/Channel.cfg",
    "specifications/transaction_commit/TCommit.cfg",
    "specifications/transaction_commit/TwoPhase.cfg",
    "specifications/acp/ACP_NB_WRONG_TLC.cfg",
    "specifications/echo/MCEcho.cfg",
    "specifications/chang_roberts/MCChangRoberts.cfg",
    "specifications/Moving_Cat_Puzzle/CatEvenBoxes.cfg",
    "specifications/SpecifyingSystems/CachingMemory/MCInternalMemory.cfg",
    "specifications/Stones/Stones.cfg",
    "specifications/MultiCarElevator/ElevatorSafetySmall.cfg",
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(id: &str) -> Result<(Checked, bool, serde_json::Value), String> {
    let golden_path = crate_dir()
        .join("../../conformance/golden/tlc-2026.10.06.014338/examples")
        .join(format!("{id}.json"));
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let root = crate_dir().join("tests/fixtures/examples");
    let module = root.join(golden["model"]["module"].as_str().unwrap_or(""));
    let cfg_path = root.join(id);
    on_big_stack(move || {
        let loaded = harness::load(Path::new(&module), &[])?;
        let text = std::fs::read_to_string(&cfg_path).map_err(|e| e.to_string())?;
        let cfg = dive_config::parse(&text)
            .into_config()
            .ok_or("cfg rejected")?;
        let b = bind_model(&loaded, &cfg, dive_eval::Options::tlc())?;
        if let dive_eval::AssumptionOutcome::Error { error, .. } =
            b.ev.check_assumptions(|d| harness::is_axiom(&loaded, d))
        {
            return Err(format!("assumption: {error}"));
        }
        let liveness = !b.liveness.is_empty();
        Ok((bfs(&b, 1_000_000), liveness, golden))
    })?
}

#[test]
fn cm1_slice_matches_tlc_goldens() {
    let mut failures = Vec::new();
    let mut full = 0;
    for id in MODELS {
        match run(id) {
            Ok((d, liveness, golden)) => match diff(&golden, &d, liveness) {
                None if !liveness => full += 1,
                None => {}
                Some(why) => failures.push(format!("{id}: {why}")),
            },
            Err(e) => failures.push(format!("{id}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    // The rest agree on the counts; their temporal properties need liveness.
    assert_eq!(MODELS.len(), 22);
    assert_eq!(full, 18);
}
