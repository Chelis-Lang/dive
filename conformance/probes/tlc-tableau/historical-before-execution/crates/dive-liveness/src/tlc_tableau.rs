//! Bounded TLC particle tableaux and ordered promise projections.
//!
//! This diagnostic stage consumes the tagged normalization projection. It does
//! not capture behavior graphs, perform SCC checks or select counterexamples.
//! Node numbering and particle order follow the pinned TLC construction, while
//! every allocation/expansion is bounded without evaluating predicates.

use crate::formula::{Atom, AtomKind, Limits, Node};
use crate::tlc_plan::Projection;

/// A syntactically interned binary expression. Indices refer to the owning
/// tableau's checked arena. `Next` is TLC's internal particle obligation, not
/// an additional TLA+ operator accepted from source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    /// Boolean constant.
    Bool(bool),
    /// Original normalization occurrence tag (index plus one).
    Atom(Atom),
    /// Negated state atom or constant.
    Not(usize),
    /// Balanced binary conjunction, preserving child order.
    And(usize, usize),
    /// Balanced binary disjunction, preserving child order.
    Or(usize, usize),
    /// Always.
    Always(usize),
    /// Eventually; also a promise when reachable from the binary root.
    Eventually(usize),
    /// Obligation at the successor particle.
    Next(usize),
}

/// One first-encounter particle node in TLC numbering order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Particle {
    /// Ordered particle expressions, including any retained duplicates.
    pub terms: Vec<usize>,
    /// Constant/state-level expressions in particle encounter order.
    pub state_predicates: Vec<usize>,
    /// Ordered successor node indices, preserving duplicate edges.
    pub successors: Vec<usize>,
    /// TLC's `!member(promise) || member(promise.body)` for each promise.
    pub fulfilling: Vec<bool>,
    /// TLC's empty-particle/sole-self-loop special accepting node flag.
    /// This is not an accepting-run or model-checking verdict.
    pub accepting: bool,
}

/// An auditable bounded tableau, with expressions interned by tagged syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tableau {
    arena: Vec<Term>,
    /// Balanced binary temporal expression root.
    pub root: usize,
    /// Initial nodes occupy this prefix, in particle-closure order.
    pub initial_count: usize,
    /// Nodes and outgoing edges in pinned first-encounter order.
    pub particles: Vec<Particle>,
    /// Preorder syntactically unique eventualities, as arena indices.
    pub promises: Vec<usize>,
}
impl Tableau {
    /// Returns the checked expression arena. Child references precede parents.
    #[must_use]
    pub fn terms(&self) -> &[Term] {
        &self.arena
    }
}

/// Named structural/resource/action refusals, never an empty proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Invalid public projection reference or unsupported structural shape.
    Reference(String),
    /// Existing formula/node/work bound or named recursion-depth bound.
    Limit(&'static str),
    /// Action predicate encountered in a tableau formula.
    Action,
}
impl Error {
    /// Stable diagnostic code, distinct from model-checking verdicts.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Limit(_) => "DIVE-L0030",
            Self::Reference(_) => "DIVE-L0031",
            Self::Action => "DIVE-L0032",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reference(s) => write!(f, "invalid TLC tableau projection: {s}"),
            Self::Limit(s) => write!(f, "TLC tableau resource limit: {s}"),
            Self::Action => write!(f, "unsupported action-containing TLC tableau formula"),
        }
    }
}
impl std::error::Error for Error {}

struct Meter {
    limits: Limits,
    terms: usize,
    nodes: usize,
    work: usize,
    held: usize,
}
impl Meter {
    fn step(&mut self, count: usize) -> Result<(), Error> {
        self.work = self
            .work
            .checked_add(count)
            .ok_or(Error::Limit("TLC tableau work"))?;
        if self.work > self.limits.expansion_steps {
            return Err(Error::Limit("TLC tableau work"));
        }
        Ok(())
    }
    fn depth(&mut self, depth: usize) -> Result<(), Error> {
        if depth > 256 {
            return Err(Error::Limit("TLC tableau closure depth"));
        }
        self.step(1)
    }
    fn pending(&self, count: usize) -> Result<(), Error> {
        self.held
            .checked_add(count)
            .filter(|n| *n <= self.limits.tableau_nodes)
            .ok_or(Error::Limit("TLC tableau pending particles"))?;
        Ok(())
    }
    fn node(&mut self) -> Result<(), Error> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .filter(|n| *n <= self.limits.tableau_nodes)
            .ok_or(Error::Limit("TLC tableau nodes"))?;
        Ok(())
    }
}
type Triples = Vec<[usize; 3]>;

struct Build<'a> {
    arena: Vec<Term>,
    meter: &'a mut Meter,
}
impl Build<'_> {
    fn intern(&mut self, term: Term) -> Result<usize, Error> {
        for (index, other) in self.arena.iter().enumerate() {
            self.meter.step(1)?;
            if *other == term {
                return Ok(index);
            }
        }
        let refs = match term {
            Term::Bool(_) | Term::Atom(_) => 0,
            Term::And(..) | Term::Or(..) => 2,
            _ => 1,
        };
        self.meter.terms = self
            .meter
            .terms
            .checked_add(1 + refs)
            .filter(|n| *n <= self.meter.limits.formula_nodes)
            .ok_or(Error::Limit("TLC tableau expression terms"))?;
        self.meter.step(1 + refs)?;
        let index = self.arena.len();
        self.arena.push(term);
        Ok(index)
    }
    fn copy(&mut self, terms: &[usize]) -> Result<Vec<usize>, Error> {
        if terms.len() > self.meter.limits.formula_nodes {
            return Err(Error::Limit("TLC tableau vector terms"));
        }
        self.meter.step(terms.len().saturating_add(1))?;
        Ok(terms.to_vec())
    }
    fn source_junction(
        &mut self,
        source: &[Node],
        and: bool,
        cs: &[usize],
        depth: usize,
    ) -> Result<usize, Error> {
        self.meter.depth(depth)?;
        if cs.is_empty() {
            return Err(Error::Reference("empty binary junction".into()));
        }
        if cs.len() == 1 {
            return self.import(source, cs[0], depth + 1);
        }
        // makeBinary splits the original child list first, then addConj/addDisj
        // flatten same-kind children separately in the left and right halves.
        let mid = cs.len() / 2;
        let left = self.flatten_half(source, and, &cs[..mid])?;
        let left = self.binary_parts(source, and, &left, depth + 1)?;
        let right = self.flatten_half(source, and, &cs[mid..])?;
        let right = self.binary_parts(source, and, &right, depth + 1)?;
        self.intern(if and {
            Term::And(left, right)
        } else {
            Term::Or(left, right)
        })
    }
    fn flatten_half(
        &mut self,
        source: &[Node],
        and: bool,
        cs: &[usize],
    ) -> Result<Vec<usize>, Error> {
        let mut flat = Vec::new();
        let mut pending = self.copy(cs)?;
        pending.reverse();
        while let Some(i) = pending.pop() {
            self.meter.step(1)?;
            let n = source
                .get(i)
                .ok_or_else(|| Error::Reference(format!("source term {i}")))?;
            if let Node::And(ds) | Node::Or(ds) = n
                && matches!(n, Node::And(_)) == and
            {
                let width = flat
                    .len()
                    .saturating_add(pending.len())
                    .saturating_add(ds.len());
                if width > self.meter.limits.formula_nodes {
                    return Err(Error::Limit("TLC tableau junction terms"));
                }
                self.meter.step(ds.len().saturating_add(1))?;
                pending.extend(ds.iter().rev().copied());
            } else {
                flat.push(i);
            }
        }
        Ok(flat)
    }
    fn binary_parts(
        &mut self,
        source: &[Node],
        and: bool,
        cs: &[usize],
        depth: usize,
    ) -> Result<usize, Error> {
        self.meter.depth(depth)?;
        if cs.len() == 1 {
            return self.import(source, cs[0], depth + 1);
        }
        if cs.is_empty() {
            return Err(Error::Reference("empty binary junction".into()));
        }
        let mid = cs.len() / 2;
        let left = self.binary_parts(source, and, &cs[..mid], depth + 1)?;
        let right = self.binary_parts(source, and, &cs[mid..], depth + 1)?;
        self.intern(if and {
            Term::And(left, right)
        } else {
            Term::Or(left, right)
        })
    }
    fn import(&mut self, source: &[Node], i: usize, depth: usize) -> Result<usize, Error> {
        self.meter.depth(depth)?;
        let node = source
            .get(i)
            .ok_or_else(|| Error::Reference(format!("source term {i}")))?;
        let term = match node {
            Node::Bool(b) => Term::Bool(*b),
            Node::Atom(a) => {
                if a.kind == AtomKind::Action {
                    return Err(Error::Action);
                }
                Term::Atom(*a)
            }
            Node::Not(c) => {
                if !matches!(source.get(*c), Some(Node::Atom(_) | Node::Bool(_))) {
                    return Err(Error::Reference("negation outside positive form".into()));
                }
                Term::Not(self.import(source, *c, depth + 1)?)
            }
            Node::Always(c) => Term::Always(self.import(source, *c, depth + 1)?),
            Node::Eventually(c) => Term::Eventually(self.import(source, *c, depth + 1)?),
            Node::And(cs) | Node::Or(cs) => {
                return self.source_junction(source, matches!(node, Node::And(_)), cs, depth + 1);
            }
        };
        self.intern(term)
    }
    fn level(&mut self, i: usize, depth: usize) -> Result<u8, Error> {
        self.meter.depth(depth)?;
        Ok(match self.arena[i] {
            Term::Bool(_) => 0,
            Term::Atom(_) => 1,
            Term::Next(_) => 2,
            Term::Always(_) | Term::Eventually(_) => 3,
            Term::Not(c) => self.level(c, depth + 1)?,
            Term::And(a, b) | Term::Or(a, b) => {
                self.level(a, depth + 1)?.max(self.level(b, depth + 1)?)
            }
        })
    }
    fn member(&mut self, terms: &[usize], i: usize) -> Result<bool, Error> {
        self.meter.step(terms.len().saturating_add(1))?;
        Ok(terms.contains(&i))
    }
    fn equal(&mut self, a: &[usize], b: &[usize]) -> Result<bool, Error> {
        for i in a {
            if !self.member(b, *i)? {
                return Ok(false);
            }
        }
        for i in b {
            if !self.member(a, *i)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn append(&mut self, terms: &mut Vec<usize>, i: usize) -> Result<(), Error> {
        self.meter.step(1)?;
        if terms.len() >= self.meter.limits.formula_nodes {
            return Err(Error::Limit("TLC tableau particle terms"));
        }
        terms.push(i);
        Ok(())
    }
    fn consistent(&mut self, terms: &[usize]) -> Result<bool, Error> {
        for i in terms {
            self.meter.step(1)?;
            if let Term::Not(c) = self.arena[*i]
                && matches!(self.arena[c], Term::Atom(_) | Term::Bool(_))
                && self.member(terms, c)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn triples(&mut self, terms: &[usize]) -> Result<(Triples, Triples), Error> {
        let mut pending = self.copy(terms)?;
        let mut positive = Vec::new();
        while let Some(i) = pending.pop() {
            self.meter.step(1)?;
            match self.arena[i] {
                Term::Not(c) => self.append(&mut pending, c)?,
                Term::Next(c) => {
                    self.append(&mut positive, i)?;
                    self.append(&mut pending, c)?;
                }
                Term::Always(c) | Term::Eventually(c) => {
                    self.append(&mut positive, i)?;
                    let next = self.intern(Term::Next(i))?;
                    self.append(&mut positive, next)?;
                    self.append(&mut pending, c)?;
                }
                Term::And(a, b) | Term::Or(a, b) => {
                    self.append(&mut pending, a)?;
                    self.append(&mut pending, b)?;
                    self.append(&mut positive, i)?;
                }
                Term::Bool(_) | Term::Atom(_) => self.append(&mut positive, i)?,
            }
        }
        let (mut alpha, mut beta) = (Vec::new(), Vec::new());
        for i in positive {
            self.meter.step(1)?;
            match self.arena[i] {
                Term::Always(c) => alpha.push([i, c, self.intern(Term::Next(i))?]),
                Term::And(a, b) => alpha.push([i, a, b]),
                Term::Eventually(c) => beta.push([i, c, self.intern(Term::Next(i))?]),
                Term::Or(a, b) => beta.push([i, a, b]),
                _ => {}
            }
        }
        Ok((alpha, beta))
    }
    fn closure(
        &mut self,
        mut terms: Vec<usize>,
        alpha: &[[usize; 3]],
        beta: &[[usize; 3]],
        depth: usize,
    ) -> Result<Vec<Vec<usize>>, Error> {
        self.meter.depth(depth)?;
        if !self.consistent(&terms)? {
            return Ok(Vec::new());
        }
        let original = terms.len();
        let mut pos = 0;
        while pos < terms.len() {
            let i = terms[pos];
            pos += 1;
            self.meter.step(1)?;
            let pair = match self.arena[i] {
                Term::Always(c) => Some([c, self.intern(Term::Next(i))?]),
                Term::And(a, b) => Some([a, b]),
                _ => None,
            };
            if let Some([a, b]) = pair {
                let has_a = self.member(&terms, a)?;
                let has_b = self.member(&terms, b)?;
                if !has_a {
                    self.append(&mut terms, a)?;
                }
                if !has_b {
                    self.append(&mut terms, b)?;
                }
            }
        }
        loop {
            let mut changed = false;
            for &[a, b, c] in alpha {
                if self.member(&terms, b)? && self.member(&terms, c)? && !self.member(&terms, a)? {
                    self.append(&mut terms, a)?;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if terms.len() > original && !self.consistent(&terms)? {
            return Ok(Vec::new());
        }
        for &i in &terms {
            let pair = match self.arena[i] {
                Term::Eventually(c) => Some([c, self.intern(Term::Next(i))?]),
                Term::Or(a, b) => Some([a, b]),
                _ => None,
            };
            if let Some([a, b]) = pair
                && !self.member(&terms, a)?
                && !self.member(&terms, b)?
            {
                let mut first = self.copy(&terms)?;
                self.append(&mut first, a)?;
                let mut left = self.closure(first, alpha, beta, depth + 1)?;
                self.meter.pending(left.len())?;
                self.meter.held += left.len();
                let right_result = (|| {
                    let mut second = self.copy(&terms)?;
                    self.append(&mut second, b)?;
                    self.closure(second, alpha, beta, depth + 1)
                })();
                self.meter.held -= left.len();
                let right = right_result?;
                // TBParVec.union drops left representatives found on the right,
                // then retains every right representative in its encounter order.
                let mut retained = Vec::new();
                for particle in left.drain(..) {
                    let mut found = false;
                    for other in &right {
                        if self.equal(&particle, other)? {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        retained.push(particle);
                    }
                }
                self.meter
                    .pending(retained.len().saturating_add(right.len()))?;
                self.meter
                    .step(retained.len().saturating_add(right.len()).saturating_add(1))?;
                retained.extend(right);
                return Ok(retained);
            }
        }
        for &[a, b, c] in beta {
            if (self.member(&terms, b)? || self.member(&terms, c)?) && !self.member(&terms, a)? {
                self.append(&mut terms, a)?;
                return self.closure(terms, alpha, beta, depth + 1);
            }
        }
        self.meter.pending(1)?;
        Ok(vec![terms])
    }
    fn particles(&mut self, terms: Vec<usize>) -> Result<Vec<Vec<usize>>, Error> {
        let (alpha, beta) = self.triples(&terms)?;
        self.closure(terms, &alpha, &beta, 0)
    }
    fn promises(&mut self, i: usize, out: &mut Vec<usize>, depth: usize) -> Result<(), Error> {
        self.meter.depth(depth)?;
        if matches!(self.arena[i], Term::Eventually(_)) && !self.member(out, i)? {
            self.append(out, i)?;
        }
        match self.arena[i] {
            Term::Not(c) | Term::Always(c) | Term::Eventually(c) | Term::Next(c) => {
                self.promises(c, out, depth + 1)?;
            }
            Term::And(a, b) | Term::Or(a, b) => {
                self.promises(a, out, depth + 1)?;
                self.promises(b, out, depth + 1)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn graph(mut self, root: usize) -> Result<Tableau, Error> {
        let initial = self.particles(vec![root])?;
        let initial_count = initial.len();
        let mut particles = Vec::new();
        for terms in initial {
            self.meter.node()?;
            particles.push(Particle {
                terms,
                state_predicates: Vec::new(),
                successors: Vec::new(),
                fulfilling: Vec::new(),
                accepting: false,
            });
        }
        let mut pos = 0;
        while pos < particles.len() {
            let mut successor_terms = Vec::new();
            for &i in &particles[pos].terms {
                self.meter.step(1)?;
                if let Term::Next(c) = self.arena[i] {
                    self.append(&mut successor_terms, c)?;
                }
            }
            for terms in self.particles(successor_terms)? {
                let mut target = None;
                for (index, node) in particles.iter().enumerate() {
                    if self.equal(&node.terms, &terms)? {
                        target = Some(index);
                        break;
                    }
                }
                let index = if let Some(index) = target {
                    index
                } else {
                    self.meter.node()?;
                    let index = particles.len();
                    particles.push(Particle {
                        terms,
                        state_predicates: Vec::new(),
                        successors: Vec::new(),
                        fulfilling: Vec::new(),
                        accepting: false,
                    });
                    index
                };
                self.meter.step(1)?;
                particles[pos].successors.push(index);
            }
            pos += 1;
        }
        let mut promises = Vec::new();
        self.promises(root, &mut promises, 0)?;
        for (index, node) in particles.iter_mut().enumerate() {
            for &i in &node.terms {
                if self.level(i, 0)? <= 1 {
                    self.append(&mut node.state_predicates, i)?;
                }
            }
            self.meter.step(promises.len().saturating_add(1))?;
            for &p in &promises {
                let Term::Eventually(body) = self.arena[p] else {
                    return Err(Error::Reference("non-eventual promise".into()));
                };
                node.fulfilling
                    .push(!self.member(&node.terms, p)? || self.member(&node.terms, body)?);
            }
            node.accepting = node.terms.is_empty() && node.successors == [index];
        }
        Ok(Tableau {
            arena: self.arena,
            root,
            initial_count,
            particles,
            promises,
        })
    }
}

/// Projects binary tableaux and promise identities for all ordered OOS groups.
/// A null temporal group yields `None`, distinct from an empty tableau. Limits
/// apply cumulatively across groups. This is a planning artifact, never proof
/// of a temporal property or permission to execute strict integrated liveness.
///
/// # Errors
/// Named term/node/pending/work/depth limits, action-containing formulas,
/// invalid edited group indices and malformed positive/binary structure.
pub fn project(input: &Projection, limits: Limits) -> Result<Vec<Option<Tableau>>, Error> {
    let mut meter = Meter {
        limits,
        terms: 0,
        nodes: 0,
        work: 0,
        held: 0,
    };
    if input.orders.len() > limits.formula_nodes {
        return Err(Error::Limit("TLC tableau group count"));
    }
    input.nodes().iter().try_fold(0usize, |count, node| {
        let references = match node {
            Node::And(cs) | Node::Or(cs) => cs.len(),
            Node::Not(_) | Node::Always(_) | Node::Eventually(_) => 1,
            Node::Bool(_) | Node::Atom(_) => 0,
        };
        meter.step(references.saturating_add(1))?;
        count
            .checked_add(references.saturating_add(1))
            .filter(|n| *n <= limits.formula_nodes)
            .ok_or(Error::Limit("TLC tableau input terms"))
    })?;
    for node in input.nodes() {
        if let Node::Atom(a) = node {
            let occurrence = input
                .occurrences
                .get(a.index)
                .ok_or_else(|| Error::Reference(format!("occurrence {}", a.index)))?;
            if occurrence.atom.kind != a.kind {
                return Err(Error::Reference("occurrence domain mismatch".into()));
            }
        }
    }
    for group in &input.orders {
        for &i in group
            .temporal
            .iter()
            .chain(&group.check_state)
            .chain(&group.check_action)
        {
            meter.step(1)?;
            if input.nodes().get(i).is_none() {
                return Err(Error::Reference(format!("group expression {i}")));
            }
        }
        for pem in &group.pems {
            for (&i, len) in pem
                .ae_action
                .iter()
                .chain(&pem.ea_action)
                .map(|i| (i, group.check_action.len()))
                .chain(pem.ae_state.iter().map(|i| (i, group.check_state.len())))
            {
                meter.step(1)?;
                if i >= len {
                    return Err(Error::Reference(format!("PEM bin index {i}")));
                }
            }
        }
    }
    let mut out = Vec::with_capacity(input.orders.len());
    for group in &input.orders {
        meter.step(1)?;
        if group.temporal.is_empty() {
            out.push(None);
            continue;
        }
        let mut build = Build {
            arena: Vec::new(),
            meter: &mut meter,
        };
        let root = build.source_junction(input.nodes(), true, &group.temporal, 0)?;
        out.push(Some(build.graph(root)?));
    }
    Ok(out)
}

/// Projects one caller-supplied, already tagged positive-form state formula.
/// Atom identities are retained rather than assigned or merged by value. This
/// diagnostic entry point is useful for particle-closure controls; bound model
/// callers should first use the conservative `tlc_plan::lower_model` entry point.
/// It never evaluates a supplied predicate or produces a checking verdict.
///
/// # Errors
/// Structural, action and named resource bounds, as for [`project`].
pub fn project_formula(input: &crate::formula::Formula, limits: Limits) -> Result<Tableau, Error> {
    if input.terms() > limits.formula_nodes {
        return Err(Error::Limit("TLC tableau input terms"));
    }
    let mut meter = Meter {
        limits,
        terms: 0,
        nodes: 0,
        work: 0,
        held: 0,
    };
    let mut build = Build {
        arena: Vec::new(),
        meter: &mut meter,
    };
    let root = build.import(input.nodes(), input.root(), 0)?;
    build.graph(root)
}
