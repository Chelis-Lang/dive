//! Differential controls for binding selection when a source guard is known.

use std::fs;
use std::process::Command;

use serde_json::Value;

const SOURCE: &str = r#"---- MODULE GuardedBindings ----
EXTENDS Integers
VARIABLES status, x
Actors == 1..34
Init == /\ x = 0
        /\ status = [i \in 1..34 |-> IF i = 1 THEN "On" ELSE "Off"]
Advance(i, j) == /\ status[i] = "On"
                 /\ 1 \div (i - 34) \in -1..1
                 /\ status' = [status EXCEPT ![i] = "Off", ![3 - i] = "On"]
                 /\ x' = 1 - x
Next == \E i,j \in Actors: Advance(i,j)
Inv == x \in {0,1}
====
"#;

#[test]
fn visits_only_possible_bindings_and_preserves_order_and_multiplicity() -> Result<(), String> {
    let scratch =
        std::env::temp_dir().join(format!("dive-guarded-bindings-{}", std::process::id()));
    fs::create_dir(&scratch).map_err(|e| e.to_string())?;
    let module = scratch.join("GuardedBindings.tla");
    let config = scratch.join("GuardedBindings.cfg");
    fs::write(
        &config,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )
    .map_err(|e| e.to_string())?;
    let run = |enabled, workers| -> Result<Value, String> {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_dive"));
        cmd.arg("check")
            .arg(&module)
            .arg("--config")
            .arg(&config)
            .args([
                "--workers",
                workers,
                "--store",
                "exact",
                "--seed",
                "0",
                "--fp",
                "0",
                "--output",
                "json",
            ]);
        cmd.env(
            "DIVE_CORE_OPT",
            if enabled {
                "+action-index"
            } else {
                "-action-index"
            },
        );
        let result = cmd.output().map_err(|e| e.to_string())?;
        serde_json::from_slice(&result.stdout)
            .map_err(|e| format!("{e}: {}", String::from_utf8_lossy(&result.stderr)))
    };
    for workers in ["1", "4"] {
        fs::write(&module, SOURCE).map_err(|e| e.to_string())?;
        let index = run(true, workers)?;
        let ordinary = run(false, workers)?;
        assert_eq!(index["verdict"], "success", "{index}");
        assert_eq!(index["statistics"], ordinary["statistics"]);
        assert_eq!(index["statistics"]["generated"], 69);
        let work = &index["core"]["metrics"]["action_schedule"];
        assert_eq!(work["slots"], 2312);
        assert_eq!(work["candidates"], 68);
        assert_eq!(work["shape"], serde_json::json!([1156, 34]));
        // Lose one real function key. Attaching a decision to that projection
        // would hide the source's demanded out-of-domain error.
        fs::write(&module, SOURCE.replace("[i \\in 1..34", "[i \\in 1..33"))
            .map_err(|e| e.to_string())?;
        let index = run(true, workers)?;
        let ordinary = run(false, workers)?;
        assert_eq!(index["exit_code"], ordinary["exit_code"]);
        assert_eq!(index["message"], ordinary["message"]);
        assert_eq!(index["statistics"], ordinary["statistics"]);
        assert_ne!(index["verdict"], "success", "{index}");
    }
    fs::remove_dir_all(scratch).map_err(|e| e.to_string())
}
