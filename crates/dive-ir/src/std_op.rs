//! Operators that TLC implements in Java instead of evaluating their TLA+
//! definitions (ADR 0013 §5).
//!
//! TLC installs them in `SpecProcessor.processModuleOverrides`
//! (`tlc2/tool/impl/SpecProcessor.java:2114-2324` at tlaplus `94d0c50`), in
//! two phases that lowering reproduces:
//!
//! 1. **Module classes** ([`Phase::ModuleClass`]). For every loaded module
//!    `M` with a class `tlc2.module.M`, each public static method without an
//!    override annotation is matched by name (after `TLARegistry.mapName`,
//!    which maps `Plus` to `+` and so on) against the definitions in `M`'s
//!    context, including the ones `M` extends. Modules are processed in
//!    dependency order and a later match overwrites an earlier one, so in a
//!    specification that loads `Integers`, `Integers`' methods replace
//!    `Naturals`' for the operators they share. (`Divide` differs: only
//!    `Integers.Divide` checks for `-2147483648 \div -1`, and only it rounds
//!    a negative quotient that truncates to 0 down to -1.)
//! 2. **Annotated methods** ([`Phase::Annotated`]: `@TLAPlusOperator`,
//!    `@Evaluation`, `@TLAPlusCallable`) of the classes listed by
//!    `tlc2.module.TLCBuiltInOverrides`, then by the `CommunityModules`'
//!    `tlc2.overrides.TLCOverrides`. Each names a module and a definition,
//!    looked up in that module's context. Several `@Evaluation`s of one
//!    definition are combined; otherwise the last one installed wins.
//!
//! Matching is by module *name* and definition name, as in TLC: a user module
//! that happens to be called `Sequences` gets the overrides, and a user
//! definition called `Len` in another module does not. TLC attaches the
//! override to the definition's body and finds it through substitution
//! wrappers, so instantiated copies (`S!Len` for `S == INSTANCE Sequences`)
//! share it.
//!
//! The table was extracted from the class files of the pinned `tla2tools.jar`
//! (`tlc-2026.10.06.014338`) and `CommunityModules-202610040242.jar`: method
//! access flags, parameter counts and annotation values. It therefore lists
//! what the pinned TLC loads, not what its sources suggest. Provenance:
//! `docs/provenance.md`.

use serde::{Deserialize, Serialize};

/// How TLC calls an override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StdKind {
    /// A `MethodValue`: the arguments are evaluated first, left to right, and
    /// passed as values.
    Value,
    /// An `EvaluatingValue` (`@Evaluation`): the method receives the argument
    /// expressions and the context, and evaluates them itself.
    Evaluating,
    /// A `CallableValue` (`@TLAPlusCallable`).
    Callable,
}

/// Which of TLC's two installation phases installs an override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    /// A module class's unannotated public static method.
    ModuleClass,
    /// An annotated method of an override index class.
    Annotated,
}

/// Whether dive plans to implement an override, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StdSupport {
    /// In the scope of dive's standard library (M5 plan, WP-C3): the standard
    /// modules, `TLC`, `Randomization`, `Json`'s `ToJson` family, the basic
    /// `TLCExt` operators, and the CommunityModules operators dive implements.
    /// An operator of this kind that the evaluator does not implement yet is
    /// refused where it is evaluated.
    Core,
    /// A `CommunityModules` or other optional operator dive does not
    /// implement yet (implemented on demand, in order of corpus use). Refused.
    Extension,
    /// Reads the model checker's own state: the current trace, the
    /// counterexample, the successor being generated. Refused.
    Introspection,
    /// File, process or environment I/O. Refused.
    Io,
}

impl StdSupport {
    /// A short description, used in refusals.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Core => "a standard-module operator",
            Self::Extension => "a CommunityModules operator dive does not implement yet",
            Self::Introspection => "an operator that reads TLC's internal checker state",
            Self::Io => "an operator that performs file, process or environment I/O",
        }
    }
}

macro_rules! std_ops {
    ($($variant:ident = ($module:literal, $name:literal, $arity:literal, $kind:ident, $phase:ident, $support:ident, $impure:literal, $java:literal);)*) => {
        /// An operator that TLC overrides in Java: one `(module, definition)`
        /// pair. Each variant's documentation names the Java method or methods.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum StdOp {
            $(
                #[doc = concat!("`", $module, "!", $name, "`: `", $java, "`.")]
                $variant,
            )*
        }

        impl StdOp {
            /// Every override, in the order of the extracted table.
            pub const ALL: &'static [StdOp] = &[$(StdOp::$variant,)*];

            /// The module whose context TLC looks the definition up in.
            #[must_use]
            pub fn module(self) -> &'static str {
                match self { $(Self::$variant => $module,)* }
            }

            /// The definition's name, as SANY spells it (`+`, `\o`, `Len`).
            #[must_use]
            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)* }
            }

            /// The Java method's parameter count; -1 for an `@Evaluation`,
            /// which receives the expressions and context instead.
            #[must_use]
            pub fn java_arity(self) -> i32 {
                match self { $(Self::$variant => $arity,)* }
            }

            /// How TLC calls it.
            #[must_use]
            pub fn kind(self) -> StdKind {
                match self { $(Self::$variant => StdKind::$kind,)* }
            }

            /// Which phase installs it.
            #[must_use]
            pub fn phase(self) -> Phase {
                match self { $(Self::$variant => Phase::$phase,)* }
            }

            /// Whether dive implements it, or why not.
            #[must_use]
            pub fn support(self) -> StdSupport {
                match self { $(Self::$variant => StdSupport::$support,)* }
            }

            /// Whether calling it can have an effect beyond computing a value:
            /// output, registers, randomness, time, I/O or checker state.
            /// `TLCGet`'s effect depends on its argument (see
            /// [`crate::effects`]).
            #[must_use]
            pub fn impure(self) -> bool {
                match self { $(Self::$variant => $impure,)* }
            }

            /// The Java method or methods TLC installs for it.
            #[must_use]
            pub fn java(self) -> &'static str {
                match self { $(Self::$variant => $java,)* }
            }
        }
    };
}

std_ops! {
    AnySetANY = ("AnySet", "ANY", 0, Value, ModuleClass, Extension, false, "tlc2.module.AnySet.ANY");
    BagsEmptyBag = ("Bags", "EmptyBag", 0, Value, ModuleClass, Core, false, "tlc2.module.Bags.EmptyBag");
    BagsIsABag = ("Bags", "IsABag", 1, Value, ModuleClass, Core, false, "tlc2.module.Bags.IsABag");
    BagsBagCardinality = ("Bags", "BagCardinality", 1, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagCardinality");
    BagsBagIn = ("Bags", "BagIn", 2, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagIn");
    BagsCopiesIn = ("Bags", "CopiesIn", 2, Value, ModuleClass, Core, false, "tlc2.module.Bags.CopiesIn");
    BagsBagCup = ("Bags", "\\oplus", 2, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagCup");
    BagsBagDiff = ("Bags", "\\ominus", 2, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagDiff");
    BagsBagUnion = ("Bags", "BagUnion", 1, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagUnion");
    BagsSqSubseteq = ("Bags", "\\sqsubseteq", 2, Value, ModuleClass, Core, false, "tlc2.module.Bags.SqSubseteq");
    BagsBagOfAll = ("Bags", "BagOfAll", 2, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagOfAll");
    BagsBagToSet = ("Bags", "BagToSet", 1, Value, ModuleClass, Core, false, "tlc2.module.Bags.BagToSet");
    BagsSetToBag = ("Bags", "SetToBag", 1, Value, ModuleClass, Core, false, "tlc2.module.Bags.SetToBag");
    FiniteSetsIsFiniteSet = ("FiniteSets", "IsFiniteSet", 1, Value, ModuleClass, Core, false, "tlc2.module.FiniteSets.IsFiniteSet");
    FiniteSetsCardinality = ("FiniteSets", "Cardinality", 1, Value, ModuleClass, Core, false, "tlc2.module.FiniteSets.Cardinality");
    IntegersInt = ("Integers", "Int", 0, Value, ModuleClass, Core, false, "tlc2.module.Integers.Int");
    IntegersNat = ("Integers", "Nat", 0, Value, ModuleClass, Core, false, "tlc2.module.Integers.Nat");
    IntegersPlus = ("Integers", "+", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.Plus");
    IntegersMinus = ("Integers", "-", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.Minus");
    IntegersTimes = ("Integers", "*", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.Times");
    IntegersLt = ("Integers", "<", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.LT");
    IntegersLeq = ("Integers", "\\leq", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.LE");
    IntegersGt = ("Integers", ">", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.GT");
    IntegersGeq = ("Integers", "\\geq", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.GEQ");
    IntegersDotDot = ("Integers", "..", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.DotDot");
    IntegersNeg = ("Integers", "-.", 1, Value, ModuleClass, Core, false, "tlc2.module.Integers.Neg");
    IntegersDiv = ("Integers", "\\div", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.Divide");
    IntegersMod = ("Integers", "%", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.Mod");
    IntegersExpt = ("Integers", "^", 2, Value, ModuleClass, Core, false, "tlc2.module.Integers.Expt");
    JsonToJson = ("Json", "ToJson", 1, Value, Annotated, Core, false, "tlc2.module.Json.toJson, tlc2.overrides.Json.toJson");
    JsonToJsonArray = ("Json", "ToJsonArray", 1, Value, Annotated, Core, false, "tlc2.module.Json.toJsonArray, tlc2.overrides.Json.toJsonArray");
    JsonToJsonObject = ("Json", "ToJsonObject", 1, Value, Annotated, Core, false, "tlc2.module.Json.toJsonObject, tlc2.overrides.Json.toJsonObject");
    JsonNdJsonDeserialize = ("Json", "ndJsonDeserialize", 1, Value, Annotated, Io, true, "tlc2.module.Json.ndDeserialize, tlc2.overrides.Json.ndDeserialize");
    JsonJsonDeserialize = ("Json", "JsonDeserialize", 1, Value, Annotated, Io, true, "tlc2.module.Json.deserialize, tlc2.overrides.Json.deserialize");
    JsonNdJsonSerialize = ("Json", "ndJsonSerialize", 2, Value, Annotated, Io, true, "tlc2.module.Json.ndSerialize, tlc2.overrides.Json.ndSerialize");
    IoUtilsSerialize = ("IOUtils", "Serialize", -1, Evaluating, Annotated, Io, true, "tlc2.module.Json.textSerialize, tlc2.overrides.IOUtils.textSerialize");
    JsonJsonSerialize = ("Json", "JsonSerialize", 2, Value, Annotated, Io, true, "tlc2.module.Json.serialize, tlc2.overrides.Json.serialize");
    NaturalsNat = ("Naturals", "Nat", 0, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Nat");
    NaturalsPlus = ("Naturals", "+", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Plus");
    NaturalsMinus = ("Naturals", "-", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Minus");
    NaturalsTimes = ("Naturals", "*", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Times");
    NaturalsLt = ("Naturals", "<", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.LT");
    NaturalsLeq = ("Naturals", "\\leq", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.LE");
    NaturalsGt = ("Naturals", ">", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.GT");
    NaturalsGeq = ("Naturals", "\\geq", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.GEQ");
    NaturalsDotDot = ("Naturals", "..", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.DotDot");
    NaturalsDiv = ("Naturals", "\\div", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Divide");
    NaturalsMod = ("Naturals", "%", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Mod");
    NaturalsExpt = ("Naturals", "^", 2, Value, ModuleClass, Core, false, "tlc2.module.Naturals.Expt");
    RandomizationRandomSubset = ("Randomization", "RandomSubset", 2, Value, ModuleClass, Core, true, "tlc2.module.Randomization.RandomSubset");
    RandomizationRandomSetOfSubsets = ("Randomization", "RandomSetOfSubsets", 3, Value, ModuleClass, Core, true, "tlc2.module.Randomization.RandomSetOfSubsets");
    RandomizationRandomSubsetSet = ("Randomization", "RandomSubsetSet", 3, Value, ModuleClass, Core, true, "tlc2.module.Randomization.RandomSubsetSet");
    SequencesSeq = ("Sequences", "Seq", 1, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Seq");
    SequencesLen = ("Sequences", "Len", 1, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Len");
    SequencesHead = ("Sequences", "Head", 1, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Head");
    SequencesTail = ("Sequences", "Tail", 1, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Tail");
    SequencesCons = ("Sequences", "Cons", 2, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Cons");
    SequencesAppend = ("Sequences", "Append", 2, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Append");
    SequencesConcat = ("Sequences", "\\o", 2, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Concat");
    SequencesSelectInSeq = ("Sequences", "SelectInSeq", 2, Value, ModuleClass, Core, false, "tlc2.module.Sequences.SelectInSeq");
    SequencesSubSeq = ("Sequences", "SubSeq", 3, Value, ModuleClass, Core, false, "tlc2.module.Sequences.SubSeq");
    SequencesSelectSeq = ("Sequences", "SelectSeq", 2, Value, ModuleClass, Core, false, "tlc2.module.Sequences.SelectSeq");
    SequencesInsert = ("Sequences", "Insert", 3, Value, ModuleClass, Core, false, "tlc2.module.Sequences.Insert");
    StringsSTRING = ("Strings", "STRING", 0, Value, ModuleClass, Core, false, "tlc2.module.Strings.STRING");
    TlcPrint = ("TLC", "Print", 2, Value, ModuleClass, Core, true, "tlc2.module.TLC.Print");
    TlcPrintT = ("TLC", "PrintT", 1, Value, ModuleClass, Core, true, "tlc2.module.TLC.PrintT");
    TlcToString = ("TLC", "ToString", 1, Value, ModuleClass, Core, false, "tlc2.module.TLC.ToString");
    TlcAssert = ("TLC", "Assert", 2, Value, ModuleClass, Core, false, "tlc2.module.TLC.Assert");
    TlcJavaTime = ("TLC", "JavaTime", 0, Value, ModuleClass, Core, true, "tlc2.module.TLC.JavaTime");
    TlcMakeFcn = ("TLC", ":>", 2, Value, ModuleClass, Core, false, "tlc2.module.TLC.MakeFcn");
    TlcCombineFcn = ("TLC", "@@", 2, Value, ModuleClass, Core, false, "tlc2.module.TLC.CombineFcn");
    TlcSortSeq = ("TLC", "SortSeq", 2, Value, ModuleClass, Core, false, "tlc2.module.TLC.SortSeq");
    TlcPermutations = ("TLC", "Permutations", 1, Value, ModuleClass, Core, false, "tlc2.module.TLC.Permutations");
    TlcRandomElement = ("TLC", "RandomElement", 1, Value, ModuleClass, Core, true, "tlc2.module.TLC.RandomElement");
    TlcAny = ("TLC", "Any", 0, Value, ModuleClass, Core, false, "tlc2.module.TLC.Any");
    TlcTLCEval = ("TLC", "TLCEval", -1, Evaluating, Annotated, Core, false, "tlc2.module.TLCEval.tlcEval");
    TlcExtAssertError = ("TLCExt", "AssertError", -1, Evaluating, Annotated, Core, false, "tlc2.module.TLCExt.assertError");
    TlcExtPickSuccessor = ("TLCExt", "PickSuccessor", -1, Evaluating, Annotated, Introspection, true, "tlc2.module.TLCExt.pickSuccessor");
    TlcExtToTrace = ("TLCExt", "ToTrace", 1, Value, Annotated, Introspection, true, "tlc2.module.TLCExt.lassoOrdinal");
    TlcExtCounterExample = ("TLCExt", "CounterExample", -1, Evaluating, Annotated, Introspection, true, "tlc2.module.TLCExt.error");
    TlcExtTrace = ("TLCExt", "Trace", -1, Evaluating, Annotated, Introspection, true, "tlc2.module.TLCExt.getTrace");
    TlcExtTLCDefer = ("TLCExt", "TLCDefer", -1, Evaluating, Annotated, Introspection, true, "tlc2.module.TLCExt.tlcDefer");
    TlcExtTLCNoOp = ("TLCExt", "TLCNoOp", 1, Value, Annotated, Core, false, "tlc2.module.TLCExt.tlcNoOp");
    TlcExtTLCModelValue = ("TLCExt", "TLCModelValue", 1, Value, Annotated, Core, false, "tlc2.module.TLCExt.tlcModelValue");
    TlcExtTLCCache = ("TLCExt", "TLCCache", -1, Evaluating, Annotated, Core, false, "tlc2.module.TLCExt.tlcEval2");
    TlcExtTLCFP = ("TLCExt", "TLCFP", 1, Value, Annotated, Core, false, "tlc2.module.TLCExt.tlcFingerprint");
    TlcExtTLCEvalDefinition = ("TLCExt", "TLCEvalDefinition", -1, Evaluating, Annotated, Core, false, "tlc2.module.TLCExt.tlcDefByName");
    TlcExtTLCGetOrDefault = ("TLCExt", "TLCGetOrDefault", 2, Value, Annotated, Core, false, "tlc2.module.TLCExt.tlcGetOrDefault");
    TlcTLCGet = ("TLC", "TLCGet", -1, Evaluating, Annotated, Core, true, "tlc2.module.TLCGetSet.TLCGetEval");
    TlcTLCSet = ("TLC", "TLCSet", 2, Value, Annotated, Core, true, "tlc2.module.TLCGetSet.TLCSet");
    TransitiveClosureWarshall = ("TransitiveClosure", "Warshall", 1, Value, ModuleClass, Extension, false, "tlc2.module.TransitiveClosure.Warshall");
    JsonTraceTLCState = ("_JsonTrace", "_TLCState", -1, Evaluating, Annotated, Introspection, true, "tlc2.module._JsonTrace.tlcState");
    PossibleCounts = ("_Possible", "_Counts", 0, Value, Annotated, Introspection, true, "tlc2.module._Possible.counts");
    TlcTraceTLCTraceSerialize = ("_TLCTrace", "_TLCTraceSerialize", 2, Value, Annotated, Io, true, "tlc2.module._TLCTrace.ioSerialize");
    TlcTraceTLCTraceDeserialize = ("_TLCTrace", "_TLCTraceDeserialize", 1, Value, Annotated, Io, true, "tlc2.module._TLCTrace.ioDeserialize");
    TlcTraceTLCState = ("_TLCTrace", "_TLCState", -1, Evaluating, Annotated, Introspection, true, "tlc2.module._TLCTrace.tlcState");
    BagsExtFoldBag = ("BagsExt", "FoldBag", 3, Value, Annotated, Core, false, "tlc2.overrides.BagsExt.foldBag");
    BitwiseAnd = ("Bitwise", "And", 4, Value, Annotated, Core, false, "tlc2.overrides.Bitwise.And");
    BitwiseOr = ("Bitwise", "Or", 4, Value, Annotated, Core, false, "tlc2.overrides.Bitwise.Or");
    BitwiseXor = ("Bitwise", "Xor", 4, Value, Annotated, Core, false, "tlc2.overrides.Bitwise.Xor");
    BitwiseNot = ("Bitwise", "Not", 1, Value, Annotated, Core, false, "tlc2.overrides.Bitwise.NotR");
    BitwiseShiftR = ("Bitwise", "shiftR", 2, Value, Annotated, Core, false, "tlc2.overrides.Bitwise.shiftR");
    CsvCSVWriteRecord = ("CSV", "CSVWriteRecord", 4, Value, Annotated, Io, true, "tlc2.overrides.CSV.writeRecord");
    CsvCSVWrite = ("CSV", "CSVWrite", 3, Value, Annotated, Io, true, "tlc2.overrides.CSV.write");
    CsvCSVRead = ("CSV", "CSVRead", 3, Value, Annotated, Io, true, "tlc2.overrides.CSV.read");
    CsvCSVRecords = ("CSV", "CSVRecords", 1, Value, Annotated, Io, true, "tlc2.overrides.CSV.records");
    CombinatoricsFactorial = ("Combinatorics", "factorial", 1, Value, Annotated, Extension, false, "tlc2.overrides.Combinatorics.fact");
    CombinatoricsChoose = ("Combinatorics", "choose", 2, Value, Annotated, Extension, false, "tlc2.overrides.Combinatorics.chse");
    DyadicRationalsReduce = ("DyadicRationals", "Reduce", 1, Value, Annotated, Core, false, "tlc2.overrides.DyadicRationals.reduce");
    FiniteSetsExtQuantify = ("FiniteSetsExt", "Quantify", 2, Value, Annotated, Core, false, "tlc2.overrides.FiniteSetsExt.quantify");
    FiniteSetsExtKSubset = ("FiniteSetsExt", "kSubset", 2, Value, Annotated, Extension, false, "tlc2.overrides.FiniteSetsExt.kSubset");
    FiniteSetsExtFoldSet = ("FiniteSetsExt", "FoldSet", 3, Value, Annotated, Core, false, "tlc2.overrides.FiniteSetsExt.foldSet");
    FunctionsIsInjective = ("Functions", "IsInjective", 1, Value, Annotated, Core, false, "tlc2.overrides.Functions.IsInjective");
    FunctionsAntiFunction = ("Functions", "AntiFunction", 1, Value, Annotated, Core, false, "tlc2.overrides.Functions.antiFunction");
    FunctionsFoldFunction = ("Functions", "FoldFunction", 3, Value, Annotated, Core, false, "tlc2.overrides.Functions.foldFunction");
    FunctionsFoldFunctionOnSet = ("Functions", "FoldFunctionOnSet", 4, Value, Annotated, Core, false, "tlc2.overrides.Functions.foldFunctionOnSet");
    GraphVizDotDiGraph = ("GraphViz", "DotDiGraph", 3, Value, Annotated, Extension, false, "tlc2.overrides.GraphViz.dotDiGraph");
    GraphsSimplePath = ("Graphs", "SimplePath", 1, Value, Annotated, Core, false, "tlc2.overrides.Graphs.simplePath");
    GraphsAreConnectedIn = ("Graphs", "AreConnectedIn", 3, Value, Annotated, Core, false, "tlc2.overrides.Graphs.areConnectedIn");
    GraphsIsStronglyConnected = ("Graphs", "IsStronglyConnected", 1, Value, Annotated, Core, false, "tlc2.overrides.Graphs.isStronglyConnected");
    IoUtilsIODeserialize = ("IOUtils", "IODeserialize", 2, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioDeserialize");
    IoUtilsIOSerialize = ("IOUtils", "IOSerialize", 3, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioSerialize");
    IoUtilsDeserialize = ("IOUtils", "Deserialize", -1, Evaluating, Annotated, Io, true, "tlc2.overrides.IOUtils.textDeserialize");
    IoUtilsAtoi = ("IOUtils", "atoi", 1, Value, Annotated, Core, false, "tlc2.overrides.IOUtils.atoi");
    IoUtilsIOEnv = ("IOUtils", "IOEnv", 0, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioEnv");
    IoUtilsIOExec = ("IOUtils", "IOExec", 1, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioExec");
    IoUtilsIOEnvExec = ("IOUtils", "IOEnvExec", 2, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioEnvExec");
    IoUtilsIOExecTemplate = ("IOUtils", "IOExecTemplate", 2, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioExecTemplate");
    IoUtilsIOEnvExecTemplate = ("IOUtils", "IOEnvExecTemplate", 3, Value, Annotated, Io, true, "tlc2.overrides.IOUtils.ioEnvExecTemplate");
    SvgSVGElemToString = ("SVG", "SVGElemToString", 1, Value, Annotated, Extension, false, "tlc2.overrides.SVG.SVGElemToString");
    SvgNodeOfRingNetwork = ("SVG", "NodeOfRingNetwork", 5, Value, Annotated, Extension, false, "tlc2.overrides.SVG.ringNetwork");
    SvgNodesOfDirectedMultiGraph = ("SVG", "NodesOfDirectedMultiGraph", 3, Value, Annotated, Extension, false, "tlc2.overrides.SVG.directedMultiGraph");
    SvgPointOnLine = ("SVG", "PointOnLine", 3, Value, Annotated, Core, false, "tlc2.overrides.SVG.pointOnLine");
    SequencesExtSetToSeq = ("SequencesExt", "SetToSeq", 1, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.SetToSeq");
    SequencesExtSetToSeqs = ("SequencesExt", "SetToSeqs", 1, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.SetToSeqs");
    SequencesExtContains = ("SequencesExt", "Contains", 2, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.Contains");
    SequencesExtLongestCommonPrefix = ("SequencesExt", "LongestCommonPrefix", 1, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.longestCommonPrefix");
    SequencesExtFoldSeq = ("SequencesExt", "FoldSeq", 3, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.foldSeq");
    SequencesExtFoldLeft = ("SequencesExt", "FoldLeft", 3, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.foldLeft");
    SequencesExtFoldRight = ("SequencesExt", "FoldRight", 3, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.foldRight");
    SequencesExtFoldLeftDomain = ("SequencesExt", "FoldLeftDomain", 3, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.foldLeftDomain");
    SequencesExtFoldRightDomain = ("SequencesExt", "FoldRightDomain", 3, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.foldRightDomain");
    SequencesExtReplaceFirstSubSeq = ("SequencesExt", "ReplaceFirstSubSeq", -1, Evaluating, Annotated, Core, false, "tlc2.overrides.SequencesExt.replaceFirstSubSeq");
    SequencesExtReplaceAllSubSeqs = ("SequencesExt", "ReplaceAllSubSeqs", -1, Evaluating, Annotated, Core, false, "tlc2.overrides.SequencesExt.replaceAllSubSeq");
    SequencesExtIsPrefix = ("SequencesExt", "IsPrefix", 2, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.isPrefix");
    SequencesExtSelectInSeq = ("SequencesExt", "SelectInSeq", 2, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.selectInSeq");
    SequencesExtSelectInSubSeq = ("SequencesExt", "SelectInSubSeq", 4, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.SelectInSubSeq");
    SequencesExtSelectLastInSeq = ("SequencesExt", "SelectLastInSeq", 2, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.selectLastInSeq");
    SequencesExtSelectLastInSubSeq = ("SequencesExt", "SelectLastInSubSeq", 4, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.SelectLastInSubSeq");
    SequencesExtRemoveFirst = ("SequencesExt", "RemoveFirst", 2, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.removeFirst");
    SequencesExtRemoveFirstMatch = ("SequencesExt", "RemoveFirstMatch", 2, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.removeFirstMatch");
    SequencesExtSuffixes = ("SequencesExt", "Suffixes", 1, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.Suffixes");
    SequencesExtAllSubSeqs = ("SequencesExt", "AllSubSeqs", 1, Value, Annotated, Core, false, "tlc2.overrides.SequencesExt.AllSubSeqs");
    StatisticsChiSquare = ("Statistics", "ChiSquare", 3, Value, Annotated, Extension, false, "tlc2.overrides.Statistics.chiSquare");
    UndirectedGraphsSimplePath = ("UndirectedGraphs", "SimplePath", 1, Value, Annotated, Core, false, "tlc2.overrides.UndirectedGraphs.simplePath");
    UndirectedGraphsAreConnectedIn = ("UndirectedGraphs", "AreConnectedIn", 3, Value, Annotated, Core, false, "tlc2.overrides.UndirectedGraphs.areConnectedIn");
    UndirectedGraphsConnectedComponents = ("UndirectedGraphs", "ConnectedComponents", 1, Value, Annotated, Core, false, "tlc2.overrides.UndirectedGraphs.connectedComponents");
    VectorClocksCausalOrder = ("VectorClocks", "CausalOrder", 4, Value, Annotated, Extension, false, "tlc2.overrides.VectorClocks.causalOrder");
}

impl StdOp {
    /// Whether the native implementation can intern string values or record
    /// field names during evaluation. TLC encounter ordering therefore needs
    /// the complete evaluation history, not only a supplied state or trace.
    /// Tuple-only calls are conservatively included until their argument types
    /// prove that no names can be created (issues #122 and #124).
    #[must_use]
    pub fn creates_runtime_names(self) -> bool {
        matches!(
            self,
            Self::TlcToString
                | Self::SequencesTail
                | Self::SequencesConcat
                | Self::SequencesSubSeq
                | Self::SequencesExtLongestCommonPrefix
                | Self::SequencesExtReplaceFirstSubSeq
                | Self::SequencesExtReplaceAllSubSeqs
                | Self::DyadicRationalsReduce
        )
    }

    /// The override for this module and definition name, if TLC has one.
    #[must_use]
    pub fn lookup(module: &str, name: &str) -> Option<StdOp> {
        StdOp::ALL
            .iter()
            .copied()
            .find(|op| op.module() == module && op.name() == name)
    }

    /// `Module!name`, the form refusals use.
    #[must_use]
    pub fn qualified_name(self) -> String {
        format!("{}!{}", self.module(), self.name())
    }
}

/// Whether TLC has a module class for the module `name` (phase 1).
#[must_use]
pub fn is_module_class(name: &str) -> bool {
    StdOp::ALL
        .iter()
        .any(|op| op.phase() == Phase::ModuleClass && op.module() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_and_name_identify_an_override() {
        let mut seen = std::collections::BTreeSet::new();
        for op in StdOp::ALL {
            assert!(seen.insert((op.module(), op.name())), "{op:?}");
            assert_eq!(StdOp::lookup(op.module(), op.name()), Some(*op));
        }
    }

    #[test]
    fn known_entries() {
        assert_eq!(
            StdOp::lookup("Sequences", "\\o"),
            Some(StdOp::SequencesConcat)
        );
        assert_eq!(StdOp::lookup("TLC", ":>"), Some(StdOp::TlcMakeFcn));
        assert_eq!(StdOp::TlcTLCGet.kind(), StdKind::Evaluating);
        assert_eq!(StdOp::TlcPrint.kind(), StdKind::Value);
        assert!(StdOp::TlcPrint.impure());
        assert!(!StdOp::SequencesLen.impure());
        assert_eq!(StdOp::IntegersDiv.phase(), Phase::ModuleClass);
        assert_eq!(StdOp::JsonToJson.phase(), Phase::Annotated);
        assert!(
            StdOp::JsonToJson
                .java()
                .contains("tlc2.overrides.Json.toJson")
        );
        assert_eq!(StdOp::IoUtilsIOExec.support(), StdSupport::Io);
        assert!(is_module_class("Naturals"));
        assert!(!is_module_class("SequencesExt"));
    }
}
