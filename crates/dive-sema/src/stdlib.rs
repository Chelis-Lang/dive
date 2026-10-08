//! The standard modules, embedded.
//!
//! These are the `.tla` files the pinned `tla2tools.jar` ships under
//! `tla2sany/StandardModules/` (tlaplus `94d0c50`), byte for byte. They are
//! MIT-licensed; see `stdlib/README.md` and `stdlib/LICENSE`. ADR 0011
//! records why they live here.

/// `(module name, source text)` for every standard module.
pub const STANDARD_MODULES: &[(&str, &str)] = &[
    ("Bags", include_str!("../stdlib/Bags.tla")),
    ("FiniteSets", include_str!("../stdlib/FiniteSets.tla")),
    ("Integers", include_str!("../stdlib/Integers.tla")),
    ("Json", include_str!("../stdlib/Json.tla")),
    ("Naturals", include_str!("../stdlib/Naturals.tla")),
    ("Randomization", include_str!("../stdlib/Randomization.tla")),
    ("RealTime", include_str!("../stdlib/RealTime.tla")),
    ("Reals", include_str!("../stdlib/Reals.tla")),
    ("Sequences", include_str!("../stdlib/Sequences.tla")),
    ("TLC", include_str!("../stdlib/TLC.tla")),
    ("TLCExt", include_str!("../stdlib/TLCExt.tla")),
    ("Toolbox", include_str!("../stdlib/Toolbox.tla")),
    ("_DotTrace", include_str!("../stdlib/_DotTrace.tla")),
    ("_JsonTrace", include_str!("../stdlib/_JsonTrace.tla")),
    ("_Possible", include_str!("../stdlib/_Possible.tla")),
    (
        "_TLAPlusCounterExample",
        include_str!("../stdlib/_TLAPlusCounterExample.tla"),
    ),
    (
        "_TLCActionTrace",
        include_str!("../stdlib/_TLCActionTrace.tla"),
    ),
    ("_TLCTESpec", include_str!("../stdlib/_TLCTESpec.tla")),
    ("_TLCTrace", include_str!("../stdlib/_TLCTrace.tla")),
    (
        "_TLCTracePlain",
        include_str!("../stdlib/_TLCTracePlain.tla"),
    ),
];

/// The source of the standard module `name`, if there is one.
#[must_use]
pub fn standard_module(name: &str) -> Option<&'static str> {
    STANDARD_MODULES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_standard_module_parses_and_is_named_after_its_file() {
        for (name, src) in STANDARD_MODULES {
            let p = dive_syntax::parse_module(src);
            assert!(p.is_ok(), "{name}: {:?}", p.diagnostics());
            assert!(
                p.check_file_name(&format!("{name}.tla")).is_none(),
                "{name}"
            );
        }
        assert_eq!(STANDARD_MODULES.len(), 20);
    }
}
