//! Help for native commands that do not have a separate command module.

pub(super) fn command(command: &str) -> Option<&'static str> {
    Some(match command {
        "parse" => {
            "USAGE: dive parse [FILE.tla] [--config FILE.cfg] [--output human|json] [--cst]\n\nParse a specification or configuration without checking a model.\n"
        }
        "analyze" => {
            "USAGE: dive analyze FILE.tla [--output human|json]\n                         [--library-path DIR]... [--module-path DIR]...\n\nResolve modules and analyze the specification without checking a model.\n"
        }
        "check" => CHECK,
        "worker" => {
            "USAGE: dive worker FILE.tla --seed N --fp N --worker-key FILE\n                        [--listen IP:port] [--worker-timeout-ms N] [--worker-max-units N]\n                        [--config FILE.cfg] [--compat=tlc] [--store fp|exact]\n                        [--output human|json]\n\nServe supported Core checking work; starting a worker proves no model.\n"
        }
        "simulate" => {
            "USAGE: dive simulate FILE.tla --traces N [--generate] [--depth N]\n                          [--seed N] [--aril N] [--workers N] [--stats none|basic]\n                          [--engine core|reference] [--compat=tlc]\n                          [--exit-codes native|tlc] [--output human|json]\n                          [--time-limit DURATION|none] [--max-memory BYTES|auto|none]\n\nRun bounded traces. Successful sampling does not prove exhaustive correctness.\n"
        }
        "dfid" => {
            "USAGE: dive dfid FILE.tla --max-level N [--config FILE.cfg] [--workers N]\n                      [--seed N] [--worker-seeds S,...] [--engine core|reference]\n                      [--compat=tlc] [--output human|json]\n                      [--time-limit DURATION|none] [--max-memory BYTES|auto|none]\n\nRun supported bounded iterative deepening; incomplete exploration gives no proof.\n"
        }
        "validate-trace" => {
            "USAGE: dive validate-trace FILE.tla [--trace TRACE.json] [--inputs RUN.json]\n                                [--config FILE.cfg] [--claim ENDING] [--sample]\n                                [--compat=tlc] [--symmetry-liveness unreduced|refuse|tlc]\n                                [--seed N] [--fp N] [-deadlock] [--output human|json]\n                                [--library-path DIR]... [--module-path DIR]...\n\nIndependently validate a counterexample against its specification.\n"
        }
        _ => return None,
    })
}

const CHECK: &str = "USAGE: dive check FILE.tla [OPTIONS]\n\n\
    --config FILE.cfg       configuration (default FILE.cfg beside the source)\n\
    --engine core|reference execution engine (default core)\n\
    --compat=tlc            select existing TLC-compatible behavior\n\
    --workers N|auto        worker count; --sequential forces sequential search\n\
    --seed N --fp N         replay seed and fingerprint polynomial\n\
    --store fp|exact        fingerprint or exact visited-state keys\n\
    --output human|json     report format (JSON schema 3)\n\
    --exit-codes native|tlc process statuses (default tlc in either profile)\n\
    -deadlock               disable deadlock checking\n\
    --time-limit DURATION|none --max-memory BYTES|auto|none\n\
    --max-states N|none --max-depth N|none\n\
    --lncheck final --lasso short\n\
    --action-composition off|tlc|tla --symmetry-liveness unreduced|refuse|tlc\n\
    --allow-io=env,files,exec\n\
    --source-coverage --coverage 0 --core-sample N\n\
    --library-path DIR --module-path DIR (repeatable)\n\n\
Supported storage options:\n\
    --frontier-memory-bytes N --frontier-directory DIR\n\
    --candidate-memory-bytes N --candidate-directory DIR --store-compact-bytes N\n\
    --predecessor-memory-bytes N --predecessor-directory DIR\n\
    --seen-disk-entries N --seen-directory DIR --seen-key-bytes N\n\
    --seen-probe-steps N --seen-summary-pairs N\n\
    --checkpoint FILE --checkpoint-after-levels N | --recover FILE\n\
    --checkpoint-counters tlc|retain\n\n\
Supported distributed options:\n\
    --distributed-worker IP:port (repeatable) --worker-key FILE\n\
    --worker-timeout-ms N --distributed-retries N\n\n\
Unsupported combinations refuse by name. Reference is currently the default;\n\
Core runs supported requests directly, while Auto may rerun through Reference.\n\
The default time limit is 30m (none under --compat=tlc); the default memory\n\
limit is half of physical memory. Native process statuses are 0 success,\n\
1 rejected input, 10 property violation, 11 deadlock, 12 assumption failure,\n\
13 evaluation error; both conventions retain 2 usage, 3 unsupported/resource\n\
limit, 4 internal error and 5 stopped by a run budget (no verdict).\n";
