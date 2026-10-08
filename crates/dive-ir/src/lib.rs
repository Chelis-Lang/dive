//! `dive-ir`: the resolved core IR shared by every backend (ADR 0013).
//!
//! [`lower`] turns an accepted [`dive_sema::Analysis`] into an [`Ir`]: one
//! immutable arena per specification holding every module's definitions,
//! declarations and expressions. Names are resolved to ids and frame slots,
//! SANY's `$`-named built-ins become explicit [`Expr`] variants, and the IR
//! records what TLC's evaluation depends on:
//!
//! - **laziness points** ([`Pass`]): operator arguments, zero-arity `LET`
//!   definitions and instance substitutions, exactly where TLC creates
//!   `LazyValue`s;
//! - **instances** as substitution nodes ([`Expr::Subst`], [`Expr::SubstRef`]),
//!   not expanded;
//! - **recursion** ([`Def::recursive`], [`Expr::RecFcn`]);
//! - **Java overrides** ([`StdOp`]), matched as TLC matches them, and
//!   refusals of the ones dive does not implement ([`Refusal`]);
//! - **effects** ([`Effects`]);
//! - **origins** with SANY's line and column ([`Origin`]) and levels
//!   ([`Level`]) for every node.
//!
//! The IR holds no values and no configuration: constants, overrides and
//! roles are bound by `dive-model` (ADR 0013 §8).
//!
//! # Example
//!
//! ```
//! use dive_ir::{Expr, lower};
//! use dive_sema::{analyze_source, Options};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\n\
//!            Init == x = 0\nNext == x' = x + 1\n====\n";
//! let analysis = analyze_source("M.tla", src, &Options::default());
//! let ir = lower(&analysis).expect("SANY accepts it");
//! let next = ir.root_def("Next").expect("defined");
//! let body = ir.expr(ir.def(next).body);
//! assert!(matches!(body, Expr::Builtin { .. })); // `=`
//! assert_eq!(ir.vars().len(), 1);
//! ```
#![forbid(unsafe_code)]

mod analysis;
pub mod effects;
pub mod expr;
pub mod ids;
mod lower;
pub mod origin;
mod pretty;
pub mod std_op;
pub mod validate;

pub use effects::Effects;
pub use expr::{
    Arg, Bound, BuiltinOp, CaseArm, ExceptUpdate, Expr, Field, JunctionKind, LocalRef, OpRef, Pass,
    QuantKind, SubstBinding, Unevaluable,
};
pub use ids::{ConstId, DeclId, DefId, ExprId, FileId, ModuleId, Slot, VarId};
pub use lower::{LowerError, lower};
pub use origin::{FileInfo, Origin};
pub use std_op::{StdKind, StdOp, StdSupport};

use serde::{Deserialize, Serialize};

/// A TLA+ level (SANY's order: constant < state < action < temporal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Constant level.
    Constant,
    /// State level: depends on unprimed variables.
    State,
    /// Action level: depends on primed variables.
    Action,
    /// Temporal level.
    Temporal,
}

impl From<dive_sema::Level> for Level {
    fn from(l: dive_sema::Level) -> Self {
        match l {
            dive_sema::Level::Constant => Self::Constant,
            dive_sema::Level::State => Self::State,
            dive_sema::Level::Action => Self::Action,
            dive_sema::Level::Temporal => Self::Temporal,
        }
    }
}

/// The version of the IR's serialized form. Bump it whenever a serialized
/// type changes; the on-disk cache (ADR 0013 §7) keys on it.
pub const IR_SCHEMA: u32 = 1;

/// One lowered specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ir {
    pub(crate) files: Vec<FileInfo>,
    pub(crate) modules: Vec<Module>,
    pub(crate) root: ModuleId,
    pub(crate) defs: Vec<Def>,
    pub(crate) exprs: Vec<Node>,
    pub(crate) decls: Vec<Decl>,
    pub(crate) vars: Vec<DeclId>,
    pub(crate) consts: Vec<DeclId>,
    pub(crate) refusals: Vec<Refusal>,
    /// For each expression, the index of the `dive-sema` node it was lowered
    /// from (`dive_sema::NodeId::index`).
    pub(crate) sema_nodes: Vec<u32>,
}

/// An expression node with its annotations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Node {
    /// The expression.
    pub expr: Expr,
    /// Where it comes from.
    pub origin: Origin,
    /// SANY's level, from level checking.
    pub level: Level,
    /// Its effects, including those of the definitions it uses.
    pub effects: Effects,
}

/// A module, file-level or inner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// The name.
    pub name: String,
    /// The file it is in.
    pub file: Option<FileId>,
    /// 0 for a file's top module.
    pub nesting: u32,
    /// The module this one is nested in.
    pub parent: Option<ModuleId>,
    /// One of the embedded standard modules.
    pub standard: bool,
    /// Some module instantiates it.
    pub instantiated: bool,
    /// Directly extended modules.
    pub extends: Vec<ModuleId>,
    /// The module's context in SANY's insertion order: every symbol visible
    /// in it, including extended and instantiated definitions and `LOCAL`
    /// ones. This is what TLC's `ModuleNode.getOpDefs` and the configuration
    /// look names up in.
    pub context: Vec<(String, Symbol)>,
    /// The `ASSUME`s, including extended ones, in order.
    pub assumptions: Vec<DefId>,
    /// The `THEOREM` statements, including extended ones, in order. TLC never
    /// evaluates them, but it does process their literals at start-up.
    pub theorems: Vec<DefId>,
}

/// What a name in a module's context denotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Symbol {
    /// A definition.
    Def(DefId),
    /// A `CONSTANT` or `VARIABLE`.
    Decl(DeclId),
    /// An instance name `I` of `I == INSTANCE M`; its definitions are the
    /// `I!Op` entries.
    Instance,
}

/// A `CONSTANT` or `VARIABLE` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decl {
    /// The name.
    pub name: String,
    /// The module that declares it.
    pub module: ModuleId,
    /// `VARIABLE` rather than `CONSTANT`.
    pub variable: bool,
    /// The arity (`CONSTANT F(_, _)` has 2).
    pub arity: u32,
    /// Its id as a root state variable, if it is one.
    pub var: Option<VarId>,
    /// Its id as a constant, if it is one.
    pub constant: Option<ConstId>,
    /// Some instance substitutes it, so references are
    /// [`Expr::SubstRef`]/[`OpRef::Subst`].
    pub substituted: bool,
    /// Where it is declared.
    pub origin: Origin,
    /// SANY's level.
    pub level: Level,
}

/// What kind of definition a [`Def`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefKind {
    /// An operator definition `Op(p) == e`.
    Operator,
    /// A function definition `f[x \in S] == e`; the body is an
    /// [`Expr::FcnCons`] or [`Expr::RecFcn`].
    Function,
    /// A `LAMBDA`.
    Lambda,
    /// A named or unnamed theorem.
    Theorem,
    /// A named or unnamed assumption.
    Assumption,
}

/// Where a definition sits, which decides its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefScope {
    /// A module-level definition, assumption or theorem: its own frame, with
    /// no parent.
    TopLevel,
    /// A `LET` operator with parameters: its own frame, whose parent is the
    /// frame the `LET` was evaluated in (its closure).
    Local {
        /// The definition whose frame is the parent.
        parent: DefId,
    },
    /// A definition lowered at its single point of use, with its own frame
    /// whose parent is the frame of the use: a `LAMBDA`, or a `LET`
    /// definition reached through a subexpression selector from outside its
    /// `LET` (`Op(1)!(2)!F(3)`). TLC finds neither in its context and
    /// evaluates the body in the context of the use, so names the selector
    /// binds are visible in it. [`Expr::Def`] of a zero-arity one evaluates
    /// the body in a fresh frame.
    Inline {
        /// The definition whose frame is the parent.
        parent: DefId,
    },
    /// A zero-arity `LET` definition: a thunk in `slot` of the frame of
    /// `parent`, created each time the `LET` is evaluated
    /// (`Tool.java:1849-1861`). Its body is evaluated in that frame.
    LetValue {
        /// The definition whose frame holds the slot.
        parent: DefId,
        /// The slot.
        slot: Slot,
        /// Whether the thunk may be evaluated eagerly.
        pass: Pass,
    },
}

/// A parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Param {
    /// The name.
    pub name: String,
    /// 0 for an ordinary parameter, the arity of an operator parameter.
    pub arity: u32,
}

/// What a frame slot holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotKind {
    /// A parameter: a thunk, or an operator for an operator parameter.
    Param,
    /// A bound variable of a quantifier, set constructor, function or
    /// `CHOOSE`: a value.
    Bound,
    /// A zero-arity `LET` definition: a thunk.
    LetValue(DefId),
    /// The name of a recursive function inside its own body.
    FcnName,
}

/// One slot of a frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlotInfo {
    /// The source name.
    pub name: String,
    /// What it holds.
    pub kind: SlotKind,
}

/// A definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Def {
    /// The name, qualified for instantiated definitions (`I!Op`). `LAMBDA`
    /// for a `LAMBDA`; empty for an unnamed assumption or theorem.
    pub name: String,
    /// The module whose context it belongs to.
    pub module: ModuleId,
    /// What it is.
    pub kind: DefKind,
    /// Where it sits.
    pub scope: DefScope,
    /// Frame nesting depth: 0 for top-level definitions, the parent's depth
    /// plus one for [`DefScope::Local`] and [`DefScope::Inline`], the parent's depth for
    /// [`DefScope::LetValue`].
    pub depth: u32,
    /// The parameters; they are slots `0..params.len()` of the frame.
    pub params: Vec<Param>,
    /// The frame's slots. Empty for [`DefScope::LetValue`], whose slots
    /// belong to the parent's frame.
    pub frame: Vec<SlotInfo>,
    /// The body. For a [`StdOp`] this is the TLA+ definition, kept for
    /// documentation; it is never evaluated (ADR 0013 §5).
    pub body: ExprId,
    /// Declared `RECURSIVE`. The definition call graph is cyclic only
    /// through these.
    pub recursive: bool,
    /// `LOCAL`.
    pub local: bool,
    /// The Java override TLC uses instead of the body.
    pub std: Option<StdOp>,
    /// For an instantiated definition, the definition it instantiates.
    pub source: Option<DefId>,
    /// Where it comes from.
    pub origin: Origin,
    /// SANY's level.
    pub level: Level,
    /// The effects of evaluating its body, to a fixed point over the
    /// definitions it uses.
    pub effects: Effects,
    /// A hash of its lowered body that ignores origins and ids (see
    /// [`crate::Ir::def_hash`]).
    pub hash: u64,
}

/// A use of a construct dive refuses (ADR 0013 §5, M5 plan). The model
/// refuses the specification as `unsupported`, naming the feature, if a
/// refused node is reachable from a role.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// What is refused.
    pub feature: Feature,
    /// The expression that uses it.
    pub expr: ExprId,
    /// The definition it is in.
    pub def: DefId,
}

/// A feature dive refuses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Feature {
    /// A Java-overridden operator outside dive's standard library.
    StdOp(StdOp),
}

impl Feature {
    /// A user-facing name for the feature.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::StdOp(op) => format!("{} ({})", op.qualified_name(), op.support().describe()),
        }
    }
}

impl Ir {
    /// The source files.
    #[must_use]
    pub fn files(&self) -> &[FileInfo] {
        &self.files
    }

    /// The modules: each file-level module after the modules it depends on,
    /// its inner modules before it.
    #[must_use]
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    /// One module.
    ///
    /// # Panics
    ///
    /// If the id is not from this IR.
    #[must_use]
    pub fn module(&self, id: ModuleId) -> &Module {
        &self.modules[id.index()]
    }

    /// The root module.
    #[must_use]
    pub fn root(&self) -> ModuleId {
        self.root
    }

    /// Every definition.
    #[must_use]
    pub fn defs(&self) -> &[Def] {
        &self.defs
    }

    /// One definition.
    ///
    /// # Panics
    ///
    /// If the id is not from this IR.
    #[must_use]
    pub fn def(&self, id: DefId) -> &Def {
        &self.defs[id.index()]
    }

    /// Every expression node.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.exprs
    }

    /// One expression node.
    ///
    /// # Panics
    ///
    /// If the id is not from this IR.
    #[must_use]
    pub fn node(&self, id: ExprId) -> &Node {
        &self.exprs[id.index()]
    }

    /// One expression.
    ///
    /// # Panics
    ///
    /// If the id is not from this IR.
    #[must_use]
    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.node(id).expr
    }

    /// Every declaration.
    #[must_use]
    pub fn decls(&self) -> &[Decl] {
        &self.decls
    }

    /// One declaration.
    ///
    /// # Panics
    ///
    /// If the id is not from this IR.
    #[must_use]
    pub fn decl(&self, id: DeclId) -> &Decl {
        &self.decls[id.index()]
    }

    /// The root module's state variables, by [`VarId`].
    #[must_use]
    pub fn vars(&self) -> &[DeclId] {
        &self.vars
    }

    /// The constants of every module, by [`ConstId`].
    #[must_use]
    pub fn consts(&self) -> &[DeclId] {
        &self.consts
    }

    /// The index of the `dive-sema` node an expression was lowered from
    /// (`dive_sema::NodeId::index` in the analysis that was lowered).
    #[must_use]
    pub fn sema_node(&self, id: ExprId) -> Option<usize> {
        self.sema_nodes.get(id.index()).map(|&n| n as usize)
    }

    /// Every use of a refused feature, in definition order.
    #[must_use]
    pub fn refusals(&self) -> &[Refusal] {
        &self.refusals
    }

    /// Looks up a name in a module's context.
    #[must_use]
    pub fn lookup(&self, module: ModuleId, name: &str) -> Option<Symbol> {
        self.modules
            .get(module.index())?
            .context
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| *s)
    }

    /// The definition `name` in the root module's context.
    #[must_use]
    pub fn root_def(&self, name: &str) -> Option<DefId> {
        match self.lookup(self.root, name)? {
            Symbol::Def(d) => Some(d),
            _ => None,
        }
    }

    /// SANY's location string for an origin, `line L, col C to line L2, col
    /// C2 of module M`, as TLC prints it.
    #[must_use]
    pub fn location(&self, origin: &Origin) -> String {
        let module = origin
            .file
            .and_then(|f| self.files.get(f.index()))
            .map_or("null", |f| f.module.as_str());
        origin::location_string(origin, module)
    }

    /// The structural hash of a definition: its lowered body with origins
    /// ignored and definitions, declarations and standard operators named
    /// rather than numbered. Equal for equal definitions across edits
    /// elsewhere in the specification.
    #[must_use]
    pub fn def_hash(&self, id: DefId) -> u64 {
        self.def(id).hash
    }

    /// The definitions reachable from `roots`: through uses, `LET`s and
    /// operator arguments, but not into the TLA+ bodies of Java overrides,
    /// which are never evaluated. Indexed by [`DefId`].
    #[must_use]
    pub fn reachable_defs(&self, roots: &[DefId]) -> Vec<bool> {
        let mut seen = vec![false; self.defs.len()];
        let mut work: Vec<DefId> = roots.to_vec();
        while let Some(d) = work.pop() {
            let Some(s) = seen.get_mut(d.index()) else {
                continue;
            };
            if *s {
                continue;
            }
            *s = true;
            let def = self.def(d);
            if def.std.is_some() {
                continue;
            }
            let mut exprs = vec![def.body];
            while let Some(e) = exprs.pop() {
                let node = self.node(e);
                if let Expr::Def(x) = &node.expr {
                    work.push(*x);
                }
                if let Expr::Let { defs, .. } = &node.expr {
                    work.extend(defs.iter().copied());
                }
                for r in node.expr.op_refs() {
                    if let OpRef::Def(x) = r {
                        work.push(*x);
                    }
                }
                exprs.extend(node.expr.children());
            }
        }
        seen
    }

    /// A deterministic, human-readable dump of the whole IR, for debugging
    /// and snapshot tests.
    #[must_use]
    pub fn pretty(&self) -> String {
        pretty::print(self)
    }
}
