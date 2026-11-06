//! WP-C4: TLC's error classes and error paths, against the pinned TLC.
//!
//! `tests/fixtures/errors/specs/` holds a matrix of minimal specifications,
//! one per error site × error kind (`ErInvDivZero` is a division by zero in
//! an invariant), plus cells for TLC behaviour found while building it (the
//! `ErWrap*` cells for TLC's replay of value errors, `ErInvReplay*` for what
//! the replay does after an invariant fails to evaluate, `ErPost*` for
//! postconditions). Each has a transcript captured with
//! `dive-conformance dev capture` under `tests/fixtures/errors/<oracle>/`;
//! the README there lists what each cell establishes.
//!
//! For every cell, `dive check` under `--compat=tlc` must reproduce TLC's
//! exit status, the code and text of the first error message TLC prints,
//! the class the conformance harness reads from the run (the first message
//! whose code explains the exit status), the counts at the stop (or their
//! absence), and the counterexample, state by state, at 1 and 4 workers in
//! both stores. The default profile must give the same result except where
//! a registered divergence applies (32-bit overflow, D-004).
//!
//! Cells dive does not reproduce, or whose class it does not claim, are
//! listed in [`UNCLAIMED`], [`REFUSED`] and [`ALIAS`] with the reason; the
//! test checks they stay that way, so that a change in either direction is
//! noticed.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Verdict, check, format_trace_state, tlc_exit_status};
use dive_store::StoreMode;

const ORACLE: &str = "tlc-2026.10.06.014338";

/// Cells where dive reports TLC's error, counts and trace but does not claim
/// TLC's deciding class (`TlcClass::verified` is false), with the reason.
const UNCLAIMED: &[(&str, &str)] = &[
    (
        "ErFpTuple",
        "tuple index out of range: TLC's replay reports TLC_FINGERPRINT_EXCEPTION when the tuple was built by an expression it re-evaluates",
    ),
    (
        "ErWrapTupleGuard",
        "tuple index out of range on a tuple literal (TLC_FINGERPRINT_EXCEPTION in TLC's replay)",
    ),
    (
        "ErWrapTupleOp",
        "tuple index out of range on a tuple literal (TLC_FINGERPRINT_EXCEPTION in TLC's replay)",
    ),
    (
        "ErWrapTupleConst",
        "tuple index out of range on a precomputed constant (TLC_NESTED_EXPRESSION): the class depends on where the tuple was built",
    ),
    (
        "ErWrapStateTuple",
        "tuple index out of range on a state's tuple (TLC_NESTED_EXPRESSION): the class depends on where the tuple was built",
    ),
    (
        "ErWrapRecord",
        "missing field of a record literal (TLC_FINGERPRINT_EXCEPTION in TLC's replay)",
    ),
    (
        "ErWrapSetMember",
        "comparison error inside membership of a set literal (TLC_FINGERPRINT_EXCEPTION in TLC's replay)",
    ),
    (
        "ErWrapConstrTuple",
        "tuple index out of range in a constraint (TLC_FINGERPRINT_EXCEPTION in TLC's replay)",
    ),
    (
        "ErWrapInitTuple",
        "tuple index out of range in the initial predicate (TLC_FINGERPRINT_EXCEPTION in TLC's replay)",
    ),
    (
        "ErFpInfinite",
        "a state holding Nat (TLC_NESTED_EXPRESSION): a set built by an expression TLC re-evaluates would be TLC_FINGERPRINT_EXCEPTION",
    ),
];

/// Cells dive refuses (`unsupported`, naming the feature), with the reason.
const REFUSED: &[(&str, &str)] = &[(
    "ErFpLambda",
    "a function over Nat stored in a state; TLC itself fails with TLC_BUG (exit 1)",
)];

/// Cells with an `ALIAS`: dive does not apply `ALIAS` to printed
/// counterexamples (it prints the states themselves, and says so), so the
/// trace's text differs; TLC turns an error in the alias into a
/// `_ALIASEvalError` field of the printed state and keeps its verdict.
const ALIAS: &[&str] = &[
    "ErAliasAssert",
    "ErAliasOverflow",
    "ErAliasChoose",
    "ErAliasDomain",
    "ErAliasDivZero",
    "ErAliasNonBool",
    "ErAliasNonBoolIf",
    "ErAliasEnabled",
    "ErAliasNonEnum",
    "ErAliasTypeErr",
];

/// One `-tool` message, with any nested message kept in its body verbatim.
#[derive(Debug)]
struct Msg {
    code: u32,
    class: u32,
    body: String,
}

fn messages(stdout: &str) -> Vec<Msg> {
    let mut out = Vec::new();
    let mut cur: Option<Msg> = None;
    let mut nested = 0usize;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("@!@!@STARTMSG ") {
            if let Some(m) = cur.as_mut() {
                nested += 1;
                m.body.push_str(line);
                m.body.push('\n');
                continue;
            }
            let head = rest.trim_end_matches(" @!@!@");
            let (code, class) = head.split_once(':').expect("code:class");
            cur = Some(Msg {
                code: code.parse().expect("code"),
                class: class.parse().expect("class"),
                body: String::new(),
            });
        } else if line.starts_with("@!@!@ENDMSG ") {
            if nested > 0 {
                nested -= 1;
                if let Some(m) = cur.as_mut() {
                    m.body.push_str(line);
                    m.body.push('\n');
                }
            } else if let Some(mut m) = cur.take() {
                m.body.truncate(m.body.trim_end().len());
                out.push(m);
            }
        } else if let Some(m) = cur.as_mut() {
            // RuntimeException messages prefix a nested STARTMSG with
            // `: ` on the same line. It is still a nested message.
            nested += usize::from(line.starts_with(": @!@!@STARTMSG "));
            m.body.push_str(line);
            m.body.push('\n');
        }
    }
    out
}

#[test]
fn message_parser_keeps_inline_nested_errors_and_quoted_markers() {
    let transcript = "@!@!@STARTMSG 1000:1 @!@!@\n: @!@!@STARTMSG 2215:0 @!@!@\nerror\n@!@!@ENDMSG 2215 @!@!@\n@!@!@ENDMSG 1000 @!@!@\n@!@!@STARTMSG 2217:4 @!@!@\nx = \"@!@!@STARTMSG 2132:0 @!@!@\\nerror\\n@!@!@ENDMSG 2132 @!@!@\"\n@!@!@ENDMSG 2217 @!@!@\n@!@!@STARTMSG 2199:0 @!@!@\n1 states generated, 1 distinct states found, 0 states left on queue.\n@!@!@ENDMSG 2199 @!@!@\n";
    let ms = messages(transcript);
    assert_eq!(ms.len(), 3);
    assert!(ms[0].body.ends_with("@!@!@ENDMSG 2215 @!@!@"));
    assert_eq!(ms[1].code, 2217);
    assert_eq!(ms[2].code, 2199);
}

/// What a run reports: TLC's, or dive's in TLC's terms.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Report {
    exit: u8,
    /// The first error message: code and text (`None` on success).
    first: Option<(u32, String)>,
    counts: Option<(u64, u64, u64, u64)>,
    /// `(label, state)` per counterexample state.
    trace: Vec<(String, String)>,
}

/// TLC's report, and the class the conformance harness reads from it.
fn tlc(dir: &Path, name: &str) -> (Report, Option<u32>) {
    let stdout = std::fs::read_to_string(dir.join(format!("{name}.stdout")))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let exit: u8 = std::fs::read_to_string(dir.join(format!("{name}.exit")))
        .expect("exit")
        .trim()
        .parse()
        .expect("exit code");
    let ms = messages(&stdout);
    let errors: Vec<&Msg> = ms
        .iter()
        .filter(|m| m.class == 1 && m.code != 2121)
        .collect();
    let first = errors.first().map(|m| (m.code, m.body.clone()));
    // The harness's rule (`dive-conformance`, `tlc/result.rs`): the first
    // error whose code explains the exit status, context codes aside.
    let decided = errors
        .iter()
        .find(|m| {
            ![2103, 2264].contains(&m.code) && u32::from(tlc_exit_status(m.code)) == u32::from(exit)
        })
        .map(|m| m.code);
    let num = |code: u32| ms.iter().rfind(|m| m.code == code).map(|m| m.body.clone());
    let counts = num(2199).map(|b| {
        let v: Vec<u64> = b
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().expect("n"))
            .collect();
        let depth: u64 = num(2194)
            .expect("depth")
            .trim_start_matches("The depth of the complete state graph search is ")
            .trim_end_matches('.')
            .parse()
            .expect("depth");
        (v[0], v[1], v[2], depth)
    });
    let mut trace = Vec::new();
    for m in &ms {
        if m.code == 2217 {
            let (head, state) = m.body.split_once('\n').unwrap_or((&m.body, ""));
            let label = head.split_once(": ").map_or(head, |x| x.1);
            trace.push((label.to_owned(), state.trim_end().to_owned()));
        } else if matches!(m.code, 2107 | 2108)
            && let Some((_, state)) = m.body.split_once('\n')
        {
            trace.push((
                "<Initial predicate>".to_owned(),
                state.trim_end().to_owned(),
            ));
        }
    }
    (
        Report {
            exit,
            first,
            counts,
            trace,
        },
        decided,
    )
}

/// dive's report, in TLC's terms, and its claimed class.
fn dive(r: &CheckResult) -> (Report, u32, bool) {
    let ctx = dive_value::Ctx::tlc();
    let class = r.verdict.tlc().expect("a verdict");
    let message = match &r.verdict {
        Verdict::EvaluationError { message, .. }
        | Verdict::ModelError { message, .. }
        | Verdict::PostconditionFalse { message, .. }
        | Verdict::NotCompletelySpecified { message, .. }
        | Verdict::AssumptionFalse { message, .. } => Some(message.trim_end().to_owned()),
        Verdict::InvariantViolated { property, .. } => {
            Some(format!("Invariant {property} is violated."))
        }
        Verdict::ActionPropertyViolated { property } => {
            Some(format!("Action property {property} is violated."))
        }
        _ => None,
    };
    let first = (class.code != 2193).then(|| (class.reported, message.unwrap_or_default()));
    let counts = match (&r.stats, r.tlc_prints_stats) {
        (Some(s), true) => Some((s.generated, s.distinct, s.left_on_queue, s.depth)),
        _ => None,
    };
    let trace = r
        .counterexample
        .iter()
        .flat_map(|c| &c.states)
        .map(|s| {
            (
                s.label.clone(),
                format_trace_state(r, s, ctx).trim_end().to_owned(),
            )
        })
        .collect();
    (
        Report {
            exit: class.exit,
            first,
            counts,
            trace,
        },
        class.code,
        class.verified,
    )
}

fn run(tla: &Path, cfg: &Path, mut opts: CheckOptions) -> CheckResult {
    let analysis = dive_sema::analyze_file(tla, &dive_sema::Options::default()).expect("readable");
    assert!(analysis.accepted(), "{} rejected", tla.display());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(&std::fs::read_to_string(cfg).expect("cfg"))
        .into_config()
        .expect("cfg parses");
    opts.no_deadlock = false;
    check(&ir, &cfg, &opts)
}

fn listed<'a>(list: &'a [(&str, &str)], name: &str) -> Option<&'a str> {
    list.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
}

/// Checks one cell. Returns the failures.
#[allow(clippy::too_many_lines, reason = "one cell's checks, in order")]
fn cell(name: &str, specs: &Path, transcripts: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    let (want, decided) = tlc(transcripts, name);
    let tla = specs.join(name).join(format!("{name}.tla"));
    let cfg = specs.join(name).join(format!("{name}.cfg"));
    let mut first_got: Option<Report> = None;
    for (store, workers) in [
        (StoreMode::Fingerprint, 1),
        (StoreMode::Fingerprint, 4),
        (StoreMode::Exact, 1),
        (StoreMode::Exact, 4),
    ] {
        let mut opts = CheckOptions::tlc(0, 0);
        opts.store = store;
        opts.workers = workers;
        let r = run(&tla, &cfg, opts);
        let at = format!("{name} ({store:?}, {workers} workers)");
        if let Some(why) = listed(REFUSED, name) {
            if !matches!(r.verdict, Verdict::Unsupported { .. }) {
                fails.push(format!(
                    "{at}: listed as refused ({why}) but dive reports {:?}",
                    r.verdict
                ));
            }
            continue;
        }
        if !r.verdict.is_verdict() {
            fails.push(format!("{at}: no verdict: {:?}", r.verdict));
            continue;
        }
        let (mut got, code, verified) = dive(&r);
        let mut want = want.clone();
        if ALIAS.contains(&name) {
            // dive prints the states, TLC the alias's records.
            want.trace.iter_mut().for_each(|t| t.1.clear());
            got.trace.iter_mut().for_each(|t| t.1.clear());
        }
        if let Some(why) = listed(UNCLAIMED, name) {
            if verified {
                fails.push(format!(
                    "{at}: listed as unclaimed ({why}) but dive claims its class {code}"
                ));
            }
            // Everything but the deciding class must still agree.
            got.exit = want.exit;
        } else {
            if !verified {
                fails.push(format!("{at}: dive does not claim its class {code}"));
            }
            if let Some(d) = decided
                && d != code
            {
                fails.push(format!(
                    "{at}: the harness reads class {d} from TLC's run, dive claims {code}"
                ));
            }
        }
        if got != want {
            fails.push(format!("{at}:\n  TLC  {want:?}\n  dive {got:?}"));
        }
        match &first_got {
            None => first_got = Some(got),
            Some(f) if *f != got => fails.push(format!("{at}: differs from 1 worker")),
            Some(_) => {}
        }
    }
    // The default profile: the same, except 32-bit overflow (D-004).
    if listed(REFUSED, name).is_none() {
        let r = run(&tla, &cfg, CheckOptions::dive(0));
        if name.ends_with("Overflow") {
            if matches!(r.verdict, Verdict::EvaluationError { code: 2178, .. }) {
                fails.push(format!("{name} (default profile): overflows"));
            }
        } else if let Some(f) = &first_got {
            let (mut got, _, _) = dive(&r);
            if ALIAS.contains(&name) {
                got.trace.iter_mut().for_each(|t| t.1.clear());
            }
            let mut expected = f.clone();
            if name == "ErWrapSetMember" {
                // D-028 constructs the mixed set mathematically. Its later
                // scalar membership remains undefined and raises the actual
                // argument error, with every other observable unchanged.
                expected.first = Some((
                    1000,
                    concat!(
                        "TLC threw an unexpected exception.\n",
                        "This was probably caused by an error in the spec or model.\n",
                        "See the User Output or TLC Console for clues to what happened.\n",
                        "The exception was a java.lang.RuntimeException\n",
                        ": Attempted to compare string \"a\" with non-string:\n2"
                    )
                    .to_owned(),
                ));
            }
            if got != expected {
                fails.push(format!(
                    "{name} (default profile):\n  expected {expected:?}\n  dive {got:?}"
                ));
            }
        }
    }
    fails
}

fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(f)
        .expect("thread")
        .join()
        .expect("no panic")
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/errors")
}

/// The cells SANY accepts (`ErAssumeEnabled` is TLC's own level error,
/// which `dive analyze` reproduces).
fn cells() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root().join("specs"))
        .expect("specs")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n != "ErAssumeEnabled")
        .collect();
    names.sort();
    names
}

#[test]
fn error_classes_agree_with_tlc() {
    let failures = on_big_stack(|| {
        let specs = root().join("specs");
        let transcripts = root().join(ORACLE);
        let names = cells();
        assert!(names.len() > 150, "{} cells", names.len());
        names
            .iter()
            .flat_map(|n| cell(n, &specs, &transcripts))
            .collect::<Vec<_>>()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn listed_cells_exist() {
    let names = cells();
    for (n, _) in UNCLAIMED.iter().chain(REFUSED) {
        assert!(names.iter().any(|x| x == n), "{n} is listed but missing");
    }
    for n in ALIAS {
        assert!(names.iter().any(|x| x == n), "{n} is listed but missing");
    }
}

#[test]
fn level_error_is_rejected_like_sany() {
    let tla = root().join("specs/ErAssumeEnabled/ErAssumeEnabled.tla");
    let analysis = dive_sema::analyze_file(&tla, &dive_sema::Options::default()).expect("readable");
    assert!(
        !analysis.accepted(),
        "ENABLED in an ASSUME is a level error"
    );
}
