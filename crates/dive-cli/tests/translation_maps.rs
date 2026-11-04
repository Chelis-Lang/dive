//! Actual translation transport retains emitter-recorded source regions.
use std::process::Command;

#[test]
fn optimized_translation_reports_a_usable_generated_range_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("dive-cli-maps-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("Mapped.tla");
    let source = "---- MODULE Mapped ----\nEXTENDS Integers,TLC\n(* --algorithm A variable x=0; begin Loop: while TRUE do x:=1-x; end while; end algorithm; *)\n====\n";
    std::fs::write(&file, source)?;
    for compat in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
        command
            .arg("translate")
            .arg(&file)
            .args(["--output", "json"]);
        if compat {
            command.arg("--compat=tlc");
        }
        let output = command.output()?;
        assert!(output.status.success(), "{output:?}");
        let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(document["accepted"], true);
        let artifact = &document["artifact"];
        let mappings = artifact["mappings"].as_array().ok_or("source mappings")?;
        assert_eq!(mappings.len(), 1);
        let offset = |side: &str, endpoint: &str| -> Result<usize, Box<dyn std::error::Error>> {
            Ok(usize::try_from(
                mappings[0][side][endpoint].as_u64().ok_or("byte offset")?,
            )?)
        };
        let tla = artifact["tla"].as_str().ok_or("TLA body")?;
        let definition = tla
            .get(offset("generated", "start")?..offset("generated", "end")?)
            .ok_or("definition")?;
        assert_eq!(definition, "Next == x' = 1-x");
        assert_eq!(
            source.get(offset("original", "start")?..offset("original", "end")?),
            Some("Loop: while TRUE do x:=1-x; end while;")
        );
        assert_eq!(std::fs::read_to_string(&file)?, source);
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
