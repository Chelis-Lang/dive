//! Semantic diagnostics and their stable codes.
//!
//! Codes have the form `DIVE-Ennnn` (errors) or `DIVE-Wnnnn` (warnings) and
//! never change meaning once published. Add new codes at the end of their
//! group; never reuse one.
//!
//! Each code names the SANY error code (`tla2sany/semantic/ErrorCode.java`
//! in the pinned build) of the check it ports, where SANY has one. SANY
//! reuses a few codes for many unrelated checks (4003 `INTERNAL_ERROR`,
//! 4004 `SUSPECTED_UNREACHABLE_CHECK`, 4005 `UNSUPPORTED_LANGUAGE_FEATURE`);
//! dive gives those situations their own codes and keeps the SANY number as
//! a cross-reference.

use dive_syntax::TextRange;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The specification is rejected, as SANY would reject it.
    Error,
    /// Accepted, but worth pointing out.
    Warning,
}

macro_rules! codes {
    ($($(#[doc = $doc:literal])* $name:ident = $text:literal, $sev:ident, $sany:expr;)*) => {
        /// A stable semantic diagnostic code.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Code {
            $($(#[doc = $doc])* $name,)*
        }

        impl Code {
            /// Every code, in table order.
            pub const ALL: &'static [Code] = &[$(Code::$name,)*];

            /// The stable string form, e.g. `DIVE-E0100`.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$name => $text,)*
                }
            }

            /// The severity diagnostics with this code have.
            #[must_use]
            pub fn severity(self) -> Severity {
                match self {
                    $(Self::$name => Severity::$sev,)*
                }
            }

            /// SANY's error code for the check this ports (`ErrorCode.java`),
            /// if SANY reports one. Parse-phase failures SANY reports without
            /// a code have `None`.
            #[must_use]
            pub fn sany_code(self) -> Option<u16> {
                match self {
                    $(Self::$name => $sany,)*
                }
            }
        }
    };
}

codes! {
    // ---- Module resolution -------------------------------------------------
    // SANY reports all of these in its parse phase except `DuplicateModule`
    // (4223), which semantic analysis (`SymbolTable.addModule`) reports.
    /// No file, library module or standard module provides an `EXTENDS`ed or
    /// `INSTANCE`d module.
    ModuleNotFound = "DIVE-E0001", Error, Some(4220);
    /// Modules extend or instantiate each other in a cycle.
    CircularDependency = "DIVE-E0002", Error, Some(4222);
    /// A dependency's file holds a module with a different name.
    DependencyNameMismatch = "DIVE-E0003", Error, Some(4221);
    /// A dependency does not parse; its syntax diagnostics follow.
    DependencyParseError = "DIVE-E0004", Error, Some(4003);
    /// Two modules with the same name are visible in one scope. Unlike the
    /// rest of this group, SANY reports this in its semantic phase
    /// ("Multiply-defined module", under `Semantic errors:`, exit 4), not
    /// its parse phase, and dive reports it during generation
    /// (`generate::add_module`). Evidence: the `tlaplus-tests` SANY goldens of
    /// `E4223_Test.tla` and `NestedModuleLetInstance.tla`.
    DuplicateModule = "DIVE-E0005", Error, Some(4223);
    /// A module named in `EXTENDS` or `INSTANCE` was not loaded (SANY aborts).
    ModuleUnavailable = "DIVE-E0006", Error, Some(4003);
    /// A file could not be read.
    UnreadableFile = "DIVE-E0007", Error, None;
    /// The root file holds a module with a different name.
    RootNameMismatch = "DIVE-E0008", Error, Some(4221);
    /// SANY's check of a PlusCal algorithm against its translation fails
    /// (an unterminated string in the algorithm, or `END TRANSLATION`
    /// without `BEGIN TRANSLATION`).
    PlusCalCheck = "DIVE-E0009", Error, Some(4003);

    // ---- Names and definitions --------------------------------------------
    /// An unknown name.
    UndefinedSymbol = "DIVE-E0100", Error, Some(4200);
    /// An operator symbol with no definition (for example `+` without
    /// `EXTENDS Naturals`).
    UndefinedOperatorSymbol = "DIVE-E0101", Error, Some(4004);
    /// A name defined or declared twice, or a bound name that shadows another.
    Redefinition = "DIVE-E0102", Error, Some(4201);
    /// A built-in operator is redefined.
    BuiltInRedefinition = "DIVE-E0103", Error, Some(4202);
    /// Two `EXTENDS`ed modules give one name different kinds of meaning.
    ExtendsConflict = "DIVE-E0104", Error, Some(4224);
    /// A module name used where an operator is expected.
    ModuleNameAsOperator = "DIVE-E0105", Error, Some(4005);
    /// `I!Op` stops at the instance name `I`.
    IncompleteOperatorName = "DIVE-E0106", Error, Some(4203);
    /// An operator applied to the wrong number of arguments.
    WrongArgumentCount = "DIVE-E0107", Error, Some(4204);
    /// `@` outside an `EXCEPT` clause, or `@` declared as a name.
    AtOutsideExcept = "DIVE-E0108", Error, Some(4261);
    /// A record constructor or set of records names a field twice.
    DuplicateRecordField = "DIVE-E0109", Error, Some(4262);
    /// A function applied to a number of arguments its definition rules out.
    FunctionArgumentCount = "DIVE-E0110", Error, Some(4260);
    /// `LAMBDA` used as an expression.
    LambdaAsExpression = "DIVE-E0111", Error, Some(4275);
    /// An expression where an operator argument is required.
    OperatorArgumentExpected = "DIVE-E0112", Error, Some(4270);
    /// An operator argument of the wrong arity.
    OperatorArgumentArity = "DIVE-E0113", Error, Some(4271);
    /// A `LAMBDA` argument of the wrong arity.
    LambdaArgumentArity = "DIVE-E0114", Error, Some(4274);
    /// An operator argument where an expression is required, or another
    /// argument SANY's arity checks reject.
    InvalidArgument = "DIVE-E0115", Error, Some(4004);
    /// A construct that is not an expression used as one (for example a bare
    /// operator symbol, or `[a, b]` with no function).
    NotAnExpression = "DIVE-E0116", Error, Some(4003);
    /// A recursive operator primes one of its parameters.
    RecursivePrimesParameter = "DIVE-E0120", Error, Some(4290);
    /// An operator declared `RECURSIVE` but never defined.
    RecursiveNotDefined = "DIVE-E0121", Error, Some(4291);
    /// A definition's arity differs from its `RECURSIVE` declaration.
    RecursiveArityMismatch = "DIVE-E0122", Error, Some(4292);
    /// A recursive operator defined at a different `LET` level than declared.
    RecursiveWrongLevel = "DIVE-E0123", Error, Some(4293);
    /// A declaration, module, theorem or similar between a `RECURSIVE`
    /// declaration and its definition.
    RecursiveSectionIllegal = "DIVE-E0124", Error, Some(4294);
    /// More recursive definitions than declarations in one section, after an
    /// earlier error (SANY aborts with an internal exception here).
    RecursiveBookkeeping = "DIVE-E0125", Error, None;
    /// `LET` nested more than 100 deep (SANY's limit).
    LetTooDeep = "DIVE-E0126", Error, Some(4003);

    // ---- Instances and substitutions --------------------------------------
    /// A `CONSTANT` or `VARIABLE` of an instantiated module has no
    /// substitution, explicit or implicit.
    SubstitutionMissing = "DIVE-E0200", Error, Some(4240);
    /// Two `WITH` substitutions for one symbol.
    SubstitutionDuplicate = "DIVE-E0201", Error, Some(4241);
    /// A `WITH` target that is not a `CONSTANT` or `VARIABLE` of the module.
    SubstitutionIllegalTarget = "DIVE-E0202", Error, Some(4242);
    /// A substitution of the wrong arity.
    SubstitutionArity = "DIVE-E0203", Error, Some(4243);
    /// An expression substituted where an operator is required.
    SubstitutionOperatorExpected = "DIVE-E0204", Error, Some(4004);

    // ---- Subexpressions, labels and selectors -----------------------------
    /// A subexpression selector (`!1`, `!:`, `!<<`, `!@`, `!(..)`) that
    /// selects nothing valid.
    BadSubexpression = "DIVE-E0300", Error, Some(4005);
    /// A label used with the wrong number of arguments.
    LabelArgumentCount = "DIVE-E0302", Error, Some(4337);
    /// An unknown label in a selector.
    LabelNotFound = "DIVE-E0303", Error, Some(4004);
    /// An `ASSUME`/`PROVE` used where an expression is required.
    AssumeProveAsExpression = "DIVE-E0304", Error, Some(4355);
    /// A `QED`, `CASE`, `HAVE`, `TAKE`, `PICK`, `WITNESS` or `SUFFICES` step
    /// used as an expression.
    ProofStepAsExpression = "DIVE-E0305", Error, Some(4351);
    /// A `DEF` item that does not name a definition.
    DefNotDefinition = "DIVE-E0306", Error, Some(4200);
    /// A step number of the wrong kind used as a fact or expression.
    StepMisuse = "DIVE-E0307", Error, Some(4004);
    /// A `USE`/`HIDE`/`BY` item that is not a name or expression SANY can
    /// interpret.
    BadUseItem = "DIVE-E0308", Error, Some(4003);
    /// A labelled `ASSUME`/`PROVE` used as an expression.
    LabeledAssumeProveAsExpression = "DIVE-E0309", Error, Some(4004);

    /// A label repeats a parameter.
    LabelParameterRepeated = "DIVE-E0400", Error, Some(4330);
    /// A label omits a bound identifier in scope.
    LabelParameterMissing = "DIVE-E0401", Error, Some(4331);
    /// A label parameter that is not a bound identifier in scope.
    LabelParameterExtra = "DIVE-E0402", Error, Some(4332);
    /// A label outside any definition or proof step.
    LabelOutsideDefinition = "DIVE-E0403", Error, Some(4333);
    /// A label inside a nested `ASSUME`/`PROVE` within the scope of a `NEW`.
    LabelInNestedAssume = "DIVE-E0404", Error, Some(4334);
    /// A label inside an `EXCEPT` clause.
    LabelInExcept = "DIVE-E0405", Error, Some(4335);
    /// Two labels with one name in one definition.
    LabelDuplicate = "DIVE-E0406", Error, Some(4336);

    // ---- Proofs -------------------------------------------------------------
    /// `<*>` or `<+>` used as the number of a named step.
    NamedImplicitStep = "DIVE-E0500", Error, Some(4350);
    /// `[]ASSUME` closed by `PROVE`, or `ASSUME` by `[]PROVE`.
    BoxAssumeMismatch = "DIVE-E0501", Error, Some(4005);
    /// `[]ASSUME` inside the scope of an ordinary `ASSUME`.
    BoxAssumeInAssume = "DIVE-E0502", Error, Some(4005);
    /// `MODULE M` in `USE`/`HIDE`/`BY` for a module that is neither extended
    /// nor instantiated.
    ModuleNotImported = "DIVE-E0503", Error, Some(4005);
    /// `USE`, `HIDE` or `BY` with nothing to use.
    EmptyUse = "DIVE-E0504", Error, Some(4003);
    /// A `HIDE` fact that is not a theorem, assumption or step name.
    HideFactInvalid = "DIVE-E0505", Error, Some(4357);

    // ---- Levels -------------------------------------------------------------
    /// An argument's level exceeds what the operator allows (for example a
    /// primed expression passed to an operator that primes its parameter).
    ArgumentLevel = "DIVE-E0600", Error, Some(4205);
    /// An `ASSUME` that is not constant level.
    AssumptionNotConstant = "DIVE-E0601", Error, Some(4206);
    /// A substitution's level exceeds the level of the symbol it replaces.
    SubstitutionLevel = "DIVE-E0602", Error, Some(4245);
    /// An operator substituted for a parameter whose arguments are used at a
    /// higher level than the operator allows.
    SubstitutionArgumentLevel = "DIVE-E0603", Error, Some(4246);
    /// Substitutions whose levels violate an operator parameter's constraints.
    SubstitutionCoparameterLevel = "DIVE-E0604", Error, Some(4247);
    /// A non-Leibniz operator (one using `ENABLED` or priming on a
    /// parameter) substituted for a parameter.
    NonLeibnizSubstitution = "DIVE-E0605", Error, Some(4244);
    /// An operator argument whose parameters accept too low a level.
    OperatorArgumentLevel = "DIVE-E0606", Error, Some(4272);
    /// Arguments whose levels violate an operator argument's constraints.
    OperatorCoparameterLevel = "DIVE-E0607", Error, Some(4273);
    /// `[]A` for an action `A` not of the form `[B]_v`.
    AlwaysAction = "DIVE-E0610", Error, Some(4310);
    /// `<>A` for an action `A` not of the form `<<B>>_v`.
    EventuallyAction = "DIVE-E0611", Error, Some(4311);
    /// An action argument of `~>` or `-+->`.
    TemporalBinaryAction = "DIVE-E0612", Error, Some(4312);
    /// `/\`, `\/`, `=>`, `<=>` or a junction list mixing actions and
    /// temporal formulas.
    MixedActionTemporal = "DIVE-E0613", Error, Some(4313);
    /// A temporal quantified formula with an action-level bound.
    TemporalQuantActionBound = "DIVE-E0614", Error, Some(4314);
    /// A temporal-level bound of a bounded quantifier or similar construct.
    TemporalBound = "DIVE-E0615", Error, Some(4315);
    /// A temporal `PICK` with a non-constant bound.
    TemporalPickBound = "DIVE-E0620", Error, Some(4354);
    /// A non-constant `TAKE`, `WITNESS` or `HAVE` in a temporal proof.
    TemporalTakeWitnessHave = "DIVE-E0621", Error, Some(4352);
    /// A non-constant `CASE` step in a temporal proof.
    TemporalCase = "DIVE-E0622", Error, Some(4353);
    /// A `NEW` symbol bounded by a temporal formula.
    NewTemporalBound = "DIVE-E0623", Error, Some(4356);

    // ---- Internal -----------------------------------------------------------
    /// One of SANY's internal-error checks fires on this input (SANY reports
    /// it as an error or aborts); the message gives the details.
    SanyInternal = "DIVE-E0900", Error, Some(4003);

    // ---- Warnings -----------------------------------------------------------
    /// Two `EXTENDS`ed modules define one name differently (SANY warns).
    ExtendsAmbiguity = "DIVE-W0800", Warning, Some(4800);
    /// A name declared or defined twice in a way SANY only warns about.
    MultipleDefinitions = "DIVE-W0801", Warning, Some(4801);
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Identifies one source file of an analysis; see [`crate::Analysis::file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub(crate) u32);

impl FileId {
    /// The index of the file in [`crate::Analysis::files`].
    #[must_use]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A source location: a file and a byte range in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// The file.
    pub file: FileId,
    /// The byte range.
    pub range: TextRange,
}

/// A secondary location attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    /// Where.
    pub span: Span,
    /// What is there.
    pub message: String,
}

/// One semantic diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable code.
    pub code: Code,
    /// Severity (always [`Code::severity`]).
    pub severity: Severity,
    /// What is wrong, naming the construct.
    pub message: String,
    /// The primary location. `None` only for problems with no source
    /// location, such as a missing root file.
    pub span: Option<Span>,
    /// A likely fix, when one can be suggested.
    pub help: Option<String>,
    /// Other relevant locations (the earlier definition, the module that
    /// imports a missing one, ...).
    pub related: Vec<Related>,
    /// SANY's error code for this check, where SANY reports one. Usually
    /// [`Code::sany_code`]; a few dive codes cover checks that SANY codes
    /// differently at different sites, and this records the site's code.
    pub sany: Option<u16>,
}

impl Diagnostic {
    pub(crate) fn new(code: Code, span: Option<Span>, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: code.severity(),
            message: message.into(),
            span,
            help: None,
            related: Vec::new(),
            sany: code.sany_code(),
        }
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub(crate) fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.related.push(Related {
            span,
            message: message.into(),
        });
        self
    }

    /// Whether this diagnostic rejects the specification.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for c in Code::ALL {
            assert!(seen.insert(c.as_str()), "{c}");
            let prefix = match c.severity() {
                Severity::Error => "DIVE-E",
                Severity::Warning => "DIVE-W",
            };
            assert!(c.as_str().starts_with(prefix), "{c}");
            assert_eq!(c.as_str().len(), 10, "{c}");
        }
    }

    #[test]
    fn sany_codes_are_in_range() {
        for c in Code::ALL {
            if let Some(s) = c.sany_code() {
                assert!((4000..5000).contains(&s), "{c}");
            }
        }
    }
}
