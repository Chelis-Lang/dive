# 0055: Checked stored-value trace transport

- **Status:** accepted (bounded opt-in implementation; broader admission remains partial)
- **Date:** 2026-10-11

## Context

Trace version 1 is normalized JSON, not a representation-preserving wire format.
The immutable #190 campaign retains all 132 originals and 396 altered words:
96 original words validate, while 36 fail shape admission; 288 altered words
fail semantic replay and 108 fail shape admission. Neither shape failure earns
semantic credit. In particular, TLC accepts a forward record/tuple set but the
normalized JSON array can no longer pass the same asymmetric constructor when
read back. Function domains that happen to be tuples or records also lose the
TLC literal/function distinction.

The existing `dive-value::codec` follows the design of pinned value `write`
methods, but has a native format and a trusted-storage decoder. It accepts
stored normalization flags and reconstructs tag 0x08 as a literal tuple. Exposing
it directly to external traces would trust unvalidated representation claims.
That storage format and its existing consumers remain unchanged here.

## Decision

Keep version 1 as the default. Add explicit `--trace-format representation-v2`
and `TraceEncoding::RepresentationV2`. Only the embedded `dive-trace` document
uses version 2; this does not relabel the enclosing checker report, change the
profile, or certify the checker result. `validate-trace` reads either version
from the document. Temporal report writing remains explicitly refused for this
first increment.

Version 2 uses explicit stored tags for literal tuples, tuple-functions, literal
records, record-functions, general functions, normalized explicit sets and Raw
sets. The JSON preserves ordered stored children and the bound value order.
Names must already exist in the receiving model's interner. Input cannot install
an arbitrary ordering table or create runtime names. Scalars retain the usual
boolean/integer/string/model forms. Deferred, interval and symbolic sets are
currently refused; their enumeration is not substituted into the transport.

A normalized set carries an index-permutation construction witness. The decoder
runs the ordinary `Value::set` constructor on that permutation and compares the
exact stored output, including nested representation, with the declared output.
A duplicate, missing or foreign index refuses. Raw normalized flags cause actual
normalization of their stored operands; an error refuses. Literal/domain/order
claims must match their ordinary constructors. None of these flags is a semantic
validation claim. The encoder tries only stored order and reverse stored order;
if neither reproduces the exact output it refuses. Its probes suppress existing
normalization markers on shared operands using the existing Quiet authority;
comparison/construction semantics remain unchanged. Providers are rejected
before probing.

Admission limits are explicit: 16,384 nodes, value depth 32, at most 512 members
per compound and at most 512 states/variables, a conservative 4 MiB charge for
JSON nodes plus escaped text, and 1,048,576 conservative comparison-node work
units per value. Two encoder witness trials are allowed. The whole word shares
its JSON node/byte budget before typed-document cloning or value construction.
JSON parser allocations and search memory are outside those budgets. These are
admission ceilings, not claims of total process memory or exact CPU accounting.

Semantic validation remains the independent Init/Next/ending path. A well-formed
transport alone is not a witness certificate. Altered concrete states, claims,
closing edges, forged flags and malformed representations must not receive
`Valid` merely because a transport can be read. Version 2 does not broaden the
random/VIEW shape guard, reverse-set normalization, temporal support or existing
runtime/effect admission.

## Consequences

Rust callers can transport admitted concrete representation without reusing the
checker/store/generator. No dependency graph change is needed. Version-1 raw
observations remain immutable, including its current failures and output.
Migration is explicit and opt-in; refusal metadata names unsupported transport
shapes rather than pretending that every emitted compound value is a validated
word. Broader finite/symbolic/deferred transport and arbitrary normalization
witness search remain future obligations under #190.

Decimal large integers are limited to 4,096 digits before parsing. A hand-built
`BigInt` fitting i64 is refused rather than silently converted to another stored
variant; ordinary canonical large integers remain supported in the big-integer
profile. This does not change legacy integer transport or model arithmetic.

The encoder also recomputes function/Raw-set constructor shape, suppressing
borrowed normalization markers during bounded probes. A trusted-storage General
function with a tuple/string domain or forged Raw normalization flag refuses
before output; it is not silently reinterpreted. Successful value encoding is
structurally readable with the same admitted names/table and profile, within
the same bounds. A different receiving name table/profile may refuse, and
independent Init/Next/ending replay remains a separate obligation.
