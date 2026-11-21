//! A prefix proves a CHOOSE witness without claiming an exhausted powerset.
#[path = "support/core_final.rs"]
mod core_final;

use dive_runtime::CheckOptions;

const SOURCE: &str = r"---- MODULE WideChoice ----
EXTENDS Naturals, FiniteSets
VARIABLES x, n
Init == x = {} /\ n = 26
Next == x' = (CHOOSE s \in SUBSET (1..n) : s = {n}) /\ n' = n
Inv == x = {} \/ x = {n}
====";
const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";

#[test]
fn choice_finds_the_last_singleton_in_either_profile() {
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let result = core_final::run(SOURCE, CFG, &opts).to_json();
        assert_eq!(result["verdict"], "success", "{result}");
        assert_eq!(result["statistics"]["distinct"], 2);
    }
}

#[test]
fn choice_requiring_a_larger_subset_remains_open() {
    let source = SOURCE.replace("s = {n}", "s = {1,n}");
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let result = core_final::run(&source, CFG, &opts).to_json();
        assert_eq!(result["verdict"], "unsupported", "{result}");
        assert!(
            result["message"]
                .as_str()
                .is_some_and(|s| s.contains("beyond the empty set and singletons"))
        );
    }
}
