# Provenance

Every algorithm ported or adapted from another implementation is recorded here. Each entry gives:
- the `dive` component;
- the source project, file and class or method, and the version or commit;
- the source's licence;
- the nature of the port: semantics matched, or algorithm adapted.

The canonical tlaplus tools are MIT-licensed. Translated code carries attribution. The PlusCal stage port retains the upstream
[MIT notice](../crates/dive-pluscal/src/translate/pcal/LICENSE).

| dive component | Source | Licence | Nature |
|---|---|---|---|
| `dive-api` ordinary TLC-profile final routing and `dive-conformance` schema-2 transport | Existing dive `final_roles`, final analyzer/independent replay, and `final_study::lasso_document`; TLC `tlc2/output/EC.java` at `94d0c50`; ADRs 0082 and 0086 | MIT | Original integration of the existing bounded final backend, preserving the actual startup binding and ordinary safety tail. Structural display/document consistency handling was independently written after reading the existing study helper; no historical whole mapper or periodic/performance cache implementation imported. Success 2193/0 and temporal violation 2116/13 retain pinned classifications; arbitrary periodic word choice and queue population are excluded from parity by ADR 0086 and the issue #241 owner contract, with fresh independent replay mandatory. Generated-count parity is required at one worker; distinct/depth/class/physical exit/property remain strict at multiple workers. The separate historical TLC oracle self-check policy is unchanged. Raw queue values and the producer's queue-zero exhausted-capture check remain; finite safety length/ending comparisons remain strict. Historical `tlc-probes` inputs, stdout, command and exits for `LvNoFair`, `LvWeakFair`, `LvWFIntermittent` and `LvActionProp` were directly read before adding ordinary-route tests; their recorded `94d0c50`, worker-1/fp-0/seed-0/final observations remain historical, not new oracle runs. No new oracle measurements are recorded here |
| TLC checking entrypoints (`dive-cli/src/tlc.rs`, shared dispatch and thin binaries) | tlaplus `tlc2/TLC.java` argument handling (`-modelcheck`, config/module suffixes, workers/seed/fp/deadlock, `-lncheck`, absolute-root `ToolIO.userDir`) and `util/SimpleFilenameToStream.java` user-directory/CWD resolution; pinned `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Checking-only argument semantics adapted after direct source reading, with the existing `dive-api` backend/report and fresh `SpecModel` replay. Historical internal X parser/validation helpers at `fb2fb3c34b8ae5697434677f1c344051bc8d3452` were consulted, not imported wholesale; no message renderer or removed API was restored. `tests/fixtures/tlc-check/primary.json` retains four explicitly historical pinned safety observations, not current-source execution evidence. Configuration-first input errors follow `tlc2/tool/impl/Spec.java` (constructor) and `ModelConfig.java` (`parse`) at the same pin; `input-priority.json` records seven fresh pinned controls (both missing, each missing independently, malformed configuration with present/missing module, valid input and explicit suffixes). Native commands and `check_file` retain module-first order. TLC tool/mode protocols remain outside this adapter; no corpus credit or full compatibility claim. |
| `dive-pluscal` translation (`translate/pcal/`) | tlaplus `pcal/ParseAlgorithm`, `PcalSymTab`, `PcalFixIDs`, `PcalTranslate`, `PcalTLAGen`, `TLAExpr`; commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Reference semantics ported stage by stage: source-order macros, labels/grouping, scoped IDs, explosion and stack frames, priming/subscripts and layout-sensitive expression substitution. Native pre-allocation bounds and source associations; expanded control-flow depth checks and bounded output-fragment construction are dive designs. No runtime Java delegation. Replaces the older `emit.rs`/`fairness.rs` generation paths; checksum serialization is retained. |
| `dive-pluscal` bounded sequential procedure checksum (`translate/checksum.rs`) | tlaplus `pcal/AST.java` (`Procedure`, `PVarDecl`, `Call`, `Return`, `CallGoto`, `VectorToSeqString`), `ParseAlgorithm.java` (`GetProcedure`, `GetCall` mutable proceduresCalled alias and CallGoto fold), `PCalTLAGenerator.translate`, `PcalTranslate.Explode`; commit `94d0c50` | MIT | Exact original post-generation header AST templates; independent development-only expanded ownership captures; shared nested-vector mutation stays refused. ADR 0058; complete 132-case selection preserves failed/native/reference stage distinctions |
| `dive-check` native checkpoint/resume (`checkpoint.rs`, ordered completed-level engine) | tlaplus `tlc2/tool/ModelChecker.java` (`checkpoint`, `recover`, `doInit`), `AbstractChecker.java`, `Worker.java`, `TLCTrace.java`, `queue/DiskStateQueue.java`, `fp/MemFPSet.java`, `util/InternTable.java`, commit `94d0c50` | MIT | Recovery resumes queue without Init, preserves concrete trace/order and restores intern names before evaluation; ADR 0038 recomputes VIEW/symmetry keys via the live configured path, retaining actual concrete discoveries; TLC generated/initial counter reset matched. Pure parallel recovery retains resolved configured keys and concrete discoveries in ordered commits, resumes the actual saved level and resolves counter/record limits at the inline prefix (ADR0038 addendum). Native bounded single-file format and completed-level pausing are dive designs, not Java byte interoperability. Source hashes/raw counts in `conformance/probes/native-checkpoint/` |
| `dive-eval` `Evaluator::intern_bfs_config` and `dive-check` startup | tlaplus `tlc2/tool/AbstractChecker.java` constructor and `createConfig`, commit `94d0c50` | MIT | Eagerly intern bfs, seed, fingerprint polynomial and install strings in source order; clean seed 0/7/14 captures in `crates/dive-check/tests/fixtures/runtime-names/` reproduce counts and chosen violation |
| `dive-syntax` lexer (`crates/dive-syntax/src/lexer.rs`) | tlaplus `tlatools/org.lamport.tlatools/javacc/tla+.jj`: token definitions and the `DEFAULT`, `PRAGMA`, `SPEC` and comment lexical states; `tla2sany/parser/SimpleCharStream.java` `UpdateLineColumn` (column counting); commit `94d0c50` | MIT | Semantics matched: same tokens, longest-match and tie-break order, same columns |
| `dive-syntax` parser (`crates/dive-syntax/src/parser/`) | tlaplus `tla+.jj` productions `CompilationUnit` to `OpenStart`, including `belchDEF`, `matchFcnConst`, `isMinusOpStart`, `isFieldNameToken`, `isLabel`, `labelDoesNotChangeParse`, `checkIndentation`, `caseSep`, `beginsProof`, `correctLevel`, and `tla2sany/parser/JunctionListContext.java`; commit `94d0c50` | MIT | Semantics matched; structure is idiomatic Rust, one function per production |
| `dive-syntax` operator stack (`crates/dive-syntax/src/parser/expr.rs`) | tlaplus `tla2sany/parser/OperatorStack.java` (`reduceStack`, `finalReduce`, `reduceRecord`, `preInEmptyTop`) and `Operator.java` (`prec`, `succ`); commit `94d0c50` | MIT | Algorithm adapted: the case analysis is kept in the same order so the two can be compared |
| `dive-syntax` operator table (`crates/dive-syntax/src/ops.rs`) | tlaplus `tla2sany/parser/Operators.java` (`CanonicalOperators`, `OperatorSynonyms`, `getMixfix`); commit `94d0c50` | MIT | Table ported verbatim |
| `dive-syntax` corpus test reader (`crates/dive-syntax/tests/corpus.rs`) | tlaplus `test/util/SyntaxCorpusFileParser.java` and `test/tla2sany/parser/TlaPlusSyntaxCorpusTests.java` (expected-failure list); commit `94d0c50` | MIT | Format and expectations matched; the corpus itself is vendored with its licence |
| `dive-syntax` node naming | tree-sitter-tlaplus `grammar.js` at `add40814` | MIT | Reference only, for node-kind vocabulary |
| `dive-config` reader (`crates/dive-config/src/parser.rs`) | tlaplus `tlc2/tool/impl/ModelConfig.java` (`ALL_KEYWORDS`, `parse`, `parseValue`, `getNextToken`); `tla2sany/parser/SimpleCharStream.java` (`FillBuff`, `UpdateLineColumn`: the line TLC reports at end of file); `tlc2/output/EC.java` and `MP.java` (`CFG_*` codes, `ERROR_CONFIG_PARSE`); `tlc2/value/impl/ModelValue.java` (creation order, typed model values); commit `94d0c50` | MIT | Semantics matched: same token stream (the `dive-syntax` lexer in SANY's `SPEC` state), same error order, codes and lines. Verified against `ModelConfig` itself on 886 corpus cfg files |
| `dive-syntax` `tokens::tokenize_spec` | tlaplus `tlc2/tool/impl/ModelConfig.java` (`new TLAplusParserTokenManager(scs, 2)`: the cfg reader starts in the `SPEC` state); commit `94d0c50` | MIT | Semantics matched |
| `dive-config` oracle dumper (`crates/dive-config/acceptance/CfgDump.java`) | Calls the pinned jar's `ModelConfig` | MIT (calls only) | Test tooling, not a port |
| `dive-conformance` `tlc/codes.rs` (`EC_CODES`, `exit_statuses_for`, `EXIT_STATUSES`) | tlaplus/tlaplus `tlc2/output/EC.java` (constants, `EC.ExitStatus`, `errorConstantToExitStatus`) at `5a47802` (v1.7.4) and `94d0c50` | MIT | Data transcribed (codes, names, exit mapping); used to classify oracle output, not ported behaviour |
| `dive-conformance` `tlc/messages.rs` | tlaplus/tlaplus `tlc2/output/MP.java` (`-tool` message delimiters and classes) | MIT | Format matched (parser written from the format) |
| `dive-sema` module resolution (`crates/dive-sema/src/loader.rs`) | tlaplus `tla2sany/modanalyzer/SpecObj.java` (`loadSpec`, `findNextUnresolvedExtention`, `findNextUnresolvedInstantiation`, `instanceResolvesToInternalModule`, `nonCircularityTest`, `resolveNamesBetween*`, `calculateDependencies`), `ParseUnit.java` (`determineModuleRelationships`, `calculateContextWithinParseUnit`, `handleExtensions`, `getInstances`, `verifyEquivalenceOfFileAndModuleNames`), `ModuleContext.java`; `util/SimpleFilenameToStream.java` and `util/FileUtil.createNamedInputStream` (search order); `util/MonolithSpecExtractor.module`; commit `94d0c50` | MIT | Semantics matched, including the stale `extentionFound`/`instantiationFound` flags |
| `dive-sema` PlusCal check (`crates/dive-sema/src/pcal_check.rs`) | tlaplus `pcal/Validator.validate`, `pcal/ParseAlgorithm.uncomment`, `pcal/trans.findTokenPair` and `removeTabs`; commit `94d0c50` | MIT | Semantics matched for the two rejecting outcomes only |
| `dive-sema` generator (`crates/dive-sema/src/generate/`) | tlaplus `tla2sany/semantic/Generator.java` (every `process*`/`generate*` method, `genIdToSelector`, `selectorToNode`, the label (`LS`) and recursion bookkeeping), `SymbolTable.java` (`resolveSymbol`, `resolveModule`, `addSymbol`, `addModule`), `Context.java` (`mergeExtendContext`), `SymbolNode.sameOriginallyDefinedInModule`, the `match` methods of `OpDefNode`, `OpDeclNode`, `FormalParamNode`, `ThmOrAssumpDefNode`, `SubstInNode` (`constructSubst`, `addExplicitSubstitute`, `matchAll`), `UseOrHideNode.factCheck`; the parser's `correctedStepNum` and proof-level stack (`tla+.jj`); commit `94d0c50` | MIT | Semantics matched check by check; control flow kept so methods can be compared |
| `dive-sema` level checking (`crates/dive-sema/src/level.rs`) | tlaplus `tla2sany/semantic/` `levelCheck` of `LevelNode` (`levelCheckSubnodes`, `addTemporalLevelConstraintToConstants`), `OpApplNode`, `OpDefNode`, `ThmOrAssumpDefNode`, `ModuleNode` (including recursive sections and `isConstant`), `SubstInNode`, `APSubstInNode`, `Subst` (`getSubLCSet`, `getSubALCSet`, `getSubALPSet`), `InstanceNode`, `LetInNode`, `LabelNode`, `AtNode`, `OpArgNode`, `AssumeProveNode`, `NewSymbNode`, `TheoremNode` (`LevelCheckTemporal`), `AssumeNode`, the proof nodes; `SetOfLevelConstraints`, `SetOfArgLevelConstraints`; commit `94d0c50` | MIT | Semantics matched, including SANY's quirks (shadowed `levelChecked` counters, `ThmOrAssumpDefNode.isLeibnizArg`) |
| `dive-sema` built-ins and codes (`crates/dive-sema/src/builtins.rs`, `diagnostics.rs`) | tlaplus `tla2sany/semantic/BuiltInOperators.java` (table), `ErrorCode.java` (cross-referenced codes); commit `94d0c50` | MIT | Table transcribed; codes cross-referenced |
| `dive-sema` standard modules (`crates/dive-sema/stdlib/`) | `tla2sany/StandardModules/*.tla` in the pinned `tla2tools.jar` (tlaplus `94d0c50`) | MIT | Copied byte for byte (ADR 0011) |
| `dive-sema` error corpus (`crates/dive-sema/tests/error-corpus/`) | tlaplus `test/tla2sany/semantic/error_corpus/` at `94d0c50`; expectations from `SemanticErrorCorpusTests.java` | MIT | Vendored test data |
| `dive-ir` lowering scopes (`crates/dive-ir/src/lower.rs`) | tlaplus `tlc2/tool/impl/Tool.java` (`evalImpl`, `evalImplLetInKind`, `evalImplSubstInKind`, `evalImplApSubstInKind`, `evalImplOpArgKind`, `evalAppl`: the `OPCODE_*` cases), `SymbolNodeValueLookupProvider.java` (`lookup`, `getVal`, `getOpContext`, `getVar`), `tlc2/util/Context.java` (`cons`, `lookup`, `branch`); commit `94d0c50` | MIT | Semantics matched: which nodes are laziness points, that substitutions are dynamic and parameters effectively lexical, how a name outside the context is resolved |
| `dive-ir` Java overrides (`crates/dive-ir/src/std_op.rs`) | tlaplus `tlc2/tool/impl/SpecProcessor.java` `processModuleOverrides`, `tlc2/tool/impl/TLAClass.loadClass`, `TLARegistry.mapName` and the `TLARegistry.put` calls of `tlc2/module/*.java`, `tlc2/module/TLCBuiltInOverrides.java`; CommunityModules `tlc2/overrides/TLCOverrides.java` at `78b98ae`. The table itself was extracted from the class files (access flags, parameter counts, annotation values) of the pinned `tla2tools.jar` (`tlc-2026.10.06.014338`) and `CommunityModules-202610040242.jar` | MIT | Matching order matched; table generated from the binaries |
| `dive-ir` origins (`crates/dive-ir/src/origin.rs`) | tlaplus `tla2sany/parser/SimpleCharStream.java` `UpdateLineColumn`, `tla2sany/st/Location.java` `toString`, `tla2sany/parser/SyntaxTreeNode.java` (location source); commit `94d0c50` | MIT | Semantics matched: same lines, columns and location string |
| `dive-ir` action tree (`crates/dive-ir/src/actions.rs`) | tlaplus `tlc2/tool/impl/Tool.java` (constructor action numbering, `getActions`, `getActionsAppl`), `tlc2/tool/Action.java` (`getLocation`, `isNamed`, the parameter list), `tlc2/tool/impl/SpecProcessor.java` (`processConfigSpec`/`processConfigNext`: the next-state `Action`), `tlc2/tool/impl/Specs.addSubsts`; commit `94d0c50` | MIT | Semantics matched rule by rule, including the exception paths; verified against `Tool.getActions()` of the pinned jar |
| `dive-ir` action oracle (`crates/dive-ir/acceptance/ActionDump.java`) | Calls the pinned jar's `FastTool`, `Tool.getActions`, `getInitStateSpec`, `getNextStateSpec`, `ModelConfig.getOverrides` | MIT (calls only) | Test tooling, not a port |
| `dive-sema` declaration order (`crates/dive-sema/src/hashtable.rs`, `generate::decls_in`, `Model::variable_decls`, `Model::constant_decls`) and `dive-ir` `Ir::tlc_var_order` | tlaplus `tla2sany/semantic/ModuleNode.java` (`getVariableDecls`, `getConstantDecls`: the reversed copy and the cache), `Context.java` (`getVariableDecls`, `getConstantDecls`, `content`, `addSymbolToContext`, `duplicate`, `mergeExtendContext`), `SymbolTable.java` (`ModuleName.hashCode`), `util/UniqueString.java` (`hashCode`, `setLoc`, `getVarLoc`), `tlc2/tool/impl/SpecProcessor.java` `processSpec` (varLoc assignment), `tlc2/tool/TLCStateMut.java` (`fingerPrint`, `toString`); commit `94d0c50`. The JDK 17 `java.util.Hashtable` layout and iteration order (default constructor, `addEntry`, `rehash`, `Enumerator`) and `String.hashCode` | MIT (tlaplus); OpenJDK GPL-2.0 with Classpath Exception (behaviour only) | Semantics matched: the `Hashtable` behaviour is reimplemented from its specified algorithm, no JDK code is copied. Verified against the pinned jar's `getVariablesNodes` / `getVariableDecls` on 29 generated specifications, 234 repository fixtures and all 424 Examples modules (`crates/dive-ir/acceptance/README.md`) |
| `dive-ir` variable-order oracle (`crates/dive-ir/acceptance/VarOrderDump.java`) | Calls the pinned jar's `FastTool`, `SpecProcessor.getVariablesNodes`, `UniqueString.getVarLoc`, `SANY.frontEndMain`, `ModuleNode.getVariableDecls` | MIT (calls only) | Test tooling, not a port |
| `dive-ir` effects (`crates/dive-ir/src/effects.rs`) | tlaplus `tlc2/module/TLCGetSet.java` (`TLCGetEval`: which keys read live checker state); commit `94d0c50` | MIT | Key list transcribed |

| `dive-value` value order (`order.rs` `total_cmp`, `tlc.rs` `compare`) | tlaplus `tlc2/value/impl/*Value.java` `compareTo` (`BoolValue`, `IntValue`, `StringValue`, `ModelValue` incl. `modelValueCompareTo`, `SetEnumValue`, `IntervalValue`, `SubsetValue`, `SetOf*Value`, `TupleValue`, `RecordValue`, `FcnRcdValue` `compareToInterval`/`compareOtherInterval`, `UserValue`), `tlc2/module/Naturals.java`, `Integers.java`, `Strings.java`, `Sequences.java` `compareTo`; `util/UniqueString.compareTo` (intern-token order); commit `94d0c50` | MIT | Semantics matched, including error messages; dispatch on the left operand's class kept |
| `dive-value` equality and membership (`tlc.rs` `equals`, `member`, `is_empty`) | the same classes' `equals`, `member` and `Value.isEmpty`; `ModelValue.modelValueEquals`/`modelValueMember`; commit `94d0c50` | MIT | Semantics matched, including error messages |
| `dive-value` normalization (`order.rs` `tlc_insertion_sort`, `tlc_fcn_normalize`, `make_fcn`) | `tlc2/value/impl/ValueVec.sort`/`search`, `SetEnumValue.normalize`, `FcnRcdValue.normalize`, `RecordValue.normalize`; commit `94d0c50` | MIT | Algorithm ported for the error path (exact comparison sequence); fast path proven equivalent when no comparison can fail |
| `dive-value` integers (`int.rs`) | `tlc2/module/Naturals.java` and `Integers.java` (`Plus`, `Minus`, `Times`, `Neg`, `Divide` (`Naturals.Divide` as `div_naturals`), `Mod`, `Expt`), `tlc2/tool/impl/SpecProcessor.java` (`NumeralKind`, `TLC_INTEGER_TOO_BIG`); commit `94d0c50` | MIT | Semantics matched in `IntMode::Tlc`, including messages |
| `dive-value` lazy sets (`enumerate.rs`, `stream.rs`, `value.rs`) | `IntervalValue`, `SubsetValue`, `SetOfFcnsValue`, `SetOfRcdsValue`, `SetOfTuplesValue` (`size`, `member`, `toSetEnum`), `Naturals`/`Integers`/`Strings`/`Sequences` `member`; commit `94d0c50` | MIT | Semantics matched (sizes, overflow messages, membership); enumeration written to produce the normalized order directly |
| `dive-value` FP64 (`fp64.rs` `Fp64`, `POLYS`) | `tlc2/util/FP64.java` (Allan Heydon and Marc Najork): `Init`, `Extend` overloads, the 131-entry `Polys` table; commit `94d0c50` | MIT | Algorithm ported; table transcribed |
| `dive-value` value fingerprints (`fp64.rs` `TlcFingerprint`) | each `tlc2/value/impl/*Value.fingerPrint(long)`, `UniqueString.fingerPrint`, `TLCStateMut.fingerPrint`; commit `94d0c50` | MIT | Semantics matched bit for bit (verified on the low 32 bits via `TLCFP`) |
| `dive-value` printing (`print.rs`) | each `tlc2/value/impl/*Value.toString(StringBuffer, int, boolean)` (expansion rules, `TLCGlobals.enumBound`), `StringValue.PrintVersion`, `FcnRcdValue.isName`/`isRcd`/`isTuple`, `TLCStateMut.toString`, `tlc2/module/TLC.java` `Print`/`PrintT`; commit `94d0c50` | MIT | Semantics matched |
| `dive-value` pretty-printer (`pprint.rs`) | `tlc2/pprint/PrettyPrint.java` (`mypp`), `Parse.java`, `Format.java`, `Node.java`; `tlc2/value/Values.ppr` (width 80); commit `94d0c50` | MIT | Algorithm ported, quirks kept |
| `dive-value` JSON (`json.rs`) | `tlc2/module/Json.java` (`getNode`, `getObjectNode`, `getArrayNode`, `isValidSequence`); Gson `JsonWriter` string escaping (no HTML-safe mode) | MIT (TLC), Apache-2.0 (Gson, format only) | Semantics matched; Gson's escaping table written from its documented behaviour |
| `dive-value` codec (`codec.rs`) | tlaplus `tlc2/value/impl/*Value.write(IValueOutputStream)` (a value is stored by representation, as `IntervalValue.write` keeps `low..high`); commit `94d0c50` | MIT | Design reference only; dive's byte format is its own |
| `dive-store` collision estimates (`collision.rs`) | tlaplus `tlc2/tool/AbstractChecker.java` (`calculateOptimisticProbability`, both `reportSuccess` overloads, `ProbabilityToString`); `tlc2/tool/ModelChecker.java` `reportSuccess(FPSet, long)` (the `1E-10` threshold); `tlc2/tool/fp/MultiFPSet.java` (`getFPSet`, `checkFPs`), `DiskFPSet.checkFPs`, `FPSetConfiguration.getFpBits` (default 1), `FPSetFactory.getFPSet`; `tlc2/output/MP.java` (`TLC_SUCCESS` text); Java 17 `Double.toString` and `BigDecimal.divide(…, MathContext)`; commit `94d0c50` | MIT | Semantics matched, quirks kept (`.5`, `10.0E-11`). Verified against TLC's own method on 75,036 cases and against every golden figure |
| `dive-store` TLC state fingerprint (`state.rs` `StateFingerprinter::tlc`) | tlaplus `tlc2/tool/TLCStateMut.java` `fingerPrint` (one FP64 stream over the values in `varLoc` order, without `VIEW` or symmetry); commit `94d0c50` | MIT | Semantics matched bit for bit: every state fingerprint of TLC's `-fp 0 -dump dot` output on 29 specifications (116 states), from the specification text through `dive-sema`, `dive-ir` and `dive-value` (`crates/dive-store/tests/tlc_state_fingerprints.rs`) |
| `dive-store` state-fingerprint mixer (`state.rs` `mix64`, used by `combine`) | SplitMix64's output function: Guy L. Steele, Doug Lea and Christine H. Flood, "Fast splittable pseudorandom number generators" (OOPSLA 2014), with David Stafford's "Mix13" constants, as in Sebastiano Vigna's `splitmix64.c` (prng.di.unimi.it) | Public domain (CC0, `splitmix64.c`) | Algorithm ported (three lines); checked against `splitmix64.c`'s first output from seed 0. The chain around it (`combine`, position keys, seed) is dive's own design (ADR 0015 §5, issue #42) |
| `dive-store` fingerprint set and resolve (`fpset.rs`, `set.rs`) | ADR 0015; tla-mc `store.rs` (sorted runs, Bloom and sparse index, planned for spill) | MIT (tla-mc, ideas only) | Independent design; TLC's `FPSet` classes consulted for semantics only (64-bit membership, no remapping of any fingerprint) |
| `dive-eval` interpreter (`crates/dive-eval/src/eval.rs`) | tlaplus `tlc2/tool/impl/Tool.java` (`evalImpl`, `evalImplLetInKind`, `evalImplSubstInKind`, `evalApplImpl`: every `OPCODE_*` case and its error message, `contexts`, `isValid`, `checkAssumptions`), `tlc2/tool/impl/ContextEnumerator.java`, `SymbolNodeValueLookupProvider.java` (`lookup`, `getVal`, `getOpContext`), `tlc2/tool/EvalControl.java`, `tla2sany/semantic/SemanticNode.toString`, `NumeralNode.toString`, `DecimalNode.toString`; commit `94d0c50` | MIT | Semantics matched construct by construct, including evaluation order, short-circuiting and error texts |
| `dive-eval` thunks (`crates/dive-eval/src/env.rs`) | `tlc2/value/impl/LazyValue.java` (`getCachedValue`, `getValue`), `tlc2/tool/TLCState.isSubset`, `EvalControl.semanticallyEquivalent`; commit `94d0c50` | MIT | Caching rule matched |
| `dive-eval` lazy values (`crates/dive-eval/src/lazy.rs`, `eval.rs`) | `tlc2/value/impl/FcnLambdaValue.java` (`apply`, `select`, `takeExcept`, `toFcnRcd`, `getDomain`, `toString`), `FcnParams.java`, `SetPredValue.java`, `SetCupValue.java`, `SetCapValue.java`, `SetDiffValue.java`, `UnionValue.java` (`union`, `member`, `elements`), `SetEnumValue`/`IntervalValue` `cup`/`cap`/`diff`, `TupleValue`/`RecordValue`/`FcnRcdValue` `apply`/`select`/`takeExcept`, `Value.select(Value[])`, `getKindString`, `getNonEnumerableErrorMsg` (and the `Naturals`, `Integers`, `Sequences` overrides), `SubsetValue`/`SetOfFcnsValue` enumerators, `UserValue.size`, every `isFinite`; commit `94d0c50` | MIT | Semantics matched, including which results stay lazy and the error texts |
| `dive-eval` standard operators (`crates/dive-eval/src/std.rs`) | `tlc2/module/Naturals.java`, `Integers.java`, `Sequences.java` (`Seq`, `Len`, `Head`, `Tail`, `Cons`, `Append`, `Concat`, `SubSeq`, `SelectSeq`), `FiniteSets.java`, `Strings.java`, `TLC.java` (`Print`, `PrintT`, `ToString`, `Assert`, `MakeFcn`, `CombineFcn`, `SortSeq`, `Permutations`), `TLCEval.java`; `tlc2/value/impl/MethodValue.java` (`eval`: which errors are wrapped) and the methods' Java signatures; `tlc2/output/MP.java` (message templates of the codes used); commit `94d0c50` | MIT | Semantics matched, including error texts and the override error wrapping; verified against TLC transcripts |
| `dive-eval` fixtures (`crates/dive-eval/tests/fixtures/`) | Transcripts of the pinned `tla2tools.jar` (`tlc-2026.10.06.014338`) captured with `dive-conformance dev capture` | MIT (TLC output) | Test data |
| `dive-model` binding (`crates/dive-model/src/bind.rs`, `spec.rs`, `tables.rs`) | tlaplus `tlc2/tool/impl/SpecProcessor.java` (`processSpec` from `initializeConstants`, `processConstantDefns` (both loops and the inner-module recursion), `processConfig`, `processConfigInvariants`, `processConfigInitAndNext`, `processConfigSpec` with `SubscriptCollector`, `processConfigProps`, `processModelConstraints`, `processActionConstraints`, `makeConstantTable`, `initializeModConstants`, `processConstants`), `tlc2/tool/impl/Spec.java` (`getViewSpec`, `getAliasSpec`, `getPostConditionSpecs`), `SymbolNodeValueLookupProvider.lookup` (tool objects of nodes and bodies), `tla2sany/semantic/ModuleNode.processConstantDefns`, `isParameterFree`, `getOpDefs`, `Context.getOpDefs`, `tlc2/tool/impl/ModelConfig.java` (the `Hashtable`s of constants and overrides), `tlc2/output/MP.java` (templates of the codes used), `tlc2/output/EC.java`; commit `94d0c50` | MIT | Semantics matched step by step, in TLC's order, including the errors and their texts; verified against TLC's own binding of 721 models (`crates/dive-model/acceptance/README.md`) |
| `dive-model` level bound (`crates/dive-model/src/level.rs`) | tlaplus `SymbolNodeValueLookupProvider.getLevelBound`, `getLevelBoundAppl`, `tlc2/tool/BuiltInOPs.isTemporal`/`isAction`, the `minLevel` of `@Evaluation`/`@TLAPlusOperator` annotations (`TLCGetSet`, `TLCExt`, `_JsonTrace`, `_TLCTrace`, CommunityModules `IOUtils`); commit `94d0c50` | MIT | Semantics matched, including the name-based `getVarLoc` test |
| `dive-model` action expansion (`crates/dive-model/src/actions.rs`) | tlaplus `tlc2/tool/impl/Tool.java` `getActions`, `getActionsAppl` (the value-dependent parts: argument evaluation in `TLCState.Empty`, `contexts`, both `catch` blocks), `tlc2/tool/Action.java` `getLocation`; commit `94d0c50` | MIT | Semantics matched; every label of the 141 Examples models and 486 test models agrees with TLC's |
| `dive-model` name feed (`crates/dive-model/src/intern.rs`, `tlc_names.rs`; `dive-ir` `intern.rs`) | Measured on the pinned jar: `util.UniqueString`'s table read with `acceptance/ModelDump.java`; tlaplus `tlc2/tool/impl/ModelConfig.parseValue` (values interned before SANY runs), `tla2sany/semantic/StringNode` (contents interned at semantic analysis); commit `94d0c50` | MIT (behaviour and two name lists) | The pre-parse and syntax-node name lists are data read from the jar; the order is reproduced, not copied code |
| `dive-model` oracle (`crates/dive-model/acceptance/ModelDump.java`) | Calls the pinned jar's `FastTool`, `ParameterizedSpecObj.PostCondition`, `SpecProcessor` getters, `Spec` getters, `UniqueString`'s `InternTable` (by reflection) | MIT (calls only) | Test tooling, not a port |
| `dive-model` fixtures (`crates/dive-model/tests/fixtures/`) | Transcripts and bindings of the pinned `tla2tools.jar` (`tlc-2026.10.06.014338`), captured with `dive-conformance dev capture` and `acceptance/dump_models.sh` | MIT (TLC output) | Test data |
| `dive-eval` scopes (`crates/dive-eval/src/scope.rs`) | tlaplus `tlc2/tool/Action.java` (`pred`, `con`), `SymbolNodeValueLookupProvider.getOpContext`, `getVal`, `tlc2/tool/Specs.addSubsts`; commit `94d0c50` | MIT | Semantics matched |
| `dive-stdlib` `java.util.Random` (`random/source.rs` `JavaRandom`, `RandomSource`) | The Java SE 17 API specification of `java.util.Random` (`Random(long)`, `setSeed`, `next`, `nextInt(int)`, `nextLong`, `nextDouble`), which fixes the algorithms; tlaplus `tlc2/util/RandomGenerator.java` (`primes`, `nextPrime(Random)`, `setSeed(long, long)`); commit `94d0c50` | Java SE specification (no OpenJDK code used); MIT (tlaplus) | Written from the specification; the primes table transcribed. Verified bit for bit against the pinned JRE and jar (`crates/dive-stdlib/tests/java_random.rs`) |
| `dive-stdlib` TLC seeding (`random/seeding.rs` `Streams`) | tlaplus `tlc2/value/RandomEnumerableValues.java` (`TLCStateRandom.initialize`: `fingerPrint() ^ seed` per expanded state; `DefaultRandom`), `tlc2/TLC.java` (`-seed` and `-aril` parsing, default seed `new RandomGenerator().nextLong()`, `setSeed` calls), `tlc2/tool/Simulator.java` (worker seeds), `tlc2/tool/ConcurrentTLCTrace.java` `getTrace` (reset for trace rebuilding), `tlc2/tool/impl/SpecProcessor.java` `processConstantDefns` (start-up evaluation order); commit `94d0c50` | MIT | Semantics matched; verified against TLC runs at six seeds (`crates/dive-stdlib/tests/tlc_random.rs`) |
| `dive-stdlib` random operators (`random/ops.rs`, `random/lcg.rs`) | tlaplus `tlc2/module/TLC.java` `RandomElement`; `tlc2/module/Randomization.java` (`RandomSubset`, `RandomSetOfSubsets`); `tlc2/value/impl/SetEnumValue.java` and `IntervalValue.java` (`randomElement`, `getRandomSubset`, `elements(int)`), `EnumerableValue.java` (`SubsetEnumerator`, `computeOptimalMandA`), `SetOfFcnsOrRcdsValue.java` (`getRandomSubset`, `SubsetEnumerator`, `BigIntegerSubsetEnumerator`), `SetOfFcnsValue`/`SetOfRcdsValue`/`SetOfTuplesValue` (`needBigInteger`, `product`), `SubsetValue.java` (`CoinTossingSubsetEnumerator`); `org/apache/commons/math3/primes/TLCPrimes.java` (`primeFactors`); `tlc2/output/MP.java` (2154, 2169, 2176 texts); commit `94d0c50` | MIT | Semantics matched: same draws in the same order, same errors. `computeOptimalMandA` checked against TLC's own method; operators against TLC output |
| `dive-value` quiet mode (`value.rs` `quiet`) | ADR 0012's rule that `==`, `Hash` and `Debug` change nothing TLC can observe | n/a | dive's own design |
| `dive-eval` successor generator (`crates/dive-eval/src/next.rs`) | tlaplus `tlc2/tool/impl/Tool.java`: `getInitStates` (both overloads, `getInitStatesAppl`), `getNextStates` (`getNextStatesImpl`, `getNextStatesImplSubstInKind`, `getNextStates0`, `getNextStatesAllAssigned`, `getNextStatesApplImpl`, `getNextStatesApplUsrDefOp`, `getNextStatesApplSwitch`: every `OPCODE_*` case), `processUnchangedImpl` (`processUnchangedImpl0Arity`, `processUnchangedImplTuple`, `processUnchangedImplVar`, `verifyUnchanged`), `enabledImpl`, `enabled(ActionItemList, ...)`, `enabledApplImpl`, `enabledUnchangedImpl`, `OPCODE_enabled` in `evalAppl`, `getActions`/`getActionsAppl`, `getFcnContext`, `contexts`; `tlc2/tool/ActionItemList.java` and `IActionItemList` (kinds); `tlc2/tool/impl/Spec.getPrimedVar`; `SymbolNodeValueLookupProvider.java` (`getVar`, `lookup`, `getVal`, `getOpContext`); `tlc2/util/Context.java` (`branch`, cut-off `lookup`); `tlc2/tool/Action.java` (`getLocation`); `tlc2/output/MP.java` (`TLC_EXPECTED_EXPRESSION_IN_COMPUTING`, `TLC_EXPECTED_EXPRESSION_IN_COMPUTING2`, `TLC_UNCHANGED_VARIABLE_CHANGED`, `TLC_ENABLED_WRONG_FORMULA` templates; `printWarning`); commit `94d0c50` | MIT | Semantics matched rule by rule, including the order of conjuncts, of `\A`'s bindings and of `UNCHANGED` tuples, duplicates, the all-assigned fast path and the error texts; verified against the TLC probes, 18 rule fixtures and the Examples goldens |
| `dive-eval` TLC enumeration order (`crates/dive-eval/src/tlc_enum.rs`) | tlaplus `tlc2/value/impl/` `elements()` and their enumerators: `SetEnumValue`, `IntervalValue`, `SubsetValue` (`elementsNormalized`), `SetOfFcnsValue` (`Enumerator`, `DomIVEnumerator`), `SetOfRcdsValue`, `SetOfTuplesValue`, `SetCupValue`, `SetCapValue`, `SetDiffValue`, `UnionValue`, `SetPredValue`; `convertAndCache`, `EnumerableValue.elements(Ordering)`; commit `94d0c50` | MIT | Semantics matched for the order and multiplicity of `x' \in S`; a history-dependent case is refused |
| `dive-eval` vendored Examples models (`crates/dive-eval/tests/fixtures/examples/`) | tlaplus/Examples at `6a7b26a`: 22 models' cfg files and modules, verbatim | MIT | Test data |
| `dive-eval` test model binding and search (`crates/dive-eval/tests/harness/model.rs`) | tlaplus `tlc2/tool/impl/SpecProcessor.java` (`processConfigInitAndNext`, `processConfigSpec`, `processConfigProps`, the invariant and constraint lists), `tlc2/tool/ModelChecker.java` (`DoInitFunctor.addElement`), `tlc2/tool/Worker.java` (`run`, `addElement`); commit `94d0c50` | MIT | Test tooling: the parts needed to compare counts with the goldens, until `dive-model` and `dive-check` exist |

| `dive-check` search (`crates/dive-check/src/engine.rs`) | tlaplus `tlc2/tool/ModelChecker.java` (`modelCheck`: assumptions, `doInit`, the no-statistics exit on an initial-state stop, `checkPostCondition` after a complete search; `DoInitFunctor.addElement`; `reportSuccess`), `tlc2/tool/Worker.java` (`run`, `addElement`: generated count, `CONSTRAINT`/`ACTION_CONSTRAINT`, insertion before checks, invariants on new or out-of-model states, implied actions on every step, deadlock as no successor generated, depth), `tlc2/tool/impl/Tool.java` (`checkPostConditionWithContext`), `tlc2/tool/TLCStateMut.java` (`fingerPrint` with `VIEW`), `tlc2/output/EC.java` (`ExitStatus.errorConstantToExitStatus`); ADR 0015 §1–§4 for the order, the counts at a stop and replay; commit `94d0c50` | MIT | Semantics matched event by event; verified on the 19 TLC probes and 11 harness fixtures (counts, depth, every counterexample state and label) and on the Examples small tier |
| Pure VIEW concrete normalization (`dive-check/src/engine.rs`, `state_key_of`) | `tlc2/tool/TLCStateMut.java` `fingerPrint` lines 229–249; `SetEnumValue.deepNormalize`, `TupleValue.deepNormalize`, `RecordValue.deepNormalize`, pinned `94d0c50` | MIT semantics | Every concrete variable is deep-normalized before VIEW even without symmetry. Concrete states remain independent from projected keys. `SetPredValue.deepNormalize` visits evaluator-owned filters’ bound sets through a callback-free `Any` downcast; predicates are never forced. Unproved deferred/cache-normalization failures explicitly refuse; no guessed EC1000 mapping. Paired clean/native source and independent witness/recovery controls: `conformance/probes/view-concrete-normalization/`. |
| `dive-eval` `Bags` and CommunityModules overrides (`crates/dive-eval/src/community.rs`) | CommunityModules `modules/tlc2/overrides/` at `78b98ae` (release `202610040242`): `SequencesExt.java` (`SetToSeq`, `SetToSeqs`, `Contains`, `longestCommonPrefix`, `foldSeq`, `foldLeft`, `foldRight`, `foldLeftDomain`, `foldRightDomain`, `replaceFirstSubSeq`, `replaceAllSubSeq`, `isPrefix`, `selectInSeq`, `SelectInSubSeq`, `selectLastInSeq`, `SelectLastInSubSeq`, `removeFirst`, `removeFirstMatch`, `Suffixes`, `AllSubSeqs`), `Functions.java` (`IsInjective`, `antiFunction`, `foldFunction`, `foldFunctionOnSet`), `FiniteSetsExt.java` (`quantify`, `foldSet`), `Bitwise.java`, `BagsExt.java` (`foldBag`), `AbstractGraphs.java`, `Graphs.java`, `UndirectedGraphs.java` (`UnionFind`), `DyadicRationals.java` (`reduce`, with commons-math `ArithmeticUtils.gcd`), `IOUtils.java` (`atoi`); tlaplus `tlc2/module/Bags.java`, `tlc2/value/impl/MethodValue.java` (`eval`: the wrapping of non-`EvalException`s, the spreader's casts) and `EvaluatingValue.java` (`eval`: every exception wrapped, the `null` fallback to the TLA+ definition), `tlc2/output/MP.java` (2145, 2169, 2176, 2177, 2183, 2283 texts); commit `94d0c50`. The JVM's exception messages (`NullPointerException`, `StringIndexOutOfBoundsException`, the spreader's `ClassCastException`) measured on the pinned JRE 17 | MIT | Semantics matched: argument checks, errors, the order arguments are walked in; refused where the Java result depends on an unnormalized function's order. Verified on 60 fixtures (`crates/dive-eval/tests/fixtures/specs/Cm*`, `ErCm*`) and CommunityModules' own `AllTests` |
| `dive-eval` vendored CommunityModules modules (`crates/dive-eval/tests/fixtures/community-modules/`) | The `.tla` files of `CommunityModules-202610040242.jar`, verbatim, with the repository's `LICENSE` | MIT | Test data: TLC resolves them from the jar on its classpath |
| `dive-eval` TLC module run state (`crates/dive-eval/src/host.rs`) | tlaplus `tlc2/module/TLCGetSet.java` (`TLCGetEval`, `TLCGetStringValue`, `TLCSet`, the static names), `tlc2/tool/AbstractChecker.java` (`createConfig`, `getStatistics`, `getAllValues`, `getAllNamedRegisterValues`), `tlc2/util/IdThread.java` (registers), `tlc2/tool/TLCState.java` (`INIT_LEVEL`, `getLevel`); commit `94d0c50` | MIT | Semantics matched; verified on the `Tm*` fixtures of `crates/dive-check/tests/fixtures/tlc-module/` and `ErCmTlc*` |
| `dive-stdlib` run host (`crates/dive-stdlib/src/host.rs`) and its use in `dive-check` (`engine.rs` `run_host`, `seed_state`, replay) | tlaplus `tlc2/value/RandomEnumerableValues.java` (the per-state reseed), `tlc2/tool/ConcurrentTLCTrace.java` `getTrace` (fresh generator for trace rebuilding), `tlc2/tool/ModelChecker.java` `stop` (`TLCSet("exit")`); commit `94d0c50` | MIT | Semantics matched; draws, levels, registers and the stop verified against TLC (`crates/dive-check/tests/tlc_module.rs`) |
| `dive-check` parallel engine (`crates/dive-check/src/parallel.rs`, `pool.rs`) and ordered `Print` capture (`crates/dive-eval/src/capture.rs`) | dive's own design (ADR 0015 §2–§3), not a port: it reproduces the single-worker semantics of the row above (`Worker.run`/`addElement`), and the per-run warning history of `tlc2/output/MP.java` (`printWarning`, `warningHistory`); commit `94d0c50` | MIT | Results equal the inline engine's at 1–64 workers and under chaos (`crates/dive-check/tests/invariance.rs`); harness at 1, 3, 4, 8 workers |
| `dive-check` error classification (`class.rs`, `engine.rs` replay and postconditions) | tlaplus `tlc2/tool/Worker.java` `run`; `ModelChecker.java` `doInit`, `doNext`, `doNextFailed`, `doNextEvalFailed`, `modelCheckImpl`; `tlc2/tool/impl/Tool.java` `checkPostConditionWithContext`, `isValid`, `getNextStates`; `tlc2/value/impl/MethodValue.java` `eval`; `tlc2/output/MP.java` error wrapping; commit `94d0c50` | MIT | Matched against clean pinned transcripts in `crates/dive-check/tests/fixtures/errors/`; value-source-dependent deciding classes remain unverified, and the harness refuses them |

| `dive-liveness` accepting-cycle search (`search.rs`) | Acceptance conditions of tlaplus `tlc2/tool/liveness/LiveWorker.java` `checkComponent`, pin `94d0c50`; the SCC, shortest-prefix and covering-walk algorithms are dive’s own implementation of ADR 0018 §5 | MIT / own design | Supplied product graphs only, independently checked against finite closed-walk search. Does not reproduce TLC’s lasso choice or yet produce model verdicts |

| `dive-trace` named periodic validation (`validate.rs`, `format.rs`) | Independent direct TLA+ periodic semantics from ADR 0018 §8; implicit stuttering after generated-successor filtering matches `tlc2/tool/Worker.java` `doNextCheckLiveness`, pin `94d0c50` | Own design / MIT semantics | No successor enumerator, graph or tableau. Eight pinned words validate, including recurring-action WF/SF certificates; enabledness-dependent fairness and unestablished closing evidence refuse. |

| `dive-trace` recurring-action fairness (`temporal.rs`) | Direct TLA+ implication `GF <<A>>_v` implies both `WF_v(A)` and `SF_v(A)`; ADR 0018 §8 | Own design | Pure predicate replay and profile equality on supplied concrete edges only; no graph, tableau or successor enumerator. Exhaustive small-word, empty-subscript and false-action controls. |
| `dive-liveness` formula/tableau kernel (`formula.rs`) | Own direct temporal semantics: negation duality, `[]F = F /\ next([]F)` and `<>F = F \/ next(<>F)`, with recurring eventuality promises (ADR 0018 §2) | Own design | Generalized Buchi language checked on 166,516 short two-atom periodic words against direct reachability semantics. Pure bound lowering is implemented separately below; TLC-exact planning/numbering remain pending. |

| `dive-liveness` pure predicate planning (`plan.rs`) | Own direct boolean/temporal lowering, retaining the bound scopes supplied by `dive-model` and `dive-eval` (ADR 0018 §2) | Own design | Seven pinned property words and ten short-word formula controls. Does not implement TLC’s complete `Liveness.astToLive`/DNF/classification/order or enable model checking. |
| `dive-liveness` explicit product (`product.rs`) | Synchronous generalized Buchi automaton product and direct edge semantics (ADR 0018 §4) | Own design | Supplied graph/label/tableau data only. Every short-word witness is checked directly; all two-vertex graph/label combinations match independent path closure. No TLC numbering, graph capture or model verdict claim. |

| `dive-check` graph capture (`capture.rs`, `engine.rs`, `parallel.rs`) | Existing ported safety event order above; implicit stuttering follows `tlc2/tool/Worker.java` `doNextCheckLiveness` at pin `94d0c50` (ADR 0018 increment) | MIT semantics / own retention design | Same counters; retains seen/duplicate accepted edges, with explicit bounded incomplete outcomes. Ordered parallel retention in fixed eight-source waves, with a separately bounded per-source scratch admission; full inline/1/4/8/chaos graph and stop controls. No re-enumeration, phase-L or VIEW/symmetry projection claim. |
| `dive-liveness` pure final coordinator (`analyze.rs`, `plan.rs` `lower_violation`) | Direct behavior-conjunction AND negated-property language, supplied by the bound model; ADR 0018 increment | Own design | Four pinned pure models match TLC counts in both stores and produce independently validated candidates. Four WF specifications refuse. No TLC-exact selection, complete claim-set, periodic or CLI verdict claim. |


| Independent finite covers (`dive-trace/src/exists.rs`) | TLA+ existential action semantics; ADR 0016 amendment 2 | n/a | Original necessary-cover derivation and Cartesian verification, without `dive-eval::next` or graph. Pinned `EnabledFiniteCover` confirms 36 enabledness decisions. |
| Reached ordinary query dispatch (`dive-eval/src/enabled.rs`, `dive-trace/src/ordinary_queries.rs`) | Existing TLC evaluation order and ADRs 0099/0100; original independent finite-cover procedure | Own design | The interpreter supplies only the reached action and actual scope/state. No checker or graph decision; weak resolver cache identity, shared independent counters, four pinned initial words and sixteen primary direct/INSTANCE WF/SF controls. General callable/planner integration remains open. |
| Independent WF/SF enabledness (`dive-trace/src/temporal.rs`) | TLA+ fairness definitions; ADR 0018 §8 | n/a | Recurring non-stuttering occurrence, otherwise GF/FG of independently decided disabledness. Pinned `LvWFIntermittent` and `LvStutterEnd`. |

| Scoped checker enabledness (`dive-eval/src/next.rs` `enabled_scoped`) | Existing `Tool.enabled` and StepAngle CHANGED continuation at pin `94d0c50` | MIT | Exposes the existing generator-side path to fairness atoms; deliberately independent of the validator finite-cover procedure. |
| WF/SF formula expansion and candidate attribution (`dive-liveness/src/plan.rs`, `formula.rs`, `analyze.rs`) | TLA+ fairness identities and direct periodic-word semantics; ADR 0018 increment | Own design | Ten pinned violations and three successes match counts; all candidates independently validate. Complete false-property set tested separately from tableau visits. No TLC numbering/selection claim. |


The final temporal API coordinator (`dive-api::temporal`) binds and validates
through separate evaluator instances. This orchestration is an original design;
it ports no Java algorithm. It uses the previously recorded safety exploration,
formula/product and independent trace semantics.


| Component | Reference | License | Relationship |
|---|---|---|---|
| Bounded expression contexts (`dive-eval::scope_bindings_bounded`) | `Tool.contexts` / `ContextEnumerator`, pinned tlaplus `94d0c50`; existing `for_each_context` semantics | MIT | Same first-variable-fastest binding/profile enumeration order, explicit domain/context bounds; no successor generator |
| Original PROPERTY claims and finite temporal expansion | TLA+ finite quantifier semantics; owned `Qt*` oracle transcripts under `crates/dive-api/tests/fixtures/temporal-quantifiers` | original design | Shared binding retains unsplit claims; planner and independent direct word evaluator evaluate them separately |

| Temporal conditional lowering and word truth (`dive-liveness/src/plan.rs`, `dive-trace/src/temporal.rs`) | Pointwise TLA+ IF semantics; tlaplus `tlc2/tool/liveness/Liveness.java` `astToLive` at `94d0c50` for supported constructors | Own formula/word algorithms; MIT reference | Three pinned positive and two negative probes; independent finite Boolean-word algebra. Action guards and temporal CASE remain refused. |
| Finite temporal-prefix oracle classification (`dive-conformance/src/tlc/result.rs`) | tlaplus `tlc2/tool/liveness/LiveCheck.java` `printErrorTrace` and `tlc2/output/EC.java` at `94d0c50` | MIT | Source-verified print-2116/store-2110 path, checked against the generated `QtEmpty` transcript and malformed/mismatched stream controls. General EC mapping unchanged. |

| Constant-argument temporal calls (`dive-liveness::plan`, `dive-trace::temporal`) | Independent formula lowering and periodic-word evaluation over the existing evaluator's value-bound scopes; pinned `Pc*` probes matched against `Tool.getOpContext` / `Liveness.astToLive` behavior at `94d0c50` | MIT reference; original implementation | Pure constant arguments only; context-sensitive preflight and explicit bounds; no TLC-exact planning-order claim |

| Scoped finite ENABLED covers (`dive-trace::exists`) | Independent necessary-cover existential search over value-bound scopes; original predicates test every witness. Pinned `EnabledScopedCover` / `Sf*` probes compare with `Tool.enabled` at `94d0c50` | MIT reference; original implementation | No successor enumerator dependency; context-sensitive primed-read scan and constant-argument purity proof; missing covers refuse |

| Pure state temporal arguments (`dive-liveness::plan`, `dive-trace::temporal`) | Original scoped-level analysis and independent periodic-word evaluation over evaluator LazyValue-style caller scopes; `Sp*` probes matched against `Liveness.astToLive` / `Tool.getOpContext` at `94d0c50` | MIT reference; original implementation | State arguments are not evaluated as constants; nested scopes and priming checked against direct word algebra; no exact planning/timing claim |

| Finite quantified predicates and necessary covers (`dive-liveness::plan`, `dive-trace::temporal`, `dive-trace::exists`) | Original scope-sensitive purity/level preflight, direct word semantics and necessary-cover union/conjunction rules; pinned `Qf*` / `EnabledQuantifiedCover` compared with `Liveness.astToLive` / `Tool.enabled` at `94d0c50` | MIT reference; original implementation | Every context inspected, original relation verifies each witness; no validator enumerator dependency or TLC-exact planning/selection claim |

| LET temporal/finite-cover scopes (`dive-liveness::plan`, `dive-trace::temporal`, `dive-trace::exists`) | Original context-sensitive lowering, direct words and necessary covers over evaluator parent frames; `Lt*` probes compared with `Tool.getOpContext` / `Liveness.astToLive` / `Tool.enabled` at `94d0c50` | MIT reference; original implementation | Lazy zero-arity values, nested local operators and priming checked against pinned words and exhaustive assignments; no validator enumerator or exact temporal selection/timing claim |
| Symbolic function-set emptiness (`dive-value::enumerate`, `dive-value::stream`, `dive-eval::core`) | `SetOfFcnsValue.elements`, `SetOfFcnsValue.size` and `Value.isEmpty`, at `94d0c50` | MIT | Empty domain/range short circuits preserve finite equality/hash/fingerprint identity; quiet operations keep normalization unobservable; nonempty non-enumerable domains remain refused |

| INSTANCE temporal scopes (`dive-liveness::plan`, `dive-trace::temporal`) | Original context-sensitive lowering and independent periodic words over evaluator substitution frames; `Is*` probes checked against `Tool.getOpContext`, `SubstInNode`, and `Liveness.astToLive` at `94d0c50` | MIT reference; original implementation | Value/variable replacements stay lazy through nested modules and priming; exhaustive short words and pinned truth/counts; no enumerator in validation or exact temporal selection/timing claim |
| Original declarations under absent temporal substitutions (`dive-liveness::plan`, `dive-trace::temporal`) | Existing evaluator lookup contract matched against `SymbolNodeValueLookupProvider.lookup` and `Tool.evalAppl` at `94d0c50`; four holding and two violated pinned constant/INSTANCE controls | MIT reference; original implementation | Checked absent-versus-unset lookup and original scoped constant-body inspection, independently implemented in planner and validator; explicit opaque/effect/recursion and finite-cover refusals remain. No admission proof or checker enumeration supplies word truth. |

| Generated ValidationTestSuite smoke corpus | `testgen.spec.render_spec` / `prepare_testcases` and `testgen.feature.combine`, ValidationTestSuite `fe99cff1ca48535bdbfc99fd95bd2706f056c12f` | Apache-2.0, NVIDIA copyright preserved | 16 unmodified single-worker TLC sides; all 36 selected cases accounted for; oracle facts generated separately; adapter original |

| Primed independent quantifier domains (`dive-trace::exists`) | Original necessary-cover soundness rule: enclosing prime affects domains as well as bodies; finite current-state bindings are not a proof about successors | n/a | #94 regression provides concrete satisfying successors before checking refusals; no successor enumerator dependency |
| Finite data-constructor temporal scopes (`dive-eval::scope`, `dive-liveness::plan`, `dive-trace::temporal`) | Context bindings matched against `ContextEnumerator`, `SetOfAllValue`, `SetPredValue` and `FcnLambdaValue` at `94d0c50`; level/purity walks and periodic words original | MIT reference; original implementation | Seven clean `Bc*` truth/count probes, exhaustive short words, bounded predicate bindings and named effect/resource refusals; no validator enumerator or exact TLC temporal-selection claim |

Finite-constructor independent covers inspect `SetOfAllValue`, `SetPredValue`
and `FcnLambdaValue` binding semantics at TLC commit `94d0c50` (MIT). The
necessary-cover search remains an original, separately implemented procedure;
it does not call `Tool.getNextStates` or the checker enumerator.

The frontend campaign grammar driver invokes `TLAplusParser.parse` and
`moduleName` exactly as `ParseUnit.parseFile` uses them at TLC `94d0c50`
(MIT). The reported PlusCal validation error is classified from
`SpecObj.validateParseUnit` / `SANY.frontEndParse`; generic exceptions never
establish a root-grammar fact. See `scripts/java/README.md` for source and
bytecode pins, and `docs/frontend-campaign.md` for the separate measurements.

| Component | Reference | License | Evidence |
|---|---|---|---|
| State-dependent finite predicate preflight (`dive-eval::finite`, `scope_binder_analysis`, `dive-liveness::plan`, `dive-trace::temporal`) | Original conservative scoped analysis and bounded concrete walks; TLA+ quantifier/constructor truth compared with TLC `ContextEnumerator`, `SetOfAllValue`, `SetPredValue`, `FcnLambdaValue` at `94d0c50` | Original implementation; MIT reference | Eight `Df*` pinned truth/count probes, independent periodic validation and exhaustive short words; no sample values, frozen domains or validator successor enumeration |

Finite CHOOSE predicate scopes extend the original conservative analysis in
`dive-liveness::plan` and `dive-trace::temporal`; evaluation continues to use
`dive-eval::Core::eval_choose`, matching TLC `Tool.java` bounded CHOOSE evaluation
at `94d0c50` (MIT). Five `Cf*` probes, independent short-word algebra and
captured-domain level controls supply evidence. No Java search/enumerator is
used by the independent validator.

Pure action-helper argument scopes (`dive-liveness::plan`, `dive-trace::temporal`)
use the original conservative argument analysis and shared lazy-expression
binding primitive, matching TLA+ value/action operator application and TLC
`Tool.evalAppl` at `94d0c50` (MIT reference). Four `Aa*` probes and independent
short-word algebra verify retained before/after context; the validator uses no
checker successor enumerator.

Temporal recurrence factoring and reachable-product construction in
`dive-liveness::formula::recurrence` / `product` are original implementations of
the infinite-word identity `( []<>A \/ []<>B ) <=> []<>(A \/ B)` and conjunctive
Buchi acceptance (ADR 0018). Atom ids/scopes are never merged. Tableau expansion
reuse preserves the first expansion's order; retained product pairs are
renumbered in the original `(state, tableau)` order. Evidence includes independent
short-word/path-closure checks, resource and discovery-order controls, and three
`Fw*` probes from the pinned TLC `94d0c50` (MIT reference). No search algorithm
is added to the independent validator.

## Removed liveness emulation (2026-10-12)

The TLC-internal liveness ports listed below in the sections on TLC planning,
tableau projection, phase-L receipts, graph capture, ordered temporal execution,
final-only SCC and witness selection, the periodic controller, lifecycle, hook,
snapshot, optional role, terminal and stopped-queue services were removed
(ADR 0082). Their rows are kept as history. The following pieces remain:

| Component | Matched upstream | Licence | Notes |
| --- | --- | --- | --- |
| `dive-liveness/src/plan.rs` `lower` | `tlc2/tool/liveness/Liveness.java` `astToLive` at `94d0c50` | MIT | One shared pure planner retains conversion errors, constant guard selection, scoped state/action fallback and WF/SF semantics. Numeric temporal operand `<>1` is checked against the pinned oracle: 2215 (`TLC_EXPECTED_VALUE`), exit 255. No tableau numbering or scheduler order. |
| `dive-liveness/src/tlc_plan.rs` `classify` | `tlc2/tool/liveness/Liveness.java` `processLiveness`, `classifyExpr`; `LNConj`/`LNDisj` `pushNeg`, `simplify`, `toDNF`, `flattenSingleJunctions` | MIT | Only the verdict: TLC errors 2214 (action outside the EA/AE forms) and 2213 (DNF size beyond a Java `int`). No tags, bins or grouping order. |
| `dive-liveness/src/periodic_backend.rs` `Growth::due` | `tlc2/tool/liveness/LiveCheck.java` `check` growth test against `TLCGlobals.livenessThreshold` | MIT | The size comparison only, in `f64`; applied to captured distinct-state counts. |
| `dive-model/src/periodic.rs` `resolve` | `tlc2.tool.impl.SpecProcessor.processPeriodic` | MIT | Binding errors only; the hook is refused. |

## Symmetry closure foundation

`dive-model/src/symmetry.rs` matches `MVPerms.permutationSubgroup` and
`MVPerm.compose` at tlaplus `94d0c50` (MIT): generators first, identity and
duplicate removal, ordered frontier closure, and first-map-then-second-map
composition. The bounded dense index representation, checked bijectivity and
resource refusals are original implementations. The 1,329 direct Java-kernel
fixtures include malformed maps. That kernel evidence does not establish
`Tool.getSymmetryPerms`, its warnings, canonicalization or state/trace reduction;
their separate implementations and controls are recorded below. The original MIT
Java driver and raw SyEmpty capture are in
`crates/dive-model/tests/fixtures/symmetry-group/`.


The pure symmetry declaration stage matches `SpecProcessor`'s pre-constant
`Defns.snapshot` and `Tool.getSymmetryPerms`' name, original-body evaluation,
enum-set normalization and function-record checks at tlaplus `94d0c50` (MIT).
Early errors are checked against the clean pinned symmetry controls;
configuration replacement and pure/effectful controls supplement them. Warning,
model-value and group checks belong to `prepare_symmetry`; safety key reduction
and concrete trace retention belong to the checker.

Division by zero while evaluating the declaration and incomparable values while
normalizing its enumerated set now use TLC's outer `GENERAL` startup class
(1000, exit 255), rather than the inner evaluator code. The source basis is
`Tool.getSymmetryPerms`, `TLC.process`'s outer exception catch and
`MP.ECGeneralMsg` at the same pin: `EvalException` retains its class and nested
2179 message; `Assert.TLCRuntimeException` is printed as `RuntimeException`.
The consumed `SyDivZero` and `SyMixedElements` specifications and model tests
check the exact messages. Public API/CLI controls check both profiles and show
these errors precede a false ASSUME without exploring states. Other unverified
declaration-evaluation and normalization errors remain named refusals; this is
not general JVM exception support or full symmetry startup certification.


## Bounded lazy binder domains

`dive-eval/src/bounded_domain.rs` is an original structural resource preflight
for `scope_bindings_bounded`; it does not port a checker enumerator. The unchanged
binding/exploration path matches TLC `ContextEnumerator` and the value constructors
`SubsetValue.elements`, `SetOfFcnsValue.elements`, `SetOfRcdsValue.elements`,
`SetOfTuplesValue.elements` at tlaplus `94d0c50` (MIT reference). Raw cardinality
upper bounds are inspected without normalization, and the original size and
exploration code subsequently determines exact errors and order. The empty-domain
function short circuit matches `SetOfFcnsValue`. Eight clean pinned `Ld*` captures,
both-profile enumeration/error-order controls and independent periodic-word checks
provide evidence. The validator shares expression binding but retains its separate
word/ENABLED algorithms and no successor enumeration feature.

## Symmetry safety

| Increment | Reference | License | Matched behavior |
|---|---|---|---|
| Symmetry preparation (`dive-model/src/symmetry/prepare.rs`) | `tlc2/tool/impl/Tool.java` `getSymmetryPerms`, `addToSubTwoSizedSymmetrySetList`, `getSetEnumValueFromArgumentNode`; `ModelConfig.getOverridenSpecNameForConfigName`; `tla2sany/parser/SyntaxTreeNode.getHumanReadableImage`; pinned `94d0c50` | MIT | Pure startup ordering, supported argument shapes, source-backed warning text and cfg aliases. Unverified errors remain refusals. |
| Symmetry canonical keys and normalization | `tlc2/tool/TLCStateMut.fingerPrint`; `SetEnumValue`, `FcnRcdValue`, `TupleValue`, `RecordValue` `permute` / `deepNormalize`; pinned `94d0c50` | MIT | Minimum image in run order, VIEW after symmetry, concrete state normalization side effects; independent concrete predecessors retained. Deferred/lazy permutations remain refused. |
## Finite safety simulation

| Increment | Reference | License | Matched behavior |
|---|---|---|---|
| `dive-sim` finite safety selection and callback ordering | tlaplus `tlc2/tool/Simulator.java` `simulate`, constructor worker seeding; `tlc2/tool/SimulationWorker.java` `simulateRandomTrace`, `randomState`, `getNextActionIndex`, `getNextActionAltIndex`, `addElement`, `getTrace`; `tlc2/util/RandomGenerator.java` `nextPrime`; commit `94d0c50` | MIT | Semantics matched for ordinary single-worker pure finite safety simulation; default per-trace seeding is dive's approved D-003 design; independent trace validation uses existing separate predicates |

Runtime namespace startup matches pinned `AbstractChecker.createConfig`
(`tlc2/tool/AbstractChecker.java`, revision `94d0c50`): mode, seed, fingerprint
and installation strings are interned before assumptions. The independent
validator's runtime-name refusal is dive's own rule (#124; ADR 0016, amendment
3): any name interned after binding refuses the verdict. It is supported by
the clean pinned capture and prior false-positive trace in
`crates/dive-trace/tests/fixtures/runtime-names/`.
## Native DFID (ADR 0021)

| Rust surface | Pinned Java behavior | License | Scope/evidence |
|---|---|---|---|
| `dive-check/src/dfid.rs`: initialization/expansion | `DFIDModelChecker.modelCheckImpl`, `doInit`, `doNext`; `AbstractChecker.createConfig`; tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Eager per-action vectors, constraints/newness/property ordering, counts and inner-DFS draining after a first failure; clean primary controls and separately labelled seeded reference |
| `dive-check/src/dfid.rs`: iteration/status/selection | `DFIDWorker.run`, `getInit`, `getNext`; `FPIntSet.incLevel`, `isCompleted`, `isLeaf`, `MemFPIntSet.setStatus`, `setLeveled`; same pin | MIT | Birth iteration, permanent done and alternating completed bits; Java nextDouble selection and swap removal; no BFS scheduler reuse |
| DFID concrete witness/collision guard/replay default | Dive design under ADR 0010 and 0021 | n/a | Independent certification corrects D-019 stale root witness; exact canonical guard refuses fingerprint collisions; recorded per-iteration seeds can replay native runs |

`SeededDfid.java` is a secondary test adapter: only the worker's selection seed
is injected after construction and before start. It is never presented as an
unmodified oracle or used to certify the complete DFID feature family.

The captured-filter extension admits only evaluator-owned `lazy::Filter`, whose
unchanged expression evaluation matches `SetPredValue.elements` and
`Tool.eval` at tlaplus `94d0c50` (MIT reference). Original input cardinality/value
slot upper bounds and a conservative output-vector bound are charged before
calling it; effectful or unresolved predicates are refused without execution.
Configured helper redirects are checked conservatively for forbidden effects;
open-effect targets remain refusals. It retains the original predicate IR span,
environment, state and profile order.
No new checker enumerator or validator dependency was introduced. Six clean
`FdFilter*`, `FdMap*` and `FdCapture*` captures, both-profile input/cache/effect/
error-order/priming controls and fresh independent periodic-word checks provide
focused final/short evidence. Recursive `FdRecursivePerm*` remains unsupported.


The captured-filter environment guard is original effect-proof admission logic.
It conservatively rejects operator slots whose runtime Rust bindings are not
proved by IR call-site analysis, and checks closed effects of all retained
thunks/functions. The existing shared-side-effect traversal behaviour is
preserved. `rust-operator-capture-before.*` and its fixed no-output regression
retain the motivating native purity gap; neither is TLC conformance evidence.

The filter-only proof treats missing captured-expression metadata as unknown
while preserving the original shared-side-effect helper's missing-node behaviour.
The companion unchecked-`ExprId` repro and both-profile no-panic refusal control
provide evidence; this does not validate every Rust caller-created IR identifier.

| Retained temporal expression arguments (`dive-liveness::plan`, independent `dive-trace::temporal`) | `tlc2/tool/liveness/Liveness.java` `astToLiveAppl`, `astToLive`; `tlc2/tool/impl/Tool.java` `getVal`/operator context; `tlc2/value/impl/LazyValue.java`; pinned commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Semantics matched for scoped lazy expression forwarding; separate bounded purity walks and periodic truth evaluation are dive designs. Fourteen clean paired raw controls include both reference-refused exported INSTANCE calls; no exact tableau/phase-L claim. ADR 0023 |

Retained temporal endpoint ID admission is dive's own boundary check. The native
unchecked-context repro records expression/root/capture panics and a constructor
redirect panic; endpoint controls now refuse the former without output. The
constructor gap is tracked separately as #129 and is not silently certified.

## Native PlusCal source parsing

| dive component | Source | Licence | Nature |
|---|---|---|---|
| `dive-pluscal` source parser (`parser.rs`, `lexer.rs`, `ast.rs`) | tlaplus `pcal/ParseAlgorithm.java` (`GetVarDecls`, `GetPVarDecl`, `GetProcedure`, `GetProcess`, `GetStmtSeq`, `GetCStmtSeq`, `GetIf`, `GetWhile`, `GetEither`, `InnerGetWith`, `GetAssign`, calls/labels/separators); `Tokenize.java` (`IsDelimiter`, `parenDepth`, quantifier/record-field rules), `PcalBuiltInSymbols.java`, `PcalCharReader.java`, `AST.java`; commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Grammar semantics adapted to a source-preserving arena AST. Macro expansion/labels/control-flow normalization and translation remain pending; expression scanner counts nesting and does not validate TLA syntax. Pinned primary/secondary evidence and limitations in ADR 0022 and docs/pluscal-parser.md |

## Bounded TLC planning

| Increment | Reference | License | Matched behavior |
|---|---|---|---|
| Bounded TLC planning projection (`dive-liveness/src/tlc_plan.rs`) | `tlc2/tool/liveness/Liveness.java` `parseLiveness`, `processLiveness`, `classifyExpr`, `addToBin`; `LNConj`/`LNDisj` `pushNeg`, `simplify`, `toDNF`, `flattenSingleJunctions`, `equals`; `LNAll`/`LNEven`/`LNNeg`/`LNBool` normalization; `LNState`/`LNAction` `tagExpr`; pinned commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Matched transformation/grouping semantics with original occurrence order; original bounded arena and work preflight. Raw supplied-formula and actual FastTool projections in `conformance/probes/tlc-planning`. Binary tableau/promise numbering is not ported or certified. ADR 0025 |

The private strict-shape lowering selector matches `Liveness.astToLive`'s WF
form `[]<>(~ENABLED <<A>>_v \/ <<A>>_v)`. The existing language kernel retains
its equivalent form and behavior. Normalization does not evaluate callbacks;
model binding delegates to the original conservative scope/effect planner.
The independent word checks use `dive-trace`'s separate periodic/ENABLED
algorithms; no validator dependency on this projection was introduced.

The `TpLocal` identity probe records TLC's separate Action/configured name
`Live` and derived temporal-name vector `Identity`. `BoundProjection` preserves
original `configured_properties` predicates/names separately from role inputs;
its role table matches the derived pinned field. Future action attribution
through tableau/witness execution remains a separate representation obligation
(#134). Grouped formula truth does not certify that attribution.


## Ordered frontier spill

`dive-store/src/spill.rs` is dive's native transient indexed frontier format
(ADR 0024), using the existing lossless value/state codec. Pinned
`tlc2/tool/queue/DiskStateQueue.java` at `94d0c50` (MIT) supplies the reference
FIFO search contract; this is not a Java checkpoint serialization port.
`crates/dive-check/tests/frontier_spill.rs` compares the same pinned safety
probes across storage, profiles, stores and scheduling;
`crates/dive-api/tests/frontier_spill.rs` independently checks concrete symmetry
witnesses. Checksums, byte accounting and non-evaluating deferred-value admission
are dive's own implementation.

## Bounded TLC tableau projection

| Implementation | Pinned reference | License | Evidence and scope |
|---|---|---|---|
| Bounded TLC tableau projection (`dive-liveness/src/tlc_tableau.rs`) | `tlc2/tool/liveness/TBGraph.java` constructor/findOrCreateNode; `TBGraphNode.java` state predicates/isAccepting; `TBPar.java` positiveClosure/particleClosure/local consistency/impliedSuccessors/isFulfilling; `TBParVec.java` union; `LNConj`/`LNDisj` makeBinary, `LNAll`/`LNEven`/`LNNext`/`LNNeg` tagged equals/extractPromises; pinned commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Particle and promise ordering/numbering semantics matched through actual pinned methods. Arena/work/pending/depth preflight is original bounded admission logic. Complete raw controls and historical selection in `conformance/probes/tlc-tableau`; no graph-capture/SCC/witness/strict execution claim. ADR 0027 |

The independent particle/periodic checker in the tests uses direct state truth,
finite reachability and mutually reachable cyclic components rather than the
production tableau expansion or SCC backend. Original scoped word/witness
validation continues through standalone `dive-trace`; no dependency on
`dive-liveness` was added. Configured/Action and derived role names remain
separate projected fields, not interchangeable attribution evidence.


## Native sequential PlusCal normalization and generation

| dive component | Source | Licence | Nature |
|---|---|---|---|
| `dive-pluscal` sequential action normalization (`translate/lower.rs`) | `pcal/PcalTranslate.java`: `Explode`, `ExplodeStmtSeq`, `CopyAndExplodeLastStmt`, conditional/either/with expansion, while/next-label placement; `pcal/PcalTLAGen.java`: program-counter omission decision; `pcal/PcalSymTab.java`: variable/label admission context; tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Bounded explicit-label sequential semantics adapted to native action trees. Processes/procedures/macros, branch-internal labelled expansion and name disambiguation remain refused. Automatic label insertion is extended separately below. |
| `dive-pluscal` TLA body generation (`translate/emit.rs`) | `pcal/PcalTLAGen.java`: `GenInit`, `GenLabeledStmt`, `GenStmtSeq`, assignment/if/either/with generation, `SortSass`, `NeedsParentheses`; `pcal/TLAExpr.java`: lexical variable substitution and record/field distinction; `pcal/ParseAlgorithm.GetAssert`: assertion keyword position; same revision | MIT | Sequential priming, simultaneous pre-group reads, variable ordering, branch identities, assertions and spec/termination bodies match admitted pinned cases. Multiline expression layout and binder substitution remain refused. Native byte associations do not claim the upstream map/checksum format. |

The complete clean translator/SANY matrix and generated-model validation controls
are in `crates/dive-pluscal/translation-acceptance/`. Both surface syntaxes, token
streams and span/trivia-free CST structure are checked. TLC runs only in the
development capture tools; the Rust runtime invokes no Java or external translator.
No algorithm is borrowed from competitor implementations.

## Public evaluator context admission

| Implementation | Reference | License | Evidence and scope |
|---|---|---|---|
| Public evaluator/context admission (`dive-eval/src/context.rs`, scope/action creation and independent endpoint checks) | Own defensive Rust API design; existing IR structural validator and TLC binding signatures matched above | Own design | Non-evaluating typed admission, source `8f451ff` panic reproduction in `crates/dive-api/tests/fixtures/public-context/`, valid recursion/operator and foreign-scope controls. The `.cfg` constant-substitution arity mapping preserves `SpecProcessor.processConstantDefns` / EC 2225 at pin `94d0c50`; no new checker algorithm port or full caller-IR certification. |


## Sequential labels and default initialization

| dive component | Source | Licence | Nature |
|---|---|---|---|
| `dive-pluscal` automatic labels (`translate/labels.rs`) | `pcal/ParseAlgorithm.java`: `AddLabelsToStmtSeq`, `InnerAddLabels`, `NeedsLabel`, `InnerGetWith` statement locations; `pcal/trans.java` label/labelRoot/reportLabels options; `PcalParams.java` default flags; revision `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Bounded sequential assignment-set/next-label recurrence, typed-label skipping and original positions; branches needing labelled expansion remain refused. Native source AST is unchanged. |
| Default initialization and sequential label modifiers (`translate/emit.rs`, `lower.rs`) | `ParseAlgorithm.GetVarDecl`, `GetLabel`, `PcalParams.DefaultVarInit`, `PcalTLAGen.GenVariables`, `GenInit`, `GenSpec`, `trans.java` defaultInitValue configuration generation; same revision | MIT | Generated constant/model-value binding and modifier-induced program-counter retention match clean controls. Native API returns required configuration as data; no file writes or speculative process fairness. |

Full owned controls and raw primary/secondary transcripts are in
`crates/dive-pluscal/normalization-acceptance/`; contract and reproduction are in
`docs/pluscal-normalization.md`, with ADR 0030. The Java AST adapter is
development-only reference evidence; runtime translation remains native.
## Temporal action short-circuiting

`dive-liveness::plan::PredicateAtom::evaluate` matches `tlc2/tool/liveness/LNAction.java::eval` at revision `94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT). The independent periodic interpreter in `dive-trace/src/temporal.rs` implements the temporal subscript-first rule without depending on the checker, liveness or successor enumerator. Direct pinned `LNAction.eval` and `Tool.eval` controls, retained native failures and scope limits are documented in `conformance/probes/temporal-action-order/README.md`. Ordinary runtime action evaluation continues to match `Tool.eval`; this increment is not strict integrated liveness certification.

| `dive-liveness` phase-L diagnostic receipts (`phase_l.rs`) | tlaplus `tlc2/util/SetOfStates.java` put/grow/iteration; `tlc2/tool/Worker.java` createSetOfStates/doNextCheckLiveness; `tlc2/tool/TLCStateMut.java` equality; `tlc2/tool/liveness/LiveCheck.java` addInitState/addNextState, `OrderOfSolution` checkState/checkAction, `LNAction` eval and `TBGraphNode` isConsistent; commit `94d0c50` | MIT | Bounded material-state collection, concrete first representatives, slot/grow order, logical multiplier history, null-tableau bin order and initial consistency matched. No native graph insertion/SCC/scheduling/lasso claim; forced-key and real bound evidence remain separate. |

## Conditional temporal planning conversion

The ordinary-final composition eligibility extension is native option dispatch,
not a Java algorithm port. Two tiny `CompositionFinal` controls were run before
implementation against pinned TLC94d0c507 with its composition property, full
pinned classpath, worker1/seed0/fp0 and final checking. Hold succeeded2193/0;
Fail violated2116/13 with FALSE,TRUE,stuttering. Both produced5generated/2distinct/
depth2. Compact consumed fixtures record identities; independent composed-Next
replay remains an explicit native refusal. No original FiniteMonotonic gain is
established by these controls.

Deferred-filter redirect-source preflight in `dive-eval/src/bounded_domain.rs`
is original bounded metadata inspection of actual value-bound scalar constants;
it ports no Java purity mechanism. Filter enumeration still uses the existing
evaluator. The tiny `FilterWF` hold/fail controls were run before implementation
against pinned TLC `94d0c5070852b37b801bb8998c436383ab7ee37b` with worker 1,
seed 0, fingerprint 0 and final checking, full pinned classpath and no
`dumpTrace` flag. Both produced 3 generated / 2 distinct / depth 2; Hold
succeeded (2193/0), Fail violated (2116/13) with FALSE, TRUE, then stuttering.
Compact input/raw identities and consumed expectations accompany the API fixture.
Independent replay of that word is a native test obligation, not inferred from
the primary receipt. No original Quicksort measurement is claimed.

Original action-target selection in `dive-liveness::analyze` follows the
first-equal-identity collection semantics of `tlc2/util/SetOfStates.java::put`
and the original-target action evaluation in `tlc2/tool/liveness/LiveCheck.java`
at pinned revision `94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT).
The implementation uses the already retained native concrete targets and
destination identities; it does not reproduce Java table slots or iteration
order. Action membership reuses the existing bounded `pushNeg` / `simplify`
semantics without DNF distribution, only at graph labeling. Existing eager
candidate attribution and independent validation can still refuse eliminated
actions, without emitting a model error or counterexample after that refusal.
The `ConstructorAlias` action, state,
duplicate-successor and eliminated-action controls were checked against that
pinned primary before implementation. Residual-tableau state error parity and
general cross-atom error ordering remain outside this repair.

`dive-liveness::plan` matches pinned `tlc2/tool/liveness/Liveness.java`
`astToLive`/`astToLiveAppl`: conditional guard probes, ordered branch/boolean
conversion and nonrecursive-call exception fallback (MIT, revision `94d0c50`).
The complete 24-row actual processLiveness capture, historical selections and
native failures are in `conformance/probes/state-if-planning/`; ADR 0036 records
remaining rendering, admission and strict execution limits.
## Native predecessor spill (ADR 0031)

`dive-store::predecessor` is a native fixed-record transient storage design.
It preserves `TraceLog` discovery IDs, strictly earlier parents and successor
ordinals (the TLC `TLCTrace`/`ModelChecker` replay semantics matched in ADR 0015),
without importing Java disk/checkpoint formats or porting an I/O algorithm.
`crates/dive-api/tests/predecessor_spill.rs` checks ordered replay and independent
symmetry witnesses across memory/disk, stores, profiles and worker widths.

## Single-process storage and scheduling

| dive component | Source | Licence | Nature |
|---|---|---|---|
| Single-process PlusCal storage/actions (`dive-pluscal/translate/process.rs`, `emit.rs`, `labels.rs`) | `pcal/ParseAlgorithm.getAlgorithm` process LabelFlag check, `checkBody`; `pcal/PcalSymTab` unconditional reserved pc symbol; `pcal/PcalTLAGen.GenProcess`, `GenVarsAndDefs`, `GenProcSet`, `GenInit`, `GenLabeledStmt`, `GenNext`, `GenSpec`, `GenTermination`, `AddSubscriptsToExpr`, `GenAssign` at `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Bounded single unfair constant-ID/finite-membership process, indexed pc, scalar/indexed locals and assignment/expression substitutions. Primary translation/SANY and normalized secondary AST stages kept separate; no Java runtime delegation. |
| `dive-check` concrete pool graph capture and `dive-liveness::phase_graph` | Existing deterministic safety pool; `tlc2/tool/Worker.java` `doNextCheckLiveness`; `tlc2/tool/liveness/LiveCheck.java` null-tableau `LiveChecker.addNextState`, `DiskGraph`/`GraphNode` transition insertion and bin persistence; pin `94d0c50` | MIT semantics / own bounded retention and scoped admission | Accepted original concrete rows are retained before representative substitution. One logical source order matches twelve completed actual Java graph projections at widths 1/3/4/8; synthetic written/unwritten collisions tested separately. Completed capture precedes bin evaluation; no integrated stopping/SCC/lasso proof. |


## Single-process fairness generation

| Rust implementation | Pinned source matched | License | Evidence and boundary |
| --- | --- | --- | --- |
| `dive-pluscal` fairness options, process normalization and `Emitter::process_fairness` | `pcal.ParseAlgorithm.getAlgorithm` explicit/default fairness precedence; `pcal.PcalTLAGen.GenSpec`, `ProcessFairness.singleLineWidth`/`format`, `FormulaPair.multiLine`; tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Complete source/option and wrapped-junction controls in `crates/dive-pluscal/fairness-acceptance/`; token, full-module CST and separate SANY checks. Preserve the sole-prefix/separator width omission because junction grouping is observable. Domain multiline/comment/tab normalization and procedure/multiple-block generation remain named refusals. |

| `dive-pluscal` multiple-process normalization/emission (`translate/process.rs`, `lower.rs`, `emit.rs`) | tlaplus `pcal/ParseAlgorithm.java` process-wide omission checks; `PcalTLAGen.java` `GenProcSet`, `GenInit`, `GenProcess`, `GenNext`, `GenSpec` and `PcalSymTab` source contexts; commit `94d0c50` | MIT | Source-owned body normalization, shared storage and CASE initialization, constant-before-quantified scheduling and separate fairness formulas matched through retained clean primary output; bounded domain-disjointness admission is native design, broader renaming remains refused |

| `dive-pluscal` native macro expansion (`translate/macros.rs`) | pinned `pcal/ParseAlgorithm.java` `GetMacro`, `ExpandMacrosInStmtSeq`, `ExpandMacroCall`, `SubstituteInStmtSeq`, `SubstituteInStmt`, `SubstituteInSingleAssign`; `pcal/TLAExpr.java` `substituteForAll`, `substituteAt`, `findNextInstanceIn`; `pcal/AST.java` `location`, `PcalTLAGen.GenAssert`; commit `94d0c50` | MIT | Source-order expansion, simultaneous lexical/selector substitution, field exclusions and composite source locations; bounded owned expression resolver. Private source AST is not rewritten; unproved binding/hygiene/procedure cases explicitly refused. |
| `dive-pluscal` source string admission (`lexer.rs`) | pinned `pcal/PcalBuiltInSymbols.java` `buildStringCharTable` and `pcal/Tokenize.java` `STRING`; commit `94d0c50` | MIT | Printable-ASCII/escape source boundary, with original illegal Unicode/control inputs and failed native overaccept retained. |



| Native PlusCal sequential procedure admission/typed continuations (`translate/procedures.rs`, `lower.rs`, `emit.rs`) | pinned `pcal.PcalTranslate.ExplodeCall`, `ExplodeReturn`, `ExplodeCallGoto`, `ExplodeLabeledStmt` (Error fallthrough); `PcalTLAGen` sequential changed-variable, GenVars/GenInit/procedure-wrapper/GenNext rules; `PcalSymTab` procedure declaration ordering; `ParseAlgorithm` labels and call-tail folds; commit `94d0c50` | MIT | Bounded one-procedure path: complete token/CST/SANY selection, clean count controls, strict namespace refusals retained and separately qualified default independent replay; no scope/recursive/process-stack certification |

| Native PlusCal nonrecursive procedure graphs and adjacent call/return (`translate/procedures.rs`, `lower.rs`, `emit.rs`) | pinned `pcal.PcalTranslate.ExplodeCall`, `ExplodeReturn`, `ExplodeCallGoto`, `ExplodeCallReturn` (caller locals/head replacement/parameter simultaneous assignment/local initialization); `PcalTLAGen` procedure/variable/action/Next/wrapper order, `ParseAlgorithm` call-tail folding; commit `94d0c50` | MIT | Complete 120-row C/P source/normalization/artifact selection, enclosing CST/SANY, clean model counts and separately qualified default replay. Acyclic disjoint scope admission and iterative cycle/global budget proof are native design; recursive/process/name rewriting and strict namespace evidence remain pending. |

| Native recursive sequential procedures (`translate/procedures.rs`, `emit.rs`) | pinned `pcal.PcalTranslate.ExplodeCall`, `ExplodeReturn`, `ExplodeCallGoto`, `ExplodeCallReturn` (same-procedure frame preservation/simultaneous parameters/local initialization); `PcalTLAGen.GenAssign` (empty-assignment invalid output), declaration/action order; commit `94d0c50` | MIT | 116 capture-first C/P rows, all primary errors/SANY-invalid outputs retained; no runtime call unrolling. Bounded graph-capture resource/namespace stops and qualified canonical witness controls are separate from strict #124 witness certification. Namespace ownership/global budgets are native design; no process/scoped-renaming certification. |

| Ordered temporal execution (`dive-check` source observer; `dive-liveness::phase_execution`) | `ModelChecker.DoInitFunctor`; `Worker.doNextCheckLiveness`; `LiveCheck.addNextState`, nested `TableauLiveChecker` initial/ordinary/recursive `addNextState`; `TableauDiskGraph`/`TableauNodePtrTable` record/done/write distinction; `TBGraphNode.isConsistent`; pin `94d0c50` | MIT semantics / own bounded observer admission | Actual multiple OOS bins, consistency, product registrations/transitions and immediate done-state action-vector re-enumeration match defined completed fields. One deterministic pool feeds one logical commit history; speculative and recursive work remain separate. Accepting-sink reconstruction, exact SCC/lasso/stopping replay and scheduling are not certified. ADR 0034 and `conformance/probes/tlc-phase-execution/` retain every case and historical observation. |

| Pinned final-only SCC and witness (`tlc_scc`, `tlc_witness`, `tlc_final`) | `tlc2/tool/liveness/LiveWorker.java` checkSccs/checkComponent/isStuttering/dfsPostFix/bfsPostFix/printTrace; DiskGraph/TableauDiskGraph getPath, TableauNodePtrTable component vector insertion/seen/parent semantics; Liveness.findViolatedProperties configured attribution; pin `94d0c50` | MIT semantics; own bounded stage admission and independent certificate boundary | Actual private-method fields and original finalCheck streams retained for all30 controls;19 completed field/word cases match at1/3/4/8. Independent Floyd/path checks do not use production traversal. Fresh standalone trace replay certifies candidates separately;11 failures gain no error agreement. ADR0040. Periodic scheduling, initial replay and ordinary strict CLI integration remain unproved. |

Final-only facade pre-bind admission follows `SpecProcessor.processConfigSpec` /
`constantDefns` startup reachability semantics and the native Def.std authority,
with dive's conservative callback-free source proof. User code and standard
module top-level zero-arity bodies are inspected; unused standard LET helpers are
not independently startup roots. No effect is evaluated to infer purity. Shared
fallible constructor/context authority must be retained at shipping integration.
See `docs/tlc-final-api.md` and the focused admission controls.

Explicit final/TLC CLI reporting follows `LiveWorker.printTrace`'s stuttering
versus back-to-state marker and configured `findViolatedProperties` name set.
The existing final SCC/witness algorithms and bounds remain unchanged. Separate
`dive-final-tlc` harness mapping requires a clean final-schedule reference and
fresh independent model/word replay; schema-1 ordinary mapping is unchanged.
Pre-bind source admission also resolves configuration names through every
`Module.context` `Symbol::Def` alias before substitution precomputation, matching
`Tables.op_defs` / `SpecProcessor.processConfigSpec`'s resolved-target semantics.
The retained local `Hidden == PrintT(TRUE)` context-alias negative control returns
before binding, zero used workers and no graph evidence.

The explicit final-mode safety-only route reuses the actual ordered ordinary
safety engine (`ModelChecker.doInit`, `Worker.doNext`/`ModelChecker.modelCheck`)
inside existing capture ceilings. Routing is derived from the freshly bound
`SpecProcessor.getImpliedTemporals` counterpart, never a supplied JSON claim or
zero-OOS projection. No new graph/SCC algorithm or result-changing default is
introduced. The original safety `CheckResult` is retained privately; finite
witnesses are replayed using `dive-trace` without successor enumeration. Nine
clean pinned controls, original constant-Next configuration rejection and
bounded native stops are retained under `conformance/probes/tlc-final-safety`.

| Pure state conditional final admission (#168) | Pinned `Liveness.astToLive`/`processLiveness`, `LNStateAST.eval`, `LiveCheck.addInitState`/`addNextState` and original `finalCheck` at revision `94d0c50` | MIT | Uses inherited exact conversion and scoped predicate authority; removes only the stale Phase-L syntax refusal. Complete raw fields/words, failures and independent closing/claim mutants retained under `conformance/probes/tlc-state-if/`; ordinary periodic/family parity unclaimed. |

| `dive-liveness::tlc_plan` shared predicate occurrence import (#170) | pinned `Liveness.astToLiveAppl` `OPCODE_ite`, `parseLiveness`/`processLiveness`; `LNState.tagExpr` and `LNAction.tagExpr`, revision `94d0c50` | MIT | Same input-object identity and last-visit tag retained; no expression-truth deduplication. Complete raw tag/stage/OOS/particle controls and independent words remain diagnostic; staged final admission unchanged. |

Direct temporal-IF final admission (#170) retains the existing shared
`Liveness.astToLiveAppl` conversion and `LNState.tagExpr` last-visit identities,
`TBGraph` ordered particles/edges, `LiveCheck.finalCheck` selected components and
lassos. Independent `dive-trace` memory/JSON replay still certifies every concrete
word separately; neither normalization equality nor backend flags certify it.
All30 actual captures and120 native outcomes are in
`conformance/probes/tlc-temporal-if-final/`; clean source-prefix/refused controls
receive no matching-error or ordinary-schedule credit.

## Native checkpoint composition with transient storage

The recovery engine streams saved frontier suffix states into the existing native
indexed Queue and reconstructs TraceLog parent/ordinal records through the existing
fallible predecessor store. No Java I/O/checkpoint format or new algorithm is
ported. The configured TLC live-search key path, discovery IDs and replay ordinals
remain those matched against `ModelChecker`/`Worker`/`TLCTrace` at revision
`94d0c50` (MIT), as recorded above. ADR 0038 and the native spill recovery study
keep original parallel source/raw evidence separate from current storage controls.



| Native single finite process procedure ownership (`translate/process.rs`, `procedures.rs`, `lower.rs`, `emit.rs`) | pinned `pcal.PcalTranslate.ExplodeCall`, `ExplodeReturn`, `ExplodeCallReturn`, `ExplodeCallGoto`; `PcalTLAGen.GenProcedure`, `GenVars`, `GenInit`, `GenNext`, `AddSubscriptsToExpr` owner contexts; `PcalSymTab` declaration order; commit `94d0c50` | MIT | 112 capture-first C/P rows, disjoint ownership, source-vs-generated stack indexing, complete raw/invalid evidence; artifact/count/strict-pending/canonical replay stages separate. Global quotas and conservative naming restrictions are native design; no multiple-block, renaming, process-declared fairness or strict namespace certification. Separate algorithm-level fair controls match `ParseAlgorithm.getAlgorithm` FairAlgorithm and `PcalTLAGen.GenSpec` global WF_vars(Next), without liveness execution credit. |

| Native shared procedures across finite process blocks (`translate/process.rs`) | Pinned `pcal.PcalTLAGen.GenMultiprocess`, `GenProcedure`, `GenVars`, `GenInit`, `GenNext`, `AddSubscriptsToExpr`; `PcalTranslate.ExplodeCall`, `ExplodeReturn`, `ExplodeCallReturn`, `ExplodeCallGoto`; `PcalSymTab` context ownership; commit `94d0c50` | MIT | One procedure normalization and borrowed per-process bodies, shared ProcSet storage and constant/procedure/member scheduling. Complete capture-first evidence remains separate from strict pending versus qualified canonical witnesses. Disjointness admission/global quotas are native design; no scoped renaming/fairness/full family certification. |

## Native process-owned procedure fairness

| Native component | Pinned reference | License | Adaptation and evidence boundary |
| --- | --- | --- | --- |
| `dive-pluscal` caller reachability and fairness generation (`translate/fairness.rs`, process/procedure context in `process.rs` and `procedures.rs`, Spec emission in `emit.rs`) | `pcal.ParseAlgorithm.GetCall`, `pcal.PcalFixIDs.FixMultiprocess`, `pcal.PcalTLAGen.GenSpec`, `FormulaPair`, `ProcessFairness`; commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Syntactic call order and declaration-ordered closure adapted to bounded sparse traversal, per-caller WF/SF and +/- ownership, UTF-16 significant junction grouping. Complete clean primary captures, SANY/operator-CST and finite counts/replay remain separate stages. Quotas and conservative disjointness are native design; no scoped renaming, sequential-modifier or liveness-execution certification. |


## Native bounded PlusCal block editing

| Native component | Pinned reference | License | Adaptation and evidence boundary |
| --- | --- | --- | --- |
| `dive-pluscal::rewrite`, `dive-api::rewrite`, CLI editing policy | `pcal.trans.performTranslation`, `findTokenPair`; `pcal.Validator.CHECKSUM_PATTERN`, `checksum(Vector)`; commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Native bounded span selection and pure replacement, UTF-8 line-concatenation CRC32 and recognized header fields. Explicit partial sentinel, overwrite grant and byte quotas are native policy. Primary-body exact-byte editing is development-only evidence; native generated artifacts have separate admission controls. This historical body-only increment preceded the separate post-translation AST serializer below. Other AST shapes, exact generator formatting and file writes remain pending; fair CRC collision retained as D-020. |

## Procedure-free post-translation PlusCal AST checksum

| Native component | Pinned reference | License | Adaptation and evidence boundary |
| --- | --- | --- | --- |
| `dive-pluscal::algorithm_checksum_at` (`translate/checksum.rs`) and best-effort rewrite policy | `pcal.AST.toString`, `TLAExpr.toString`, AST indentation helpers; `ParseAlgorithm` default declarations; `PcalTranslate.removeNameConflicts`, `translate`, `Explode`; `trans.performTranslation` and `Validator` AST CRC expression; commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Bounded sequential reference templates/token encoding and proved transparent C block flattening; native serialized bytes compared to each primary post-translation capture. Name-fix/unproved AST contexts refuse; Java probes are development-only. Exact CRC derivation and body/source replacement remain separate from TLA admission/model proof. Unavailable algorithm CRC is not a rewrite gate. The fair-only CRC weakness is reproduced, not corrected (D-020). |

### Scoped final attribution and cardinality-one DNF (#134)

`dive-model/src/spec.rs::Binder::props` matches pinned94d0c50
`SpecProcessor.processConfigProps`: a zero-arity LET OpDef changes the derived
name; formal LazyValue forwarding preserves it. Expression and caller scope
remain unchanged. `dive-liveness/src/tlc_plan.rs::Builder::dnf` matches
`LNConj.toDNF` cardinality-one `new LNConj(nes)` including singleton LNDisj
factors. Both MIT source rules have exact field-level raw controls in
`conformance/probes/tlc-final-scopes/`; original failed observations are retained.
Expanded original Action names remain reference-only, not inferred native data.

`dive-liveness/src/plan.rs` TLC-shape directly syntactic `~ENABLED A` matches
`Liveness.astToLiveAppl` OPCODE_lnot's state-level fallback: the original source
predicate is one atom, evaluated by negating the existing pure bounded action
search. The default formula kernel and independent Word implementation stay
unchanged. Complete positive/negative fields and periodic/closing controls
retain the old extra-negation mismatch; this is normalization fidelity only.


Pure-runtime-name native recovery (ADR 0038, 2026-10-11) reuses dive's ordered name
archive and the `dive-model` typed encounter-order fallback. It introduces no
ported Java checkpoint codec. Clean `tlc2.TLC` safety runs with `-workers 1`,
`-seed` 0/7/14 and `-fp 0` provide the separately retained constructor/CHOOSE
counterparts in `conformance/probes/native-checkpoint-runtime-names/tlc-primary/`.
The independent TLC trace runtime-name refusal remains unchanged.

The separate checksum-expression increment additionally ports
`ParseAlgorithm.GetIf` / `InnerGetWith` nesting and optional C trailing
separator behavior, `TLAExpr.normalize` first-token minimum columns and
`TLAExpr.toString` line/token grouping, and `trans.removeTabs` eight-column
expansion (Java UTF-16 columns), all at the same `94d0c50` pin. Native flat
branch/binder serialization and source position indexing are iterative bounded
adaptations. The complete original/corrected/layout/separator selections, initial
parser disagreements and separate SANY-invalid artifacts are retained in
`crates/dive-pluscal/checksum-expression-acceptance/`. No Java delegation,
name-fix/process/procedure serialization or stronger checksum default is added.

| `dive-pluscal` empty-Else generation (`translate/emit.rs`) | `pcal.PcalTLAGen.GenIf` empty Else TRUE before identity updates; `ParseAlgorithm.GetIf`/`GetStmtSeq` P empty branches and C nonempty bodies; tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Retained original operator-CST mismatch and complete paired empty/explicit-Skip/elsif controls. Empty Then generation remains explicitly unsupported. |

| Native PlusCal value-expression parenthesization (`translate/emit.rs`) | Pinned `pcal.PcalTLAGen.NeedsParentheses`, commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | A top-level CASE arrow contains the reference greater-than trigger. C/P whole-module node/token CST controls retain parentheses so the CASE arm cannot absorb following action conjuncts. Original failed grouping controls and source remain archived; first-line scanning behavior is unchanged. |

| Expanded temporal Action origins in `dive-model` and final-only API/JSON | `tlc2/tool/impl/SpecProcessor.java`, `processConfigProps`: unchanged `configName` passed into every implied-temporal `new Action`, independently of mutable derived `name`; pin `94d0c50` | MIT | Complete18 actual primary projections retain configured, expanded Action and derived role vectors separately;16 complete/2 exported-formal failures. Metadata only, unchanged witness/scheduling contract. |

| Bounded sequential multiline expression generation | `pcal/PcalTLAGen.java` `AddSubscriptsToExpr`, `NeedsParentheses`, `addExprToTLA`; `pcal/TLAExpr.java` token line normalization, pin `94d0c50` | MIT | Non-junction expression token order and changed-variable substitution, qualified by whole-module CST/SANY/CRC/finite controls in `crates/dive-pluscal/multiline-acceptance/`; indentation-sensitive/macro/process/procedure/binder layouts explicitly refused; primary pretty bytes not claimed |


The finite-process checksum continuation ports `AST.Multiprocess.toString`,
`AST.Process.toString` and `AST.VectorToSeqQuotedString` at the same `94d0c50`
pin, with existing `VarDecl`/`LabeledStmt`/expression serialization. Native label
ownership uses a bounded source-offset index rather than Java vector allocation
order. Captured process modifiers, declarations, CRLF/tab/UTF-16 positions and
post-Explode bytes are independently compared in
`crates/dive-pluscal/process-checksum-acceptance/`. Runtime has no Java. Unproved
options/scoped shapes refuse; raw CRC, artifact admission and model proof remain
separate. D-020 is reproduced, not silently corrected.

The source-backed singleton identity spelling additionally matches
`PcalTLAGen.GenLabeledStmt`, `GenIf` and `GenEither` (same `94d0c50` pin):
`UNCHANGED name` for Java UTF-16 length greater than five, equality otherwise.
The original defaultInitValue whole-CST failure and length-five/six/branch
controls remain retained. This is artifact fidelity, not a semantic divergence.

Source option admission guard: pinned `pcal.trans.performTranslation / ParseAlgorithm.ProcessOptions` token context
and `Validator.validate` reparsing options, commit `94d0c5070852b37b801bb8998c436383ab7ee37b`
(MIT). Native recognizes a conservative context for named refusal, not execution
of the reference option parser. Captures: `crates/dive-pluscal/source-option-acceptance/`.

| Bounded pure operator-formal admission (`scope_operator_analysis`, scoped temporal/independent Word and finite/capture proof) | `tlc2.tool.impl.Tool.getOpContext`, operator-param lookup / `OpLambdaValue` captured Context; `Liveness.astToLive` application context; pin `94d0c50` | MIT | Callback-free known user-body proof retains captured lexical/dynamic scope; actual application remains scope_apply_param; constant-origin ambiguity and all opaque/native/recursive targets explicitly refuse. Exact primary/projection/word/witness controls: `conformance/probes/temporal-operators/README.md`; no ordinary scheduling claim |

### Process checksum option context

| Implementation | Reference | License | Evidence and limits |
| --- | --- | --- | --- |
| Native process option AST checksum (`dive-pluscal/translate/checksum.rs`) | `pcal.ParseAlgorithm.getAlgorithm` 377–394 (declared fairness precedes wf/sf defaults), `InnerAddLabels`, `AST.Process.toString`, `AST.Multiprocess.toString`, `PcalTranslate.Explode`; pinned `94d0c50` | MIT | Exact native post-translation bytes/CRC on retained source/option pairs; generator fairness suppression remains separate, all unproved source-option contexts refuse |
| Source-option preflight (`dive-pluscal/translate/mod.rs`) | `pcal.ParseAlgorithm.ProcessOptions`, `FindToken`; `pcal.trans.performTranslation`; `pcal.Validator.validate` | MIT | Conservative detection/refusal, not a port of option-comment parsing; retained original ignored-option wrong artifacts and configured/unconfigured primary evidence |

Original-source PlusCal option preflight matches the pinned translator
`pcal.trans`/`ProcessOptions` reading source context before output replacement.
The native callback-free declaration span is shared with the high-level rewrite
facade before `mask_previous`; no Java executes in the product. The minimal
old-output fairness-loss control is retained in
`conformance/probes/pluscal-source-option-masking/`; full option parsing remains
pending.

| Bounded native temporal operator actuals | pinned `Tool.getOpContext`, `OpLambdaValue`, `Liveness.astToLive`, native `Sequences.Len/Head` and `FiniteSets.Cardinality`; commit94d0c50 | MIT | Metadata authority and actual scoped native dispatch; independent bounded admission/Word implementation. Raw20-selection reference failures remain separate; no new schedule/default. |

`conformance/probes/pluscal-source-option-masking/`; broader option parsing
remains pending. The bounded continuation below preserves the public header-span
alias while using full resolved extents for overlap admission.

| Implementation | Reference | License | Evidence and limits |
| --- | --- | --- | --- |
| `dive-pluscal` source option context (`translate/options.rs`) | pinned `pcal/trans.performTranslation`, `parseAndProcessArguments`; `pcal/ParseAlgorithm.FindToken`, `GotoNextNonSpace`, `NextNonIdChar`, `ProcessOptions`, `NextDelimiterCol`; commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | Whole-file first-declaration scan, first-fairness CLI override and bounded label/root processing; exact context/stage captures retained; unknown/malformed/partial reference errors and unproved Unicode keyword boundaries refused |

## Stored successor representation admission

The bounded final TLC path uses explicit stored Enum/Raw set elements and stored
function entries before fingerprints/comparison, including normalized Raw sets.
This is a native representation admission change, not a changed TLC semantic
default. Cached provider/deferred sets remain refused. Existing8m/depth256
Session bounds and separate50k/128 context bounds are unchanged. Pinned
`LiveCheck.addNextState`/`SetOfStates` concrete-value behavior is retained; see
stored-state guide and
paired source/evidence.
Ordinary schedules and whole-family parity remain partial.

### Actual-bound RandomElement stream authority (2026-10-11)

`dive-model::reach` and `dive-check::engine` match native target selection and
`RandomEnumerableValues.TLCStateRandom.initialize/initializedFor` at pin
`94d0c50`: startup uses the run seed, and expansion uses predecessor fingerprint
XOR seed. Actual ConstBinding/redirect authority participates independently of
source operator references. The independent validator refuses every random
draw (ADR 0016, amendment 3). `TLCTrace.printTrace` and
`Tool.getState(TLCState,TLCState)` initial-object reuse is captured as a remaining
strict trace-printing refusal, not ported or certified. Clean raw primary and
current native producer identities are separated in
`conformance/probes/configured-random-stream/`. Licence: MIT reference semantics.

| Pure native `Append` temporal operator actual admission | `tlc2.module.Sequences.Append`, pinned `94d0c50`; actual arity and processLiveness projections under `conformance/probes/native-sequence-operators` | MIT | Existing native evaluator unchanged; bounded operand/context proof retains original calls and independent word/witness replay; Tail/SubSeq remain refused |

| Bounded partial SCC/source checkpoint projection | `LiveWorker.checkSccs` non-final UNDONE root/target skips, `checkComponent` EA-root/reverse-stack order, `LiveCheck.check0` repeated pointer restoration and `AbstractDiskGraph.getPath`; pin94d0c50 | MIT | Complete32 actual explicit-prefix fields plus8 original repeated checks; private native source-slot authority, independent acceptance/word/mutant controls; ordinary clock schedule remains unimplemented |

| Periodic backend controller (`dive-liveness/src/periodic.rs`) | `tlc2/tool/ModelChecker.java` `doPeriodicWork`, commit `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | All296 actual method callback/force/code/effect controls; controlled queue/SCC/checkpoint/ratio callbacks. Clock-ratio math, graph authority and ordinary verdicts remain unproved. |

| Runtime-ratio arithmetic (`periodic::calculate_runtime_ratio`) | `ModelChecker.updateRuntimeRatio`, pinned `94d0c50`; actual method called with controlled initial fields | MIT | 196 complete numeric observations with raw double bits and real clock brackets, three negative assertion controls; no scheduling/clock-origin/model-proof credit |

| Source-qualified periodic backend (#189) | `LiveCheck.doLiveCheck`/`check`/`check0`, `LiveWorker.call`, `DiskGraph.size`, `TableauDiskGraph.size`, `NodePtrTable`/`TableauNodePtrTable` registration and `AbstractDiskGraph.recordSize`/`makeNodePtrTbl`; pin94d0c50 | MIT | Actual32 source-growth projections, four controlled thrown-check failures and complete pointer/snapshot/threshold controls. Native private model-owned checkpoints, cumulative original work bounds and independent witness mutants; no ordinary clock/queue-suspension or model-proof claim. ADR0051 |

## Bounded nonrecursive macro Full checksum serialization

The serializer matches pinned `pcal.ParseAlgorithm.ExpandMacrosInStmtSeq`,
`ExpandMacroCall` and `SubstituteInStmt`, `pcal.TLAExpr.substituteForAll` and
`substituteAt`, `pcal.AST.toString` (macro vectors intentionally omitted), and
`pcal.trans`/`pcal.Validator` CRC stages at `94d0c5070852b37b801bb8998c436383ab7ee37b`
(MIT). Logical single-line substituted token positions are separate from native
original-source provenance spans. Embedded trailing-star decoration matches the
measured comment boundary without weakening standalone EOF. Full artifact
admission and independent model proof remain separate. See
`crates/dive-pluscal/macro-checksum-acceptance/` and ADR 0049.

| `dive-api` PlusCal generated declaration insertion (`translate.rs`) | pinned `pcal.trans` translation-section placement at the existing block, otherwise immediately after the owned algorithm closing line, commit `94d0c50` | MIT semantics | C/P trailing generated-name references and real unknown-name negatives, with original `Lock` failure retained; strict enclosing-module analysis remains required |

| Algorithm closing-line admission (`dive-api/src/translate.rs`, `dive-pluscal/src/rewrite.rs`) | pinned `pcal/trans.java` line847 `endStuff` admission and line853 section insertion; Java `String.trim` through ASCII 0x20 | MIT semantics | Both C/P empty/space/tab/CRLF/line-comment positives and inline/block-comment/NBSP negatives; original standalone false acceptances and SANY-invalid formfeed outputs retained |

Algorithm marker ownership matches pinned `pcal/trans.java` lines713–743:
line-first search, `--algorithm` before `--fair` on that line only, followed by
normal parser validation. All 32 paired C/P controls retain clean primary
outputs. The new development-only `MarkerOptionProbe` uses the same source
selection; its separate class/name preserves the earlier helper's wrong
same-line order as historical evidence. It is never a native dependency.


### Native periodic lifecycle

The native lifecycle diagnostic follows `ModelChecker.doPeriodicWork` and
`updateRuntimeRatio`, with `StateQueue.suspendAll`, `resumeAll` and `finishAll`
compared against actual selected `DiskStateQueue` thread controls at MIT-licensed
pin `94d0c50`. The 296 controller, 196 updater and 96 queue observations retain
separate producers. Borrowed native dispatch authority and the internal
source-commit timer/EOF SCC seam are diagnostics; they do not establish ordinary
scheduler equivalence. See [ADR 0052](decisions/0086-rescue-reset.md).

### VIEW-backed RandomElement authority (2026-10-11)

`dive-check::Search::seed_state` matches pinned
`RandomEnumerableValues.TLCStateRandom.initializedFor` and
`TLCStateMut.fingerPrint` (`94d0c50`, MIT): state fingerprint applies concrete
normalization and VIEW before stream selection. Concrete predecessor retention
is unchanged. Callback-free bounded source and stored-value admission is native
policy, not a Java error-class port or independent reachability certificate.
The shared `dive-model::random_view` helper does not enumerate successors or
invoke providers; trace replay stays independently built without enumeration.
Original 84 diagnostic rows, wider seeds and shipping producer evidence remain
separate under `conformance/probes/view-random-stream/`. Symmetry and other RNG
families/recovery are excluded until their own contracts are proved.

## Bounded PlusCal global DEFINE sections

`dive-pluscal` global definition placement and expression rendering match the pinned MIT `pcal/PcalTLAGen.java` methods `GenVarsAndDefs`, `GenVarDecl`, `addExprToTLA`; `TLAExpr.java` normalization/rendering and `AST.java` definition serialization at `94d0c5070852b37b801bb8998c436383ab7ee37b`. Global declarations precede helpers and process locals follow. UTF-16/tab-expanded token columns and serialized definitions have source-backed evidence. Conservative declaration/cycle admission is native policy; reference adapters are development tools.

The DEFINE evidence directory retains the original runtime scope gap and separately archived committed test promotions. Its portable original records and the fresh integrated gate are distinct authorities.

## Supplied periodic hook composition

The lifecycle foundation retains the separately captured controller, runtime-ratio and real queue controls under ADR0052; those controls do not certify ordinary scheduling.


`dive-liveness::periodic_hook` matches the bounded supplied-expression portion of pinned MIT `tlc2.tool.ModelChecker.doPeriodicWork`, `Tool.eval(SemanticNode)` with empty state/context and Clear, and `BoolValue.equals` at `94d0c50`. All 59 actual method controls are retained. Configuration-owned snapshots and ordinary scheduling remain separate obligations.

## LOCAL and function DEFINE admission

`dive-pluscal` LOCAL and function definition placement matches the pinned MIT `pcal/PcalTLAGen.java` methods `GenVarsAndDefs` and `addExprToTLA`, and `AST.java` `Uniprocess`/`Multiprocess` definition serialization at `94d0c5070852b37b801bb8998c436383ab7ee37b`. Opaque declaration text and order, LOCAL visibility and function-domain positions remain reference-equivalent. Bounded domain/body dependency checks and action/temporal admission are native policy. Stage probes are development tools.

## Configuration-owned periodic snapshot (ADR 0057)

`dive-model::periodic` and the diagnostic flag in the shared binder match pinned `tlc2.tool.impl.SpecProcessor` constructor snapshot timing and `processPeriodic` (1634–1650), with `tlc2.tool.Defns.snapshot`: substitutions precede snapshot, constant caching precedes role validation, selected original body and configured spelling remain distinct. Pin `94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT); actual Java reflection reads metadata only. `dive-liveness::periodic_snapshot` prebinding grammar is a conservative native policy, not a port of TLC evaluation. Its structural nonconstant witness follows the shared `dive-model::LevelCx::try_level` cache authority rather than editable node/definition levels. All 38 primary controls and raw captures are retained in `conformance/probes/periodic-snapshot`; startup restrictions and Java-exception refusals earn no error-class parity credit.

## Checked stored trace transport (removed)

The opt-in trace format version 2 (`dive-trace/src/value_repr.rs`) was removed
on 2026-10-12 (ADR 0016, amendment 3). No code from it remains.

| Native producer request/claim-pause diagnostic (#201, ADR0059) | Pin94d0c50 `AbstractChecker.runTLC`, `ModelChecker.doPeriodicWork`, local `tlc2.tool.Worker.run`, `StateQueue.suspendAll/resumeAll/finishAll`; complete33 controlled/natural observations in `conformance/probes/progress-queue` | MIT | Actual native timer/claim gate and joined mutex pause are a separate bounded design; no Java queue/ordinary scheduling/count/model verdict equivalence is asserted. Distributed TLCWorker excluded. |

### Structural storage format 2 (#195)

The pinned `tlc2/value/impl/TupleValue` and `FcnRcdValue` application, comparison
and write contracts at `94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT) retain
distinct concrete classes despite mathematical function equality. The native
`codec::encode_step`/decoder use separate tuple and tuple-function tags; this is
dive's versioned format. Canonical encoding and FP64 are unchanged. Structural
CP02/FQ02/FI02 and predecessor formats are documented in ADR 0083.

The consumed inputs in
`crates/dive-api/tests/fixtures/storage-tuple-function/specs/` cover application,
an independently checked invariant witness, and comparison of literal tuples
and integer-domain functions against a string-domain record. Pinned TLC reports
GENERAL 1000 followed by NESTED_EXPRESSION 2103, exit 75, generated/distinct 1/1
and depth 1 for both comparison inputs. Default D-009 structural comparison
finishes with generated/distinct 3/2 and depth 2. The storage matrix compares the
complete model report across memory, each frontier/predecessor spill combination,
and level-zero pause/recovery with retained counters, in both profiles and both
stores at workers 1/4/8. The measured comparison error site stays unverified;
preserving its diagnostic is not independent error-witness validation. Raw runs
are regenerated from the pins and kept outside the source tree.
| Actual request-prefix publication (#204, ADR0062) | Pin94d0c50 local `tlc2.tool.Worker.run/doNext`, `ModelChecker.doNextCheckLiveness/doPeriodicWork`, inherited real `StateQueue.suspendAll`; complete24 controlled observations in `conformance/probes/committed-prefix` | MIT | Shared ordered-prefix transaction is a separate bounded design. Original Next-return, graph-write, error and cleanup boundaries retained; no ordinary clock/queue/count/model-proof parity asserted. |

The #204 primary root-pointer observer queries `getPtr(fp, -1)`: authoritative
for DiskGraph, an invalid tableau index for TableauDiskGraph. Its raw result is
preserved as an observer limitation, without tableau write-authority credit.
Worker/controller outcomes and native owned publication remain separately
measured; original #204 aggregate gate and disk-exhaustion epoch are retained.
| Historical retired `dive-pluscal` nested procedure header view (`translate/checksum_transfers.rs`, `checksum.rs`) | tlaplus `pcal/PcalTranslate.java` `CopyAndExplodeLastStmt`, `ExplodeLabeledStmt`, `ExplodeWhile`, `ExplodeCall`, `ExplodeReturn`, `UpdatePC`; `PCalTLAGen.GenAssign/SortSass`; `AST.Assign/SingleAssign` and `TLAExpr.toString`; commit `94d0c50` | MIT | Typed shared child-vector mutation only; outer source vectors remain copied/preserved. Clean original-after-generation CRC authority is separate from direct exploded ownership. ADR 0061; complete 180-row declared selection |

Nested procedure header continuation also matches pinned `PcalTranslate.IsTRUE`
(single logical token line), `ExplodeWhile` tail mutation/discard, and
`PCalTLAGen.GenAssign` sorting only for actually generated assignments. Separate
192 primary rows (original180 plus4+8 token-line/tail controls) retain original
failed native producer results. Native work reservation/indexing is a bounded
implementation policy, not ported evaluator complexity or allocation counts.

## Fallible disk seen-state library

`dive-store/src/seen.rs` and `seen/files.rs` match the error boundary of pinned
`tlc2.tool.fp.FPSet.put`, `contains` and `checkFPs` (`94d0c50`, MIT): I/O is
fallible and cannot be a membership answer. `DiskFPSet.diskLookup` and
`checkFPs` are reference semantics, not copied Java storage layouts. Native
open-addressed disk indexing, private ownership, bounded rehash/audit and
first-error poisoning are dive designs (ADR 0063). Fingerprint gap uses existing
`collision::closer`/subset authority; bounded pair work refuses explicitly.
No ordinary engine or checkpoint admission is established by this library.
The independent tuple-history admission (ADR 0067) was removed on 2026-10-12
(ADR 0016, amendment 3).

- `dive-trace::tuple_history` (ADR 0071): removed on 2026-10-12.

### Positioned disk invariant-error replay (#217)

`dive-check::Search::replay_after_disk/replay_disk_states` matches MIT pinned
94d0c50 `tlc2.tool.ModelChecker.doNext`, `doNextSetErr` and invariant/action
continuation ordering. Full disk keys and Result operations use the existing
ADR0063 library; no Boolean StateSet adapter or resident key mirror exists.
Fresh clean six-fixture stdout/exit evidence is in
`conformance/probes/fallible-invariant-replay/primary/`. Original wrong-count and
refusal epochs are unchanged. Class parity is limited to actual known verified
sites, and the private bound action-property control does not lift public
PROPERTY admission. Inline fallback, logical versus physical replay counts,
first cause/cleanup retention and non-certificate limits are ADR0072.

The ADR0072 Print boundary is native admission, not a ported output algorithm:
`Evaluator::has_print_event` observes actual `Core::print` dispatch before
capture/sink calls, separately from warnings. The disk error route refuses
unproved repeated output after reconstruction. Pinned `ModelChecker.doNextSetErr`
and `TLCTrace` reconstruction yield three events in clean `ReplayPrint`; the
pre-review native run's single event is retained as failed output evidence.


## Historical retired Acyclic procedure-body checksum calls (#219)

The removed `checksum_transfers.rs` discovered source-owned procedure call vectors separately
from main's LAST-header alias. `checksum.rs` emits each body vector then distinct
main targets only on the last declaration. The matched source is pinned
`pcal.ParseAlgorithm.GetProcedure` (reset/store shared vector) and `GetCall`
(first source encounter/dedup), plus `AST.Procedure.toString`, at commit
`94d0c507` (MIT). Target frame and current return-slot ownership still match the
separate `PcalTranslate.ExplodeCall/ExplodeReturn` view; original post-generation
header authority is not replaced with direct exploded xast. Native iterative
acyclic admission and conservative pre-allocation reservations are dive's own
controls, not Java algorithms or exact instruction counts. No runtime Java
is delegated to. ADR0073 and the complete130-row declared study retain all old
errors/refusals and the first ENOSPC capture separately.


## Historical retired Recursive sequential procedure Full checksum graphs (#223)

The removed `checksum_transfers.rs` retained bounded source-owned call vectors separately
from `ParseAlgorithm.GetProcedure/GetCall` LAST-main alias. `PcalTranslate`
`CopyAndExplodeLastStmt` and `ExplodeCallReturn` (1284–1490) provide self/cross-owner
tail assignment/frame/local/entry order; `PcalTLAGen.GenAssign` supplies
visited-assignment sorting. `AST.CallReturn.toString` supplies unmodified
original header formatting. All source references use pinned commit `94d0c50`
(MIT). Iterative finite graph admission/shared reservations are dive's design,
not runtime unrolling or termination analysis. The 162 primary and separate
exploded rows retain two generated/SANY-invalid artifacts without public credit.
See [ADR0077](decisions/0084-pluscal-translation.md) and
[the executable contract](pluscal-translation.md).

Candidate delivery10af preserves full45-stage/1057-test source receipts, default release feature isolation,27 separately corrected public outcomes, fresh memory/JSON witness mutants and all37 required safety gate models at1/4/8. The original unparenthesized IF material workload correctly produced an incomplete successor (four pin exit75 controls); four separately parenthesized controls give exit13. Every failed observer/source/script epoch remains retained, with no ordinary scheduling or corpus/family gain claimed.

## Optional configured periodic role (ADR0074, #220)

The shared binder's optional mode follows pinned94d0c50 MIT
`tlc2.tool.impl.SpecProcessor.processPeriodic` (1634–1650) null/definition/arity
selection and actual pre-constant `Defns.snapshot` authority. Selected real
`ModelChecker.doPeriodicWork` observations preserve absent-hook skip, separately
forced suspension/resume, false2104 and type1000/error2179 outcomes. The complete26
rows in `conformance/probes/optional-periodic-hook/primary` retain actual Java
exception and observer queue cleanup. Additive optional native tokens and shared
independent candidate admission are dive architecture; they do not establish
ordinary Java queue/clock/count/model-proof equivalence. Existing mandatory APIs
and original startup/predicate bounds remain unchanged.

Optional-hook Init precedence includes a separate unmodified full TLC invocation:
GENERAL1000 contains inner2179, then nested-expression2103 decides exit75. This is
not substituted with the selected direct doInit2179 exception surface. Original
failed assertion/source and both primary stages remain retained.

The clean e553 optional producer has 46 closed gate stages/1,067 zero-skip tests,
479 exact compiler inputs including 50 embedded literal payloads, 46 default-library
runtime rows, 12 independently Valid EOF words and 48 strict Invalid mutants through
both memory/JSON routes. Default feature isolation, executable Cargo guide,
mandatory API controls and 37 safety models at 1/4/8 are separate receipts. Historical
matrix hashes remain tied to their original archived sources; the failed packaging
assumption is retained. No ordinary schedule/model/family certification follows.


### Exhausted-queue terminal service (#222)

Bounded additive Rust terminal diagnostics preserve
pinned `AbstractChecker.runTLC`, `ModelChecker.doPeriodicWork/modelCheck`,
`Worker.run` and `StateQueue.finishAll/suspendAll` ordering. A genuinely finished
queue refuses suspension and skips the hook before final selection. The complete
60 corrected full CLI and 60 controlled actual outer/queue observations remain
separate from native authority. Original misleading safe labels and observer
compile failures are retained; independent replay confirms their legal stutter.
No new TLC bug/default divergence is inferred. Existing ordinary CLI/clock/queue,
checkpoint/error/count equivalence remains unproved; F-023 stays missing and
audit-required. Controller Complete and independently Valid concrete words are
not temporal Holds or model completion. See [ADR0076](decisions/0086-rescue-reset.md).


### Distinct partial-stop and full-CLI observations (#222)

The separately controlled 12-row queue study holds a genuinely dequeued state:
physical queue size 0 is not finished while a Worker owns outstanding work.
Actual suspension succeeds, partial checking precedes the hook, and FALSE or
division stops without final selection; a deciding temporal2116 precedes FALSE.
These bounded scheduler observations are separate from the natural full CLI.
The original observer discards the integer returned by `AbstractChecker.modelCheck`;
its earlier description as a void method was incorrect and is preserved as history.
The separate corrected twelve-row observer records actual returns at widths 1/4/8:
TRUE 0, FALSE 2104, division 1000, and temporal violation 2116. Reflected
`errorCode` 0 and observer process exit 0 are separate fields. The subprocess
exits zero because its adapter does not forward the method result to `System.exit`;
none of these early-stop observations establishes temporal success.

The separate unmodified full CLI FALSE-hook counter exits 10, with 3,913,590
states generated/distinct and one queued state after 4.2456s; it prints no success
and no 2104 message. The declared 512MiB heap and 30s observer bound remain recorded.
This is an intentional-boundary observation, not a newly declared TLC bug. A
native early hook/service stop remains incomplete or refused and cannot be Holds.
All original selections, cleanup events and raw command/timing facts are retained
in the evidence.

## Stopped-queue worker diagnostics (ADR 0078, issue 224)

`Worker.run`, `ModelChecker.setErrState/setError`, `runTLC` and
`AbstractChecker.modelCheck` determine first-error/finished-queue behavior. The
complete `conformance/probes/stopped-periodic-service/primary` selection preserves
actual integer returns and independent CLI exits at 1/4/8, plus all earlier
rejected selections. Native private transaction facts retain original source
error/counters through joins; no claim reproduces Java graph writes after errors,
natural clock scheduling or ordinary model proof. See
[ADR0078](decisions/0086-rescue-reset.md) and the
executable guide.

## Historical retired Process-owned procedure Full headers (#225)

The removed `translate/checksum_transfers.rs` followed pinned `ParseAlgorithm.GetProcedure`
(694/720) and `GetProcess` (737/755): each process resets and stores its own
proceduresCalled vector, while sequential main aliases LAST. Original parsed
source spans identify each owner; vectors retain encounter order and duplicate
suppression with shared preallocation charges. `PcalTranslate.ExplodeMultiprocess`
(383 onward), `CopyAndExplodeLastStmt`/`ExplodeCall`/`ExplodeCallReturn`/`UpdatePC`
and `PCalTLAGen.GenAssign` match original stored shared-vector mutation, while
`AST.Multiprocess`, `Procedure`, `Process` and `VectorToSeqString` define exact
header serialization. `PCalTLAGen.GenCall`/`GenReturn`/`GenInit` separately own
process-indexed state generation; no header indexing or runtime unrolling is
added. Source commit `94d0c5070852b37b801bb8998c436383ab7ee37b`, MIT.

The complete declared242 study separates clean primary generation/original
post-generation AST/CRC from direct exploded xast. Only one unfair finite process
and one disjoint labelled procedure are lifted; fairness/scoped/multiple/option
boundaries remain explicit. See ADR0079 and the executable user guide. Java is
development-only. Artifact and strict saved-frame replay credit remain separate.


## Terminal finalization and postcondition order (#228, ADR0080)

The bounded Rust terminal guide exposes actual private
capture/resource completion, OOS/PEM final checks, independently replayed words
and subsequent direct post errors. `ModelChecker.modelCheck` runs finalCheck
before normal `Tool.checkPostCondition`; `LiveWorker.printTrace` also calls
`checkPostConditionWithCounterExample`, ignoring returned errors while getter
exceptions can abort rendering. The complete24 minimal pinned rows distinguish
2405/CLI10, ignored2405 then2116/CLI13, cached-role GENERAL1000/CLI255 and the
candidate getter exception/CLI1 with no method return. These are measured
ordering contracts, not a newly declared TLC defect or approved default change.
The unchanged8820 wrong post-before-final observation and all84/84/84/96
reference development/selection epochs remain in
the acceptance evidence.

Only direct bounded scalar/Boolean posts are admitted; calls, effects, configured
constants, constructors and unproved partial-render/getter exceptions refuse.
Current final records require original source/commit fingerprint alignment,
per-node write coverage and actual tableau done authority under existing graph
budgets. Completion remains diagnostic: ordinary clock/queue/error/count,
no-action and checkpoint composition are unfinished. No Holds, model-proof or
family credit is claimed; F-023 remains missing/audit-required.

### Independent indexed tuple-stack replay (#227)

`dive-trace::tuple_history` and `exists::TupleProof` (ADR 0081) were removed on
2026-10-12 (ADR 0016, amendment 3).


### Independent Strict substitution admission (#234): removed

The trace-side pre-entry proof for Strict substitutions (`context_admission`)
was removed on 2026-10-12 (ADR 0016, amendment 3). `dive-trace` now checks
only the bindings' effects annotations and contexts before entry.


### Independent initial-predicate context



Pinned TLC `tlc2.tool.impl.Tool.getInitStates` evaluates predicates with
`EvalControl.Init`; generated-state checks use `EvalControl.Clear`.
`tlc2.module.TLCGetSet.TLCGet` implements startup level 0,
while `TLCState.INIT_LEVEL` is 1. Native replay uses `St::initial` only for
Init and keeps ordinary state/action contexts for later checks.
`crates/dive-trace/tests/fixtures/startup-level/primary.json` records the
pinned one-worker control; the test reads its concrete states, and API and
trace tests reject altered initial, step and ending claims. This fixes an
inherited context mismatch without changing register or random admission.

## Action composition

`dive-eval/src/next/cdot.rs` follows `tlc2.tool.impl.Tool` at tlaplus
`94d0c50` (MIT): `OPCODE_cdot` in `getNextStates` (lines 1488–1531), `eval`
(2772–2808) and `enabled` (3371–3393). TLC's composition switch and intermediate
state accounting are retained in `ActionComposition::Tlc`. Intermediate counts
from nested `StateVec`/anonymous functors are intentionally discarded, as in the
pinned source; DFID and simulation use emitted successors only.

D-022 records the default TLA+ counterpart, which counts composed successors and
refuses enclosing conjunctions whose continuation or preassigned variables TLC
would distort. Tests retain the complete existing six-probe selection and its
consumed pinned oracle fixtures. The trace validator refuses action
composition, including in `ENABLED` (ADR 0103). Pure TLC-profile capture now uses the
inline fallback and counts intermediates as generated; only emitted successors
become transition edges. Ordinary liveness integration remains pending.

### PlusCal native check-report protocol

`xtask/src/pluscal_corpus/check_protocol.rs` is a bounded data inventory, not a
model-checking algorithm or a primary oracle. Its code/name pairs and failure
exits were checked against MIT `tlc2/output/EC.java`, including
`ExitStatus.errorConstantToExitStatus`, at pinned commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`. The success envelope uses
`TLC_SUCCESS`/2193 and exit 0 (`NO_ERROR`), rather than passing that progress
message through the failure-code switch.

Allowed native verdicts and default process exits come from dive's public
`crates/dive-api/src/check.rs` and `crates/dive-check/src/result.rs`; binding
and evaluation class inventories come from `crates/dive-model/src/error.rs`,
`crates/dive-eval/src/error.rs` and `crates/dive-check/src/class.rs`. Binding
evaluation errors are reported as `model-error`. Non-evaluation results report
their deciding code. Evaluation replay may preserve an evaluator error code or
a GENERAL/initial-state/invariant/action evaluation wrapper; unrelated terminal
or binding-only classes are rejected. Direct site wrappers report themselves.
Frontend rejection 3002 alone may omit the separate `reported` field. Assumption evaluation class 2105 uses default
exit 12 or TLC exit 10. These are native protocol associations, not independently
measured TLC model semantics. Unknown codes, names, exits or verdict associations
earn no agreement; the tool still compares dive on two translations.

The actual-main helper continuation preserves the complete bounded inventory and
pin loader. Native observations in both profiles cover success, invariant,
startup model/frontend rejection, assumption and evaluation errors, plus the
merged action-composition-off 2103/1000 replay envelope. The compact JSON under
`xtask/src/pluscal_corpus/fixtures/` retains only fields consumed by the scorer;
forged reported codes are explicit negative protocol controls. Neither injected
reports nor these native observations are independent TLC model measurements.
Option parsing and relative executable resolution are native tool behavior.
Discovery remains sorted lexical selection; unreadable selected inputs fail the
run. The pinned JRE archive identifies the unpacked tree, without attesting every
unpacked file. Full corpus measurement, gates and actual-main acceptance require
fresh execution after integration; prior producer/reviewer results are historical.


## PlusCal option semantics and artifact preferences

The shared `translate/options.rs` context and public CLI/API flags follow pinned
`pcal/trans.java` `parseAndProcessArguments` at commit
`94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT): termination/fairness defaults,
source spellings, noDoneDisjunct, width, version, reporting and backup/cfg
preferences. `PcalParams.VersionToNumber` supplies decimal thousandths and Java
i32 wrapping; `ParseAlgorithm` supplies the pre-1.5 pc/stutter optimization switch
and exactly-1.5 sequential fair spelling. `PcalTLAGen.GenNext` supplies the
no-done-disjunct effect. Its secondary single-assignment wrap guard tests an empty
self expression; admitted process IDs and `self` are nonempty, so that guard does
not add a reachable decision for these controls.

`crates/dive-pluscal/tests/fixtures/remaining-options/primary.json` is derived from
the recorded pinned translator controls and independently SANY-validated version
artifacts. The consumed 28 rows assert exact body bytes, body CRC, original/header
AST CRC, termination cfg requirements and requested label locations. These are
translator controls, not model-check parity. API and CLI tests keep both profiles,
source overrides, replacement/checksum consistency and no-write/no-proof results.
The external preparation histories retain the forced-CRLF writer experiment,
writeAST exit255, debug CRC difference, illegal-source-option exit0 and corrected
local-spec x=42 output; no primary was rerun for this implementation cut.

The earlier simple original-AST increment ports `AST.lineCol`, `Indent`/`EndIndent`,
`VarDecl`, `LabeledStmt`, `Assign`, `SingleAssign` and `Lhs` serialization from
the same source pin. `ParseAlgorithm.GetAssign` and `InnerMakeLabeledStmtSeq`
provide the source locations; `trans.WriteAST` provides the original-stage wrapper.
The original operation stops before `removeNameConflicts` and `translate`, skips
the post-generation shared-transfer overlay and returns no header CRC. The existing
post-generation checksum path retains that overlay and its prior constraints.
`fixtures/original-ast/primary.json` consumes two retained actual `-writeAST`
artifacts, the actual `-debug` header/body CRC pair and four retained pinned
DEFINE original-tree serializations exported after `removeNameConflicts` and
`translate` by `AstDump`. Those exports are not pre-generation `-writeAST`
measurements. The selected collision-free definitions and assignment/skip bodies
retain their bytes through these stages; procedure/macro stage ownership is not
inferred. No primary process was run for this increment.
At that increment, procedure/macro original ownership, debug compound locations,
pre-1.5 process/procedure ASTs and specification-backed translation stayed
unimplemented by name; the semantic continuation below updates the current scope.
File preferences are in-memory plans, not filesystem side-effect parity. The
measurement tool's `--label` transports the actual `-label` argument to both
translators; no new oracle or campaign result is claimed.

The semantic original-stage continuation ports `ParseAlgorithm.GetProcedure`,
`GetProcess`, `GetCall`, `ExpandMacroCall`, `SubstituteInSingleAssign`,
`InnerMakeLabeledStmtSeq` and `ClassifyStmtSeq`, plus the AST procedure/process,
compound and expression serialization templates from the same source pin.
`pcal::original` reuses typed build/labels/macro/expression kernels. Its original-only
builder mode retains parser-vector ownership through the last sequential
procedure/main boundary; process vectors reset independently. Original expressions
retain internal empty physical lines and Java UTF-16 token widths from `TLAToken.getWidth`;
normalized If/Either debug locations
remain distinct from the grouped label's saved source position. The existing
post-generation shared-transfer Writer remains independent.

`fixtures/original-ast-ownership` contains 32 genuine `pcal.trans -writeAST` /
`-debug -writeAST` artifacts captured before fixing or generation, eight supported
debug-header/body controls, three SANY-valid debug headers with unfinished native
labelled-transfer ownership, and four invalid source families. Successful WriteAST
controls produce AST.tla and exit255 through the pinned null translation path;
these records do not establish a universal exit rule or native file-write parity.
All eleven separately captured debug translations pass pinned SANY. No post-generation
AstDump export is relabelled as an original-stage measurement. The modern original
serializer does not certify a translated module or model, and does not expose an
early-stage header CRC. At that increment, generated debug procedure/labelled-transfer/
renaming ownership, historical fields, backends, filesystem writes and full P stayed
pending; the following debug-transfer continuation updates the current scope.

The debug-transfer continuation ports the observable alias rules of
`PcalTranslate.CopyAndExplodeLastStmt`, its WithGoto wrapper, `ExplodeLabelIf`,
`ExplodeLabelEither`, `ExplodeLabeledStmt` and `ExplodeWhile`. Labelled entry vectors
are shallow copies; nonempty nested compound vectors remain shared, while empty
inputs produce fresh vectors that are not assigned back to LabelIf/Clause fields.
Converted source If/Either coordinates become explicit before generated compound
nodes inherit their source LabelIf/LabelEither locations. Header growth uses the
existing shared budget and real UpdatePC constructor; original artifacts remain
before these mutations. The existing plain transfer Writer is unchanged.

The three retained labelled controls now consume exact primary header/body pairs.
The measured While CRC `5df9de` equals its original CRC, rather than proving a
mutation. Separately named `owners_sequences_translation_debug` invokes genuine
pcal.trans -debug with explicit EXTENDS Sequences, then actual SANY. It binds header
`b8cfe3ef` and body `5cd8f98a`. A reached exact-body failure exposed three extra
spaces before frame RHS `\o`; the common frame writer now follows
`MakeNewStackTopExprPretty` (operator column2 minus first-token column1). This is a
shared layout prerequisite, not fixture-specific formatting or a generation redesign.

Only flat, disjoint, explicitly labelled sequential procedural headers are lifted;
reached structural tests preserve named compound/process, macro/DEFINE,
automatic/fairness-label, renaming, simultaneous-ordering, unresolved-call,
argument-count and return-owner boundaries. SANY reaches semantic processing and
linting of Case with one changed-since-translation warning in all twelve debug
controls; the old nested-macro control also has an embedded missing-label validator
error because SANY lacks the translator's -label context. No clean original
checksum-validation credit follows from exit0. The actual-main source continuation
preserves the complete translator stages and the separately corrected shared
effective-grammar selector. Original AST ownership now uses that selector directly;
malformed-first, source-option masking, resource and strict-region boundaries remain.
The measurement helper retains its current pin/protocol kernels while adding only
actual `-label` transport to both translation command builders and the result record.
Full P, full149, fresh final review, gates, model parity and real filesystem writes
remain pending, and #246 is untouched.



### Private trace startup and deterministic initial membership

The private validator reuses `dive-model`'s actual binding operation, whose
constant processing order is the port of `SpecProcessor.processSpec` and
`processConstantDefns` at tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b`.
It repeats failed/unused eligible constant definitions rather than cloning
cached values. Config installation follows the existing API's post-bind,
pre-ASSUME lifecycle (`TLCGetSet` / `AbstractChecker.createConfig`).
`dive-trace/src/initial.rs` is an independently written ordered single-candidate
relation operation: empty slots, computed equality assignments and filters,
then witness comparison. It does not port or invoke TLC/dive state generation.
Runtime host/key checks preserve named refusals across swallowed startup errors.
The retained primary startup-register, startup-unread and off-trace-writer
controls motivate the stage boundary; they do not prove general register
replay or random membership. See ADR 0016 amendment 4 and `docs/user/traces.md`.


### Direct initial RandomElement relation

`dive-eval/src/random_relation.rs` is an independently written support-membership
operation, not a port of RNG/stream-position execution. The pinned TLC module
defines `RandomElement(s) == CHOOSE x IN s : TRUE`; its Java override at tlaplus
`94d0c5070852b37b801bb8998c436383ab7ee37b` selects from nonempty finite
intervals/sets, and reports EC2176 for non-set/infinite domains and EC2154 for
empty domains. The relation uses actual domain evaluation and `dive-value`
membership/comparison, retaining those error identities for its narrow stored
scalar-set/interval support. It does not consume or emulate the native RNG.
The actual retained P9 seed 0 witness 730968 is a positive control; the old
x=777 rejection is historical producer-sample mismatch evidence. Corrected
#257 and ADR 0016 amendment 5 separate existential witness validity from seeded
producer reproduction. Multiple/cached/correlated calls remain unsupported.


### SelectSeq callbacks in pure temporal predicates

The native selection loop in `crates/dive-eval/src/std.rs` continues the existing
port of `tlc2.module.Sequences.SelectSeq` at tlaplus `94d0c50` (MIT): convert the
first argument, return on an empty sequence, visit original positions in order,
apply the original predicate once per position, and preserve selected duplicates
and original typed errors. No new oracle algorithm is ported.

The evaluator's callback resolver and invocation-local bounded mode are native
Rust capability/work checks. They retain the actual lexical frame and refuse
unsupported binder, provider, effect, dynamic and ownership shapes without
normalizing providers or evaluating predicate truth. Definition/thunk caches
are bypassed only for that mode; ordinary evaluation keeps its existing cache
semantics. Authoritative configured/prebound and startup values are unchanged
and are not runtime callback certificates. Existing pure constant/IF probes
also enter the bounded mode with their original limits and error order. Each
atom replay initializes its own aggregate resource counters, shared through
sequential sub-evaluations; the trace evaluator initializes separate counters.
Planning and `dive-trace` independently walk returned scopes and
compute levels; neither shares a checker certificate or predicate result cache.
Scoped controls cover actual callback shapes, empty/error order, real bindings,
aggregate duplicate work, cold/warm cache isolation and fresh word/document
validation. These source controls confer no corpus parity credit; the three
previously reached corpus sites still require the normal measured gates.

The runtime capture-storage traversal treats only the pointer-identical current
SelectSeq input as a sequence shell, preserving per-element first-error order.
It does not use value equality or normalization; callback reads perform the
full reached-value traversal and unrelated captures remain strict. Structural
source-only projections receive no evaluated sequence and keep their direct
prebound-scope capability refusal. This is resource checking, not admission or
truth evidence.

The enabledness extension propagates that resource context through the existing
`Enab` state/control constructors and continuations. Ordinary `enabled_scoped`
and `exists_successor` retain their previous None-context paths. The independent
finite-cover search initializes its own context and retains its original cover,
assignment and first-witness rules; it does not use the checker enumerator or
checker admission. Ordinary unary <=State helper actuals stay lazy source
thunks. Source dependency scanning preserves successor bookkeeping through
sequence, callback and helper state captures without admitting explicit primed
or action callback sources. Unsupported composition refuses before generator
delegation. Scoped controls exercise enabledness/negation/WF/SF, complete and
missing covers, shared exhaustion, stored profile/owner errors and synthetic
warm-cache isolation. Actual bound startup semantics are unchanged.

For an invalid compound first SelectSeq operand, the existing bounded stored-value
walk runs before error rendering; deferred providers inside sets, records or
nonsequence functions refuse without enumeration. Ordinary scalar/type/empty
priority remains unchanged, and valid sequence elements are not preflighted.

## Bounded pure Json conversion

`crates/dive-eval/src/json.rs` adapts MIT `tlc2.module.Json` (`getNode`,
`isValidSequence`, `getObjectNode`, `getArrayNode`) at tlaplus
`94d0c5070852b37b801bb8998c436383ab7ee37b`, with the matching CommunityModules
`modules/tlc2/overrides/Json.java` and `modules/Json.tla` at
`78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3`. Gson's compact non-HTML-safe string
escaping is shared with the existing value-layer serializer; its format source
is Apache-2.0. Runtime conversion is native and performs no host I/O.

The salvage reference is historical source `1f21101a8e8ccc7bda05be6d8dab24378f139254`,
tree `7ad7e85c9a98ad3603cd5eff0718fc771cd4d74f`. Only its two Json commits are
adapted onto accepted main `8036d335fcfcd4cd8ff9848632b6c06a94010ba2`; no old
checker, validator, host, PlusCal, simulation or dependency-graph implementation
is imported. Tests use production model binding. The opaque deferred-key control
now exercises the public evaluator rather than relying on the removed opaque
context-admission requirement; it measures provider calls after construction.

Pinned `getObjectNode(FcnRcdValue)` prints non-string domains with `toString()`.
`RecordValue.toString` retains original field order, absent from normalized dive
records. The consumed `JsonDomainHistory` keeps `Unused == [a |-> 0]` before a
`[b |-> 1, a |-> 2]` construction: historical primary output retained `b,a`,
including inside tuple/set/function keys, and the equality predicate was TRUE.
TLC value order refuses these missing-history cases; canonical default order
follows D-006. Domain traversal borrows stored children under the evaluator
recursion guard before sorting/printing and refuses symbolic/deferred expansion.
Representation-dependent Array/Object entry points and host I/O remain refused.

The consumed compact `tests/fixtures/json/oracle.json` records ten historical
outputs from `tlc-2026.10.06.014338` with the configured full CommunityModules
classpath. Genuine raw producer files remain outside the repository; prior
filename-collision comparisons receive no credit. Focused native/pinned-oracle
controls and a fresh independent review validate the supported conversion and
refusals. The runtime-name regression distinguishes names already interned by
model binding from newly created JSON strings; the latter retain the existing
TLC-profile trace-validation refusal (#124). Full-gate and corpus measurements
use their actual recorded source revisions rather than historical source labels.

## Basic simulation action inventories and flows

The bounded action inventory and sampled-flow port follows tlaplus commit
`94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT):
`SimulationWorkerStatistics.collectPostSuccessor`,
`Simulator.getActionFlowGraphSnapshot(REDUCE)`, `Action.getDefinition`,
`SpecProcessor.processConstantDefns`, `SymbolNodeValueLookupProvider.lookup` and
`Tool.getActionsAppl`. Sparse chosen-action counts retain source-location context
reduction. Actual value lookups preserve invocation predicates; configured
definition targets supply body trees without flattening lookup state.

The bounded conformance controls cover twenty generated inventory cases, public
inventories, chosen pairs and supported production DOT weights, including
cached-disjunction seed/aril cases. The current leaf passed independent scoped
review, the full workspace gate and all four unchanged small-corpus comparisons
against the accepted baseline. Earlier oracle and source measurements retain
their recorded revisions. Configured constant-effect refusals remain explicit;
broader simulation, generation, schedulers, DFID and coverage remain pending.
See [the simulation contract](user/simulation.md).

## Ordinary simulation Print/PrintT

The return and rendering behavior follows MIT `tlc2.module.TLC.Print` and
`PrintT` at pinned tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b`.
The existing evaluator performs argument evaluation and rendering; a fallible
host hook then emits or discards the event. Private fresh-word validation permits
these value semantics while discarding its own binding/replay output without
copying caller callbacks, hosts or atoms. Other reached effects retain their
existing refusals, including D-017 host-access restrictions.

Nine clean finite primary simulation probes at seed 0/aril 0, one worker and
one trace establish direct, higher-order and configured logger output phases,
including the original startup Print event before Init/runtime printing. The
compact consumed fixture records actual generated counts, exits, deciding codes
and words. Reached RandomElement/register cases remain native refusals, not
claimed agreements. Genuine raw producer data and tool/input bindings remain
outside the repository; native scoped verification, fresh review and publication
must retain their actual new-source identities. Full M/#239 remains open.

This Print source rederive preserves the accepted basic-statistics/inventory
implementation and adds combined Print plus none/basic controls. The genuine
nine-probe fixture and earlier scoped measurements retain their original source
bindings. Latest-main native qualification, fresh review, full gate and Print
publication remain pending; the accepted statistics gate is not Print gate credit.

## Empty-property final checking and model-error origin

When the configured `PROPERTY` list is literally empty,
`dive_api::temporal::check_final` delegates to ordinary safety checking and
returns `Outcome::Safety`. It does not bind twice, build a temporal capture,
consume temporal limits or issue a temporal holding claim. Binding and
symmetry-preparation model errors retain their actual `binding` origin;
engine model errors retain `capture`. `CheckResult::binding_model_error`
records that origin without inferring it from an error class or statistics.
Configured state/action properties keep the existing final-mode route.

The reviewed runtime source is `72ae013e8897b56eb76574ad3fb7a7293a7b53c3`, tree
`a5dbf925b8165bf7b6f345d55bf34359dc9a2c8a`, directly based on accepted main
`24b4994d78403b36bb716c6aca408370c8e7393f`. Its actual full gate passed
1,726 workspace tests without skips, 84
doctests and 456 SANY comparisons; CM1 and validation-smoke passed at 1/4/8.
The completed original 154-model matrix retains the original 600-second bounds,
pins, goldens and per-ID no-loss obligations at TLC/default workers 1/4.
No corpus agreement is added by this bounded routing repair.

Scoped independent review covers 93 tests, eight production 2PC controls and
107 adversarial cases across 203 CLI calls. These and the full-gate/matrix
measurements retain their actual runtime source identity; the two later
Markdown acceptance notes do not retag them. The earlier `565bffe8` failed gate
is preserved and receives no corrected-source completion credit. Publication
source/main CI checks are separate obligations. Ordinary liveness, broader API
coverage, the runtime-name trace refusal (#124) and the complete roadmap remain
open. See [the temporal API contract](temporal-api.md) and
[the temporal checking guide](user/temporal.md).

## Current-parent Print capture and shared-core acceptance

Ordinary finite simulation retains invocation-owned Print/PrintT output under
its declared event/text limits. Independent private startup/replay retains value
semantics while discarding its own output. A shared evaluator core's simulation
lease preserves operational ownership through host restoration; overlapping or
reentrant simulation refuses without taking another invocation's output.
Reached random/register/I/O and other unsupported effects retain their refusals.
None/basic sampled-flow settings keep their existing bounds and meaning.

Runtime source `2fa57f734adf1a065555d7ab18f40b1cd2ca416b`, tree `24f070f2b326d5a6526c8342b09f471e62179b21`, directly based on actual accepted API main `4b6b6fd4a6bb1e0c69a9b0abfce61c52a0237571`.

Its actual full gate passed 1,745 workspace tests with no skips, 86 doctests and 456 SANY comparisons, with CM1 and validation-smoke at 1/4/8. All four original 154-ID TLC/default 1/4-worker reports retain original pins, goldens, 600-second bounds and per-ID no-loss obligations.

Current-parent scoped execution passed 73 tests with no failures, ignores or filtering and recorded 36 JSON CLI cases; the separate human controls and fresh review keep their actual receipt bindings.

Historical failed overlap, rejected cuts and invalid experiments remain preserved and uncredited. The later two-Markdown publication transfer does not retag runtime measurements. Actual source/main CI is a separate obligation. Full M/#239, generation, schedulers, distribution/DFID and general coverage remain open; sampling is no safety/liveness proof.
See [the simulation contract](user/simulation.md) and
[the independent trace contract](user/traces.md).

## Bounded SVG PointOnLine numeric override

`crates/dive-eval/src/svg.rs` follows the installed
[`SVG.pointOnLine`](https://github.com/tlaplus/CommunityModules/blob/78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3/modules/tlc2/overrides/SVG.java)
(MIT, release `202610040242`). Signed32 coordinate subtraction precedes
binary64 division/addition and Java double-to-int narrowing. Wrapping subtraction
and a truncating/saturating float-to-signed32 cast preserve this order, including
NaN zero; mathematical floor division and unbounded differences are not the
installed operation. Both profiles use the same result on this slice.

Pinned tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b` `MethodValue.eval`
invokes the typed Java method on evaluated arguments; `RecordValue.select`
returns null for missing coordinates. Unimplemented JVM representation/error
shapes remain named refusals. Deferred function values stay unmaterialized while
later argument expressions are evaluated; existing argument errors and explicit
TLA redirects retain their precedence. Records are bounded at 256 stored fields
and evaluator depth remains an independent resource guard.

The historical implementation `68909913`, corrected only for reference inventory
as `ca72da1e`, supplies the reviewed numeric kernel and consumed fixtures. Its
nineteen actual primary controls include eleven numeric results, seven JVM-shape
errors and a later division error; a separate primary checker fixture reports
Inv/2110, exit12, two generated/distinct states and depth2. The historical
independent review's thirteen primary controls and native good/forged words keep
their original revisions. They are not new current-source runs.

This source-only rederivation adds the small helper to current evaluator routing,
retains accepted Json/M/S code and adapts its CLI control to ADR0097 physical
statuses. Compact inputs at `crates/dive-eval/tests/fixtures/svg-point/inputs.json`
are consumed by tests; the small SVG module is a binding fixture, not an oracle
snapshot. The earlier direct24b499 leaf `bc070808` passed eighteen scoped tests and four
real CLI profile/store good/forged/refusal groups, followed by an independent
bounded numerical/public-API review. Those measurements retain that revision.
This new API/Print-parent transfer still needs current-source qualification,
fresh transfer review, full gate and complete corpus matrices. No new Examples/VTS agreement, SVG completion
or coverage percentage is claimed.

## Current-parent bounded SVG PointOnLine acceptance

The bounded signed32 stored-record PointOnLine helper follows the pinned
CommunityModules override: wrapping coordinate subtraction, binary64
interpolation and truncating/saturating narrowing. Deferred and unimplemented
representations retain named refusals and existing error/host/redirect priority.
Accepted API error routing and Print shared-Core ownership remain intact.

Runtime source `d40c12e626a0c300faeea20adebc1ba9a5f0ec97`, tree `045ccf994a7aadad2ae7f121487a1bab258d88fe`, directly based on actual accepted Print main `952c84bb38e0c044da5188c03712c3758604f219`.

Its actual full gate passed 1,753 workspace tests with no skips, 86 doctests and 456 SANY comparisons, with CM1 and validation-smoke at 1/4/8. All four original 154-ID TLC/default 1/4-worker reports retain original pins, goldens, 600-second bounds and per-ID no-loss obligations.

Current-parent finite scoped execution passed 76 tests with no failures, ignores or filtering and recorded 36 JSON CLI cases; all three external probes and the genuinely fresh independent review keep their actual receipt bindings.

Historical compile/parser failures, invalid compound-initial fixture and review build attempts remain preserved and uncredited. The later two-Markdown publication transfer does not retag runtime measurements. Actual source/main CI is a separate obligation. Full SVG/C, layout, serialization, animation and general CommunityModules parity remain open; issue #272 remains open.
See [the module contract](user/modules.md).

The experimental configured-safety-role route reuses the ordinary checker
binding and complete post-binding phases from ADR 0015. It inspects the real
`dive-model` role split after one binding under the original checker host, and
never strips configured PROPERTY obligations or resets startup effects. Its
initial option/source scope is deliberately narrow so excluded temporal
admission priorities remain unchanged. Historical PlusCal refinement refusals
remain their original observations; scoped qualification, independent review and
full gates for this source are pending. No temporal-class or corpus parity credit
is inferred from the refactor.

## Current-parent configured safety-role checking acceptance

Explicit final/short checking can run configured properties that bind entirely
to initial, invariant or action obligations on the narrow default-memory route.
One checker-owned startup binding/host enters the complete original ordinary
checker tail; no property obligation is erased or startup stream reset.
Excluded options/transformed randomness and genuine temporal obligations retain
their existing routes, limits and error/refusal priority. Accepted API binding
origin, Print shared-Core ownership and bounded SVG numeric behavior remain.

Runtime source `f939f4bfdcf131b94a259bd445655e206d9a4c4f`, tree `a5235897aee36ddbfe8182d1acae528bfd256aec`, directly based on actual accepted SVG main `69cfb8b0d0bcd2721157331d792e58539cf52072`.

Its actual full gate passed 1,767 workspace tests with no skips, 86 doctests and 456 SANY comparisons, with CM1 and validation-smoke at 1/4/8. All four original 154-ID TLC/default 1/4-worker reports retain original pins, goldens, 600-second bounds and per-ID no-loss obligations.

Current-parent finite execution passed 140 selected tests with no failures, ignores or unexecuted selections; 142 other tests are explicitly unselected. It recorded 38 expected-status Print CLI controls and 32 doctests. Fresh independent review passed the same selected population, 4 additional Rust probes and 24 supplemental CLI commands. Failed diagnostic/helper attempts remain their actual negatives.

Historical source/runtime failures, invalid external helpers and review experiments remain preserved and uncredited. This later two-Markdown transfer does not retag measurements; actual source/main CI is separately verified. General API, temporal and TLC parity, broad P/C/M completion and issue #272 remain open; no state-space gain or roadmap completion follows from this bounded change.
See [the temporal API contract](temporal-api.md) and [final checking](user/temporal.md).

## Current PlusCal best-effort checksum retirement

Issue241's expert DELETE disposition retires `translate/checksum_transfers.rs` and
its procedure-header alias/expansion assertions. Earlier provenance sections for
nested, acyclic, recursive and process-owned Full headers describe historical
captures and removed compatibility machinery, not current algorithm-CRC support.
Procedure-free exact serialization and line-concatenation body CRC remain real
features. `pcal.Validator` (pinned94d0c507) recognizes a disabled pcal value while
still checking TLA CRC; `pcal.trans` preserves that spelling and updates present
body CRCs. Genuine ordinary pcal/SANY/TLC controls confirmed algorithm/body warnings,
literal omission losing body warnings, and Noop overwrites before implementation.
The new consumed best-effort-checksum fixtures are byte copies of actual pinned
primary outputs: original source and exact/disabled-input routes remain distinct.
No native runtime invokes Java, and header choice is not model-parity credit.

The retired procedure-header capture filenames in the historical sections above
are provenance records, not active fixture consumers. Only the485 orphan `.ast`
files in those six families and the procedural Elsif family are removed. All740 original manifest facts remain;
688 row instances have genuine current Git inputs, with explicit sibling lineage.
The52 absent-input instances/26 IDs (including14 successful primary facts) are
unexercised debt, not native refusals or successes. Original-AST ownership captures
and generation/body-CRC fixtures are separate active controls.

The separate procedural Elsif fixture family retains32 genuine inputs and primary
body-CRC facts, with generator/resource and API module-admission controls. It adds
32 input rows to the688 source-closed rows above; the52 missing historical rows
remain unexercised. Its former32 header-alias AST captures are included in the485
retired orphan files. No procedure-free exact CRC or original-AST fixture is removed.

The rewrite separator follows pinned `PcalTLAGen.GenSpec`/`GenTermination`'s final
empty line before `END TRANSLATION`. The actual Case primary control exposed a
missing native separator even with equal algorithm/body CRCs. Editing now retains
that separator and charges its bytes to the rewrite quota; no header stripping,
body-CRC substitution or primary fixture change is used to claim equality.

The physical-line coordinate fix follows Java `BufferedReader.readLine`: bare CR,
LF and CRLF each delimit one line while native byte offsets remain original.
Actual pinned LF/bareCR translation modules are identical (body CRC `c38de1c8`);
a bound pre-fix native API probe produced `98ffe10c` for bareCR. The fix changes
only generator/original-tree position indexing; legacy direct exact CRC retains
its named bareCR Unsupported boundary. Uniform/mixed line, span and tab controls
and genuine module/body comparisons cover the change separately from checksum
admission retirement.

The compatibility overwrite choice is a distinct later implementation decision
from retaining native protection in the initial checksum plan. `pcal.trans`'s
ordinary Noop behavior is the counterpart: default native protection is retained,
explicit native overwrite remains available, and the global TLC profile resolves
that overwrite automatically. The newly captured exact modified-body pcal/SANY
control regenerates the original complete primary module with physical exit0.

## Verified local-class overrides

`dive-model` explicitly recognizes the Hanoi `Bits.class` from
`tlaplus/Examples` commit `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, path
`specifications/tower_of_hanoi/Hanoi.toolbox/Model_1/Bits.class`. SHA-256:
`225d2d3d163b5a2c2923544de619fb6f23cb86804ed06bd2842ca27a242b846b`.
The project license is MIT, copyright 2016 The TLA+ project and other contributors.
The primary source `specifications/tower_of_hanoi/Bits.java` declares four
`IntValue` arguments and returns `IntValue.gen(x.val & y.val)`. Its historical
import names `tlc2.value.IntValue`; the compiled class uses `tlc2.value.impl.IntValue`.
`javap -c -s` confirms that descriptor and the entire method body: load the first
argument's `val:I`, load the second's, `iand`, invoke `IntValue.gen:(I)`, return.
There are no other static methods or class initializer. `dive-eval` ports only
this signed 32-bit AND; all four arguments must be TLC integers, even though
`n` and `m` do not affect the result. No Java execution is involved. The 472-byte
test fixture is the pinned class itself and is read by the activation tests.
[ADR 0087](decisions/0087-known-local-class-overrides.md) records hash activation
and unknown-class refusal.

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

Earlier e5ca/876/cbb/d1 receipts retain their recorded source identities. New
API+Print-parent transfer measurements remain pending and are not retagged.

The startup-refusal regression uses `Bits!And(x,y,n,m) == 99` with a constant
invariant `And(7,3,0,7) = 3`, an initial state `x=0`, and transitions incrementing
`x` through3. Pinned TLC94d0c507 accepts both the exact472-byte class and a
JVM-valid variant changing its final SourceFile metadata index from13 to12;
each reports success2193/physical0,4distinct/4generated/depth4. The variant's
SHA-256 is `9fd6119264baeef76354cc4b70d365788edd2edbd9e6aa2a1a646e3d7bf2c070`.
It deliberately remains outside native hash admission. Previously the native
binder evaluated the unlike TLA fallback and reported2230 instead of refusing.
Binding now preserves configuration substitution and constant-operator arity
errors, then refuses unknown local classes before startup evaluation. Constant-role, assumption and typed
fallback-error controls exercise both public checking loaders in both profiles;
the exact class still executes and no-class modules retain TLA semantics.

### Direct configured operators in temporal atoms

The direct `ConstBinding::Def` call projection in temporal planning, finite
preflight and independent word validation reuses the evaluator's checked lazy
`scope_apply` contexts. Reference semantics are `Liveness.astToLiveAppl`
(`lookup`, lazy `getVal` formals and retained state/action call),
`Tool.getOpContext` and `Tool.lookup` at pinned tlaplus commit
`94d0c5070852b37b801bb8998c436383ab7ee37b` (MIT). The structural checks and
independent word algorithm are original Rust implementations. Configured origin
is taken from the direct constant declaration; captured operator analysis keeps
its separate configured-origin refusal. No native evaluator algorithm changes.

The dedicated `configured-temporal-operators` corpus contains two small original
Reply models, with harness-generated goldens from `tlc-2026.10.06.014338`:
Holds is success/2193/exit 0; Fails is temporal violation/2116/exit 13 naming
`Live`. Both have 2 distinct, 3 generated states and depth 2. The primary runs
use worker 1, fp 0, seed 0 and final checking. Fails carries the independently
checkable 0-to-1 word stuttering at 1. These are new fixture observations; older
real-corpus measurements keep their own source and binary identities. No
memory-model, allocator or general temporal parity gain is claimed here.

## Finite local recursive-function temporal scopes

This extension exposes existing runtime RecFcn/LazyFcn captures; it does not port
a new recursive evaluator. The existing MIT TLC `FcnLambdaValue` behavior at
pinned tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b` remains the execution
reference. The tiny PermSeqs fixture preserves the recursive self/domain, nested
LET constructor and Append map shape from the pinned Examples allocator.

Seven actual primary captures precede implementation: empty/one/two-element
weak-fairness holds/fails and a direct action control. The consumed compact
fixture is generated from pinned TLC/JRE, complete configured classpath and real
physical/trace receipts; raw outputs remain external. Two-element recursion
generates duplicate successor work, retained in the one-worker count contract.
Original allocator measurements retain their earlier producer identities; this
transferred source has no new original-model or corpus qualification.

Configured-call source inspection retains its direct declaration origin through
the substituted target body, helpers and lazy slots in each of the four private
walks. This independently preserves the existing configured-recursion refusal
when an ordinary first-order target contains a local RecFcn; Def.recursive alone
is not that shape. Actual Function slots refuse before body projection or memo
reuse. Ordinary local recursive analysis keeps its prior behavior and caps.
The existing FdRecursivePerm temporal-domain fixtures still refuse: self/formal
analysis uses conservative state unknowns and does not establish a closed
recursive result constant for temporal expansion. Typed planning/source/evidence
controls retain this boundary; the changed diagnostic is not new parity.

The sequential capture classifier retains separate real-effect, runtime-name and
TLC composition-accounting categories during the existing bound checking-path
walk. The ordinary sequential reason list and checkpoint/DFID decisions are
unchanged; capture reads the real-effect category without parsing diagnostic
text. The consumed `name-cdot` fixture records two genuine pinned TLC
`tlc-2026.10.06.014338` runs with state-dependent `SubSeq` operands and composition:
holding success and a failing `baba` to `ba` stuttering behavior, both 5 generated,
2 distinct and depth 2. The emitted string is measured rather than inferred from
an encounter-order prediction. An earlier one-character fixture is also retained;
pinned TLC reports success for it (5 generated, 2 distinct, depth 2), which dive
matches since liveness atoms are evaluated only where evaluation reaches them. Its `a`/`b` names were
already interned at binding, so that fixture does not demonstrate name growth. Independent composed-word replay remains refused;
this is no new purity or replay admission proof (#241).

## Nested Boolean ENABLED guards

The native execution remains the existing MIT TLC `Tool.enabled` / `Enab`
semantics at pinned tlaplus `94d0c5070852b37b801bb8998c436383ab7ee37b`:
queries branch the lexical environment and start a fresh inner successor from
actual s0. No new native successor search algorithm is ported. The independent
validator extends its necessary-cover procedure with a private ordered Boolean
spine and its own nested existential decisions; it consumes no producer approval.

Two primary-first references in
`crates/dive-liveness/tests/fixtures/nested-enabled/` were executed with pinned
TLC/JRE, worker1, final checking, fp0 and seed0 before implementation. Hold has
physical0/code2193; Fail has13/code2116/propertyFail. Both have generated3,
distinct2, depth2; the actual failure word is x0,x1 with lasso back0. Tests consume
the compact generated observations and independently validate both native and
primary words plus corruptions. Raw physical/tool/input receipts remain external.
No new original-model/corpus agreement, general error parity or universal native
assignment-work guarantee follows. Existing original-model measurements retain
their own source and binary identities; current-source qualification is pending.

## Compact internal product acceptance

The coordinator's flat acceptance representation is an original Rust storage
change over the existing product and deterministic search. It reuses the same
reachable-pair builder, legacy logical guards, renumbering and original-edge
projection as public owned products. Promise and recurrence truth are stored in
flat `u64` columns; one offset vector preserves per-source product-edge positions.
Recurrent-edge truth is implicit. Recurrences are evaluated once at construction
from existing labels with temporary reusable Boolean scratch, not during search.
This storage change ports no new TLC algorithm. Existing owned/legacy APIs keep
those logical guards. The additive byte policy (ADR 0098) can instead retain
recurrence truth by behavior-source/original-edge position, sharing immutable
Boolean labels across tableau transitions. Existing product projection maps
supply constant-time search lookup; equal-target parallel edges remain separate.
Checked layout/work metadata selects once between behavior and product domains,
with no speculative evaluation/retry or new predicate calls. Covered buffers and
metadata/initialization/projection/Boolean work retain the 8 MB/32 million
allowances. Finite truth, full-witness and legacy-limit controls retain the public
owned implementation as a comparison route alongside independent language checks.

## Direct scalar simulation Init RandomElement

Startup/worker separation follows MIT tlaplus commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`: `Simulator` constructor/`simulate`,
`SimulationWorker` seed setup, `RandomEnumerableValues` and `TLC.RandomElement`.
The implementation reuses existing `dive-stdlib` startup streams and native
domain behavior, with no algorithm port or independent-validator change.
Eleven tiny primary observations were captured before production implementation
from pinned `tlc-2026.10.06.014338` and the configured JRE. Their consumed compact
fixture records real words, counts, physical exits, errors and Print events;
raw commands and outputs remain external. The reached Next-random row is a
refusal boundary, not an agreement. The observations retain their original source
and binary identities; this adds no corpus or complete-mode claim.

## INSTANCE query namespace controls

The `instance-temporal` corpus records pinned MIT tlaplus commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`, oracle
`tlc-2026.10.06.014338`, before changing runtime semantics. Ten exact finite
Boolean inputs retain their original primary bindings in
`conformance/corpus/instance-temporal/selection.json`. Their exported conflict
and old-binding cases fail the initial invariant (2107/12); direct, outside,
nested shared-name and fresh-binding controls distinguish the cutoff.

The source explanation uses `tlc2.util.Context.branch`/`lookup`,
`tlc2.tool.impl.Spec.getPrimedVar`,
`tlc2.tool.impl.SymbolNodeValueLookupProvider.getVar`,
`tlc2.tool.TLCStateFun.bind`/`lookup`, and
`tlc2.tool.impl.Tool.enabled` in that upstream tree. Cut lookup follows a lazy
binding's captured context. Unprimed reads retain actual current-state lookup;
the query's fresh successor can contain names absent from root state variables.
`TLCStateFun` keys by `UniqueString` name, so two hidden references named `q`
share a successor slot rather than acquiring distinct declaration-ID slots.
This explains why nested aliases can remain correlated while exported `y`/`z`
choices separate. It is a source interpretation constrained by the measured
controls, not a claim that every INSTANCE case is implemented.

No algorithm or runtime change accompanied the separate #300 corpus import. Existing harness
generation supplies SANY/model goldens and clean-versus-trace consistency fields;
a TLC self-check gives no independent trace-validation or original-model gain
credit. General instantiated temporal planning, successor namespace execution,
callable origin and independent full-word validation remain open.


The following evaluator/checker implementation uses those observed semantics:
`dive-eval::NamedState` owns query-local canonical-name assignments;
`Core::state_variable` reads either indexed root state or that named state;
`Enab` retains its original source-ordered continuation search but binds
canonical names. This is an idiomatic Rust implementation of the behavior,
not a structural Java port. Prime/UNCHANGED retain complete named candidates,
cached lazy values require compatible assignments and actual evaluator context,
and dynamic branch projection preserves captured versus newly entered links.
Indexed graph states and existing root-only Init/Next generation are unchanged.
Action composition refuses assigned hidden-name projections instead of losing
them. No independent trace-search implementation is borrowed from `Enab`;
name-aware independent covers, membership, word validation and callable/temporal
traversal remain follow-up work. See ADR 0099.


### Native callback ENABLED lookup control

Pinned `tlaplus/tlaplus` commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`,
`tlc2.tool.impl.Tool.evalImplOpArgKind`,
`tlc2.value.impl.OpLambdaValue` construction and `eval`, and
`tlc2.module.Sequences.SelectSeq` define the callback context. A native override
requests `EvalControl.Clear`; `OpLambdaValue.eval` restores the `Enabled` flag
from the context where its operator argument became a native callable. That
flag controls primed substitution cutoff. Clearing it can resolve an inner
primed variable through an unrelated same-named outer variable.

Dive invokes these native callbacks synchronously with their creation states
and dynamic context. `Core::apply_op_values` preserves that invocation's
`ENABLED` bit while clearing its other control bits. Lexical frames, named
candidate assignments, callback work accounting and temporal admission guards
retain their existing owners. This follows the observed callback semantics;
it does not replace general callable/temporal scope admission with a proof cache.

The upstream `Github725g.tla`/`Github725h.cfg` safety control was run against the
pinned primary. It reports the initial invariant violation (2107/12), while the
previous Dive runtime reported success. Two minimized `CallbackEnabled`
specifications pass strict pinned SANY: requiring the callback result
`<<1, 2, 3>>` makes the action disabled and violates `Inv` initially; requiring
`<<2, 3>>` makes it enabled and satisfies `Inv` (2193/0, two generated states,
one distinct state, depth one). Both old native profiles invert these verdicts.
The compact primary fixture in
`crates/dive-api/tests/fixtures/callback-enabled/primary.json` was derived from
these actual streams with the existing conformance classifier, not from native
output. The public API regression checks both profiles, storage modes and
worker requests, plus independent genuine and forged finite-word replay.
No existing golden, original model bound or corpus population is changed.

Independent word validation projects the original callback body and lexical
capture for a reached query without evaluating analysis-only formals. Primed
reads join its name-aware dependency set and require independently established
finite covers. Runtime callback inspection uses the actual named candidate and
preserves query cutoff control. Ordinary state-helper admission remains
separate; effects, providers, opaque captures, unsupported binders and nested
query data positions retain their refusals. Source inspection and reached
element evaluation use the existing invocation work bounds. No checker
successor or cached witness is used by replay. Direct typed and document
validation cover the minimized genuine and forged words; renamed/nested module
controls are native metamorphic tests and add no primary corpus credit.

### Hidden-query LazyValue cache controls

The six `lazy-enabled-cache` fixtures are minimal dive-authored controls derived
from the INSTANCE/SelectSeq scenario investigated in issues #310–#313. Primary
source inspection uses MIT-licensed tlaplus commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`: `LazyValue.java` constructors,
`getCachedValue`/`getValue`; `TLCState.java` `isSubset`; `TLCStateFun.java`
immutable bind/copy; `Tool.java` `enabledApplImpl`'s `OPCODE_eq` and
`OPCODE_in` cases; `SymbolNodeValueLookupProvider.java` `getOpContext`/`getVal`;
and `Sequences.java` `SelectSeq`. No implementation algorithm
is ported in this prerequisite. Default pinned oracle generation supplies the
goldens; cache-disabled research results are not conformance references.

### Independent query lexical lifetime controls

The 24 dive-authored `lazy-query-lifetimes` fixtures distinguish lexical
context lifetime from state inclusion under default pinned TLC. Reference
source is MIT-licensed tlaplus commit `94d0c5070852b37b801bb8998c436383ab7ee37b`,
`Tool.java` `enabledImpl` LET/application handling and `enabledIn` recursion,
`LazyValue.java` `getCachedValue`/`getValue`, and `TLCState.java` `isSubset`.
Only primary fixture behavior is recorded here; no implementation algorithm
is ported in this prerequisite. Goldens are produced by the pinned harness
without diagnostic cache switches.

The independent partial-state executor uses the original source, necessary
finite covers and its own bounded pending predicates. The pinned `Tool.java`
`enabledApplImpl` conjunction/disjunction, equality/membership, bounded
quantifier, IF/CASE, action-subscript and UNCHANGED cases define the observable
source binding and reachability semantics. LET/application contexts are
created when reached, not during cover inspection. The query memo's weak
function capture follows ADR0101; lazy functions retain the originally forced
partial states and shared work without retaining the memo itself.
