# 0115: Operative native host bindings

- **Status:** accepted
- **Date:** 2026-10-25

## Context

The native evaluator calls a host permission hook before arguments and kernels.
The built-in checking host inherits the success default, but a custom host can
refuse, mutate state, replace itself, or panic. Purity metadata and expression
cache domains do not prove these hooks unobservable. Future enumeration proofs
must distinguish actual execution policy from a provider's claim of quietness.

## Decision

Store an explicit Callback or Unrestricted policy with the installed host.
Unrestricted operationally omits only the native permission hook; all other
host permissions, state operations and cooperative control remain. Legacy host
setters always select Callback. Unrestricted selection declines without change
for an absent host, nonzero independent cache domain, or unavailable generation.
No public constructor can mint a new unrestricted binding. This preserves guard
cache authority without changing the force/cache hot path. Ordinary domain zero
is shared authority: callback restoration does not revoke values warmed there.
Complete replacement and scopes move the owner,
policy and installed cache-domain snapshot together. Fresh constructors query
the host cache domain at installation as before; mode changes/restoration do not
invoke extra host callbacks. Each replacement/restoration gets a new generation;
exhaustion disables optional admission without changing evaluator execution.

Fresh ordinary safety uses the existing built-in host and bound run state.
Fresh owned temporal analysis uses its existing private source-bound ownership
and preserves capture's separate fresh-host reset. Borrowed/public capture,
legacy custom hosts, persistent checkpoint/recovery, trace/guard contexts and
diagnostic replay retain Callback execution. An explicit cross-crate operative
installer is not a private-owner certificate. It cannot turn caller-owned
models into fresh source-bound runs or authorize skipping any expression.

## Consequences

No dependency, report schema, CLI option, result cache or enumeration mask is
added. The native callback runs outside the existing host mutex; callback mode
adds no extra mutex or generation read per native operation. Installation
snapshots are not leases against concurrent evaluator clones: binding scopes
require driver ownership of replacement. Generations are evaluator-local and
cannot alone prove ownership, purity or cross-evaluator identity. Any future
pruning algorithm still needs its complete semantic proof and qualification;
this foundation claims no instruction or throughput improvement.
