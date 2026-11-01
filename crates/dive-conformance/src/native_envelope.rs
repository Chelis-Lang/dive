//! Native check-report envelope admission shared with the PlusCal runner controls.
//!
//! Numeric names and TLC exits follow pinned `tlc2/output/EC.java` at
//! `94d0c5070852b37b801bb8998c436383ab7ee37b`. Verdict associations follow
//! dive's public API/result contract, not a primary model-check observation.
//! This data table keeps compiler/backend crates out of the repository hooks.

use serde_json::Value;

pub(super) type CheckClass = (u64, &'static str, u64, &'static [&'static str]);

const CLASSES: &[CheckClass] = &[
    (1000, "GENERAL", 255, &["evaluation-error", "model-error"]),
    (2102, "TLC_INITIAL_STATE", 255, &["incomplete-state"]),
    (2103, "TLC_NESTED_EXPRESSION", 75, &["evaluation-error"]),
    (2104, "TLC_ASSUMPTION_FALSE", 10, &["assumption-failure"]),
    (
        2105,
        "TLC_ASSUMPTION_EVALUATION_ERROR",
        10,
        &["evaluation-error"],
    ),
    (
        2107,
        "TLC_INVARIANT_VIOLATED_INITIAL",
        12,
        &["invariant-violation"],
    ),
    (
        2108,
        "TLC_PROPERTY_VIOLATED_INITIAL",
        13,
        &["property-violation"],
    ),
    (
        2109,
        "TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT",
        75,
        &["incomplete-state"],
    ),
    (
        2110,
        "TLC_INVARIANT_VIOLATED_BEHAVIOR",
        12,
        &["invariant-violation"],
    ),
    (
        2111,
        "TLC_INVARIANT_EVALUATION_FAILED",
        76,
        &["evaluation-error"],
    ),
    (
        2112,
        "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR",
        13,
        &["action-property-violation"],
    ),
    (
        2113,
        "TLC_ACTION_PROPERTY_EVALUATION_FAILED",
        13,
        &["evaluation-error"],
    ),
    (2114, "TLC_DEADLOCK_REACHED", 11, &["deadlock"]),
    (
        2116,
        "TLC_TEMPORAL_PROPERTY_VIOLATED",
        13,
        &["liveness-violation"],
    ),
    (
        2132,
        "TLC_VALUE_ASSERT_FAILED",
        14,
        &["assertion-failure", "model-error"],
    ),
    (
        2139,
        "TLC_COULD_NOT_DETERMINE_SUBSCRIPT",
        255,
        &["model-error"],
    ),
    (
        2140,
        "TLC_SUBSCRIPT_CONTAIN_NO_STATE_VAR",
        255,
        &["model-error"],
    ),
    (
        2145,
        "TLC_MODULE_TLCGET_UNDEFINED",
        255,
        &["evaluation-error", "model-error"],
    ),
    (2146, "TLC_INVARIANT_VIOLATED_LEVEL", 76, &["model-error"]),
    (
        2148,
        "TLC_STATE_NOT_COMPLETELY_SPECIFIED_LIVE",
        255,
        &["evaluation-error", "model-error"],
    ),
    (2149, "TLC_INVARIANT_CONSTANT_LEVEL", 255, &["model-error"]),
    (
        2154,
        "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2155,
        "TLC_MODULE_COMPARE_VALUE",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2158,
        "TLC_MODULE_CHECK_MEMBER_OF",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2169,
        "TLC_MODULE_ARGUMENT_ERROR",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2170,
        "TLC_ARGUMENT_MISMATCH",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2176,
        "TLC_MODULE_APPLYING_TO_WRONG_VALUE",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2177,
        "TLC_MODULE_BAG_UNION1",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2178,
        "TLC_MODULE_OVERFLOW",
        255,
        &["model-error", "overflow"],
    ),
    (
        2179,
        "TLC_MODULE_DIVISION_BY_ZERO",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2180,
        "TLC_MODULE_NULL_POWER_NULL",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2181,
        "TLC_MODULE_COMPUTING_CARDINALITY",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2182,
        "TLC_MODULE_EVALUATING",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2183,
        "TLC_MODULE_ARGUMENT_NOT_IN_DOMAIN",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2184,
        "TLC_MODULE_APPLY_EMPTY_SEQ",
        255,
        &["evaluation-error", "model-error"],
    ),
    (2193, "TLC_SUCCESS", 0, &["success"]),
    (
        2213,
        "TLC_LIVE_CANNOT_HANDLE_FORMULA",
        255,
        &["model-error"],
    ),
    (
        2215,
        "TLC_EXPECTED_VALUE",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2222,
        "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM",
        151,
        &["model-error"],
    ),
    (
        2223,
        "TLC_CONFIG_RHS_ID_APPEARED_AFTER_LHS_ID",
        151,
        &["model-error"],
    ),
    (2224, "TLC_CONFIG_WRONG_SUBSTITUTION", 151, &["model-error"]),
    (
        2225,
        "TLC_CONFIG_WRONG_SUBSTITUTION_NUMBER_OF_ARGS",
        151,
        &["model-error"],
    ),
    (
        2226,
        "TLC_CONFIG_ID_DOES_NOT_APPEAR_IN_SPEC",
        151,
        &["model-error"],
    ),
    (
        2227,
        "TLC_CONFIG_NOT_BOTH_SPEC_AND_INIT",
        151,
        &["model-error"],
    ),
    (2228, "TLC_CONFIG_ID_REQUIRES_NO_ARG", 151, &["model-error"]),
    (
        2229,
        "TLC_CONFIG_SPECIFIED_NOT_DEFINED",
        151,
        &["model-error"],
    ),
    (2230, "TLC_CONFIG_ID_HAS_VALUE", 151, &["model-error"]),
    (2231, "TLC_CONFIG_MISSING_INIT", 151, &["model-error"]),
    (2232, "TLC_CONFIG_MISSING_NEXT", 151, &["model-error"]),
    (
        2233,
        "TLC_CONFIG_ID_MUST_NOT_BE_CONSTANT",
        151,
        &["model-error"],
    ),
    (2234, "TLC_CONFIG_OP_NO_ARGS", 151, &["model-error"]),
    (2235, "TLC_CONFIG_OP_NOT_IN_SPEC", 151, &["model-error"]),
    (2236, "TLC_CONFIG_OP_IS_EQUAL", 151, &["model-error"]),
    (2237, "TLC_CONFIG_SPEC_IS_TRIVIAL", 151, &["model-error"]),
    (2238, "TLC_CANT_HANDLE_SUBSCRIPT", 151, &["model-error"]),
    (2239, "TLC_CANT_HANDLE_CONJUNCT", 151, &["model-error"]),
    (
        2240,
        "TLC_CANT_HANDLE_TOO_MANY_NEXT_STATE_RELS",
        151,
        &["model-error"],
    ),
    (
        2241,
        "TLC_CONFIG_PROPERTY_NOT_CORRECTLY_DEFINED",
        151,
        &["model-error"],
    ),
    (
        2242,
        "TLC_CONFIG_OP_ARITY_INCONSISTENT",
        151,
        &["model-error"],
    ),
    (
        2244,
        "TLC_CANT_HANDLE_REAL_NUMBERS",
        151,
        &["evaluation-error", "model-error"],
    ),
    (2245, "TLC_NO_MODULES", 151, &["model-error"]),
    (
        2247,
        "TLC_EXPECTED_EXPRESSION_IN_COMPUTING",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2248,
        "TLC_EXPECTED_EXPRESSION_IN_COMPUTING2",
        255,
        &["evaluation-error", "model-error"],
    ),
    (2253, "TLC_LIVE_FORMULA_TAUTOLOGY", 77, &["model-error"]),
    (2255, "TLC_LIVE_FORMULA_STATE_LEVEL", 255, &["model-error"]),
    (
        2260,
        "TLC_ENABLED_WRONG_FORMULA",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2261,
        "TLC_ENCOUNTERED_FORMULA_IN_PREDICATE",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2265,
        "TLC_INTEGER_TOO_BIG",
        255,
        &["model-error", "overflow"],
    ),
    (
        2266,
        "TLC_MODULE_ARGUMENT_ERROR_AN",
        255,
        &["evaluation-error", "model-error"],
    ),
    (
        2272,
        "TLC_CONFIG_PROPERTY_ACTION_LEVEL",
        151,
        &["model-error"],
    ),
    (
        2273,
        "TLC_CONFIG_PROPERTY_ACTION_LEVEL_SQUARE_A_SUB_V",
        151,
        &["model-error"],
    ),
    (
        2274,
        "TLC_CONFIG_PROPERTY_ACTION_LEVEL_ANGLE_A_SUB_V",
        151,
        &["model-error"],
    ),
    (
        2280,
        "TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR",
        151,
        &["evaluation-error", "model-error"],
    ),
    (
        2281,
        "TLC_CONFIG_SUBSTITUTION_NON_CONSTANT",
        151,
        &["model-error"],
    ),
    (
        2283,
        "TLC_MODULE_ONE_ARGUMENT_ERROR",
        255,
        &["evaluation-error", "model-error"],
    ),
    (2300, "TLC_SYMMETRY_SET_TOO_SMALL", 255, &["model-error"]),
    (
        2301,
        "TLC_SPECIFICATION_FEATURES_TEMPORAL_QUANTIFIER",
        255,
        &["model-error"],
    ),
    (
        2404,
        "TLC_POSTCONDITION_FALSE",
        10,
        &["postcondition-failure"],
    ),
    (
        2405,
        "TLC_POSTCONDITION_EVALUATION_ERROR",
        10,
        &["evaluation-error"],
    ),
    (3002, "TLC_PARSING_FAILED", 150, &["rejected"]),
];

pub(super) fn class(code: u64) -> Option<&'static CheckClass> {
    CLASSES.iter().find(|entry| entry.0 == code)
}

// `Verdict::tlc` reports its deciding code except for EvaluationError.
// `class.rs` can preserve the first evaluation error through recurring/replay
// or wrap it as GENERAL, INITIAL_STATE, invariant or action evaluation failure.
// Assumption/postcondition/direct invariant wrappers themselves are not replay
// deciding codes. This validates the native envelope, not the injected outcome.
pub(super) fn reported_matches(verdict: &str, deciding: u64, reported: u64) -> bool {
    if deciding == reported {
        return true;
    }
    if !matches!(
        verdict,
        "evaluation-error" | "assertion-failure" | "overflow"
    ) || !(deciding == 2103 || evaluator_code(deciding))
    {
        return false;
    }
    matches!(reported, 1000 | 2102 | 2111 | 2113) || evaluator_code(reported)
}

// `dive-eval::Ec` codes are the table rows with an evaluation verdict, excluding
// the site wrappers introduced by dive-check's class constructors.
fn evaluator_code(code: u64) -> bool {
    !matches!(code, 2103 | 2105 | 2111 | 2113 | 2405)
        && class(code).is_some_and(|entry| {
            entry.3.iter().any(|verdict| {
                matches!(
                    *verdict,
                    "evaluation-error" | "assertion-failure" | "overflow"
                )
            })
        })
}

// Validate the observed native protocol before comparing it. Equality of
// missing fields is not a model outcome. The default/TLC exit pair comes from
// dive-api's public check contract; no compiler crate enters the hook graph.
pub(super) fn valid_check_report(physical_exit: Option<i32>, report: &Value) -> bool {
    if report["command"] != "check" {
        return false;
    }
    let Some(verdict) = report["verdict"].as_str() else {
        return false;
    };
    if matches!(verdict, "unsupported" | "resource-limit" | "internal-error") {
        return true; // Classified as uncredited below, never as agreement.
    }
    let (native_exit, needs_stats, needs_property) = match verdict {
        "success" => (0, true, false),
        "invariant-violation"
        | "property-violation"
        | "action-property-violation"
        | "liveness-violation" => (10, true, true),
        "deadlock" => (11, true, false),
        "postcondition-failure" => (10, false, true),
        "assumption-failure" => (12, false, false),
        "model-error" | "rejected" => (1, false, false),
        "incomplete-state" | "assertion-failure" | "overflow" | "evaluation-error" => {
            (13, false, false)
        }
        _ => return false,
    };
    let Some(stats) = report.get("statistics") else {
        return false;
    };
    if stats.is_null() {
        if needs_stats {
            return false;
        }
    } else {
        let (Some(distinct), Some(generated), Some(_depth)) = (
            stats["distinct"].as_u64(),
            stats["generated"].as_u64(),
            stats["depth"].as_u64(),
        ) else {
            return false;
        };
        // Every distinct state was generated. Empty Init can legally leave
        // both counters zero; depth remains an observed unsigned value.
        if generated < distinct {
            return false;
        }
    }
    let property = report.get("property");
    if needs_property {
        if property.is_none_or(|p| p.as_str().is_none_or(str::is_empty)) {
            return false;
        }
    } else if property.is_some_and(|p| !p.is_null()) {
        return false;
    }
    let metadata = &report["tlc"];
    let Some(code) = metadata["code"]
        .as_u64()
        .filter(|&c| c >= 1000 && u32::try_from(c).is_ok())
    else {
        return false;
    };
    let Some(tlc_exit) = metadata["exit_status"].as_u64().filter(|&c| c <= 255) else {
        return false;
    };
    let Some(name) = metadata["name"].as_str() else {
        return false;
    };
    let Some((_, expected_name, expected_exit, verdicts)) = class(code) else {
        return false;
    };
    if metadata["verified"] != true
        || !metadata.get("reported").map_or_else(
            // `CheckReport::to_json` omits this field only for frontend SANY
            // rejection; all checked-result classes carry it, even success.
            || verdict == "rejected" && code == 3002,
            |reported| {
                reported
                    .as_u64()
                    .is_some_and(|reported| reported_matches(verdict, code, reported))
            },
        )
        || name != *expected_name
        || tlc_exit != *expected_exit
        || !verdicts.contains(&verdict)
    {
        return false;
    }
    // Assumption evaluation errors use the default assumption exit. A real
    // TLC-profile run instead uses the declared deciding class's exit.
    physical_exit.is_some_and(|exit| {
        exit == if code == 2105 { 12 } else { native_exit }
            || u64::try_from(exit).ok() == Some(tlc_exit)
    })
}
