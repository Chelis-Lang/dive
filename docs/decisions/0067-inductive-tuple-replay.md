# 0067: Sealed inductive tuple histories for independent whole-word replay

- **Status:** accepted
- **Date:** 2026-10-11

## Context

ADR 0064 admits unevaluated tuple operands but refuses `Tail(stack)` when stack
is a variable. A witness cannot prove its own type or string encounter history.
Clean pinned-primary captures distinguish eight tuple-preserving controls from
three string/name-producing controls and an incomplete next-state assignment.
The first capture’s missing token spacing and the first corrected driver’s
assumption of a supported trace for an incomplete state are retained failures.

## Decision

Privately derive necessary tuple Init facts, then remove candidate variables
until all remaining hypotheses are preserved on every Next branch. This is a
bounded logical proof, without state evaluation, namespace insertion, domain
forcing, successor enumeration or witness-derived facts. AND admits a necessary
conjunct; OR/IF require every branch; equalities and UNCHANGED prove assignments.
Actual redirected definitions and parameter scopes are retained. Opaque shapes,
recursive definitions and budget exhaustion refuse.

Keep the certificate sealed inside `dive-trace`. Use it only in whole-word
runtime-name admission, where Init and each Next pair are subsequently checked.
Do not grant these hypotheses to public arbitrary-state enabledness or lasso
APIs. Their existing empty-variable tuple proof remains unchanged. The all-role
nonconstructing audit is still mandatory, including off-path nested constructors.

The history policy is 256 variables, 32,768 predicate visits/depth 64, and the
existing 256-step operand proof. No dependency edge, profile default, checker
marker, search order or enumeration behavior changes. User scope is described
in `docs/inductive-tuple-trace-replay.md`.

## Consequences

Some tuple-preserving variable histories become independently replayable. This
is not general namespace provenance, all saved-frame replay, periodic scheduling
or a family certificate. Preserve original refused controls and report each
newly admitted case independently. Publication requires the full gate and fresh
CLI transport, corrupted-word, negative-branch and existing conformance controls.
