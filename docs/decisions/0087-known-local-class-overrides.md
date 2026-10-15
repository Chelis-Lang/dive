# 0087: Native overrides for verified local classes

- **Status:** accepted
- **Date:** 2026-10-12

## Context

The pinned Examples Hanoi model includes a local `Bits.class`. TLC substitutes
its four-argument `And` method for the TLA+ body. Refusing all local classes
prevents this model from reaching the supported evaluator, while recognizing
only a module or method name would execute unrelated code with invented semantics.
The primary `Bits.java` and the compiled class independently establish signed
32-bit AND of the first two arguments, with four `IntValue` parameters.

## Decision

Model binding recognizes only the exact pinned class by SHA-256, computed with a
fixed-size streaming buffer. Add the existing workspace `sha2` dependency to
`dive-model`; no internal dependency edge changes. A separate native installation
phase requires explicit binding and never activates an override from a module
name alone. A checked IR setter verifies its defining module, method name and
value-parameter arity, including instantiated copies. Configuration substitutions
retain precedence. Unknown and unreadable classes remain named refusals.

## Consequences

The pinned Hanoi class can run without a JVM. This does not support arbitrary
Java plugins. Another class needs its own identity, semantic review and tests;
a different build of the same Java source remains unsupported until reviewed.
See [provenance](../provenance.md#verified-local-class-overrides) for the pin and
bytecode evidence. Non-IntValue arguments and integers outside the TLC 32-bit
range are explicitly unsupported in both profiles.

Native checkpoint/recovery includes sorted adjacent module names and streaming
class SHA-256 digests in its existing IR identity, only when classes are present.
The no-class IR hash stays unchanged. Removing or altering a class rejects actual
recovery before saved states are used. A second digest comparison after binding
rejects class inputs changed during binding. This extends source-input identity,
without a new archive schema or Java execution; the pinned native override can
pause and recover normally. Unknown classes retain their named binding refusal.

## Local-class native reference contract (candidate revision)

Three retained directTLC4 Hanoi runs report243distinct/907generated/depth33;
historical native4 and the primary/golden1 report233/768/depth32, with the same
NotSolved/2110/physical12 and valid32state native words. All raw observations and
negative comparisons remain historical and are not regraded. The former local-
class one-worker restriction fenced that stopping-prefix discrepancy. The
existing comparator uses the one-worker reference for native implementations
at every width; only TLC oracle self-checks have multiworker leniency.

The proposed revised checker follows the maintained native contract: every
supported native width compares to the pinned one-worker reference. Required
verdict/property/class/physicalexit, counts/depth and independent validation of
the actual concrete native word remain unchanged. This removes only the local
class width guard; exact class admission and checkpoint class-source identity
stay binding. General store/effect/capture, simulation and DFID scopes retain
existing restrictions. No comparator, golden, ExpectedDivergence, queue-order or
timing exemption changes.

Acceptance requires latest-parent original-MC1 qualification, a separate
complete-search primary/native1 diagnostic, supported native4 pinned-reference
and independently validated actual-word controls, fresh review/fullgate/all4/CI.
Issue #272 remains open; this source proposal is not runtime acceptance.
