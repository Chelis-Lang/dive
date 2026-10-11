# Provenance

Every algorithm ported or adapted from another implementation is recorded here. Each entry gives:
- the `dive` component;
- the source project, file and class or method, and the version or commit;
- the source's licence;
- the nature of the port: semantics matched, or algorithm adapted.

The canonical tlaplus tools are MIT-licensed. Translated code carries attribution.

| dive component | Source | Licence | Nature |
|---|---|---|---|
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

## Symmetry closure foundation

`dive-model/src/symmetry.rs` matches `MVPerms.permutationSubgroup` and
`MVPerm.compose` at tlaplus `94d0c50` (MIT): generators first, identity and
duplicate removal, ordered frontier closure, and first-map-then-second-map
composition. The bounded dense index representation, checked bijectivity and
resource refusals are original implementations. The 1,329 direct Java-kernel
fixtures include malformed maps. This does not port `Tool.getSymmetryPerms`,
its warnings, canonicalization or state/trace reduction yet. The original MIT
Java driver and raw SyEmpty capture are in
`crates/dive-model/tests/fixtures/symmetry-group/`.


The pure symmetry declaration stage matches `SpecProcessor`'s pre-constant
`Defns.snapshot` and `Tool.getSymmetryPerms`' name, original-body evaluation,
enum-set normalization and function-record checks at tlaplus `94d0c50` (MIT).
Four early errors are checked against the existing clean raw symmetry captures;
configuration replacement and pure/effectful controls supplement them. Warning,
model-value and group checks are separate pending stages; no reduction is enabled.

Unverified declaration-evaluation and normalization startup errors refuse by
name rather than reuse an evaluator diagnostic as an exact startup message.
A division-by-zero control verifies that no TLC code is claimed for that refusal.


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
validator's runtime-name refusal is dive's own admission rule (#124), supported
by the clean pinned capture and prior false-positive trace in
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
