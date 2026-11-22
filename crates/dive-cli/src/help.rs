//! Help for native commands that do not have a separate command module.

pub(super) fn command(command: &str) -> Option<&'static str> {
    Some(match command {
        "parse" => {
            "USAGE: dive parse [FILE.tla] [--config FILE.cfg] [--output human|json] [--cst]\n\nParse a specification or configuration without checking a model.\n"
        }
        "analyze" => {
            "USAGE: dive analyze FILE.tla [--output human|json] [--orbit-observers]\n                         [--library-path DIR]... [--module-path DIR]...\n\nResolve modules and analyze the specification without checking a model.\nWith --output json, --orbit-observers lists choice domains and symmetry refusals.\n"
        }
        "check" => CHECK,
        "worker" => {
            "USAGE: dive worker FILE.tla --seed N --fp N --worker-key FILE\n                        [--listen IP:port] [--worker-timeout-ms N] [--worker-max-units N]\n                        [--config FILE.cfg] [--compat=tlc] [--store fp|exact]\n                        [--output human|json]\n\nServe pure Core safety work for `dive check --distributed-worker`; starting a\n\
worker proves no model. Give it the same build, FILE.tla, configuration,\n\
--seed, --fp, --compat, --store and -deadlock as the coordinator.\n\n\
    --worker-key FILE       local file holding the shared 32-byte secret\n\
    --listen IP:port        listening endpoint (default 127.0.0.1:0, printed)\n\
    --worker-timeout-ms N   per-operation and per-unit deadline, 1..60000 (5000)\n\
    --worker-max-units N    exit after N completed units (default: serve until killed)\n"
        }
        "simulate" => {
            "USAGE: dive simulate FILE.tla --traces N|unlimited [--generate] [--depth N]\n                          [--constructive-init]\n                          [--seed N] [--aril N] [--stats none|basic]\n                          [--workers COUNT] [--keep-traces K|all]\n                          [--max-report-bytes BYTES] [--max-generated N]\n                          [--sample-unbounded geometric|off] [--sample-mean N]\n                          [--liveness-extend N|auto] [--liveness-budget N|auto]\n                          [--trace-dir DIR] [--trace-format dive|json|tla]\n                          [--trace-ids A..B] [--compat=tlc]\n                          [--exit-codes native|tlc] [--output human|json]\n                          [--time-limit DURATION|none] [--max-memory BYTES|auto|none]\n\nRun sampled traces. The report is the same at any worker count; it keeps the\ndeciding trace and the first K others (default 10). --traces unlimited needs\n--time-limit. --trace-dir writes every validated trace to its own file;\n--trace-ids A..B (instead of --traces) regenerates those traces of the seed.\nMembers of Nat, Int and Seq(S) are drawn (geometric, mean 8) unless\n--sample-unbounded off or --compat=tlc. A temporal PROPERTY is checked on each\ntrace's own behaviors; --liveness-extend continues a trace cut by the depth\n(default: the depth; 0 under --compat=tlc). Successful sampling does not prove\nexhaustive correctness.\n"
        }
        "dfid" => {
            "USAGE: dive dfid FILE.tla --max-level N [--config FILE.cfg]\n                      [--seed N] [--worker-seeds S,...]\n                      [--compat=tlc] [--output human|json]\n                      [--time-limit DURATION|none] [--max-memory BYTES|auto|none]\n\nRun supported bounded iterative deepening; incomplete exploration gives no proof.\n"
        }
        "validate-trace" => {
            "USAGE: dive validate-trace FILE.tla [--trace TRACE.json] [--inputs RUN.json]\n                                [--config FILE.cfg] [--claim ENDING] [--sample | --repair]\n                                [--compat=tlc] [--symmetry-liveness unreduced|refuse|tlc]\n                                [--seed N] [--fp N] [-deadlock] [--output human|json]\n                                [--library-path DIR]... [--module-path DIR]...\n\nIndependently validate a counterexample against its specification.\n--repair searches for short replacement paths for a named periodic word and requires --output json.\n"
        }
        _ => return None,
    })
}

const CHECK: &str = "USAGE: dive check FILE.tla [OPTIONS]\n\n\
    --strategy auto         select exhaustive checking from the bound model and requested options\n\
    --strategy bounded-path --path-bound K  check paths through K explicit Next transitions\n\
    --strategy symbolic     exact finite safety, symbolic JSON reports (Linux release bundle)\n\
    --strategy finite-induction  portable finite induction certificates (Linux release bundle)\n\
    --strategy occupancy    exact labeled safety for interchangeable finite status tables (Linux)\n\
    --strategy compiled     compile a finite exact safety search (Linux, installed Rust compiler)\n\
    --recheck MODULE CFG    check another symbolic revision in this invocation\n\
    --refine-domains        learn reachable members of wide sets in a symbolic search\n\
    --config FILE.cfg       configuration (default FILE.cfg beside the source)\n\
    --compat=tlc            select existing TLC-compatible behavior\n\
    --workers N|auto        worker count; --sequential forces sequential search\n\
    --seed N --fp N         replay seed and fingerprint polynomial\n\
    --store fp|exact        fingerprint or exact visited-state keys\n\
    --output human|json     report format (JSON schema 4)\n\
    --exit-codes native|tlc process statuses (default tlc in either profile)\n\
    -deadlock               disable deadlock checking\n\
    --time-limit DURATION|none --max-memory BYTES|auto|none\n\
    --max-states N|none --max-depth N|none\n\
    --action-composition off|tla --symmetry-liveness unreduced|refuse|tlc\n\
    --periodic-min-states N  first completed-level liveness check (default 65536)\n\
    --lncheck cold-fair     seek a validated fair word from one original Init state\n\
    --lncheck periodic-all  search every temporal property at each scheduled level\n\
    --core-sample N         compare N sampled states' successors (diagnostic)\n\
    --library-path DIR --module-path DIR (repeatable)\n\n\
Supported storage options:\n\
    --independent-frontier --frontier-memory-bytes N --frontier-directory DIR\n\
    --candidate-memory-bytes N --candidate-directory DIR --store-compact-bytes N\n\
    --predecessor-memory-bytes N --predecessor-directory DIR\n\
    --seen-disk-entries N --partitioned-fingerprints --seen-directory DIR --seen-key-bytes N\n\
    --seen-probe-steps N --seen-summary-pairs N\n\
    --checkpoint FILE --checkpoint-after-levels N | --recover FILE\n\
    --checkpoint-counters tlc|retain\n\n\
Supported distributed options (pure safety; see\n\
    https://github.com/Chelis-Lang/dive/blob/main/docs/user/distributed-safety.md):\n\
    --distributed-worker IP:port  a running `dive worker` endpoint; repeat, 2..64\n\
    --worker-key FILE       local file holding the shared 32-byte secret\n\
    --worker-timeout-ms N   per-operation and per-unit deadline, 1..60000 (5000)\n\
    --distributed-retries N retries of a lost source unit, 0..8 (2)\n\
    Requires --seed and --fp; temporal properties, SYMMETRY, VIEW and runtime\n\
    effects refuse by name. Worker loss past the retries stops with exit 3.\n\n\
Core is the only engine. Options it does not implement are not listed here;\n\
they and unsupported combinations refuse by name (exit 3), for example\n\
--symmetry-liveness tlc (the --compat=tlc default) with SYMMETRY and temporal\n\
properties.\n\
There is no default time limit, as in TLC; the default memory\n\
limit is half of physical memory. Native process statuses are 0 success,\n\
1 rejected input, 10 property violation, 11 deadlock, 12 assumption failure,\n\
13 evaluation error; both conventions retain 2 usage, 3 unsupported/resource\n\
limit, 4 internal error and 5 stopped by a run budget (no verdict).\n";
