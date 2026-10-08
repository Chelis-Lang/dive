# 0009: Syntax tree and parser

- **Status:** accepted
- **Date:** 2026-10-08

## Context

M3 needs a TLA+ lexer and parser (`dive-syntax`) with these properties:
- a lossless concrete syntax tree (CST) with spans, which the LSP can use later;
- error recovery and stable error codes;
- **parse acceptance identical to the pinned SANY parser** (tla2tools 1.8.0 at tlaplus
  `94d0c50`, sha256 `7beec0f0…`).

Semantic analysis is `dive-sema` (M4).

SANY's parser is a JavaCC LL(k) grammar, `tlatools/org.lamport.tlatools/javacc/tla+.jj`.
Its acceptance depends on machinery beyond the grammar text:

- **`belchDEF`.** Before each definition, SANY scans ahead to the next `==`. It then
  inserts a fictitious `DEFBREAK` token at the start of that definition's left-hand side,
  found by walking backwards over parentheses and operator tokens. Definitions are
  recognised only where such a token sits. The scan stops at `THEOREM`, `LEMMA`,
  `PROPOSITION`, `COROLLARY`, `AXIOM`, `ASSUMPTION` and `====`.
- **`OperatorStack.java`.** It resolves precedence with *ranges*. It rejects
  overlapping ranges, and it applies prefix `-` through its "mixfix" form `-.`. This is
  why SANY rejects `1 * -2`, `2 ^ -1`, `ENABLED x'`, `[]x'`, `a * b / c` and
  `SUBSET A \X B`. It accepts `1 + ~2 = 3`.
- **`JunctionListContext.java`.** Tokens are compared by begin column. JavaCC counts one
  column per UTF-16 code unit, and a tab advances to the next multiple of 8. Every
  token of a `/\` or `\/` item must lie strictly right of its bullet.
- **Lookahead quirks**, all confirmed against SANY with minimal specs in
  `crates/dive-syntax/tests/sany-cases/`:
  - `matchFcnConst` makes `{a, b \in S}` a syntax error.
  - `{f(x) \in S : x \in T}` is rejected, but only with the ASCII `\in` (the `∈` form is
    accepted).
  - Keywords become identifiers after `.`, in `INSTANCE`/`EXTENDS`/module headers, and
    as the *first* field of a record constructor.
  - `(+)` and `(\X)` are operator tokens, so `F(+)` does not pass `+`.
  - `<4>` is a step token anywhere.
- **Proof levels** (`proofLevelStack`, `beginsProof`, `correctLevel`) decide where proofs
  begin and end.

**tree-sitter-tlaplus** is a good node-type vocabulary, but it is deliberately permissive
(`docs/survey/adjacent.md`), so its acceptance is not an oracle. Its corpus, as copied
into tlaplus at `test/tla2sany/corpus`, is the best structural test set available.

### CST representation: rowan or a bespoke arena

| | rowan 0.16 (rust-analyzer) | bespoke arena CST |
|---|---|---|
| Lossless | yes, by construction (tokens carry text) | yes, if built so |
| Cheap clones | `GreenNode` is an `Arc`; subtrees share | an `Arc` of the whole arena; no subtree sharing |
| LSP readiness | proven: offsets, `token_at_offset`, `covering_element`, parent/sibling cursors, `TextRange`; the base for incremental reparsing | all of it to write |
| Thread safety | green tree `Send + Sync`; red cursors `!Send` (re-root per thread) | `Send + Sync` |
| Speed | node cache and allocation per node; measured about 29 MiB/s end to end on Examples (below) | likely faster to build |
| Deep trees | drop is recursive, so it can overflow on trees thousands deep | can be iterative |
| Dependencies | rowan, countme, hashbrown, rustc-hash, text-size (MIT/Apache) | none |

## Decision

1. **Use rowan** for the CST. Its cursor API, structural sharing and track record in an
   LSP outweigh the build-speed advantage of an arena, and its speed is ample.
   - Red cursors are `!Send`. Callers share the `GreenNode` and re-root where needed.
   - The recursive drop is handled by a depth cap (item 6).
2. **Port SANY's acceptance, not its structure.**
   - The lexer is hand-written. It follows `tla+.jj`'s lexical states (`DEFAULT`,
     `PRAGMA`, `SPEC`, nested comments) with longest-match and definition-order
     tie-breaks. It computes columns exactly as `SimpleCharStream`.
   - The parser is recursive descent, one function per SANY production. Each choice
     uses the same token tests as JavaCC's lookahead, including the virtual `DEFBREAK`
     tokens from a faithful `belchDEF` port.
   - Expressions use a port of `OperatorStack.reduceStack`/`finalReduce` rather than a
     Pratt parser. Exact conflict semantics, prefix-minus handling and SANY's quirks
     (`getMixfix("..")` is `...`) fall out of the same case analysis, which can be
     compared line by line.
   - The parser builds an owned intermediate tree, so stack reductions can rearrange
     subtrees. It then emits the green tree in one iterative pass, reinserting trivia;
     leading trivia stays outside the node it precedes.
3. **The CST is SANY-shaped but cleaned up**, with tree-sitter-like names where they fit.
   - `INFIX_EXPR` holds `lhs op rhs`.
   - `\X` chains are one n-ary `CARTESIAN_PRODUCT`, as SANY's `N_Times`.
   - Qualified names are `GENERAL_ID` with an `ID_PREFIX`.
   - Text before the module and after `====` is `PRELUDE`/`TRAILER` trivia. PlusCal
     stays inside `BLOCK_COMMENT` trivia, for `dive-pluscal`.
4. **Errors.**
   - The first error matches SANY's verdict. A module is accepted iff there are no error
     diagnostics.
   - Codes are `DIVE-Snnnn`, listed in `crates/dive-syntax/src/diagnostics.rs` and never
     reused. Each diagnostic has a primary span, a message naming the construct, and a
     `help` fix where one is known.
   - SANY's non-fatal `PErrors` (`!.@`, fairness linking) are reported without
     unwinding, so they still reject.
   - After a fatal error the parser recovers at the next module-level unit. It skips to
     a line-initial declaration keyword, `====`, a nested module, or a definition start
     found by `belchDEF`. The skipped tokens go into an `ERROR` node. The tree is always
     complete and lossless.
5. **API and CLI.**
   - `dive_syntax::parse_module(&str) -> Parse { green, diagnostics }`, with typed views
     in `dive_syntax::ast` for the main node kinds.
   - `Parse::check_file_name` is SANY's module-name check, which is part of SANY's
     parse phase for a file.
   - `dive parse FILE [--output human|json] [--cst]` goes through
     `dive_api::parse_file`. Its JSON has `schema_version: 1`.
6. **Explicit limits where SANY's only limit is the Java stack.** These are refusals with
   code `DIVE-S0040`, never silent approximations (soundness rule 3):
   - parser recursion is capped at 256 nested expressions, proofs or modules;
   - one unit's tree is capped at a height of 2048, because rowan frees green trees
     recursively.

   A 1500-term chain is accepted; a 20000-term chain is refused. No corpus module comes
   near either limit.

## Consequences

Evidence for the decision is from the pinned oracle, 2026-10-08.
- **Parse acceptance agrees with SANY on every module checked, 13,256 in total:**

  | Corpus | Modules | Both accept | Both reject | Disagree |
  |---|---|---|---|---|
  | tlaplus/Examples | 424 | 424 | 0 | 0 |
  | CommunityModules | 78 | 78 | 0 | 0 |
  | tlaplus `test/**` and `test-model/**` (including `error_corpus`) | 808 | 804 | 4 | 0 |
  | tlaplus syntax corpus | 355 | 326 | 29 | 0 |
  | Minimal SANY-behaviour cases | 174 | 110 | 64 | 0 |
  | Mutants of corpus, Examples and test modules (token edits, operator swaps, re-indentation, tabs, CRLF and CR line endings) | 11,417 | 5,807 | 5,610 | 0 |

  The procedure is in `crates/dive-syntax/acceptance/README.md`.
- The syntax corpus also passes a structural comparison against its expected trees.
  This is a projection onto operator, junction-list, bracket, binder and definition
  nodes.
- **Speed:** the Examples corpus (424 modules, 3.0 MB) parses in about 0.1 s
  (28–30 MiB/s), including lexing and green-tree construction. This is a release build on
  an Apple M3 (8 cores, 24 GB), single-threaded.
- **Upkeep:**
  - Moving the SANY pin means re-running the acceptance procedure and regenerating the
    verdict files.
  - SANY grammar changes upstream must be ported by hand.
  - The production-by-production structure keeps that tractable.
- **Recovery granularity is the unit.** An error inside a definition turns the whole
  definition into an `ERROR` node. Finer recovery inside expressions and proofs is
  future work for the LSP.
- **Once the conformance harness exists, it owns the acceptance check.** It should run
  `dive parse --output json` against SANY goldens for every corpus module. The scripts
  here document the classification rules it needs: SANY's `-s`, restart after a fatal
  error, root-module-only verdicts, and `name_mismatch` as a separate class.
