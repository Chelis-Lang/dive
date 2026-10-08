//! TLC message codes (`tlc2.output.EC`) and process exit statuses
//! (`EC.ExitStatus`).
//!
//! [`EC_CODES`] was generated from `tlc2/output/EC.java` at tlaplus/tlaplus
//! `5a47802` (v1.7.4) and `94d0c50` (the default oracle); regenerate it when an
//! oracle pin moves. [`exit_statuses_for`] mirrors
//! `EC.ExitStatus.errorConstantToExitStatus` in both revisions.

/// `(code, symbolic name, present in v1.7.4, present in 94d0c50)`.
pub const EC_CODES: &[(i32, &str, bool, bool)] = &[
    (1000, "GENERAL", true, true),
    (1001, "SYSTEM_OUT_OF_MEMORY", true, true),
    (1002, "SYSTEM_OUT_OF_MEMORY_TOO_MANY_INIT", true, true),
    (1003, "SYSTEM_OUT_OF_MEMORY_LIVENESS", true, true),
    (1005, "SYSTEM_STACK_OVERFLOW", true, true),
    (1101, "WRONG_COMMANDLINE_PARAMS_SIMULATOR", true, true),
    (1102, "WRONG_COMMANDLINE_PARAMS_TLC", true, true),
    (2000, "TLC_PP_PARSING_VALUE", true, true),
    (2001, "TLC_PP_FORMATING_VALUE", true, true),
    (2100, "TLC_METADIR_EXISTS", true, true),
    (2101, "TLC_METADIR_CAN_NOT_BE_CREATED", true, true),
    (2102, "TLC_INITIAL_STATE", true, true),
    (2103, "TLC_NESTED_EXPRESSION", true, true),
    (2104, "TLC_ASSUMPTION_FALSE", true, true),
    (2105, "TLC_ASSUMPTION_EVALUATION_ERROR", true, true),
    (
        2106,
        "TLC_STATE_NOT_COMPLETELY_SPECIFIED_INITIAL",
        true,
        true,
    ),
    (2107, "TLC_INVARIANT_VIOLATED_INITIAL", true, true),
    (2108, "TLC_PROPERTY_VIOLATED_INITIAL", true, true),
    (2109, "TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT", true, true),
    (2110, "TLC_INVARIANT_VIOLATED_BEHAVIOR", true, true),
    (2111, "TLC_INVARIANT_EVALUATION_FAILED", true, true),
    (2112, "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR", true, true),
    (2113, "TLC_ACTION_PROPERTY_EVALUATION_FAILED", true, true),
    (2114, "TLC_DEADLOCK_REACHED", true, true),
    (2115, "TLC_STATES_AND_NO_NEXT_ACTION", true, true),
    (2116, "TLC_TEMPORAL_PROPERTY_VIOLATED", true, true),
    (2117, "TLC_FAILED_TO_RECOVER_NEXT", true, true),
    (2118, "TLC_NO_STATES_SATISFYING_INIT", true, true),
    (2119, "TLC_STRING_MODULE_NOT_FOUND", true, true),
    (2120, "TLC_ERROR_STATE", true, true),
    (2121, "TLC_BEHAVIOR_UP_TO_THIS_POINT", true, true),
    (2122, "TLC_BACK_TO_STATE", true, true),
    (2123, "TLC_FAILED_TO_RECOVER_INIT", true, true),
    (2124, "TLC_REPORTER_DIED", true, true),
    (2125, "SYSTEM_ERROR_READING_POOL", true, true),
    (2126, "SYSTEM_CHECKPOINT_RECOVERY_CORRUPT", true, true),
    (2127, "SYSTEM_ERROR_WRITING_POOL", true, true),
    (2128, "TLC_BUG", true, true),
    (2129, "SYSTEM_DISKGRAPH_ACCESS", true, true),
    (2130, "TLC_AAAAAAA", true, true),
    (2131, "TLC_REGISTRY_INIT_ERROR", true, true),
    (2132, "TLC_VALUE_ASSERT_FAILED", true, true),
    (2133, "TLC_FP_NOT_IN_SET", true, true),
    (2134, "SYSTEM_INDEX_ERROR", true, true),
    (2135, "SYSTEM_STREAM_EMPTY", true, true),
    (2136, "TLC_PARAMETER_MUST_BE_POSTFIX", true, true),
    (2137, "SYSTEM_FILE_NULL", true, true),
    (2138, "SYSTEM_INTERRUPTED", true, true),
    (2139, "TLC_COULD_NOT_DETERMINE_SUBSCRIPT", true, true),
    (2140, "TLC_SUBSCRIPT_CONTAIN_NO_STATE_VAR", true, true),
    (2141, "TLC_WRONG_TUPLE_FIELD_NAME", true, true),
    (2142, "TLC_WRONG_RECORD_FIELD_NAME", true, true),
    (2143, "TLC_UNCHANGED_VARIABLE_CHANGED", true, true),
    (2144, "TLC_EXCEPT_APPLIED_TO_UNKNOWN_FIELD", true, true),
    (2145, "TLC_MODULE_TLCGET_UNDEFINED", true, true),
    (2146, "TLC_INVARIANT_VIOLATED_LEVEL", true, true),
    (2147, "TLC_FINGERPRINT_EXCEPTION", true, true),
    (2148, "TLC_STATE_NOT_COMPLETELY_SPECIFIED_LIVE", false, true),
    (2149, "TLC_INVARIANT_CONSTANT_LEVEL", false, true),
    (2154, "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE", true, true),
    (2155, "TLC_MODULE_COMPARE_VALUE", true, true),
    (2156, "TLC_FEATURE_UNSUPPORTED", true, true),
    (2157, "TLC_MODULE_TRANSITIVE_CLOSURE", true, true),
    (2158, "TLC_MODULE_CHECK_MEMBER_OF", true, true),
    (2159, "TLC_LIVE_BEGRAPH_FAILED_TO_CONSTRUCT", true, true),
    (2160, "SYSTEM_UNABLE_NOT_RENAME_FILE", true, true),
    (2161, "SYSTEM_DISK_IO_ERROR_FOR_FILE", true, true),
    (2162, "SYSTEM_METADIR_EXISTS", true, true),
    (2163, "SYSTEM_METADIR_CREATION_ERROR", true, true),
    (2164, "TLC_CHOOSE_ARGUMENTS_WRONG", true, true),
    (2165, "TLC_CHOOSE_UPPER_BOUND", true, true),
    (2166, "TLC_FP_VALUE_ALREADY_ON_DISK", true, true),
    (2167, "SYSTEM_UNABLE_TO_OPEN_FILE", true, true),
    (
        2168,
        "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE_LOADED",
        true,
        true,
    ),
    (2169, "TLC_MODULE_ARGUMENT_ERROR", true, true),
    (2170, "TLC_ARGUMENT_MISMATCH", true, true),
    (2171, "TLC_PARSING_FAILED2", true, true),
    (2172, "TLC_TOO_MNY_POSSIBLE_STATES", true, true),
    (2173, "TLC_ERROR_REPLACING_MODULES", true, true),
    (2174, "SYSTEM_ERROR_READING_STATES", true, true),
    (2175, "SYSTEM_ERROR_WRITING_STATES", true, true),
    (2176, "TLC_MODULE_APPLYING_TO_WRONG_VALUE", true, true),
    (2177, "TLC_MODULE_BAG_UNION1", true, true),
    (2178, "TLC_MODULE_OVERFLOW", true, true),
    (2179, "TLC_MODULE_DIVISION_BY_ZERO", true, true),
    (2180, "TLC_MODULE_NULL_POWER_NULL", true, true),
    (2181, "TLC_MODULE_COMPUTING_CARDINALITY", true, true),
    (2182, "TLC_MODULE_EVALUATING", true, true),
    (2183, "TLC_MODULE_ARGUMENT_NOT_IN_DOMAIN", true, true),
    (2184, "TLC_MODULE_APPLY_EMPTY_SEQ", true, true),
    (2185, "TLC_STARTING", true, true),
    (2186, "TLC_FINISHED", true, true),
    (2187, "TLC_MODE_MC", true, true),
    (2188, "TLC_MODE_SIMU", true, true),
    (2189, "TLC_COMPUTING_INIT", true, true),
    (2190, "TLC_INIT_GENERATED1", true, true),
    (2191, "TLC_INIT_GENERATED2", true, true),
    (2192, "TLC_CHECKING_TEMPORAL_PROPS", true, true),
    (2193, "TLC_SUCCESS", true, true),
    (2194, "TLC_SEARCH_DEPTH", true, true),
    (2195, "TLC_CHECKPOINT_START", true, true),
    (2196, "TLC_CHECKPOINT_END", true, true),
    (2197, "TLC_CHECKPOINT_RECOVER_START", true, true),
    (2198, "TLC_CHECKPOINT_RECOVER_END", true, true),
    (2199, "TLC_STATS", true, true),
    (2200, "TLC_PROGRESS_STATS", true, true),
    (2201, "TLC_COVERAGE_START", true, true),
    (2202, "TLC_COVERAGE_END", true, true),
    (2203, "TLC_CHECKPOINT_RECOVER_END_DFID", true, true),
    (2204, "TLC_STATS_DFID", true, true),
    (2205, "TLC_PROGRESS_START_STATS_DFID", true, true),
    (2206, "TLC_PROGRESS_STATS_DFID", true, true),
    (2207, "TLC_INIT_GENERATED3", true, true),
    (2208, "TLC_INIT_GENERATED4", true, true),
    (2209, "TLC_PROGRESS_SIMU", true, true),
    (2210, "TLC_STATS_SIMU", true, true),
    (2211, "TLC_FP_COMPLETED", true, true),
    (2212, "TLC_LIVE_IMPLIED", true, true),
    (2213, "TLC_LIVE_CANNOT_HANDLE_FORMULA", true, true),
    (2214, "TLC_LIVE_WRONG_FORMULA_FORMAT", true, true),
    (2215, "TLC_EXPECTED_VALUE", true, true),
    (2216, "TLC_STATE_PRINT1", true, true),
    (2217, "TLC_STATE_PRINT2", true, true),
    (2218, "TLC_STATE_PRINT3", true, true),
    (2219, "TLC_SANY_END", true, true),
    (2220, "TLC_SANY_START", true, true),
    (2221, "TLC_COVERAGE_VALUE", true, true),
    (
        2222,
        "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM",
        true,
        true,
    ),
    (2223, "TLC_CONFIG_RHS_ID_APPEARED_AFTER_LHS_ID", true, true),
    (2224, "TLC_CONFIG_WRONG_SUBSTITUTION", true, true),
    (
        2225,
        "TLC_CONFIG_WRONG_SUBSTITUTION_NUMBER_OF_ARGS",
        true,
        true,
    ),
    (2226, "TLC_CONFIG_ID_DOES_NOT_APPEAR_IN_SPEC", true, true),
    (2227, "TLC_CONFIG_NOT_BOTH_SPEC_AND_INIT", true, true),
    (2228, "TLC_CONFIG_ID_REQUIRES_NO_ARG", true, true),
    (2229, "TLC_CONFIG_SPECIFIED_NOT_DEFINED", true, true),
    (2230, "TLC_CONFIG_ID_HAS_VALUE", true, true),
    (2231, "TLC_CONFIG_MISSING_INIT", true, true),
    (2232, "TLC_CONFIG_MISSING_NEXT", true, true),
    (2233, "TLC_CONFIG_ID_MUST_NOT_BE_CONSTANT", true, true),
    (2234, "TLC_CONFIG_OP_NO_ARGS", true, true),
    (2235, "TLC_CONFIG_OP_NOT_IN_SPEC", true, true),
    (2236, "TLC_CONFIG_OP_IS_EQUAL", true, true),
    (2237, "TLC_CONFIG_SPEC_IS_TRIVIAL", true, true),
    (2238, "TLC_CANT_HANDLE_SUBSCRIPT", true, true),
    (2239, "TLC_CANT_HANDLE_CONJUNCT", true, true),
    (2240, "TLC_CANT_HANDLE_TOO_MANY_NEXT_STATE_RELS", true, true),
    (
        2241,
        "TLC_CONFIG_PROPERTY_NOT_CORRECTLY_DEFINED",
        true,
        true,
    ),
    (2242, "TLC_CONFIG_OP_ARITY_INCONSISTENT", true, true),
    (2243, "TLC_CONFIG_NO_STATE_TYPE", true, true),
    (2244, "TLC_CANT_HANDLE_REAL_NUMBERS", true, true),
    (2245, "TLC_NO_MODULES", true, true),
    (2246, "TLC_EXPECTED_EXPRESSION", true, true),
    (2247, "TLC_EXPECTED_EXPRESSION_IN_COMPUTING", true, true),
    (2248, "TLC_EXPECTED_EXPRESSION_IN_COMPUTING2", true, true),
    (2249, "TLC_LIVE_ENCOUNTERED_ACTIONS", true, true),
    (2250, "TLC_LIVE_STATE_PREDICATE_NON_BOOL", true, true),
    (2251, "TLC_LIVE_CANNOT_EVAL_FORMULA", true, true),
    (2252, "TLC_LIVE_ENCOUNTERED_NONBOOL_PREDICATE", true, true),
    (2253, "TLC_LIVE_FORMULA_TAUTOLOGY", true, true),
    (2254, "SYSTEM_FINGERPRINT_OVERFLOW_ERROR", false, true),
    (2255, "TLC_LIVE_FORMULA_STATE_LEVEL", false, true),
    (
        2256,
        "TLC_NO_STATES_SATISFYING_INIT_AND_CONSTRAINT",
        false,
        true,
    ),
    (2257, "TLC_CONFIG_NO_SPEC_BUT_PROPERTY", false, true),
    (2258, "TLC_LIVE_FORMULA_AND_FAIRNESS_TAUTOLOGY", false, true),
    (
        2259,
        "TLC_CONFIG_NO_FAIRNESS_BUT_LIVE_PROPERTY",
        false,
        true,
    ),
    (2260, "TLC_ENABLED_WRONG_FORMULA", true, true),
    (2261, "TLC_ENCOUNTERED_FORMULA_IN_PREDICATE", true, true),
    (2262, "TLC_VERSION", true, true),
    (2263, "TLC_LIVE_IMPLIED_DEBUG", false, true),
    (2264, "TLC_COUNTER_EXAMPLE", true, true),
    (2265, "TLC_INTEGER_TOO_BIG", true, true),
    (2266, "TLC_MODULE_ARGUMENT_ERROR_AN", true, true),
    (2267, "TLC_CHECKING_TEMPORAL_PROPS_END", true, true),
    (2268, "TLC_STATE_GRAPH_OUTDEGREE", true, true),
    (2269, "TLC_COMPUTING_INIT_PROGRESS", true, true),
    (2270, "SYSTEM_ERROR_CLEANING_POOL", true, true),
    (2271, "TLC_MODE_MC_DFS", true, true),
    (2272, "TLC_CONFIG_PROPERTY_ACTION_LEVEL", false, true),
    (
        2273,
        "TLC_CONFIG_PROPERTY_ACTION_LEVEL_SQUARE_A_SUB_V",
        false,
        true,
    ),
    (
        2274,
        "TLC_CONFIG_PROPERTY_ACTION_LEVEL_ANGLE_A_SUB_V",
        false,
        true,
    ),
    (
        2279,
        "TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY",
        true,
        true,
    ),
    (2280, "TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR", true, true),
    (2281, "TLC_CONFIG_SUBSTITUTION_NON_CONSTANT", true, true),
    (2282, "TLC_TRACE_TOO_LONG", true, true),
    (2283, "TLC_MODULE_ONE_ARGUMENT_ERROR", true, true),
    (2284, "TLC_FEATURE_LIVENESS_CONSTRAINTS", true, true),
    (2300, "TLC_SYMMETRY_SET_TOO_SMALL", true, true),
    (
        2301,
        "TLC_SPECIFICATION_FEATURES_TEMPORAL_QUANTIFIER",
        true,
        true,
    ),
    (
        2400,
        "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE_MISMATCH",
        true,
        true,
    ),
    (2401, "TLC_ENVIRONMENT_JVM_GC", true, true),
    (
        2402,
        "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE_MODULE_MISMATCH",
        true,
        true,
    ),
    (
        2403,
        "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE_IDENTIFIER_MISMATCH",
        true,
        true,
    ),
    (2404, "TLC_POSTCONDITION_FALSE", false, true),
    (2405, "TLC_POSTCONDITION_EVALUATION_ERROR", false, true),
    (2501, "TLC_TE_SPEC_GENERATION_COMPLETE", false, true),
    (2502, "TLC_TE_SPEC_GENERATION_ERROR", false, true),
    (2772, "TLC_COVERAGE_NEXT", true, true),
    (2773, "TLC_COVERAGE_INIT", true, true),
    (2774, "TLC_COVERAGE_PROPERTY", true, true),
    (2775, "TLC_COVERAGE_VALUE_COST", true, true),
    (2776, "TLC_COVERAGE_MISMATCH", true, true),
    (2777, "TLC_COVERAGE_END_OVERHEAD", true, true),
    (2778, "TLC_COVERAGE_CONSTRAINT", false, true),
    (2779, "TLC_COVERAGE_VAR", false, true),
    (2780, "TLC_POSSIBLE_UNWITNESSED", false, true),
    (3000, "CHECK_FAILED_TO_CHECK", true, true),
    (3001, "CHECK_COULD_NOT_READ_TRACE", true, true),
    (3002, "TLC_PARSING_FAILED", true, true),
    (3100, "CHECK_PARAM_EXPECT_CONFIG_FILENAME", true, true),
    (3101, "CHECK_PARAM_USAGE", true, true),
    (3102, "CHECK_PARAM_MISSING_TLA_MODULE", true, true),
    (3103, "CHECK_PARAM_NEED_TO_SPECIFY_CONFIG_DIR", true, true),
    (3104, "CHECK_PARAM_WORKER_NUMBER_REQUIRED", true, true),
    (3105, "CHECK_PARAM_WORKER_NUMBER_TOO_SMALL", true, true),
    (3106, "CHECK_PARAM_WORKER_NUMBER_REQUIRED2", true, true),
    (3107, "CHECK_PARAM_DEPTH_REQUIRED", true, true),
    (3108, "CHECK_PARAM_DEPTH_REQUIRED2", true, true),
    (3109, "CHECK_PARAM_TRACE_REQUIRED", true, true),
    (3110, "CHECK_PARAM_COVREAGE_REQUIRED", true, true),
    (3111, "CHECK_PARAM_COVREAGE_REQUIRED2", true, true),
    (3112, "CHECK_PARAM_COVREAGE_TOO_SMALL", true, true),
    (3113, "CHECK_PARAM_UNRECOGNIZED", true, true),
    (3114, "CHECK_PARAM_TOO_MANY_INPUT_FILES", true, true),
    (4000, "SANY_PARSER_CHECK_1", true, true),
    (4001, "SANY_PARSER_CHECK_2", true, true),
    (4002, "SANY_PARSER_CHECK_3", true, true),
    (5001, "CFG_ERROR_READING_FILE", true, true),
    (5002, "CFG_GENERAL", true, true),
    (5003, "CFG_MISSING_ID", true, true),
    (5004, "CFG_TWICE_KEYWORD", true, true),
    (5005, "CFG_EXPECT_ID", true, true),
    (5006, "CFG_EXPECTED_SYMBOL", true, true),
    (5007, "CFG_LEXICAL_ERROR", false, true),
    (7000, "TLC_DISTRIBUTED_SERVER_RUNNING", true, true),
    (7001, "TLC_DISTRIBUTED_WORKER_REGISTERED", true, true),
    (7002, "TLC_DISTRIBUTED_WORKER_DEREGISTERED", true, true),
    (7003, "TLC_DISTRIBUTED_WORKER_STATS", true, true),
    (7004, "TLC_DISTRIBUTED_SERVER_NOT_RUNNING", true, true),
    (7005, "TLC_DISTRIBUTED_VM_VERSION", true, true),
    (7006, "TLC_DISTRIBUTED_WORKER_LOST", true, true),
    (7007, "TLC_DISTRIBUTED_EXCEED_BLOCKSIZE", true, true),
    (7008, "TLC_DISTRIBUTED_SERVER_FPSET_WAITING", true, true),
    (7009, "TLC_DISTRIBUTED_SERVER_FPSET_REGISTERED", true, true),
    (7010, "TLC_DISTRIBUTED_SERVER_FINISHED", true, true),
    (20000, "TLC_MODULE_OVERRIDE_STDOUT", true, true),
];

use super::result::Verdict;

/// Every exit status TLC defines in `EC.ExitStatus`, with its name.
pub const EXIT_STATUSES: &[(i32, &str)] = &[
    (0, "SUCCESS"),
    (10, "VIOLATION_ASSUMPTION"),
    (11, "VIOLATION_DEADLOCK"),
    (12, "VIOLATION_SAFETY"),
    (13, "VIOLATION_LIVENESS"),
    (14, "VIOLATION_ASSERT"),
    (75, "FAILURE_SPEC_EVAL"),
    (76, "FAILURE_SAFETY_EVAL"),
    (77, "FAILURE_LIVENESS_EVAL"),
    (150, "ERROR_SPEC_PARSE"),
    (151, "ERROR_CONFIG_PARSE"),
    (152, "ERROR_STATESPACE_TOO_LARGE"),
    (153, "ERROR_SYSTEM"),
    (255, "ERROR"),
];

/// The symbolic name of message code `code`, if either pinned revision
/// declares it.
#[must_use]
pub fn ec_name(code: i32) -> Option<&'static str> {
    EC_CODES
        .iter()
        .find(|(c, ..)| *c == code)
        .map(|(_, name, ..)| *name)
}

/// The `EC.ExitStatus` name of exit code `exit`, if TLC defines it.
#[must_use]
pub fn exit_status_name(exit: i32) -> Option<&'static str> {
    EXIT_STATUSES
        .iter()
        .find(|(c, _)| *c == exit)
        .map(|(_, n)| *n)
}

/// Configuration-file error codes that `errorConstantToExitStatus` maps to
/// `ERROR_CONFIG_PARSE` (151) in both revisions (2272–2274 exist only in
/// `94d0c50`).
const CONFIG_CODES: &[i32] = &[
    2222, 2223, 2224, 2225, 2226, 2227, 2228, 2229, 2230, 2231, 2232, 2233, 2234, 2235, 2236, 2237,
    2238, 2239, 2240, 2241, 2242, 2243, 2244, 2245, 2272, 2273, 2274, 2280, 2281,
];

/// `CFG_*` codes: 151 in `94d0c50`, but absent from the v1.7.4 switch, which
/// therefore maps them to 255.
const CFG_CODES: &[i32] = &[5001, 5002, 5003, 5004, 5005, 5006, 5007];

/// The exit statuses TLC can produce when `code` is the error it returns,
/// mirroring `EC.ExitStatus.errorConstantToExitStatus` in both pinned
/// revisions (their union where they differ). Codes outside the switch map
/// to `ERROR` (255).
#[must_use]
pub fn exit_statuses_for(code: i32) -> &'static [i32] {
    match code {
        2253 => &[77],
        2103 | 2109 | 2115 | 2147 => &[75],
        2111 | 2146 => &[76],
        2107 | 2110 => &[12],
        2108 | 2112 | 2113 | 2116 => &[13],
        2114 => &[11],
        2104 | 2105 | 2404 | 2405 | 2780 => &[10],
        2132 => &[14],
        2171 | 3002 => &[150],
        c if CFG_CODES.contains(&c) => &[151, 255],
        c if CONFIG_CODES.contains(&c) => &[151],
        _ => &[255],
    }
}

/// Codes that accompany a primary error (the trace header, the nested
/// expression stack). They decide the verdict only when no other candidate
/// explains the exit status.
pub const CONTEXT_CODES: &[i32] = &[2103, 2121, 2264];

/// Parse failures, which need SANY's untagged text to tell a syntax error
/// from a semantic one.
pub const PARSE_FAILURE_CODES: &[i32] = &[2171, 3002];

/// The verdict class a primary error code denotes. Returns `None` for codes
/// in [`PARSE_FAILURE_CODES`] (decided from text) and for codes that neither
/// pinned revision declares (unclassified).
#[must_use]
pub fn verdict_for_code(code: i32) -> Option<Verdict> {
    let v = match code {
        2193 => Verdict::Success,
        2104 | 2105 => Verdict::AssumptionFailure,
        2114 => Verdict::Deadlock,
        // 2108: a PROPERTY that is a state predicate is violated by an
        // initial state. TLC exits 13 (as for liveness), but the
        // counterexample is one initial state: a safety-style violation.
        2107 | 2108 | 2110 => Verdict::SafetyViolation,
        2112 => Verdict::ActionPropertyViolation,
        2116 => Verdict::LivenessViolation,
        2132 => Verdict::AssertionFailure,
        2178 | 2265 => Verdict::Overflow,
        // Evaluation failures: of the spec, invariants, action properties and
        // liveness formulas, plus TLC's module-level evaluation errors.
        2103
        | 2109
        | 2111
        | 2113
        | 2115
        | 2120
        | 2141..=2147
        | 2155
        | 2157
        | 2158
        | 2164
        | 2165
        | 2169
        | 2170
        | 2176
        | 2177
        | 2179..=2184
        | 2215
        | 2246..=2253
        | 2260
        | 2261
        | 2266
        | 2283 => Verdict::EvaluationError,
        c if PARSE_FAILURE_CODES.contains(&c) => return None,
        c if CONFIG_CODES.contains(&c) || CFG_CODES.contains(&c) || c == 2257 || c == 2259 => {
            Verdict::ConfigError
        }
        c if ec_name(c).is_some() => Verdict::OtherError,
        _ => return None,
    };
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        assert!(EC_CODES.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn names_and_exits() {
        assert_eq!(ec_name(2110), Some("TLC_INVARIANT_VIOLATED_BEHAVIOR"));
        assert_eq!(ec_name(2404), Some("TLC_POSTCONDITION_FALSE"));
        assert_eq!(ec_name(9999), None);
        assert_eq!(exit_statuses_for(2110), &[12]);
        assert_eq!(exit_statuses_for(2178), &[255]);
        assert_eq!(exit_statuses_for(5002), &[151, 255]);
        assert_eq!(exit_status_name(13), Some("VIOLATION_LIVENESS"));
        assert_eq!(exit_status_name(3), None);
    }

    #[test]
    fn verdict_mapping() {
        assert_eq!(
            verdict_for_code(2112),
            Some(Verdict::ActionPropertyViolation)
        );
        assert_eq!(verdict_for_code(2178), Some(Verdict::Overflow));
        assert_eq!(verdict_for_code(2147), Some(Verdict::EvaluationError));
        assert_eq!(verdict_for_code(2222), Some(Verdict::ConfigError));
        assert_eq!(verdict_for_code(1001), Some(Verdict::OtherError));
        assert_eq!(verdict_for_code(3002), None);
        assert_eq!(verdict_for_code(4242), None);
    }
}
