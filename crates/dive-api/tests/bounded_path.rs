//! Exact bounded-path semantics and source-resolved sequence predicates.

use dive_api::bounded::{Report, Request, check_file};
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str, source: &str, cfg: &str) -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("dive-bounded-{name}-{}", std::process::id()));
        std::fs::create_dir(&dir)?;
        let fixture = Self(dir);
        std::fs::write(fixture.0.join("Bounded.tla"), source)?;
        std::fs::write(fixture.0.join("Bounded.cfg"), cfg)?;
        Ok(fixture)
    }

    fn report(&self, bound: usize) -> Result<Report> {
        let file = Path::new(&self.0).join("Bounded.tla");
        Ok(check_file(&Request {
            file,
            path_bound: bound,
            ..Request::default()
        })?)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";

#[test]
fn repeated_states_remain_at_later_bounds() -> Result {
    let fixture = Fixture::new(
        "cyclic",
        r"---- MODULE Bounded ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1 - x
Inv == x \in 0..1
====",
        CFG,
    )?;
    let bound = fixture.report(5)?;
    assert_eq!(
        bound.to_json()["statistics"]["checked_layers"],
        serde_json::json!([1, 1, 1, 1, 1, 1])
    );
    assert_eq!(bound.to_json()["result"], "bounded-safe");
    assert_eq!(bound.to_json()["model_proved"], false);
    assert_eq!(bound.to_json()["path_bound"], 5);
    Ok(())
}

#[test]
fn terminal_prefixes_and_depth_zero_violations_have_distinct_outcomes() -> Result {
    let source = r"---- MODULE Bounded ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x = 0 /\ x' = 1
Inv == x \in 0..1
BadInit == x = 1
====";
    let short = Fixture::new("terminal", source, CFG)?;
    assert_eq!(short.report(1)?.to_json()["result"], "bounded-safe");
    assert_eq!(short.report(2)?.to_json()["result"], "executions-too-short");
    assert_eq!(
        short.report(2)?.to_json()["statistics"]["terminal_sources"],
        1
    );
    let bug = Fixture::new(
        "initial",
        source,
        "INIT Init\nNEXT Next\nINVARIANT BadInit\n",
    )?;
    let report = bug.report(5)?;
    assert_eq!(report.exit_status(), 10);
    assert_eq!(report.to_json()["violation_depth"], 0);
    assert_eq!(report.to_json()["violation_validated"], true);
    Ok(())
}

#[test]
fn sequence_prefix_dispatch_preserves_finite_demand_and_binding() -> Result {
    let source = r#"---- MODULE Bounded ----
EXTENDS Naturals, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = IF /\ IsPrefix(<<>>, <<1>>)
                /\ IsPrefix(<<1>>, <<1>>)
                /\ IsPrefix(<<1>>, <<1, 2>>)
                /\ ~IsPrefix(<<1, 2>>, <<1>>)
                /\ ~IsPrefix(<<0, "not compared">>, <<1, 2>>)
             THEN 0 ELSE 1
Inv == x = 0
===="#;
    let bounded = Fixture::new("prefix", source, CFG)?;
    assert_eq!(bounded.report(3)?.to_json()["result"], "bounded-safe");
    let invalid = source.replace("IsPrefix(<<>>, <<1>>)", "IsPrefix(42, <<1>>)");
    let invalid = Fixture::new("prefix-domain", &invalid, CFG)?.report(1)?;
    assert_eq!(invalid.to_json()["result"], "unsupported");
    assert!(
        invalid.to_json()["message"]
            .as_str()
            .is_some_and(|message| message.contains("IsPrefix"))
    );
    let local = Fixture::new(
        "prefix-shadow",
        r"---- MODULE Bounded ----
EXTENDS Naturals
VARIABLE x
IsPrefix(a, b) == FALSE
Init == x = 0
Next == x' = IF IsPrefix(<<>>, <<1>>) THEN 1 ELSE 0
Inv == x = 0
====",
        CFG,
    )?;
    assert_eq!(local.report(1)?.to_json()["result"], "bounded-safe");
    Ok(())
}
