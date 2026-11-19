# Native Core debugging

> **Experimental.** `dive debug` works on Core in both profiles and is covered
> by the CI test suite, but its protocol surface and limits may change between
> releases.

`dive debug FILE.tla --stdio` runs a Debug Adapter Protocol (DAP) adapter for
the Core safety checker. Your DAP client sends initialize, launch, source and
exception breakpoint requests, and configurationDone. The search then runs
until a breakpoint, step, pause, exception, budget stop or completion. Both
semantic profiles work: `--compat=tlc` or `DIVE_COMPAT=tlc` selects TLC value
semantics. `dive check` never starts the debugger.

For a client that attaches over TCP:

```sh
dive debug Spec.tla --listen 127.0.0.1:0 --seed 7 --fp 0 --compat=tlc
```

The bound address is printed on stderr. The checker loads and compiles the
model, then waits. Connect a framed DAP client to that address, send initialize
then attach, and wait for initialized before installing source breakpoints and
sending configurationDone. A stdio client sends launch instead of attach.
The command line fixes the program: a DAP request cannot change the program,
environment, working directory or host-I/O grant.

A source breakpoint sits on an executable source line. A breakpoint on an
expression the compiler removed, folded or aliased has no instruction and stays
unverified. Frames follow the instructions that ran, including helpers and
INSTANCE substitution context. Source coordinates use one-based UTF-16 columns.

The state, successor and local scopes show integers, booleans and bounded
strings. An unassigned or unforced slot is labelled unavailable, and compound
values are opaque. Reading a scope evaluates nothing. While dive replays a
step to build a diagnostic, the debugger does not stop, and the replay cannot
replace the snapshot you are looking at.

The adapter supports continue, pause, next, stepIn, stepOut, stackTrace,
scopes, variables, loaded-source retrieval, bounded evaluate, exceptionInfo and
terminate. The `all` exception filter stops on an evaluation error or a false
configured safety property, before the check reports it. Every resume
invalidates the old frame and scope handles.

Evaluate needs a current frame and accepts bounded integer and boolean
arithmetic over captured names. It cannot call source helpers, run effects,
load modules or evaluate at a global or source position. Errors and stale
requests fail with `DIVE-D0001`.

To stop, send terminate. A `disconnect` from an attached client fails unless
it also terminates, because detaching while the check continues is not
supported. If the transport drops, the check is cancelled and does not report
success.

The `dive/checkResult` event carries the check report with its native exit
code, followed by exited and terminated. The process exits with the same
status. Terminating an incomplete check produces a refusal, never success.
Budget stops and unsupported cases report as they do in `dive check`.

Rust callers use the same session without a CLI or external tools:

```rust,no_run
use dive_api::{CheckRequest, debug::{Session, Event}};
use std::time::Duration;
let mut session = Session::launch(CheckRequest {
    file: "Spec.tla".into(), seed: Some(7), fp: Some(0),
    ..Default::default()
}, false)?;
assert!(matches!(session.wait_event(Duration::from_secs(10))?, Some(Event::Prepared)));
// Install breakpoints using the loaded source's path and executable line.
session.configuration_done()?;
while let Some(event) = session.wait_event(Duration::from_secs(10))? {
    match event {
        Event::Stopped { .. } => session.continue_check()?,
        Event::Completed(_) => break,
        _ => {}
    }
}
let report = session.finish()?;
# let _ = report;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Limits

The debugger runs one local Core worker. The report keeps your worker choice in
`options.workers.requested` and reports one worker in `options.workers.used`.

These are not supported:

- temporal properties;
- checkpoint and recovery, Store reclamation and distributed workers;
- source coverage and TLC logical coverage, sampling and host grants;
- conditional, log and column breakpoints;
- reverse steps, mutation and expanding compound values;
- TLC's `-debugger` protocol;
- PlusCal breakpoints: source locations name the executed TLA+.

Checking requests and managed sessions refuse reclamation, coverage and
distribution before reading source. Preparing a distributed worker with
`CheckRequest.debugger` also refuses.

Input frames are limited to 1 MiB and headers to 8 KiB. Output frames, maps,
activations and expression work have their own fixed limits. The listener
address must be loopback. The design is in
[compiled source maps and native debugging](../decisions/0152-compiled-source-maps-and-native-debugging.md).
