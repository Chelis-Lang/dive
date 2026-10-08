# Provenance

Every algorithm ported or adapted from another implementation is recorded here. Each entry gives:
- the `dive` component;
- the source project, file and class or method, and the version or commit;
- the source's licence;
- the nature of the port: semantics matched, or algorithm adapted.

The canonical tlaplus tools are MIT-licensed. Translated code carries attribution.

| dive component | Source | Licence | Nature |
|---|---|---|---|
| `dive-syntax` lexer (`crates/dive-syntax/src/lexer.rs`) | tlaplus `tlatools/org.lamport.tlatools/javacc/tla+.jj`: token definitions and the `DEFAULT`, `PRAGMA`, `SPEC` and comment lexical states; `tla2sany/parser/SimpleCharStream.java` `UpdateLineColumn` (column counting); commit `94d0c50` | MIT | Semantics matched: same tokens, longest-match and tie-break order, same columns |
| `dive-syntax` parser (`crates/dive-syntax/src/parser/`) | tlaplus `tla+.jj` productions `CompilationUnit` to `OpenStart`, including `belchDEF`, `matchFcnConst`, `isMinusOpStart`, `isFieldNameToken`, `isLabel`, `labelDoesNotChangeParse`, `checkIndentation`, `caseSep`, `beginsProof`, `correctLevel`, and `tla2sany/parser/JunctionListContext.java`; commit `94d0c50` | MIT | Semantics matched; structure is idiomatic Rust, one function per production |
| `dive-syntax` operator stack (`crates/dive-syntax/src/parser/expr.rs`) | tlaplus `tla2sany/parser/OperatorStack.java` (`reduceStack`, `finalReduce`, `reduceRecord`, `preInEmptyTop`) and `Operator.java` (`prec`, `succ`); commit `94d0c50` | MIT | Algorithm adapted: the case analysis is kept in the same order so the two can be compared |
| `dive-syntax` operator table (`crates/dive-syntax/src/ops.rs`) | tlaplus `tla2sany/parser/Operators.java` (`CanonicalOperators`, `OperatorSynonyms`, `getMixfix`); commit `94d0c50` | MIT | Table ported verbatim |
| `dive-syntax` corpus test reader (`crates/dive-syntax/tests/corpus.rs`) | tlaplus `test/util/SyntaxCorpusFileParser.java` and `test/tla2sany/parser/TlaPlusSyntaxCorpusTests.java` (expected-failure list); commit `94d0c50` | MIT | Format and expectations matched; the corpus itself is vendored with its licence |
| `dive-syntax` node naming | tree-sitter-tlaplus `grammar.js` at `add40814` | MIT | Reference only, for node-kind vocabulary |
| `dive-conformance` `tlc/codes.rs` (`EC_CODES`, `exit_statuses_for`, `EXIT_STATUSES`) | tlaplus/tlaplus `tlc2/output/EC.java` (constants, `EC.ExitStatus`, `errorConstantToExitStatus`) at `5a47802` (v1.7.4) and `94d0c50` | MIT | Data transcribed (codes, names, exit mapping); used to classify oracle output, not ported behaviour |
| `dive-conformance` `tlc/messages.rs` | tlaplus/tlaplus `tlc2/output/MP.java` (`-tool` message delimiters and classes) | MIT | Format matched (parser written from the format) |

No checker algorithm has been ported yet.
