# 0113: Independently replayed temporal state-error endpoints

- **Status:** accepted
- **Date:** 2026-10-25

## Context

A reached capture row alone does not establish TLC's temporal error demand.
The pinned references distinguish startup tautology 2253/77 from a reached
2179/2182 error, and compiled action bins can demand an error before an earlier
source-state predicate. The certified row consumer therefore requires original
producer evidence, scoped ordered demand and complete must-read coverage.

A finite `behavior` trace validates only the supplied behavior; it does not
validate an error. Generic trace-version-1 `error` endings remain unsupported
because their original successor-expansion errors cannot be independently
re-raised. Relabeling a temporal request as safety or trusting a producer's
validation Boolean would not repair this gap.

## Decision

Retain trace version 1 and report schema 3. Add an optional, strictly decoded
`endpoint` to an existing `kind: "error"` ending:

```json
{"kind":"error","ec":"TLC_MODULE_DIVISION_BY_ZERO","endpoint":{"stage":"temporal-state-row","source":{"module":"M","start":40,"end":49,"instance":null}}}
```

The descriptor is untrusted. It selects no predicate, scope or producer frame.
A fresh private source/configuration session independently reconstructs the
whole ordered, residual-free TLC demand program and proves canonical producer,
capture and normalization stability before evaluating it. All needed scoped
truths must be covered by the always-executed bins' must-read union. The
validator checks finite Init/Next, constraints and safety obligations, then
executes its own ordered program at the final supplied state. Its first actual
core 2179/2182 error must match the requested class and inner source context.
Distinct INSTANCE actuals remain separate occurrences even when spans coincide.
Unknown effects, providers, representations or scope rules refuse before replay.

This proves a finite path and its independently reconstructed error endpoint.
It does not certify BFS counts, exploration completion, sibling search timing,
a shortest witness, a lasso or a violated temporal property. Ordinary promotion
still requires the separate opaque reached-row evidence and actual statistics.
Zero-solution startup 2253 is outside this capability and never becomes success
or a later division error.

The new endpoint compiler and canonical traversal use explicit stacks, checked
allocation and existing cooperative controls. They introduce no formula-node,
value-depth or proof-work count limit. Pinned TLC's Java-int overflow applies
only to the DNF product of disjunctive factors; an allocation failure is not that
TLC error. Existing explicit bounded APIs and unrelated validator limits retain
their contracts.

`Session::try_read` and `SpecModel::try_read`/`try_validate` carry resource and
execution exceptions outside `ValidationReport`. The controlled fresh-session
path retains original typed interruption and validation progress. Infallible
legacy entrypoints refuse endpoint requests before binding or replay and direct
callers to the fallible path. They do not convert endpoint exhaustion into an
unsupported result. Ordinary CLI validation and the harness use the fallible
path. An aborted `validate-trace --output json` emits schema 3, command,
`validation_completed: false` and an error/resource envelope, with no `result`
or checker `verdict`. Physical status remains nonzero: resource 3,
interruption 5, internal execution failure 4. A completed validation report's
closed result vocabulary is unchanged.

The harness accepts a finite temporal error document only with physical255,
verified matching 2179/2182, the actual capture-row site, incomplete execution,
matching finite displayed states and no property claim. It retains actual
partial counts and independently validates the endpoint on another fresh
session. Generic non-violation words retain their original rejection.

## Consequences

Old trace1 readers interpret this as a generic error and remain unsupported.
Unknown or malformed endpoint fields fail closed in new readers. Generic error
endings without a descriptor and borrowed caller-owned Model validation gain
no endpoint authority. There is no dependency on the checker, liveness graph or
store, no new CLI option, no oracle/golden edit and no baseline refresh from
source implementation alone. Independent replay support may conservatively
refuse an otherwise producer-certified candidate; that remains a missing
validation capability, not agreement or a valid error.
