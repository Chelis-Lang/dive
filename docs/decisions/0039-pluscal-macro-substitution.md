# 0039: Native bounded PlusCal macro substitution

Status: accepted (bounded implementation; full-family certification pending)

## Context

The source grammar owns macro declarations and calls in both C/P syntaxes.
Translation must expand them before labels and control flow are normalized.
The pinned translator performs lexical substitution, not general TLA+ capture
avoidance. A macro formal shadowed by a PlusCal `with` remains substituted in
its body. Replacing a formal which is a qualified operator name can generate
invalid TLA+. A translator exit of zero is not sufficient evidence of validity.

## Decision

Expand declarations in source order in native Rust, using only earlier macros.
Substitute all actual arguments simultaneously into the original token sequence.
Respect the pinned exclusions for strings, record labels and field selectors;
parenthesize multi-token actuals in larger expressions. For an assignment formal,
require the reference's identifier-leading actual, prepend actual selectors,
and separately validate the complete generated module.

The public source AST remains unchanged. A private resolver owns generated
expression tokens and text, with original call spans for diagnostics. Statements
retain both macro definition and call positions where the reference exposes a
composite assertion/inserted-label location. Prepared declaration templates
are removed before reachable-program label and program-counter decisions.
Process admission inspects resolved reachable expression names, so expansion
cannot conceal a free `self` or a foreign process-local reference.

Charge expanded nodes, allocated tokens/text and resolved expression
amplification before retention/output. Bounds are named resource results. Neither
raw generator artifacts nor public translation reports imply exhaustive checking
or a model proof.

Both profiles use the measured reference behavior. No new capture-avoidance
semantics is adopted. Recursion/forward macro references are reference grammar
errors; duplicate formal parameters, multiline/unproved binder substitution,
procedure interactions and unproved namespace rewrites remain named refusals.

## Evidence and limits

See [the maintained study](../pluscal-macros.md) and the complete immutable source,
primary/normalization/SANY stages in `macro-acceptance`. Invalid primary artifacts
are retained as diagnostic evidence and never credited as admitted artifacts.
Public CLI/API tests reject them in both profiles. Every admitted artifact is
compared as significant tokens and full enclosing-module operator CST, followed
by pinned SANY; curated generated models compare counts/verdict/depth and use
independent positive/corrupted witnesses. Full historical flags, source-map
formatting, file rewrite/checksum and procedure contracts remain pending.

This adds no dependency edge. F-034/F-035 stay partial and audit-required.
