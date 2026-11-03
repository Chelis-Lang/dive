# 0148: Explicit categories and recorded startup inputs

- **Status:** accepted
- **Date:** 2026-11-03

## Context

D-017 requires explicit env/files/exec grants in every profile. A seed alone
cannot reproduce external inputs. Ordinary native words have no host truth or
input-replay certificate, and independent validation must not access the ambient
environment or filesystem to supply missing evidence.

## Decision

Keep category authorization in immutable evaluator options, default-empty in
both profiles. Check native category access after configured redirects and
before host callbacks. Actual input refusals are remembered by the
evaluator; fresh proof admission cannot swallow them and certify a word. The
low-level binder retains TLC's soft constant-precomputation failure behavior;
ordinary checking retains its impure-refusal inventory.
Unimplemented value-I/O calls with one direct unassigned constant retain their
existing pure undefined-constant error during precomputation, without a callback
or effectful argument prefix. The denial remains latched for proof admission.
This preserves unconfigured `_JsonTrace` reader templates and their pinned
startup controls; it does not execute or authorize I/O.
The recording host owns one binding operation and closes before assumptions.

The first producer supports ordinary Reference safety checking with IOEnv and
well-typed TXT/UTF-8 Deserialize reads during binding. Auto selects Reference
before effects. Captured values remain available in later evaluation. Runtime
reads, writes, exec, Core, temporal production and checkpoint/recovery remain
unimplemented. Invalid argument/file-error record classes are refused rather
than approximated. Empty IOEnv RecordValue representation is also refused.

Record raw environment/text responses against exact operation/path requests in
source execution order. Bounds are 256 operations, 1 MiB of total UTF-8 data,
paths and names, and 4,096 environment entries. Repeated IOEnv responses retain
one environment. Only one reached TXT Deserialize request per fresh evaluator
is admitted; reuse its captured result for later expressions. Pinned TXT results
share a mutable name array: normalizing one can corrupt later result fields.
Until its representation is proved, subsequent requests refuse before their
arguments and callbacks in both profiles (#761).
These are external assumptions, not certificates of past operating-system truth.

Independent sessions create a fresh replay cursor, evaluator, interner, host and
values for every binding and choice-search attempt. The real source operations
and arguments must consume all records in order; missing, mismatched, extra or
post-binding requests refuse. A supplied state or cached producer value cannot
replace a source operation. Source-created input strings exist before word
decoding. Borrowed-model validation with input grants refuses in favor of a
fresh Session with explicit assumptions.

An optional `external_inputs` field extends ordinary report schema 3. The CLI
reads that context through `validate-trace --inputs RUN.json`, selecting the
report's counterexample unless a separate `--trace` is given. The library takes
the same explicit context through TraceOptions/Session. Native trace payloads,
versions and existing endings retain their meanings; no migration is required.

Pinned EnvironmentOrder shows a real failure if TLC IOEnv map iteration is
replaced by sorted string construction: TLC violates at DIVE_LABEL while a
prototype incorrectly succeeded at DIVE_COUNT. Do not emulate arbitrary Java map
internals. TLC mode refuses snapshots introducing more than one previously
unknown string, including keys and values. It also refuses a first IOEnv reached
after a TXT request: the pinned override constructs static ENV before any file
result, regardless of source definition order. TextBeforeEnvOrder demonstrates
the resulting CHOOSE difference even with only one new environment string.
Capture the environment first to use the admitted combined path. The canonical
default keeps its approved order.
Broader demand-scoped ordering proof is tracked in #758.

## Consequences

Environment/TXT settings gain usable checking and offline CLI/API replay, with
strict denied-callback, forged-word, missing/mismatched/extra-record and native
representation controls. The overall I/O family remains partial (#757).
Operating-system read latency is not cooperatively bounded, and input-byte
bounds do not measure the whole process heap. Large reports and unsupported
classes retain explicit refusals. No dependency graph, native trace version,
TLC golden or corpus coverage total changes.
