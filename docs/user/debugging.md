# Native Core debugging

`dive debug FILE.tla --stdio` runs a native DAP adapter for the existing Core
safety checker. A DAP client sends initialize, launch, source/exception breakpoint
requests and configurationDone. Search then executes normally until a breakpoint,
step, pause, exception, budget stop or completion. Both semantic profiles work;
`--compat=tlc` or `DIVE_COMPAT=tlc` selects TLC value semantics. Ordinary checking
keeps debugging off.

For a client that attaches over TCP:

```sh
dive debug Spec.tla --listen 127.0.0.1:0 --seed 7 --fp 0 --compat=tlc
```

The actual bound address appears on stderr. The checker starts loading/compiling
and waits for client configuration. Connect a framed DAP client to that address,
send initialize then attach, and wait for initialized before installing source
breakpoints and sending configurationDone. A stdio client uses launch instead.
The program is fixed by the command line; a DAP request cannot select a different
program, environment, working directory or host-I/O grant.

Source breakpoints are exact executable source lines. A removed/folded/aliased
expression without an instruction stays unverified. Frames describe actual
instruction activations, including helpers and INSTANCE substitution context.
Source coordinates use one-based UTF-16 columns. State, successor and local
scopes expose existing integers, booleans and bounded strings. An unassigned or
unforced slot is labelled unavailable; compounds remain opaque. Looking at a
scope does not evaluate source or normalize producer values.
Diagnostic replay suspends observation even when it reuses the primary machine;
reconstruction cannot publish another producer stop or replace its snapshot.

The adapter supports continue, pause, next, stepIn, stepOut, stackTrace, scopes,
variables, loaded-source retrieval, bounded evaluate, exceptionInfo and terminate.
The `all` exception filter stops on actual evaluation errors or false configured
safety obligations before ordinary reporting continues. Every resume invalidates
old frame/scope handles. Evaluate requires a current frame and accepts bounded
integer/boolean arithmetic over captured names; source helpers, effects, new
modules and global/source-position evaluation are unsupported. Errors and stale
requests return failed responses with `DIVE-D0001`. An attach `disconnect`
request without explicit termination fails; use terminate explicitly.
Detaching while checking continues is unsupported. Losing the
transport cancels the owned check without inferring success.

The `dive/checkResult` event contains the ordinary check report with its native
exit code, followed by exited and terminated. The process uses that same native
exit status. A breakpoint or available map is not a model verdict. Terminating
an incomplete check produces an explicit refusal, never success; ordinary budget
stops and unsupported diagnostic/word cases retain their original authority.

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

This slice runs one actual local Core worker. The report preserves the caller's
worker selection in `options.workers.requested` and reports one worker in
`options.workers.used`. Reference/Auto debugging, temporal,
checkpoint/recovery, Store reclamation, distributed observation, native source or
TLC logical coverage, sampling, host grants, conditional/log/column breakpoints,
reverse steps, mutation and compound expansion remain unsupported. Input frames are bounded to 1 MiB and
headers to 8 KiB; output frames, maps, activations and expression work have explicit
bounds. Listener addresses must be loopback. Source locations identify executed
TLA+; the separate PlusCal emission maps do not yet supply runtime PlusCal
breakpoints. TLC `-debugger` and complete debugger
parity remain open; see [ADR 0152](../decisions/0152-compiled-source-maps-and-native-debugging.md).
Public checking requests and managed sessions refuse reclamation, coverage and
distribution before source/key access or observer preparation. Preparing a native
worker with `CheckRequest.debugger` also refuses explicitly.
