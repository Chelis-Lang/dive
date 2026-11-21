//! Temporal properties on sampled traces (ADR 0162 §4).
//!
//! Each trace `s0 .. sn` defines a trace graph `G`: its nodes are the
//! trace's distinct states under exact value equality, its edges the sampled
//! steps, and every node has a stuttering self-loop. Every infinite path of
//! `G` from `s0` is a behavior of `Init /\ [][Next]_vars`. A violation of a
//! property `p` is a path on which every fairness conjunct holds and `p` is
//! false; this module finds one by searching `G` times an automaton for
//! `Fair /\ ~p`.
//!
//! The formula `Fair /\ ~p` comes from `dive_liveness::plan::lower_violation`.
//! Its negation-normal form is split into acceptance clauses (disjunctions
//! of `[]<>B` and `<>[]B` with `B` free of temporal operators, as `WF` and
//! `SF` lower) and a residual compiled to a generalized Büchi tableau
//! (`dive_liveness::formula::compile`). A strongly connected part of the
//! product accepts when it fulfils every tableau promise and every clause:
//! a `[]<>B` term by an edge where `B` holds, a `<>[]B` term by restricting
//! the part to edges where `B` holds and searching again (an Emerson-Lei
//! check). Conditions read a state predicate on an edge's source and an
//! action predicate on the edge.
//!
//! A found word is compressed (consecutive equal states removed; every TLA+
//! formula is invariant under stuttering), its complete set of false
//! configured properties is computed with `Formula::evaluate_word`, and the
//! caller validates it independently with `dive-trace` before reporting it.
//! Nothing here reports that a property holds: a trace on which no violation
//! was found either contributed a fair behavior (`checked`) or did not.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use dive_eval::St;
use dive_eval::next::StateVals;
use dive_liveness::formula::{
    Atom, AtomKind, Formula, Limits, Literal, Node, Tableau, TableauNode, compile,
};
use dive_liveness::plan::{AtomTest, Plan, lower_violation};
use dive_model::Model;
use dive_sim::{Outcome, TemporalEnding, Unchecked};

/// Work units charged for one `ENABLED` evaluation: a successor search.
const ENABLED_COST: u64 = 16;

/// Word cells `Formula::evaluate_word` may fill for one claim.
const WORD_CELLS: usize = 64 * 1024 * 1024;

/// A configured property, planned once per run.
struct Claim {
    name: String,
    plan: Plan,
    /// `Fair /\ ~p`.
    violation: Compiled,
    /// `p` alone over the plan's nodes, for the false-property set of a word.
    property: Formula,
    /// Atoms `property` reads.
    property_atoms: Vec<usize>,
    /// `p` in negation-normal form, for finite refutation.
    positive: (Arena, usize),
}

/// The per-run temporal checker. Shared by every worker; a trace's check
/// reads only the trace, so its result is a function of the trace.
pub(crate) struct Checker {
    claims: Vec<Claim>,
    /// `Fair` alone, over the first claim's atoms.
    fair: Compiled,
    /// Leading atoms of every claim's plan that are its fairness atoms,
    /// identical across claims and therefore evaluated once.
    shared_atoms: usize,
    bad_prefix: bool,
    budget: Option<u64>,
}

/// A per-trace verdict.
pub(crate) enum Verdict {
    /// A counterexample word, not yet independently validated.
    Violation(Word),
    /// For each claim, `None` when checked, else why not.
    Coverage(Vec<Option<Unchecked>>),
}

/// A counterexample: states in order, the action into each next state, how
/// the word continues, and the configured properties false on it.
pub(crate) struct Word {
    pub(crate) states: Vec<StateVals>,
    pub(crate) actions: Vec<usize>,
    pub(crate) ending: TemporalEnding,
    pub(crate) properties: Vec<String>,
}

/// Why a check stopped without a verdict for the trace.
enum Stop {
    Budget,
    Failed(Outcome),
}

impl Checker {
    /// Plans every configured property. A planning refusal is a named
    /// refusal of the run; no state has been generated yet.
    pub(crate) fn new(
        model: &Model,
        bad_prefix: bool,
        budget: Option<u64>,
    ) -> Result<Option<Self>, Outcome> {
        if model.implied_temporals.is_empty() {
            return Ok(None);
        }
        let refused = |name: &str, detail: String| {
            Outcome::Unsupported(format!("simulation temporal property {name}: {detail}"))
        };
        let mut claims = Vec::new();
        let mut fair = None;
        for named in model.property_claims() {
            let plan = lower_violation(model, &named.pred, Limits::default())
                .map_err(|error| refused(&named.name, error.to_string()))?;
            let nodes = plan.formula.nodes();
            let Some(Node::And(conjuncts)) = nodes.get(plan.formula.root()) else {
                return Err(Outcome::Internal(
                    "simulation temporal plan is not a conjunction".into(),
                ));
            };
            let Some((&negated, fairness)) = conjuncts.split_last() else {
                return Err(Outcome::Internal(
                    "simulation temporal plan has no property".into(),
                ));
            };
            let Some(&Node::Not(property)) = nodes.get(negated) else {
                return Err(Outcome::Internal(
                    "simulation temporal plan does not negate its property".into(),
                ));
            };
            let mut arena = Arena::default();
            let root = arena.nnf(nodes, plan.formula.root(), true);
            let violation =
                Compiled::new(&arena, root).map_err(|detail| refused(&named.name, detail))?;
            if fair.is_none() {
                let mut arena = Arena::default();
                let parts: Vec<usize> = fairness
                    .iter()
                    .map(|&c| arena.nnf(nodes, c, true))
                    .collect();
                let root = arena.and(parts);
                fair = Some(
                    Compiled::new(&arena, root).map_err(|detail| refused(&named.name, detail))?,
                );
            }
            let property_formula = Formula::new(nodes.to_vec(), property)
                .map_err(|error| refused(&named.name, error.to_string()))?;
            let property_atoms = reachable_atoms(nodes, property);
            let mut positive = Arena::default();
            let top = positive.nnf(nodes, property, true);
            claims.push(Claim {
                name: named.name.clone(),
                plan,
                violation,
                property: property_formula,
                property_atoms,
                positive: (positive, top),
            });
        }
        let Some(fair) = fair else {
            return Ok(None);
        };
        // Fairness is lowered first, in the same order, for every claim.
        let shared_atoms = claims.first().map_or(0, |first| {
            let fairness = fairness_atoms(&first.plan);
            if claims.iter().all(|claim| {
                fairness_atoms(&claim.plan) == fairness
                    && claim.plan.atoms[..fairness]
                        .iter()
                        .zip(&first.plan.atoms[..fairness])
                        .all(|(a, b)| {
                            a.pred.expr == b.pred.expr && a.kind == b.kind && a.test == b.test
                        })
            }) {
                fairness
            } else {
                0
            }
        });
        Ok(Some(Self {
            claims,
            fair,
            shared_atoms,
            bad_prefix,
            budget,
        }))
    }

    /// Configured property names, in order.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.claims.iter().map(|claim| claim.name.as_str())
    }

    /// Checks one trace. `truncated` says the trace was cut by the depth
    /// (it neither closed nor terminated); `actions[k]` led from trace state
    /// `k` to `k + 1`.
    pub(crate) fn check(
        &self,
        model: &Model,
        graph: &TraceGraph,
        actions: &[usize],
        truncated: bool,
    ) -> Result<Verdict, Outcome> {
        let budget = self
            .budget
            .unwrap_or_else(|| dive_sim::liveness_budget(graph.path.len()));
        let mut cx = Cx {
            model,
            checker: self,
            graph,
            state: HashMap::new(),
            action: HashMap::new(),
            work: 0,
            budget,
        };
        match cx.run(actions) {
            Ok(Some(word)) => Ok(Verdict::Violation(word)),
            Ok(None) => {
                let fair = match cx.search(&self.fair, None, false) {
                    Ok(found) => found.is_some(),
                    Err(Stop::Budget) => {
                        return Ok(Verdict::Coverage(vec![
                            Some(Unchecked::Budget);
                            self.claims.len()
                        ]));
                    }
                    Err(Stop::Failed(outcome)) => return Err(outcome),
                };
                let reason = if fair {
                    None
                } else if truncated {
                    Some(Unchecked::Truncated)
                } else {
                    Some(Unchecked::NoFairBehavior)
                };
                Ok(Verdict::Coverage(vec![reason; self.claims.len()]))
            }
            Err(Stop::Budget) => Ok(Verdict::Coverage(vec![
                Some(Unchecked::Budget);
                self.claims.len()
            ])),
            Err(Stop::Failed(outcome)) => Err(outcome),
        }
    }
}

/// Atoms at the start of a plan that its fairness conjuncts own: those the
/// conjuncts before the negated property read.
fn fairness_atoms(plan: &Plan) -> usize {
    let nodes = plan.formula.nodes();
    let Some(Node::And(conjuncts)) = nodes.get(plan.formula.root()) else {
        return 0;
    };
    let Some((_, fairness)) = conjuncts.split_last() else {
        return 0;
    };
    fairness
        .iter()
        .flat_map(|&c| reachable_atoms(nodes, c))
        .max()
        .map_or(0, |max| max + 1)
}

/// The atom indices a node reads, sorted.
fn reachable_atoms(nodes: &[Node], root: usize) -> Vec<usize> {
    let mut seen = vec![false; nodes.len()];
    let mut stack = vec![root];
    let mut atoms = Vec::new();
    while let Some(n) = stack.pop() {
        if std::mem::replace(&mut seen[n], true) {
            continue;
        }
        match &nodes[n] {
            Node::Bool(_) => {}
            Node::Atom(atom) => atoms.push(atom.index),
            Node::Not(c) | Node::Always(c) | Node::Eventually(c) => stack.push(*c),
            Node::And(cs) | Node::Or(cs) => stack.extend(cs.iter().copied()),
        }
    }
    atoms.sort_unstable();
    atoms.dedup();
    atoms
}

/// A formula in negation-normal form, hash-consed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum F {
    Bool(bool),
    Lit(Atom, bool),
    And(Vec<usize>),
    Or(Vec<usize>),
    Always(usize),
    Eventually(usize),
}

#[derive(Default)]
struct Arena {
    nodes: Vec<F>,
    interned: HashMap<F, usize>,
    converted: HashMap<(usize, bool), usize>,
}

impl Arena {
    fn intern(&mut self, f: F) -> usize {
        if let Some(&id) = self.interned.get(&f) {
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(f.clone());
        self.interned.insert(f, id);
        id
    }

    fn junction(&mut self, conjunction: bool, items: Vec<usize>) -> usize {
        let mut flat = Vec::new();
        for item in items {
            match &self.nodes[item] {
                F::Bool(b) if *b == conjunction => {}
                F::Bool(_) => return self.intern(F::Bool(!conjunction)),
                F::And(cs) if conjunction => flat.extend(cs.iter().copied()),
                F::Or(cs) if !conjunction => flat.extend(cs.iter().copied()),
                _ => flat.push(item),
            }
        }
        let mut seen = std::collections::HashSet::new();
        flat.retain(|item| seen.insert(*item));
        match flat.len() {
            0 => self.intern(F::Bool(conjunction)),
            1 => flat[0],
            _ if conjunction => self.intern(F::And(flat)),
            _ => self.intern(F::Or(flat)),
        }
    }

    fn and(&mut self, items: Vec<usize>) -> usize {
        self.junction(true, items)
    }

    /// The negation-normal form of `node` (negated unless `positive`).
    fn nnf(&mut self, nodes: &[Node], node: usize, positive: bool) -> usize {
        if let Some(&id) = self.converted.get(&(node, positive)) {
            return id;
        }
        let id = match &nodes[node] {
            Node::Bool(b) => self.intern(F::Bool(*b == positive)),
            Node::Atom(atom) => self.intern(F::Lit(*atom, positive)),
            Node::Not(c) => self.nnf(nodes, *c, !positive),
            Node::And(cs) | Node::Or(cs) => {
                let conjunction = matches!(nodes[node], Node::And(_)) == positive;
                let items = cs.iter().map(|&c| self.nnf(nodes, c, positive)).collect();
                self.junction(conjunction, items)
            }
            Node::Always(c) | Node::Eventually(c) => {
                let always = matches!(nodes[node], Node::Always(_)) == positive;
                let child = self.nnf(nodes, *c, positive);
                match self.nodes[child] {
                    F::Bool(_) => child,
                    _ if always => self.intern(F::Always(child)),
                    _ => self.intern(F::Eventually(child)),
                }
            }
        };
        self.converted.insert((node, positive), id);
        id
    }

    fn temporal(&self, f: usize) -> bool {
        match &self.nodes[f] {
            F::Bool(_) | F::Lit(..) => false,
            F::Always(_) | F::Eventually(_) => true,
            F::And(cs) | F::Or(cs) => cs.iter().any(|&c| self.temporal(c)),
        }
    }

    /// `[]<>B` (`Some((true, B))`) or `<>[]B` (`Some((false, B))`), `B`
    /// free of temporal operators.
    fn acceptance_term(&self, f: usize) -> Option<(bool, usize)> {
        let (outer, recur) = match self.nodes[f] {
            F::Always(c) => (c, true),
            F::Eventually(c) => (c, false),
            _ => return None,
        };
        let inner = match (recur, &self.nodes[outer]) {
            (true, F::Eventually(b)) | (false, F::Always(b)) => *b,
            _ => return None,
        };
        (!self.temporal(inner)).then_some((recur, inner))
    }
}

/// One disjunct of an acceptance clause: `[]<>cond` when `recur`, else
/// `<>[]cond`.
#[derive(Clone, Copy)]
struct Term {
    recur: bool,
    cond: usize,
}

/// An automaton for a formula: acceptance clauses plus a tableau for the
/// rest. Conditions are nodes of `arena`.
struct Compiled {
    arena: Vec<F>,
    clauses: Vec<Vec<Term>>,
    tableau: Tableau,
}

impl Compiled {
    fn new(arena: &Arena, root: usize) -> Result<Self, String> {
        let conjuncts = match &arena.nodes[root] {
            F::And(cs) => cs.clone(),
            _ => vec![root],
        };
        let mut clauses = Vec::new();
        let mut residual = Vec::new();
        for conjunct in conjuncts {
            let disjuncts = match &arena.nodes[conjunct] {
                F::Or(cs) => cs.clone(),
                _ => vec![conjunct],
            };
            let terms: Option<Vec<Term>> = disjuncts
                .iter()
                .map(|&d| {
                    arena
                        .acceptance_term(d)
                        .map(|(recur, cond)| Term { recur, cond })
                })
                .collect();
            match terms {
                Some(terms) => clauses.push(terms),
                None => residual.push(conjunct),
            }
        }
        let tableau = if residual.is_empty() {
            Tableau {
                nodes: vec![TableauNode {
                    literals: Vec::new(),
                    successors: vec![0],
                    fulfilled: Vec::new(),
                }],
                roots: vec![0],
                promise_count: 0,
            }
        } else {
            let mut nodes = Vec::new();
            let mut exported = HashMap::new();
            let parts: Vec<usize> = residual
                .iter()
                .map(|&r| export(arena, r, &mut nodes, &mut exported))
                .collect();
            let root = if parts.len() == 1 {
                parts[0]
            } else {
                nodes.push(Node::And(parts));
                nodes.len() - 1
            };
            let formula = Formula::new(nodes, root).map_err(|e| e.to_string())?;
            compile(&formula, Limits::default()).map_err(|e| e.to_string())?
        };
        Ok(Self {
            arena: arena.nodes.clone(),
            clauses,
            tableau,
        })
    }
}

/// Writes an arena node as liveness formula nodes, children first.
fn export(
    arena: &Arena,
    f: usize,
    nodes: &mut Vec<Node>,
    exported: &mut HashMap<usize, usize>,
) -> usize {
    if let Some(&id) = exported.get(&f) {
        return id;
    }
    let node = match &arena.nodes[f] {
        F::Bool(b) => Node::Bool(*b),
        F::Lit(atom, true) => Node::Atom(*atom),
        F::Lit(atom, false) => {
            nodes.push(Node::Atom(*atom));
            Node::Not(nodes.len() - 1)
        }
        F::And(cs) => Node::And(
            cs.iter()
                .map(|&c| export(arena, c, nodes, exported))
                .collect(),
        ),
        F::Or(cs) => Node::Or(
            cs.iter()
                .map(|&c| export(arena, c, nodes, exported))
                .collect(),
        ),
        F::Always(c) => Node::Always(export(arena, *c, nodes, exported)),
        F::Eventually(c) => Node::Eventually(export(arena, *c, nodes, exported)),
    };
    nodes.push(node);
    let id = nodes.len() - 1;
    exported.insert(f, id);
    id
}

/// A trace graph: distinct states under exact equality, sampled steps and
/// a stuttering self-loop at every node.
#[derive(Default)]
pub(crate) struct TraceGraph {
    states: Vec<StateVals>,
    buckets: HashMap<u64, Vec<usize>>,
    /// `(from, to, action)`; a self-loop has no action.
    edges: Vec<(usize, usize, Option<usize>)>,
    edge_index: HashMap<(usize, usize), usize>,
    out: Vec<Vec<usize>>,
    /// The node of each trace position.
    path: Vec<usize>,
    /// The trace position whose step first reached an earlier state.
    closed_at: Option<usize>,
}

impl TraceGraph {
    /// Appends the next trace state, reached by `action` from the previous
    /// one. True when this step reached a state already in the trace.
    ///
    /// # Errors
    /// An incomparable value.
    pub(crate) fn push(
        &mut self,
        model: &Model,
        state: StateVals,
        action: Option<usize>,
    ) -> Result<bool, String> {
        let mut hasher = DefaultHasher::new();
        state.hash(&mut hasher);
        let key = hasher.finish();
        let mut found = None;
        for &candidate in self.buckets.get(&key).into_iter().flatten() {
            if same(model, &self.states[candidate], &state)? {
                found = Some(candidate);
                break;
            }
        }
        let node = if let Some(node) = found {
            node
        } else {
            let node = self.states.len();
            self.states.push(state);
            self.out.push(Vec::new());
            self.buckets.entry(key).or_default().push(node);
            self.edge(node, node, None);
            node
        };
        let closed = found.is_some() && !self.path.is_empty();
        if let Some(&previous) = self.path.last() {
            self.edge(previous, node, action);
        }
        if closed && self.closed_at.is_none() {
            self.closed_at = Some(self.path.len());
        }
        self.path.push(node);
        Ok(closed)
    }

    fn edge(&mut self, from: usize, to: usize, action: Option<usize>) {
        if self.edge_index.contains_key(&(from, to)) {
            return;
        }
        let id = self.edges.len();
        self.edges.push((from, to, action));
        self.edge_index.insert((from, to), id);
        self.out[from].push(id);
    }

    /// The trace position of the step that first closed a loop, if any.
    pub(crate) const fn closed_at(&self) -> Option<usize> {
        self.closed_at
    }
}

fn same(model: &Model, a: &StateVals, b: &StateVals) -> Result<bool, String> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (left, right) in a.iter().zip(b.iter()) {
        match (left, right) {
            (Some(left), Some(right)) => {
                if !model
                    .evaluator
                    .equal_values(left, right)
                    .map_err(|error| format!("simulation trace state equality: {error}"))?
                {
                    return Ok(false);
                }
            }
            (None, None) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// A product node: trace-graph node and tableau node.
type Pn = (usize, usize);

/// A product graph, built breadth-first from its initial nodes.
struct Product {
    nodes: Vec<Pn>,
    /// `(from, to, trace-graph edge)`.
    edges: Vec<(usize, usize, usize)>,
    out: Vec<Vec<usize>>,
    initial: Vec<usize>,
}

/// An accepting part of a product: its edges and the waypoints a cycle
/// through it must visit.
struct Accepting {
    allowed: Vec<bool>,
    nodes: Vec<usize>,
    waypoints: Vec<Waypoint>,
}

#[derive(Clone, Copy)]
enum Waypoint {
    Node(usize),
    Edge(usize),
}

/// One trace's check: cached atom values and the work spent.
struct Cx<'a> {
    model: &'a Model,
    checker: &'a Checker,
    graph: &'a TraceGraph,
    /// `(owner, atom, node)`.
    state: HashMap<(usize, usize, usize), bool>,
    /// `(owner, atom, edge)`.
    action: HashMap<(usize, usize, usize), bool>,
    work: u64,
    budget: u64,
}

impl Cx<'_> {
    fn charge(&mut self, units: u64) -> Result<(), Stop> {
        self.work = self.work.saturating_add(units);
        if self.work > self.budget {
            Err(Stop::Budget)
        } else {
            Ok(())
        }
    }

    /// Claims are searched in configuration order; the first word found is
    /// the counterexample. Then, in the default profile, finite refutations.
    fn run(&mut self, actions: &[usize]) -> Result<Option<Word>, Stop> {
        let checker = self.checker;
        for (index, claim) in checker.claims.iter().enumerate() {
            if let Some(found) = self.search(&claim.violation, Some(index), true)? {
                let (sequence, back) = found;
                return self.word(&sequence, back, index).map(Some);
            }
        }
        if checker.bad_prefix {
            for (index, claim) in checker.claims.iter().enumerate() {
                if let Some(length) = self.refutation(index)? {
                    return Ok(Some(Word {
                        states: self.graph.path[..length]
                            .iter()
                            .map(|&n| self.graph.states[n].clone())
                            .collect(),
                        actions: actions[..length - 1].to_vec(),
                        ending: TemporalEnding::BadPrefix(claim.name.clone()),
                        properties: Vec::new(),
                    }));
                }
            }
        }
        Ok(None)
    }

    /// The owner of `atom` in claim `claim`'s plan (`None`: the fairness
    /// automaton, over the first claim's atoms).
    fn owner(&self, claim: Option<usize>, atom: usize) -> (usize, usize) {
        match claim {
            Some(index) if atom >= self.checker.shared_atoms => (index + 1, index),
            Some(_) | None => (0, 0),
        }
    }

    fn atom(
        &mut self,
        claim: Option<usize>,
        atom: Atom,
        edge: usize,
        expensive: bool,
    ) -> Result<Option<bool>, Stop> {
        let (owner, plan) = self.owner(claim, atom.index);
        let (from, to, _) = self.graph.edges[edge];
        let key = if atom.kind == AtomKind::State {
            (owner, atom.index, from)
        } else {
            (owner, atom.index, edge)
        };
        let cache = if atom.kind == AtomKind::State {
            &self.state
        } else {
            &self.action
        };
        if let Some(&value) = cache.get(&key) {
            return Ok(Some(value));
        }
        let predicate = &self.checker.claims[plan].plan.atoms[atom.index];
        let costly = matches!(
            predicate.test,
            AtomTest::Enabled { .. } | AtomTest::NegatedEnabled { .. }
        );
        if costly && !expensive {
            return Ok(None);
        }
        self.charge(if costly { ENABLED_COST } else { 1 })?;
        let states = &self.graph.states;
        let st = if atom.kind == AtomKind::State {
            St::state(states[from].clone())
        } else {
            St::action(states[from].clone(), states[to].clone())
        };
        let value = predicate.evaluate(self.model, &st).map_err(|error| {
            Stop::Failed(if error.interruption().is_some() {
                Outcome::Cancelled
            } else {
                Outcome::Unsupported(format!(
                    "simulation temporal predicate evaluation: {}",
                    error.tool_text()
                ))
            })
        })?;
        if atom.kind == AtomKind::State {
            self.state.insert(key, value);
        } else {
            self.action.insert(key, value);
        }
        Ok(Some(value))
    }

    /// A condition's truth on a trace-graph edge, evaluating `ENABLED`
    /// only when the cheaper atoms leave it undecided.
    fn condition(
        &mut self,
        claim: Option<usize>,
        arena: &[F],
        f: usize,
        edge: usize,
    ) -> Result<bool, Stop> {
        if let Some(value) = self.kleene(claim, arena, f, edge, false)? {
            return Ok(value);
        }
        Ok(self.kleene(claim, arena, f, edge, true)?.unwrap_or(false))
    }

    fn kleene(
        &mut self,
        claim: Option<usize>,
        arena: &[F],
        f: usize,
        edge: usize,
        expensive: bool,
    ) -> Result<Option<bool>, Stop> {
        Ok(match &arena[f] {
            F::Bool(b) => Some(*b),
            F::Lit(atom, positive) => self
                .atom(claim, *atom, edge, expensive)?
                .map(|v| v == *positive),
            F::And(cs) | F::Or(cs) => {
                let conjunction = matches!(arena[f], F::And(_));
                let mut unknown = false;
                for &c in cs {
                    match self.kleene(claim, arena, c, edge, expensive)? {
                        Some(v) if v != conjunction => return Ok(Some(v)),
                        Some(_) => {}
                        None => unknown = true,
                    }
                }
                (!unknown).then_some(conjunction)
            }
            F::Always(_) | F::Eventually(_) => {
                return Err(Stop::Failed(Outcome::Internal(
                    "temporal operator in a simulation acceptance condition".into(),
                )));
            }
        })
    }

    fn literals(
        &mut self,
        claim: Option<usize>,
        literals: &[Literal],
        kind: AtomKind,
        edge: usize,
    ) -> Result<bool, Stop> {
        for literal in literals.iter().filter(|l| l.atom.kind == kind) {
            if self.atom(claim, literal.atom, edge, true)? != Some(literal.positive) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn self_loop(&self, node: usize) -> usize {
        self.graph.edge_index[&(node, node)]
    }

    fn product(&mut self, automaton: &Compiled, claim: Option<usize>) -> Result<Product, Stop> {
        let tableau = &automaton.tableau;
        let mut product = Product {
            nodes: Vec::new(),
            edges: Vec::new(),
            out: Vec::new(),
            initial: Vec::new(),
        };
        let mut index: HashMap<Pn, usize> = HashMap::new();
        let start = self.graph.path[0];
        let mut queue = std::collections::VecDeque::new();
        for &root in &tableau.roots {
            let at = self.self_loop(start);
            if self.literals(claim, &tableau.nodes[root].literals, AtomKind::State, at)? {
                let id = product.nodes.len();
                product.nodes.push((start, root));
                product.out.push(Vec::new());
                index.insert((start, root), id);
                product.initial.push(id);
                queue.push_back(id);
            }
        }
        while let Some(id) = queue.pop_front() {
            self.charge(1)?;
            let (v, t) = product.nodes[id];
            for &edge in &self.graph.out[v] {
                if !self.literals(claim, &tableau.nodes[t].literals, AtomKind::Action, edge)? {
                    continue;
                }
                let w = self.graph.edges[edge].1;
                let at = self.self_loop(w);
                for &next in &tableau.nodes[t].successors {
                    if !self.literals(claim, &tableau.nodes[next].literals, AtomKind::State, at)? {
                        continue;
                    }
                    self.charge(1)?;
                    let target = if let Some(&target) = index.get(&(w, next)) {
                        target
                    } else {
                        let target = product.nodes.len();
                        product.nodes.push((w, next));
                        product.out.push(Vec::new());
                        index.insert((w, next), target);
                        queue.push_back(target);
                        target
                    };
                    let e = product.edges.len();
                    product.edges.push((id, target, edge));
                    product.out[id].push(e);
                }
            }
        }
        Ok(product)
    }

    /// Searches for an accepting lasso. With `witness`, returns the word as
    /// trace-graph nodes and the index its loop returns to.
    fn search(
        &mut self,
        automaton: &Compiled,
        claim: Option<usize>,
        witness: bool,
    ) -> Result<Option<(Vec<usize>, usize)>, Stop> {
        let product = self.product(automaton, claim)?;
        if product.nodes.is_empty() {
            return Ok(None);
        }
        let allowed = vec![true; product.edges.len()];
        let satisfied = vec![false; automaton.clauses.len()];
        let Some(accepting) = self.accept(automaton, claim, &product, &allowed, &satisfied)? else {
            return Ok(None);
        };
        if !witness {
            return Ok(Some((Vec::new(), 0)));
        }
        Ok(Some(lasso(&product, &accepting)))
    }

    /// Emerson-Lei acceptance over the strongly connected parts of the
    /// product restricted to `allowed` edges.
    fn accept(
        &mut self,
        automaton: &Compiled,
        claim: Option<usize>,
        product: &Product,
        allowed: &[bool],
        satisfied: &[bool],
    ) -> Result<Option<Accepting>, Stop> {
        let promises = automaton.tableau.promise_count;
        for part in components(product, allowed) {
            self.charge(part.len() as u64)?;
            let mut member = vec![false; product.nodes.len()];
            for &n in &part {
                member[n] = true;
            }
            let internal: Vec<usize> = part
                .iter()
                .flat_map(|&n| product.out[n].iter().copied())
                .filter(|&e| allowed[e] && member[product.edges[e].1])
                .collect();
            let Some(&first) = internal.first() else {
                continue;
            };
            let mut waypoints = Vec::new();
            let mut fulfilled = true;
            for promise in 0..promises {
                let Some(&n) = part
                    .iter()
                    .find(|&&n| automaton.tableau.nodes[product.nodes[n].1].fulfilled[promise])
                else {
                    fulfilled = false;
                    break;
                };
                waypoints.push(Waypoint::Node(n));
            }
            if !fulfilled {
                continue;
            }
            let mut open = None;
            for (number, clause) in automaton.clauses.iter().enumerate() {
                if satisfied[number] {
                    continue;
                }
                let mut met = None;
                'terms: for term in clause.iter().filter(|t| t.recur) {
                    for &e in &internal {
                        self.charge(1)?;
                        if self.condition(claim, &automaton.arena, term.cond, product.edges[e].2)? {
                            met = Some(e);
                            break 'terms;
                        }
                    }
                }
                let Some(e) = met else {
                    open = Some(number);
                    break;
                };
                waypoints.push(Waypoint::Edge(e));
            }
            let Some(open) = open else {
                waypoints.push(Waypoint::Edge(first));
                let mut allowed_here = vec![false; product.edges.len()];
                for &e in &internal {
                    allowed_here[e] = true;
                }
                return Ok(Some(Accepting {
                    allowed: allowed_here,
                    nodes: part,
                    waypoints,
                }));
            };
            // The open clause can hold only through one of its `<>[]` terms:
            // keep the part's edges where that term's condition holds.
            for term in automaton.clauses[open].iter().filter(|t| !t.recur) {
                let mut restricted = vec![false; product.edges.len()];
                for &e in &internal {
                    self.charge(1)?;
                    restricted[e] =
                        self.condition(claim, &automaton.arena, term.cond, product.edges[e].2)?;
                }
                let mut now = satisfied.to_vec();
                now[open] = true;
                if let Some(found) = self.accept(automaton, claim, product, &restricted, &now)? {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }

    /// Compresses a found word, computes its false properties and builds
    /// the counterexample.
    fn word(&mut self, sequence: &[usize], back: usize, searched: usize) -> Result<Word, Stop> {
        // Remove consecutive repeats, cyclically within the loop.
        let mut states: Vec<usize> = Vec::new();
        let mut back_to = 0;
        for (position, &node) in sequence.iter().enumerate() {
            if states.last() != Some(&node) {
                states.push(node);
            }
            if position == back {
                back_to = states.len() - 1;
            }
        }
        while states.len() > back_to + 1 && states.last() == states.get(back_to) {
            states.pop();
        }
        let stuttering = states.len() == back_to + 1;
        let successor = |i: usize| if i + 1 < states.len() { i + 1 } else { back_to };
        let edges: Vec<usize> = (0..states.len())
            .map(|i| self.graph.edge_index[&(states[i], states[successor(i)])])
            .collect();
        let mut properties = Vec::new();
        let checker = self.checker;
        for (index, claim) in checker.claims.iter().enumerate() {
            let atoms = claim.plan.atoms.len();
            let mut state_rows = vec![vec![false; atoms]; states.len()];
            let mut action_rows = vec![vec![false; atoms]; states.len()];
            for &a in &claim.property_atoms {
                let atom = Atom {
                    kind: claim.plan.atoms[a].kind,
                    index: a,
                };
                for (position, &edge) in edges.iter().enumerate() {
                    let value = self.atom(Some(index), atom, edge, true)?.unwrap_or(false);
                    if atom.kind == AtomKind::State {
                        state_rows[position][a] = value;
                    } else {
                        action_rows[position][a] = value;
                    }
                }
            }
            let truth = claim
                .property
                .evaluate_word(&state_rows, &action_rows, back_to, WORD_CELLS)
                .map_err(|error| {
                    Stop::Failed(Outcome::Unsupported(format!(
                        "simulation counterexample attribution: {error}"
                    )))
                })?;
            if !truth[0] {
                properties.push(claim.name.clone());
            }
        }
        // Every TLA+ property is invariant under stuttering, so the
        // compressed word still violates the property it was found for.
        if !properties.contains(&checker.claims[searched].name) {
            return Err(Stop::Failed(Outcome::Unsupported(format!(
                "simulation counterexample for {} does not survive stuttering compression",
                checker.claims[searched].name
            ))));
        }
        let actions = (0..states.len() - 1)
            .filter_map(|i| self.graph.edges[edges[i]].2)
            .collect();
        Ok(Word {
            states: states
                .iter()
                .map(|&n| self.graph.states[n].clone())
                .collect(),
            actions,
            ending: if stuttering {
                TemporalEnding::Stuttering
            } else {
                TemporalEnding::Lasso { back_to }
            },
            properties,
        })
    }

    /// The length of the shortest prefix of the trace that no infinite
    /// extension can make satisfy claim `index`, if the trace has one.
    fn refutation(&mut self, index: usize) -> Result<Option<usize>, Stop> {
        let length = self.graph.path.len();
        if !self.refutes(index, length)? {
            return Ok(None);
        }
        // Refutation is monotone in the prefix.
        let (mut low, mut high) = (1, length);
        while low < high {
            let mid = low + (high - low) / 2;
            if self.refutes(index, mid)? {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        Ok(Some(high))
    }

    /// Three-valued truth of claim `index` on the first `length` trace
    /// states: false at the start means no extension satisfies it.
    fn refutes(&mut self, index: usize, length: usize) -> Result<bool, Stop> {
        let checker = self.checker;
        let (arena, top) = &checker.claims[index].positive;
        let mut memo: HashMap<usize, (Vec<bool>, Vec<bool>)> = HashMap::new();
        let (may, _) = self.prefix(index, arena, *top, length, &mut memo)?;
        Ok(!may[0])
    }

    fn prefix(
        &mut self,
        index: usize,
        arena: &Arena,
        f: usize,
        length: usize,
        memo: &mut HashMap<usize, (Vec<bool>, Vec<bool>)>,
    ) -> Result<(Vec<bool>, Vec<bool>), Stop> {
        if let Some(found) = memo.get(&f) {
            return Ok(found.clone());
        }
        self.charge(length as u64)?;
        let path = &self.graph.path;
        let result = match &arena.nodes[f] {
            F::Bool(b) => (vec![*b; length], vec![*b; length]),
            F::Lit(atom, positive) => {
                let mut may = vec![true; length];
                let mut must = vec![false; length];
                for i in 0..length {
                    let edge = if atom.kind == AtomKind::State {
                        self.graph.edge_index[&(path[i], path[i])]
                    } else if i + 1 < length {
                        self.graph.edge_index[&(path[i], path[i + 1])]
                    } else {
                        continue;
                    };
                    let value = self.atom(Some(index), *atom, edge, true)? == Some(*positive);
                    may[i] = value;
                    must[i] = value;
                }
                (may, must)
            }
            F::And(cs) | F::Or(cs) => {
                let conjunction = matches!(arena.nodes[f], F::And(_));
                let mut may = vec![conjunction; length];
                let mut must = vec![conjunction; length];
                for &c in cs {
                    let (m, n) = self.prefix(index, arena, c, length, memo)?;
                    for i in 0..length {
                        if conjunction {
                            may[i] &= m[i];
                            must[i] &= n[i];
                        } else {
                            may[i] |= m[i];
                            must[i] |= n[i];
                        }
                    }
                }
                (may, must)
            }
            F::Always(c) => {
                let (mut may, _) = self.prefix(index, arena, *c, length, memo)?;
                for i in (0..length.saturating_sub(1)).rev() {
                    may[i] = may[i] && may[i + 1];
                }
                (may, vec![false; length])
            }
            F::Eventually(c) => {
                let (_, mut must) = self.prefix(index, arena, *c, length, memo)?;
                for i in (0..length.saturating_sub(1)).rev() {
                    must[i] = must[i] || must[i + 1];
                }
                (vec![true; length], must)
            }
        };
        memo.insert(f, result.clone());
        Ok(result)
    }
}

/// Strongly connected parts of the product restricted to `allowed` edges,
/// in a deterministic order (Tarjan's algorithm, iteratively).
fn components(product: &Product, allowed: &[bool]) -> Vec<Vec<usize>> {
    let n = product.nodes.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut parts = Vec::new();
    let mut next = 0;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut frames: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (v, ref mut position)) = frames.last_mut() {
            if let Some(&e) = product.out[v].get(*position) {
                *position += 1;
                if !allowed[e] {
                    continue;
                }
                let w = product.edges[e].1;
                if index[w] == usize::MAX {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    frames.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut part = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        part.push(w);
                        if w == v {
                            break;
                        }
                    }
                    part.sort_unstable();
                    parts.push(part);
                }
            }
        }
    }
    parts.sort_by_key(|part| part[0]);
    parts
}

/// A breadth-first path of product edges from any of `from` to a node
/// satisfying `goal`, over `allowed` edges; the nodes after the start.
fn path_to(
    product: &Product,
    from: &[usize],
    allowed: &dyn Fn(usize) -> bool,
    goal: &dyn Fn(usize) -> bool,
) -> Option<(usize, Vec<usize>)> {
    let mut parent: HashMap<usize, Option<usize>> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for &s in from {
        if goal(s) {
            return Some((s, Vec::new()));
        }
        parent.insert(s, None);
        queue.push_back(s);
    }
    while let Some(v) = queue.pop_front() {
        for &e in &product.out[v] {
            let w = product.edges[e].1;
            if !allowed(e) || parent.contains_key(&w) {
                continue;
            }
            parent.insert(w, Some(v));
            if goal(w) {
                let mut nodes = vec![w];
                let mut at = v;
                while let Some(Some(p)) = parent.get(&at) {
                    nodes.push(at);
                    at = *p;
                }
                nodes.reverse();
                return Some((at, nodes));
            }
            queue.push_back(w);
        }
    }
    None
}

/// The lasso of an accepting part: a shortest prefix to the part, then a
/// cycle through every waypoint, as trace-graph nodes, and the index the
/// loop returns to.
fn lasso(product: &Product, accepting: &Accepting) -> (Vec<usize>, usize) {
    let mut member = vec![false; product.nodes.len()];
    for &n in &accepting.nodes {
        member[n] = true;
    }
    // Every product node was reached from an initial node.
    let (mut sequence, entry) = match path_to(product, &product.initial, &|_| true, &|n| member[n])
    {
        Some((start, nodes)) => {
            let mut sequence = vec![start];
            sequence.extend(nodes);
            let entry = sequence.pop().unwrap_or(start);
            (sequence, entry)
        }
        None => (Vec::new(), accepting.nodes[0]),
    };
    let inside = |e: usize| accepting.allowed[e];
    let mut cycle = vec![entry];
    let mut at = entry;
    for waypoint in &accepting.waypoints {
        let target = match *waypoint {
            Waypoint::Node(n) => n,
            Waypoint::Edge(e) => product.edges[e].0,
        };
        if let Some((_, nodes)) = path_to(product, &[at], &inside, &|n| n == target) {
            cycle.extend(nodes);
            at = target;
        }
        if let Waypoint::Edge(e) = *waypoint {
            at = product.edges[e].1;
            cycle.push(at);
        }
    }
    if at != entry
        && let Some((_, nodes)) = path_to(product, &[at], &inside, &|n| n == entry)
    {
        cycle.extend(nodes);
    }
    // The cycle ends where it started; the loop returns there.
    cycle.pop();
    let back = sequence.len();
    sequence.extend(cycle);
    (sequence.iter().map(|&n| product.nodes[n].0).collect(), back)
}
