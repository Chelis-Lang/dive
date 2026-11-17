//! The resolved semantic model: SANY's semantic graph as an arena.
//!
//! Every semantic object (module, declaration, definition, expression,
//! proof step) is a [`Node`] in one arena, addressed by a [`NodeId`].
//! References are resolved: an operator application points at the
//! declaration or definition of its operator, a bound identifier at its
//! binder, an instantiated definition at a substitution wrapper around the
//! original body, and so on. Levels computed by level checking are stored on
//! the nodes.
//!
//! The shapes follow SANY's `tla2sany.semantic` classes one to one (see the
//! variant docs), so a consumer that knows SANY's graph knows this one, and
//! the port can be compared class by class. `dive-ir` lowers from it.
//!
//! Built-in operators are [`NodeKind::OpDef`] nodes of kind [`DefKind::BuiltIn`] whose
//! names are SANY's (`\in`, `$FcnApply`, `$SetEnumerate`, ...); the full list
//! is [`crate::builtins::BUILT_INS`].

use crate::diagnostics::Span;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// An index into [`Model::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    /// The node's index in [`Model::nodes`].
    #[must_use]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An interned name; see [`Model::name`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(pub(crate) u32);

/// An index into the model's symbol contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CtxId(pub(crate) u32);

/// An index into the model's label tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LabelsId(pub(crate) u32);

/// A TLA+ level. The order is SANY's: constant < state < action < temporal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Constant level (SANY level 0).
    Constant = 0,
    /// State level: depends on unprimed variables (1).
    State = 1,
    /// Action level: depends on primed variables (2).
    Action = 2,
    /// Temporal level (3).
    Temporal = 3,
}

impl Level {
    /// The level from SANY's number, saturating at temporal.
    #[must_use]
    pub fn from_u8(n: u8) -> Self {
        match n {
            0 => Self::Constant,
            1 => Self::State,
            2 => Self::Action,
            _ => Self::Temporal,
        }
    }
}

/// `(operator parameter, argument position, parameter)`: SANY's
/// `ArgLevelParam`.
pub type ArgLevelParam = (NodeId, u32, NodeId);

/// The level-checking data SANY keeps on every `LevelNode`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelData {
    /// The level (0..=3).
    pub level: u8,
    /// Constant and variable parameters the level depends on.
    pub level_params: BTreeSet<NodeId>,
    /// All parameters that appear.
    pub all_params: BTreeSet<NodeId>,
    /// Parameters appearing in non-Leibniz positions.
    pub non_leibniz_params: BTreeSet<NodeId>,
    /// Upper bounds on the levels of parameters.
    pub level_constraints: BTreeMap<NodeId, u8>,
    /// Lower bounds on the levels of operator parameters' arguments.
    pub arg_level_constraints: BTreeMap<(NodeId, u32), u8>,
    /// SANY's `argLevelParams`.
    pub arg_level_params: BTreeSet<ArgLevelParam>,
    /// The iteration that last level-checked this node (0: never).
    pub checked: u32,
    /// Whether level checking found it correct.
    pub correct: bool,
}

/// One node of the semantic graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Where the node comes from. Built-in operators have no span.
    pub span: Option<Span>,
    /// What the node is.
    pub kind: NodeKind,
    /// Level data, filled in by level checking.
    pub level: LevelData,
}

/// SANY's node kinds (`ASTConstants`), used where its checks compare kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs, reason = "the names are SANY's ASTConstants")]
pub enum Kind {
    Module,
    ConstantDecl,
    VariableDecl,
    BoundSymbol,
    UserDefinedOp,
    ModuleInstance,
    BuiltIn,
    OpArg,
    OpAppl,
    LetIn,
    FormalParam,
    Theorem,
    SubstIn,
    AssumeProve,
    Numeral,
    Decimal,
    String,
    AtNode,
    Assume,
    Instance,
    NewSymb,
    ThmOrAssumpDef,
    NewConstant,
    NewVariable,
    NewState,
    NewAction,
    NewTemporal,
    Label,
    APSubstIn,
    Use,
    Hide,
    LeafProof,
    NonLeafProof,
    DefStep,
    NumberedProofStep,
    /// A placeholder left where an error was reported (SANY's `nullOAN`).
    Error,
}

/// What a node is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// A module (`ModuleNode`).
    Module(Box<ModuleData>),
    /// A `CONSTANT`, `VARIABLE` or `NEW` declaration (`OpDeclNode`).
    OpDecl(OpDeclData),
    /// A formal parameter or bound identifier (`FormalParamNode`).
    FormalParam(FormalParamData),
    /// An operator definition, built-in operator, module instance name or
    /// numbered proof step (`OpDefNode`).
    OpDef(Box<OpDefData>),
    /// A named theorem, assumption or proof step (`ThmOrAssumpDefNode`).
    ThmOrAssumpDef(Box<ThmDefData>),
    /// An operator application (`OpApplNode`), including every built-in
    /// construct (`$SetEnumerate`, `$BoundedForall`, ...).
    OpAppl(Box<OpApplData>),
    /// An operator passed as an argument (`OpArgNode`).
    OpArg(OpArgData),
    /// `LET ... IN e` (`LetInNode`).
    LetIn(Box<LetInData>),
    /// A natural number literal (`NumeralNode`), as written.
    Numeral(String),
    /// A decimal literal `a.b` (`DecimalNode`).
    Decimal(String, String),
    /// A string literal (`StringNode`), unescaped as SANY does, or a record
    /// field name.
    Str(String),
    /// `@` inside an `EXCEPT` clause (`AtNode`).
    At {
        /// The `$Except` application.
        except: NodeId,
        /// The `$Pair` of the clause the `@` is in.
        component: NodeId,
    },
    /// `lab :: e` (`LabelNode`).
    Label(Box<LabelData>),
    /// The body of an instantiated definition under its substitutions
    /// (`SubstInNode`).
    SubstIn(Box<SubstInData>),
    /// The same for a theorem or assumption body (`APSubstInNode`).
    ApSubstIn(Box<SubstInData>),
    /// `ASSUME ... PROVE ...` (`AssumeProveNode`).
    AssumeProve(Box<AssumeProveData>),
    /// `NEW x \in S` in an `ASSUME` (`NewSymbNode`).
    NewSymb {
        /// The declared symbol (an [`NodeKind::OpDecl`]).
        decl: NodeId,
        /// The bounding set, if any.
        set: Option<NodeId>,
    },
    /// A theorem or proof step (`TheoremNode`).
    Theorem(Box<TheoremData>),
    /// An `ASSUME` (`AssumeNode`).
    Assume(AssumeData),
    /// An `INSTANCE` (`InstanceNode`).
    Instance(Box<InstanceData>),
    /// `USE` or `HIDE` (`UseOrHideNode`).
    UseOrHide(Box<UseOrHideData>),
    /// A `DEFINE` step (`DefStepNode`).
    DefStep {
        /// The step's number, if numbered.
        step: Option<Name>,
        /// The definitions.
        defs: Vec<NodeId>,
    },
    /// A structured proof (`NonLeafProofNode`).
    NonLeafProof {
        /// The steps, the last being `QED`.
        steps: Vec<NodeId>,
        /// Instances made by `Id == INSTANCE` steps.
        insts: Vec<NodeId>,
        /// The proof's symbols.
        ctx: CtxId,
    },
    /// `BY`, `OBVIOUS` or `OMITTED` (`LeafProofNode`).
    LeafProof {
        /// The facts.
        facts: Vec<NodeId>,
        /// The `DEF` items.
        defs: Vec<NodeId>,
        /// `OMITTED`.
        omitted: bool,
        /// `BY ONLY`.
        only: bool,
    },
    /// A placeholder where an error was reported.
    Error,
}

/// A module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleData {
    /// The name.
    pub name: Name,
    /// The module's symbols (its top-level context).
    pub ctx: CtxId,
    /// Directly `EXTENDS`ed modules.
    pub extendees: Vec<NodeId>,
    /// Definitions in source order, as SANY's `getDefinitions`.
    pub definitions: Vec<NodeId>,
    /// `INSTANCE` nodes.
    pub instances: Vec<NodeId>,
    /// `ASSUME` nodes, including extended ones.
    pub assumptions: Vec<NodeId>,
    /// `THEOREM` nodes, including extended ones.
    pub theorems: Vec<NodeId>,
    /// Top-level assumptions, theorems, instances and `USE`/`HIDE`, in order,
    /// including those of extended modules.
    pub top_level: Vec<NodeId>,
    /// Nesting depth (0 for a file's top module).
    pub nesting: u32,
    /// Whether this is one of the standard modules.
    pub is_standard: bool,
    /// Whether some module instantiates this one.
    pub is_instantiated: bool,
    pub(crate) recursive_decls: Vec<NodeId>,
    pub(crate) op_defs_in_recursive_section: Vec<NodeId>,
    pub(crate) constant_decls: Option<Vec<NodeId>>,
    pub(crate) variable_decls: Option<Vec<NodeId>>,
}

/// What an [`OpDeclData`] declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclKind {
    /// `CONSTANT`.
    Constant,
    /// `VARIABLE`.
    Variable,
    /// `NEW` or `NEW CONSTANT`.
    NewConstant,
    /// `NEW VARIABLE`.
    NewVariable,
    /// `NEW STATE`.
    NewState,
    /// `NEW ACTION`.
    NewAction,
    /// `NEW TEMPORAL`.
    NewTemporal,
    /// The marker SANY declares inside ordinary `ASSUME`s.
    Marker,
}

/// A declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDeclData {
    /// The name.
    pub name: Name,
    /// What kind of declaration.
    pub decl: DeclKind,
    /// The arity (`F(_, _)` has 2).
    pub arity: i32,
    /// The module it was declared in.
    pub module: Option<NodeId>,
}

/// A formal parameter or bound identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormalParamData {
    /// The name (`None` for the placeholders of a `RECURSIVE` declaration).
    pub name: Option<Name>,
    /// The arity (0 except for operator parameters).
    pub arity: i32,
    /// The module.
    pub module: Option<NodeId>,
}

/// What an [`OpDefData`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefKind {
    /// A user definition (also `LAMBDA` and selector-built operators).
    UserDefined,
    /// A built-in operator.
    BuiltIn,
    /// The name `I` of `I == INSTANCE M`.
    ModuleInstance,
    /// A numbered proof step that is not an assertion.
    NumberedProofStep,
    /// SANY's `nullODN` placeholder.
    Null,
}

/// An operator definition.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "SANY's OpDefNode flags")]
pub struct OpDefData {
    /// The name, qualified for instantiated definitions (`I!Op`).
    pub name: Name,
    /// The kind.
    pub kind: DefKind,
    /// Formal parameters.
    pub params: Vec<NodeId>,
    /// The arity; -1 for variadic built-ins.
    pub arity: i32,
    /// `LOCAL`.
    pub local: bool,
    /// The body (absent for built-ins and module instance names).
    pub body: Option<NodeId>,
    /// The module the definition belongs to for SANY's "same definition"
    /// test (`originallyDefinedInModule`).
    pub module: Option<NodeId>,
    /// The definition this one was instantiated from, if any.
    pub source: Option<NodeId>,
    /// Whether the definition has been seen (false between a `RECURSIVE`
    /// declaration and its definition).
    pub defined: bool,
    /// For a numbered proof step, the step.
    pub step: Option<NodeId>,
    /// Labels inside the body.
    pub labels: Option<LabelsId>,
    /// SANY's `letInLevel`.
    pub let_in_level: i32,
    /// Declared in a `RECURSIVE` statement.
    pub in_recursive: bool,
    pub(crate) in_recursive_section: bool,
    pub(crate) recursive_section: i32,
    pub(crate) lv: OpLevels,
}

/// SANY's operator level arrays (`maxLevels`, `weights`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpLevels {
    /// Maximum level of each argument.
    pub max_levels: Vec<u8>,
    /// Whether each argument's level contributes (1) or not (0).
    pub weights: Vec<u8>,
    /// Minimum of the maximum levels of operator arguments' arguments.
    pub min_max_level: Option<Vec<Vec<u8>>>,
    /// SANY's `opLevelCond`.
    pub op_level_cond: Option<Vec<Vec<Vec<bool>>>>,
    /// Whether each argument is Leibniz.
    pub is_leibniz_arg: Option<Vec<bool>>,
    /// Whether every argument is Leibniz.
    pub is_leibniz: bool,
}

/// A named theorem, assumption or assertion step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThmDefData {
    /// The name.
    pub name: Name,
    /// A theorem (or step) rather than an assumption.
    pub theorem: bool,
    /// The statement.
    pub body: Option<NodeId>,
    /// The module.
    pub module: Option<NodeId>,
    /// Parameters (from a parameterised instance).
    pub params: Vec<NodeId>,
    /// The module it was instantiated from, if any.
    pub instantiated_from: Option<NodeId>,
    /// The original this was instantiated from, if any.
    pub source: Option<NodeId>,
    /// `LOCAL` (only through `LOCAL INSTANCE`).
    pub local: bool,
    /// A `SUFFICES` step.
    pub suffices: bool,
    /// Labels inside the statement.
    pub labels: Option<LabelsId>,
    pub(crate) lv: OpLevels,
}

/// An operator application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpApplData {
    /// The operator: a definition, declaration, parameter or built-in.
    pub operator: NodeId,
    /// The arguments; `None` where SANY has a null operand (the guard of a
    /// `CASE` `OTHER` arm).
    pub operands: Vec<Option<NodeId>>,
    /// Unbounded bound identifiers (`\A x : P`), or the function name of a
    /// recursive function definition.
    pub unbounded: Option<Vec<NodeId>>,
    /// Bounded identifiers, one group per `x, y \in S` or `<<x, y>> \in S`.
    pub bounded: Option<Vec<Vec<NodeId>>>,
    /// Whether each group is a tuple pattern.
    pub tuples: Option<Vec<bool>>,
    /// The bounding sets, one per group.
    pub ranges: Vec<NodeId>,
    /// The definition this is a subexpression of, for selector-built nodes.
    pub subexpression_of: Option<NodeId>,
}

/// An operator argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpArgData {
    /// The operator passed.
    pub op: NodeId,
    /// Its arity.
    pub arity: i32,
}

/// `LET ... IN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetInData {
    /// The definitions (operators, functions, instance names, instantiated
    /// definitions), in order.
    pub defs: Vec<NodeId>,
    /// Instances made by `I == INSTANCE M` in the `LET`.
    pub insts: Vec<NodeId>,
    /// The body.
    pub body: Option<NodeId>,
    /// The `LET`'s symbols.
    pub ctx: CtxId,
}

/// A label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelData {
    /// The name.
    pub name: Name,
    /// Parameters (bound identifiers in scope).
    pub params: Vec<NodeId>,
    /// The named theorem or step whose `ASSUME`/`PROVE` contains it, if any.
    pub goal: Option<NodeId>,
    /// Which clause of that `ASSUME`/`PROVE`.
    pub goal_clause: i32,
    /// The labelled expression or `ASSUME`/`PROVE`.
    pub body: Option<NodeId>,
    /// Labels an `ASSUME`/`PROVE`.
    pub is_assume_prove: bool,
    /// Labels nested in the body.
    pub labels: Option<LabelsId>,
}

/// One substitution `c <- e` of an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subst {
    /// The substituted declaration of the instantiated module.
    pub op: NodeId,
    /// What replaces it: an expression or an [`NodeKind::OpArg`].
    pub expr: NodeId,
    /// Implicit (`c <- c` because the instantiating module has `c`).
    pub implicit: bool,
}

/// A substitution wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstInData {
    /// The substitutions.
    pub substs: Vec<Subst>,
    /// The wrapped body.
    pub body: Option<NodeId>,
    /// The instantiating module.
    pub instantiating: NodeId,
    /// The instantiated module.
    pub instantiated: NodeId,
}

/// `ASSUME ... PROVE ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumeProveData {
    /// The assumptions: expressions, `NEW` declarations, nested
    /// `ASSUME`/`PROVE`s, or labelled ones.
    pub assumes: Vec<Option<NodeId>>,
    /// The goal.
    pub prove: Option<NodeId>,
    /// Whether each clause is in the scope of an earlier `NEW`.
    pub in_scope_of_decl: Vec<bool>,
    /// `[]ASSUME`/`[]PROVE`.
    pub is_box: bool,
    /// A `SUFFICES` step.
    pub suffices: bool,
    /// The named theorem or step, for a top-level `ASSUME`/`PROVE`.
    pub goal: Option<NodeId>,
    pub(crate) in_proof: bool,
}

/// A theorem or proof step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremData {
    /// The statement (expression or `ASSUME`/`PROVE`), or the step's
    /// pseudo-operator application (`$Have`, `$Pick`, `$Qed`, ...).
    pub body: Option<NodeId>,
    /// The proof, if any.
    pub proof: Option<NodeId>,
    /// The name's definition, if named.
    pub def: Option<NodeId>,
    /// A `SUFFICES` step.
    pub suffices: bool,
    pub(crate) checked: u32,
}

/// An assumption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumeData {
    /// The assumed expression.
    pub expr: Option<NodeId>,
    /// The name's definition, if named.
    pub def: Option<NodeId>,
    /// Written `AXIOM`.
    pub is_axiom: bool,
    pub(crate) checked: u32,
}

/// An `INSTANCE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceData {
    /// `I` of `I == INSTANCE M`, if named.
    pub name: Option<Name>,
    /// `LOCAL`.
    pub local: bool,
    /// Parameters of `I(x) == INSTANCE M`.
    pub params: Vec<NodeId>,
    /// The instantiated module.
    pub module: NodeId,
    /// The substitutions, explicit and implicit.
    pub substs: Vec<Subst>,
    /// The step number, for an `INSTANCE` proof step.
    pub step: Option<Name>,
}

/// `USE` or `HIDE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseOrHideData {
    /// `HIDE` rather than `USE`.
    pub hide: bool,
    /// The facts.
    pub facts: Vec<NodeId>,
    /// The `DEF` items.
    pub defs: Vec<NodeId>,
    /// `USE ONLY`.
    pub only: bool,
    /// The step number, for a proof step.
    pub step: Option<Name>,
}

/// A context key: a symbol name or a module name (SANY keeps both in one
/// table, the latter wrapped in `ModuleName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// An operator, declaration or parameter name.
    Sym(Name),
    /// A module name.
    Module(Name),
}

/// A symbol context (SANY's `Context`): names in insertion order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub(crate) entries: Vec<(Key, NodeId)>,
    pub(crate) index: HashMap<Key, usize>,
}

impl Context {
    /// The node bound to `key`.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<NodeId> {
        self.index
            .get(&key)
            .and_then(|&i| self.entries.get(i))
            .map(|e| e.1)
    }

    /// Binds `key` (replacing an earlier binding, as `Hashtable.put`).
    pub(crate) fn put(&mut self, key: Key, node: NodeId) {
        if let Some(&i) = self.index.get(&key) {
            if let Some(e) = self.entries.get_mut(i) {
                e.1 = node;
            }
        } else {
            self.index.insert(key, self.entries.len());
            self.entries.push((key, node));
        }
    }

    /// The bindings in insertion order.
    pub fn entries(&self) -> impl Iterator<Item = (Key, NodeId)> + '_ {
        self.entries.iter().copied()
    }
}

/// The name interner.
#[derive(Debug, Clone, Default)]
pub(crate) struct Interner {
    map: HashMap<Box<str>, Name>,
    strs: Vec<Box<str>>,
}

impl Interner {
    pub(crate) fn intern(&mut self, s: &str) -> Name {
        if let Some(&n) = self.map.get(s) {
            return n;
        }
        let n = Name(u32::try_from(self.strs.len()).unwrap_or(u32::MAX));
        self.strs.push(s.into());
        self.map.insert(s.into(), n);
        n
    }

    pub(crate) fn get(&self, s: &str) -> Option<Name> {
        self.map.get(s).copied()
    }

    pub(crate) fn resolve(&self, n: Name) -> &str {
        self.strs.get(n.0 as usize).map_or("", |s| s)
    }
}

/// The semantic graph of a whole specification: the root module and every
/// module it depends on, the built-in operators, and all their contents.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub(crate) names: Interner,
    pub(crate) nodes: Vec<Node>,
    pub(crate) contexts: Vec<Context>,
    pub(crate) labels: Vec<BTreeMap<Name, NodeId>>,
    /// External (file-level) modules in semantic-analysis order: every
    /// module after the modules it depends on. The root module is last.
    pub(crate) modules: Vec<NodeId>,
    pub(crate) root: Option<NodeId>,
    pub(crate) builtins: Vec<NodeId>,
}

impl Model {
    /// The text of a name.
    #[must_use]
    pub fn name(&self, n: Name) -> &str {
        self.names.resolve(n)
    }

    /// Looks up an interned name.
    #[must_use]
    pub fn lookup_name(&self, s: &str) -> Option<Name> {
        self.names.get(s)
    }

    /// A node.
    ///
    /// # Panics
    ///
    /// Never for ids taken from this model; an id from another model may
    /// index out of range, which returns the first node instead.
    #[must_use]
    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes
            .get(id.index())
            .or_else(|| self.nodes.first())
            .unwrap_or(&NULL_NODE)
    }

    /// All nodes.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// A context.
    #[must_use]
    pub fn context(&self, id: CtxId) -> &Context {
        self.contexts.get(id.0 as usize).unwrap_or(&EMPTY_CTX)
    }

    /// A label table.
    #[must_use]
    pub fn labels(&self, id: LabelsId) -> &BTreeMap<Name, NodeId> {
        self.labels.get(id.0 as usize).unwrap_or(&EMPTY_LABELS)
    }

    /// The external modules in analysis order (dependencies first).
    #[must_use]
    pub fn modules(&self) -> &[NodeId] {
        &self.modules
    }

    /// The root module, if semantic analysis reached it.
    #[must_use]
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// The built-in operators.
    #[must_use]
    pub fn builtins(&self) -> &[NodeId] {
        &self.builtins
    }

    /// A module's `VARIABLE` declarations, including extended ones, in SANY's
    /// order (`ModuleNode.getVariableDecls`). For the root module this is
    /// TLC's state-variable order: the `i`-th declaration's `varLoc` is `i`
    /// (`SpecProcessor.processSpec`), and `TLCStateMut.fingerPrint` walks the
    /// values in this order.
    ///
    /// SANY computes the array on first use and caches it; so does this, when
    /// analysis asked for it. Otherwise it is computed from the final context.
    /// It is the reverse of the context `Hashtable`'s enumeration order
    /// ([`crate::hashtable`]), so besides the declarations' names it depends on
    /// how many keys the context has and when each declaration was inserted.
    ///
    /// ```
    /// use dive_sema::{analyze_source, Options};
    /// let src = "---- MODULE M ----\nVARIABLES x, y, z\n====\n";
    /// let a = analyze_source("M.tla", src, &Options::default());
    /// let m = a.model();
    /// let vars: Vec<&str> = m
    ///     .variable_decls(m.root().unwrap())
    ///     .into_iter()
    ///     .filter_map(|v| m.symbol_name(v))
    ///     .collect();
    /// assert_eq!(vars, ["x", "y", "z"]);
    /// ```
    #[must_use]
    pub fn variable_decls(&self, module: NodeId) -> Vec<NodeId> {
        match &self.node(module).kind {
            NodeKind::Module(d) => d
                .variable_decls
                .clone()
                .unwrap_or_else(|| crate::generate::decls_in(self, module, true)),
            _ => Vec::new(),
        }
    }

    /// A module's `CONSTANT` declarations, including extended ones, in SANY's
    /// order (`ModuleNode.getConstantDecls`), computed and cached as
    /// [`Model::variable_decls`].
    #[must_use]
    pub fn constant_decls(&self, module: NodeId) -> Vec<NodeId> {
        match &self.node(module).kind {
            NodeKind::Module(d) => d
                .constant_decls
                .clone()
                .unwrap_or_else(|| crate::generate::decls_in(self, module, false)),
            _ => Vec::new(),
        }
    }

    /// The level of a node after level checking, as SANY's `getLevel`.
    #[must_use]
    pub fn level(&self, id: NodeId) -> Level {
        Level::from_u8(crate::level::level_of(self, id))
    }

    /// SANY's kind of a node.
    #[must_use]
    pub fn kind(&self, id: NodeId) -> Kind {
        match &self.node(id).kind {
            NodeKind::Module(_) => Kind::Module,
            NodeKind::OpDecl(d) => match d.decl {
                DeclKind::Constant | DeclKind::Marker => Kind::ConstantDecl,
                DeclKind::Variable => Kind::VariableDecl,
                DeclKind::NewConstant => Kind::NewConstant,
                DeclKind::NewVariable => Kind::NewVariable,
                DeclKind::NewState => Kind::NewState,
                DeclKind::NewAction => Kind::NewAction,
                DeclKind::NewTemporal => Kind::NewTemporal,
            },
            NodeKind::FormalParam(_) => Kind::FormalParam,
            NodeKind::OpDef(d) => match d.kind {
                DefKind::UserDefined => Kind::UserDefinedOp,
                DefKind::BuiltIn => Kind::BuiltIn,
                DefKind::ModuleInstance => Kind::ModuleInstance,
                DefKind::NumberedProofStep => Kind::NumberedProofStep,
                DefKind::Null => Kind::Error,
            },
            NodeKind::ThmOrAssumpDef(_) => Kind::ThmOrAssumpDef,
            NodeKind::OpAppl(_) => Kind::OpAppl,
            NodeKind::OpArg(_) => Kind::OpArg,
            NodeKind::LetIn(_) => Kind::LetIn,
            NodeKind::Numeral(_) => Kind::Numeral,
            NodeKind::Decimal(..) => Kind::Decimal,
            NodeKind::Str(_) => Kind::String,
            NodeKind::At { .. } => Kind::AtNode,
            NodeKind::Label(_) => Kind::Label,
            NodeKind::SubstIn(_) => Kind::SubstIn,
            NodeKind::ApSubstIn(_) => Kind::APSubstIn,
            NodeKind::AssumeProve(_) => Kind::AssumeProve,
            NodeKind::NewSymb { .. } => Kind::NewSymb,
            NodeKind::Theorem(_) => Kind::Theorem,
            NodeKind::Assume(_) => Kind::Assume,
            NodeKind::Instance(_) => Kind::Instance,
            NodeKind::UseOrHide(d) => {
                if d.hide {
                    Kind::Hide
                } else {
                    Kind::Use
                }
            }
            NodeKind::DefStep { .. } => Kind::DefStep,
            NodeKind::NonLeafProof { .. } => Kind::NonLeafProof,
            NodeKind::LeafProof { .. } => Kind::LeafProof,
            NodeKind::Error => Kind::Error,
        }
    }

    /// The name of a symbol node (declaration, parameter, definition,
    /// theorem name, module or label), if it has one.
    #[must_use]
    pub fn symbol_name(&self, id: NodeId) -> Option<&str> {
        let n = match &self.node(id).kind {
            NodeKind::Module(d) => Some(d.name),
            NodeKind::OpDecl(d) => Some(d.name),
            NodeKind::FormalParam(d) => d.name,
            NodeKind::OpDef(d) => Some(d.name),
            NodeKind::ThmOrAssumpDef(d) => Some(d.name),
            NodeKind::Label(d) => Some(d.name),
            _ => None,
        };
        n.map(|n| self.name(n))
    }

    /// The nodes a node refers to: operators, operands, bodies, parameters,
    /// bound identifiers, substitutions, proof steps and, for a module, the
    /// symbols of its context and its top-level items (not
    /// [`ModuleData::definitions`], which also lists definitions that lost a
    /// name clash SANY only warns about and that no name reaches). Following `children`
    /// from [`Model::modules`] visits everything reachable in the
    /// specification.
    #[must_use]
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let subst = |out: &mut Vec<NodeId>, substs: &[Subst]| {
            for s in substs {
                out.push(s.op);
                out.push(s.expr);
            }
        };
        match &self.node(id).kind {
            NodeKind::Module(d) => {
                out.extend(self.context(d.ctx).entries().map(|e| e.1));
                out.extend(d.top_level.iter().copied());
            }
            NodeKind::OpDecl(_)
            | NodeKind::FormalParam(_)
            | NodeKind::Numeral(_)
            | NodeKind::Decimal(..)
            | NodeKind::Str(_)
            | NodeKind::Error => {}
            NodeKind::OpDef(d) => {
                out.extend(d.params.iter().copied());
                out.extend(d.body);
                out.extend(d.step);
            }
            NodeKind::ThmOrAssumpDef(d) => {
                out.extend(d.params.iter().copied());
                out.extend(d.body);
            }
            NodeKind::OpAppl(d) => {
                out.push(d.operator);
                out.extend(d.operands.iter().flatten().copied());
                out.extend(d.unbounded.iter().flatten().copied());
                out.extend(d.bounded.iter().flatten().flatten().copied());
                out.extend(d.ranges.iter().copied());
            }
            NodeKind::OpArg(d) => out.push(d.op),
            NodeKind::LetIn(d) => {
                out.extend(d.defs.iter().copied());
                out.extend(d.insts.iter().copied());
                out.extend(d.body);
            }
            NodeKind::At { except, component } => {
                out.push(*except);
                out.push(*component);
            }
            NodeKind::Label(d) => {
                out.extend(d.params.iter().copied());
                out.extend(d.body);
            }
            NodeKind::SubstIn(d) | NodeKind::ApSubstIn(d) => {
                subst(&mut out, &d.substs);
                out.extend(d.body);
            }
            NodeKind::AssumeProve(d) => {
                out.extend(d.assumes.iter().flatten().copied());
                out.extend(d.prove);
            }
            NodeKind::NewSymb { decl, set } => {
                out.push(*decl);
                out.extend(*set);
            }
            NodeKind::Theorem(d) => {
                out.extend(d.body);
                out.extend(d.proof);
                out.extend(d.def);
            }
            NodeKind::Assume(d) => {
                out.extend(d.expr);
                out.extend(d.def);
            }
            NodeKind::Instance(d) => {
                out.extend(d.params.iter().copied());
                subst(&mut out, &d.substs);
            }
            NodeKind::UseOrHide(d) => {
                out.extend(d.facts.iter().copied());
                out.extend(d.defs.iter().copied());
            }
            NodeKind::DefStep { defs, .. } => out.extend(defs.iter().copied()),
            NodeKind::NonLeafProof { steps, insts, .. } => {
                out.extend(steps.iter().copied());
                out.extend(insts.iter().copied());
            }
            NodeKind::LeafProof { facts, defs, .. } => {
                out.extend(facts.iter().copied());
                out.extend(defs.iter().copied());
            }
        }
        out
    }

    pub(crate) fn add(&mut self, span: Option<Span>, kind: NodeKind) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        self.nodes.push(Node {
            span,
            kind,
            level: LevelData {
                correct: true,
                ..LevelData::default()
            },
        });
        id
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.index())
    }

    pub(crate) fn new_context(&mut self) -> CtxId {
        let id = CtxId(u32::try_from(self.contexts.len()).unwrap_or(u32::MAX));
        self.contexts.push(Context::default());
        id
    }

    pub(crate) fn ctx_mut(&mut self, id: CtxId) -> Option<&mut Context> {
        self.contexts.get_mut(id.0 as usize)
    }

    pub(crate) fn new_labels(&mut self, table: BTreeMap<Name, NodeId>) -> LabelsId {
        let id = LabelsId(u32::try_from(self.labels.len()).unwrap_or(u32::MAX));
        self.labels.push(table);
        id
    }
}

static NULL_NODE: Node = Node {
    span: None,
    kind: NodeKind::Error,
    level: LevelData {
        level: 0,
        level_params: BTreeSet::new(),
        all_params: BTreeSet::new(),
        non_leibniz_params: BTreeSet::new(),
        level_constraints: BTreeMap::new(),
        arg_level_constraints: BTreeMap::new(),
        arg_level_params: BTreeSet::new(),
        checked: 0,
        correct: true,
    },
};

static EMPTY_CTX: std::sync::LazyLock<Context> = std::sync::LazyLock::new(Context::default);
static EMPTY_LABELS: BTreeMap<Name, NodeId> = BTreeMap::new();
