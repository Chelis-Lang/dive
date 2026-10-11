# 0056: Bounded pure global LOCAL and function DEFINE declarations

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Pinned `PcalTLAGen.GenVarsAndDefs` copies the DEFINE expression between global
and process-local variable declarations. The post-translation AST serializes
that same expression. The retained C/P delta captures LOCAL ordinary helpers,
function definitions, tuple/multiple bounds, dependency domains, comments,
layout and invalid/recursive/action/temporal/conflicting declarations. The
original DEFINE selection remains intact. Two fixture filename mistakes are
retained separately from genuine reference-invalid artifacts.

## Decision

Extend the source-owned declaration admission from ADR 0053 to ordinary LOCAL
operators and global function definitions, including LOCAL functions. Preserve
all tokens, source positions, export visibility and pre-state lookup. Prove
both every function bound's domain and the body under the same bounded native
syntax, name and dependency checks. Do not inline, flatten or export LOCAL
helpers, and do not claim that syntax admission proves domains are finite.

Retain conservative spelling-based cycle detection and all existing output,
node and dependency-work ceilings. Action/temporal domains and bodies,
RECURSIVE/symbolic/INSTANCE declarations, procedure scopes and unproved name
conflicts remain explicit refusals. Generated enclosing modules must still
pass native admission; primary artifacts that fail SANY earn no credit.
No dependency graph, compatibility profile, checksum default or runtime
execution policy changes.

## Consequences

Pure global LOCAL helpers and nonrecursive TLA function declarations can use
the native translation and Full-checksum APIs. Complete raw serialization,
public CST/SANY/Validator, finite model counts and independent witnesses stay
separate evidence. F-034/F-035 remain partial. Strict #124 replay gaps,
D-020 fair-header/source identity limits, formatting, disk writes and broader
renaming/procedure scopes remain qualified in the user guide.
