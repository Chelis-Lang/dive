//! Level checking: a port of the `levelCheck` methods of SANY's semantic
//! nodes (`LevelNode`, `OpApplNode`, `OpDefNode`, `ThmOrAssumpDefNode`,
//! `ModuleNode`, `SubstInNode`, `APSubstInNode`, `Subst`, `InstanceNode`,
//! `LetInNode`, `LabelNode`, `AtNode`, `OpArgNode`, `AssumeProveNode`,
//! `NewSymbNode`, `TheoremNode`, `AssumeNode`, the proof nodes), including
//! the special checks of `OpApplNode.levelCheck` (`[]A`, `<>A`, `~>`,
//! mixed action/temporal junctions, temporal quantifier bounds).
//!
//! Levels are 0 (constant), 1 (state), 2 (action), 3 (temporal). Each node
//! keeps SANY's level data (see [`crate::model::LevelData`]); recursive
//! definitions are checked with SANY's iteration numbers.
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    reason = "functions mirror SANY's levelCheck methods one to one, with SANY's names"
)]

use crate::diagnostics::{Code, Diagnostic};
use crate::model::{
    ArgLevelParam, DefKind, Kind, LevelData, Model, NodeId, NodeKind, OpLevels, Subst,
};
use std::collections::{BTreeMap, BTreeSet};

type Set = BTreeSet<NodeId>;
type Lc = BTreeMap<NodeId, u8>;
type Alc = BTreeMap<(NodeId, u32), u8>;
type Alp = BTreeSet<ArgLevelParam>;

/// SANY's level of a node after level checking (`getLevel`).
pub(crate) fn level_of(m: &Model, id: NodeId) -> u8 {
    match &m.node(id).kind {
        NodeKind::Label(d) => d.body.map_or(0, |b| level_of(m, b)),
        NodeKind::Assume(d) => d.expr.map_or(0, |e| level_of(m, e)),
        NodeKind::Instance(_) => 0,
        _ => m.node(id).level.level,
    }
}

/// `SetOfLevelConstraints.put`: keeps the minimum.
fn lc_put(lc: &mut Lc, k: NodeId, v: u8) {
    let e = lc.entry(k).or_insert(3);
    *e = (*e).min(v);
}

/// `SetOfArgLevelConstraints.put`: keeps the maximum.
fn alc_put(alc: &mut Alc, k: (NodeId, u32), v: u8) {
    let e = alc.entry(k).or_insert(0);
    *e = (*e).max(v);
}

fn u(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

pub(crate) struct Lv<'a> {
    pub(crate) m: &'a mut Model,
    pub(crate) diags: &'a mut Vec<Diagnostic>,
    pub(crate) errors: &'a mut usize,
}

impl Lv<'_> {
    fn err(&mut self, code: Code, at: NodeId, msg: impl Into<String>) {
        let span = self.m.node(at).span;
        *self.errors += 1;
        self.diags.push(Diagnostic::new(code, span, msg));
    }

    fn kind(&self, id: NodeId) -> Kind {
        self.m.kind(id)
    }

    fn data(&self, id: NodeId) -> &LevelData {
        &self.m.node(id).level
    }

    fn data_mut(&mut self, id: NodeId) -> Option<&mut LevelData> {
        self.m.node_mut(id).map(|n| &mut n.level)
    }

    fn name(&self, id: NodeId) -> String {
        self.m.symbol_name(id).unwrap_or_default().to_owned()
    }

    /// The node whose level data a getter returns (`LabelNode` and
    /// `AssumeNode` forward to their bodies).
    fn target(&self, id: NodeId) -> NodeId {
        match &self.m.node(id).kind {
            NodeKind::Label(d) => d.body.map_or(id, |b| self.target(b)),
            NodeKind::Assume(d) => d.expr.map_or(id, |e| self.target(e)),
            _ => id,
        }
    }

    fn level(&self, id: NodeId) -> u8 {
        level_of(self.m, id)
    }

    fn lp(&self, id: NodeId) -> Set {
        self.data(self.target(id)).level_params.clone()
    }

    fn ap(&self, id: NodeId) -> Set {
        self.data(self.target(id)).all_params.clone()
    }

    fn nlp(&self, id: NodeId) -> Set {
        match &self.m.node(id).kind {
            NodeKind::Assume(_) => self.data(self.target(id)).non_leibniz_params.clone(),
            _ => self.data(id).non_leibniz_params.clone(),
        }
    }

    fn lc(&self, id: NodeId) -> Lc {
        self.data(self.target(id)).level_constraints.clone()
    }

    fn alc(&self, id: NodeId) -> Alc {
        self.data(self.target(id)).arg_level_constraints.clone()
    }

    fn alp(&self, id: NodeId) -> Alp {
        self.data(self.target(id)).arg_level_params.clone()
    }

    fn arity(&self, id: NodeId) -> i32 {
        match &self.m.node(id).kind {
            NodeKind::OpDecl(d) => d.arity,
            NodeKind::FormalParam(d) => d.arity,
            NodeKind::OpDef(d) => d.arity,
            NodeKind::ThmOrAssumpDef(d) => i32::try_from(d.params.len()).unwrap_or(0),
            NodeKind::OpArg(d) => d.arity,
            _ => 0,
        }
    }

    fn is_any_def(&self, id: NodeId) -> bool {
        matches!(
            self.m.node(id).kind,
            NodeKind::OpDef(_) | NodeKind::ThmOrAssumpDef(_)
        )
    }

    /// `SymbolNode.isParam`.
    fn is_param(&self, id: NodeId) -> bool {
        matches!(
            self.m.node(id).kind,
            NodeKind::OpDecl(_) | NodeKind::FormalParam(_)
        )
    }

    fn params(&self, id: NodeId) -> Vec<NodeId> {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => d.params.clone(),
            NodeKind::ThmOrAssumpDef(d) => d.params.clone(),
            _ => Vec::new(),
        }
    }

    fn levels(&self, id: NodeId) -> Option<&OpLevels> {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => Some(&d.lv),
            NodeKind::ThmOrAssumpDef(d) => Some(&d.lv),
            _ => None,
        }
    }

    fn levels_mut(&mut self, id: NodeId) -> Option<&mut OpLevels> {
        match self.m.node_mut(id).map(|n| &mut n.kind) {
            Some(NodeKind::OpDef(d)) => Some(&mut d.lv),
            Some(NodeKind::ThmOrAssumpDef(d)) => Some(&mut d.lv),
            _ => None,
        }
    }

    fn arg_index(&self, op: NodeId, i: usize) -> usize {
        if self.arity(op) == -1 { 0 } else { i }
    }

    /// `getMaxLevel`.
    fn max_level(&self, op: NodeId, i: usize) -> u8 {
        let idx = self.arg_index(op, i);
        self.levels(op)
            .and_then(|l| l.max_levels.get(idx).copied())
            .unwrap_or(3)
    }

    /// `getWeight`.
    fn weight(&self, op: NodeId, i: usize) -> u8 {
        let idx = self.arg_index(op, i);
        self.levels(op)
            .and_then(|l| l.weights.get(idx).copied())
            .unwrap_or(0)
    }

    /// `getMinMaxLevel`.
    fn min_max_level(&self, op: NodeId, i: usize, j: usize) -> u8 {
        self.levels(op)
            .and_then(|l| l.min_max_level.as_ref())
            .and_then(|m| m.get(i))
            .and_then(|r| r.get(j))
            .copied()
            .unwrap_or(0)
    }

    /// `getOpLevelCond`.
    fn op_level_cond(&self, op: NodeId, i: usize, j: usize, k: usize) -> bool {
        self.levels(op)
            .and_then(|l| l.op_level_cond.as_ref())
            .and_then(|m| m.get(i))
            .and_then(|r| r.get(j))
            .and_then(|r| r.get(k))
            .copied()
            .unwrap_or(false)
    }

    fn is_leibniz_arg(&self, op: NodeId, i: usize) -> bool {
        let idx = self.arg_index(op, i);
        self.levels(op)
            .and_then(|l| l.is_leibniz_arg.as_ref())
            .and_then(|v| v.get(idx).copied())
            .unwrap_or(true)
    }

    fn is_leibniz(&self, op: NodeId) -> bool {
        self.levels(op).is_some_and(|l| l.is_leibniz)
    }

    fn checked(&self, id: NodeId) -> u32 {
        self.data(id).checked
    }

    fn set_checked(&mut self, id: NodeId, itr: u32) {
        if let Some(d) = self.data_mut(id) {
            d.checked = itr;
        }
    }

    fn correct(&self, id: NodeId) -> bool {
        self.data(id).correct
    }

    fn set_correct(&mut self, id: NodeId, v: bool) {
        if let Some(d) = self.data_mut(id) {
            d.correct = v;
        }
    }

    /// `levelCheck(iter, errors)`, dispatched on the node's class.
    pub(crate) fn check(&mut self, id: NodeId, itr: u32) -> bool {
        match &self.m.node(id).kind {
            NodeKind::OpDecl(_) | NodeKind::Error => true,
            NodeKind::FormalParam(_) => {
                if self.checked(id) == 0
                    && let Some(d) = self.data_mut(id)
                {
                    d.checked = itr;
                    d.level_params.insert(id);
                    d.all_params.insert(id);
                }
                true
            }
            NodeKind::Numeral(_) | NodeKind::Decimal(..) | NodeKind::Str(_) => {
                self.set_checked(id, itr);
                true
            }
            NodeKind::OpDef(_) => self.check_opdef(id, itr),
            NodeKind::ThmOrAssumpDef(_) => self.check_thmdef(id, itr),
            NodeKind::OpAppl(_) => self.check_appl(id, itr),
            NodeKind::OpArg(_) => self.check_op_arg(id, itr),
            NodeKind::LetIn(_) => self.check_let_in(id, itr),
            NodeKind::At { .. } => self.check_at(id, itr),
            NodeKind::Label(_) => self.check_label(id, itr),
            NodeKind::SubstIn(_) => self.check_subst_in(id, itr, false),
            NodeKind::ApSubstIn(_) => self.check_subst_in(id, itr, true),
            NodeKind::AssumeProve(_) => self.check_assume_prove(id, itr),
            NodeKind::NewSymb { .. } => self.check_new_symb(id, itr),
            NodeKind::Theorem(_) => self.check_theorem(id, itr),
            NodeKind::Assume(_) => self.check_assume(id, itr),
            NodeKind::Instance(_) => self.check_instance(id, itr),
            NodeKind::UseOrHide(d) => {
                let facts = d.facts.clone();
                if self.checked(id) >= itr {
                    return self.correct(id);
                }
                self.check_subnodes(id, itr, &facts)
            }
            NodeKind::LeafProof { facts, .. } => {
                let facts = facts.clone();
                if self.checked(id) >= itr {
                    return self.correct(id);
                }
                self.check_subnodes(id, itr, &facts)
            }
            NodeKind::NonLeafProof { steps, insts, .. } => {
                let mut subs = steps.clone();
                subs.extend(insts.iter().copied());
                if self.checked(id) >= itr {
                    return self.correct(id);
                }
                self.check_subnodes(id, itr, &subs)
            }
            NodeKind::DefStep { defs, .. } => {
                let defs = defs.clone();
                self.check_subnodes(id, itr, &defs)
            }
            NodeKind::Module(_) => self.check_module(id, itr),
        }
    }

    /// `LevelNode.levelCheckSubnodes`.
    fn check_subnodes(&mut self, id: NodeId, itr: u32, subs: &[NodeId]) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let mut correct = self.correct(id);
        let mut level = self.data(id).level;
        let mut lp = self.data(id).level_params.clone();
        let mut lc = self.data(id).level_constraints.clone();
        let mut alc = self.data(id).arg_level_constraints.clone();
        let mut alp = self.data(id).arg_level_params.clone();
        let mut ap = self.data(id).all_params.clone();
        let mut nlp = self.data(id).non_leibniz_params.clone();
        for &s in subs {
            if matches!(self.kind(s), Kind::Module | Kind::ModuleInstance) {
                continue;
            }
            correct = self.check(s, itr) && correct;
            level = level.max(self.level(s));
            lp.extend(self.lp(s));
            for (k, v) in self.lc(s) {
                lc_put(&mut lc, k, v);
            }
            for (k, v) in self.alc(s) {
                alc_put(&mut alc, k, v);
            }
            alp.extend(self.alp(s));
            ap.extend(self.ap(s));
            nlp.extend(self.nlp(s));
        }
        if let Some(d) = self.data_mut(id) {
            d.correct = correct;
            d.level = level;
            d.level_params = lp;
            d.level_constraints = lc;
            d.arg_level_constraints = alc;
            d.arg_level_params = alp;
            d.all_params = ap;
            d.non_leibniz_params = nlp;
        }
        correct
    }

    /// `OpDefNode.levelCheck`.
    fn check_opdef(&mut self, id: NodeId, itr: u32) -> bool {
        let (kind, in_section, body, step) = match &self.m.node(id).kind {
            NodeKind::OpDef(d) => (d.kind, d.in_recursive_section, d.body, d.step),
            _ => return true,
        };
        let checked = self.checked(id);
        if checked >= itr || (!in_section && checked > 0) {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        if kind == DefKind::NumberedProofStep {
            let Some(s) = step else { return true };
            let c = self.check(s, itr);
            self.set_correct(id, c);
            // levelCheckSubnodes sees levelChecked >= itr and returns at once.
            return self.check_subnodes(id, itr, &[s]);
        }
        if matches!(
            kind,
            DefKind::BuiltIn | DefKind::ModuleInstance | DefKind::Null
        ) {
            return true;
        }
        let Some(body) = body else { return true };
        let correct = self.check(body, itr);
        let params = self.params(id);
        let body_level = self.level(body);
        let lc_body = self.lc(body);
        let lp_body = self.lp(body);
        let alc_body = self.alc(body);
        let alp_body = self.alp(body);
        let ap_body = self.ap(body);
        let nlp_body = self.nlp(body);
        let n = params.len();
        let param_arities: Vec<usize> = params
            .iter()
            .map(|&p| usize::try_from(self.arity(p)).unwrap_or(0))
            .collect();
        let mut lv = self.levels(id).cloned().unwrap_or_default();
        lv.max_levels.resize(n, 3);
        lv.weights.resize(n, 0);
        for (i, p) in params.iter().enumerate() {
            if let Some(&pl) = lc_body.get(p) {
                lv.max_levels[i] = lv.max_levels[i].min(pl);
            }
            if lp_body.contains(p) {
                lv.weights[i] = lv.weights[i].max(1);
            }
        }
        lv.min_max_level = Some(
            params
                .iter()
                .zip(&param_arities)
                .map(|(&p, &alen)| {
                    (0..alen)
                        .map(|j| alc_body.get(&(p, u(j))).copied().unwrap_or(0))
                        .collect()
                })
                .collect(),
        );
        lv.op_level_cond = Some(
            params
                .iter()
                .zip(&param_arities)
                .map(|(&pi, &alen)| {
                    params
                        .iter()
                        .map(|&pj| {
                            (0..alen)
                                .map(|k| alp_body.contains(&(pi, u(k), pj)))
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        );
        let mut d = self.data(id).clone();
        d.correct = correct;
        d.level = d.level.max(body_level);
        d.level_params.extend(lp_body);
        d.all_params.extend(ap_body);
        d.non_leibniz_params.extend(nlp_body);
        let mut leibniz_arg = lv.is_leibniz_arg.clone().unwrap_or_else(|| vec![true; n]);
        leibniz_arg.resize(n, true);
        for (i, p) in params.iter().enumerate() {
            d.level_params.remove(p);
            d.all_params.remove(p);
            if d.non_leibniz_params.remove(p) {
                leibniz_arg[i] = false;
                lv.is_leibniz = false;
            }
        }
        lv.is_leibniz_arg = Some(leibniz_arg);
        let mut lc = lc_body;
        for p in &params {
            lc.remove(p);
        }
        d.level_constraints = lc;
        let mut alc = alc_body;
        for (p, &alen) in params.iter().zip(&param_arities) {
            for j in 0..alen {
                alc.remove(&(*p, u(j)));
            }
        }
        d.arg_level_constraints = alc;
        for alp in alp_body {
            if !params.contains(&alp.0) || !params.contains(&alp.2) {
                d.arg_level_params.insert(alp);
            }
        }
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        if let Some(l) = self.levels_mut(id) {
            *l = lv;
        }
        correct
    }

    /// `ThmOrAssumpDefNode.levelCheck`.
    fn check_thmdef(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let params = self.params(id);
        let n = params.len();
        let mut lv = OpLevels {
            max_levels: vec![3; n],
            weights: vec![0; n],
            min_max_level: None,
            op_level_cond: None,
            is_leibniz_arg: None,
            is_leibniz: false,
        };
        if n > 0 {
            // SANY reallocates isLeibnizArg on every iteration of its loop,
            // so only the last entry ends up true.
            let mut v = vec![false; n];
            v[n - 1] = true;
            lv.is_leibniz_arg = Some(v);
            lv.is_leibniz = true;
        }
        let body = match &self.m.node(id).kind {
            NodeKind::ThmOrAssumpDef(d) => d.body,
            _ => None,
        };
        let Some(body) = body else {
            if let Some(l) = self.levels_mut(id) {
                *l = lv;
            }
            return true;
        };
        let correct = self.check(body, itr);
        let lc_body = self.lc(body);
        let alc_body = self.alc(body);
        let alp_body = self.alp(body);
        let param_arities: Vec<usize> = params
            .iter()
            .map(|&p| usize::try_from(self.arity(p)).unwrap_or(0))
            .collect();
        for (i, p) in params.iter().enumerate() {
            if let Some(&pl) = lc_body.get(p) {
                lv.max_levels[i] = pl;
            }
        }
        lv.min_max_level = Some(
            params
                .iter()
                .zip(&param_arities)
                .map(|(&p, &alen)| {
                    (0..alen)
                        .map(|j| alc_body.get(&(p, u(j))).copied().unwrap_or(0))
                        .collect()
                })
                .collect(),
        );
        lv.op_level_cond = Some(
            params
                .iter()
                .zip(&param_arities)
                .map(|(&pi, &alen)| {
                    params
                        .iter()
                        .map(|&pj| {
                            (0..alen)
                                .map(|k| alp_body.contains(&(pi, u(k), pj)))
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        );
        let mut d = self.data(id).clone();
        d.correct = correct;
        d.level = self.level(body);
        d.level_params.extend(self.lp(body));
        d.all_params.extend(self.ap(body));
        d.non_leibniz_params.extend(self.nlp(body));
        for (i, p) in params.iter().enumerate() {
            d.level_params.remove(p);
            d.all_params.remove(p);
            if d.non_leibniz_params.remove(p) {
                if let Some(v) = lv.is_leibniz_arg.as_mut()
                    && let Some(x) = v.get_mut(i)
                {
                    *x = false;
                }
                lv.is_leibniz = false;
            }
        }
        let mut lc = lc_body;
        for p in &params {
            lc.remove(p);
        }
        d.level_constraints = lc;
        let mut alc = alc_body;
        for (p, &alen) in params.iter().zip(&param_arities) {
            for j in 0..alen {
                alc.remove(&(*p, u(j)));
            }
        }
        d.arg_level_constraints = alc;
        for alp in alp_body {
            if !params.contains(&alp.0) || !params.contains(&alp.2) {
                d.arg_level_params.insert(alp);
            }
        }
        if let Some(nd) = self.m.node_mut(id) {
            nd.level = d;
        }
        if let Some(l) = self.levels_mut(id) {
            *l = lv;
        }
        correct
    }

    /// `OpApplNode.getArg`: the operand passed for formal parameter `param`.
    fn get_arg(&self, op: NodeId, operands: &[Option<NodeId>], param: NodeId) -> Option<NodeId> {
        let formals = self.params(op);
        for (i, o) in operands.iter().enumerate() {
            if formals.get(i) == Some(&param) {
                return *o;
            }
        }
        None
    }

    fn op_arg_target(&self, id: NodeId) -> Option<NodeId> {
        match &self.m.node(id).kind {
            NodeKind::OpArg(d) => Some(d.op),
            _ => None,
        }
    }

    /// `OpApplNode.levelCheck`.
    #[allow(
        clippy::too_many_lines,
        reason = "a direct port of OpApplNode.levelCheck"
    )]
    fn check_appl(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let Some(NodeKind::OpAppl(data)) = self.m.nodes.get(id.index()).map(|n| n.kind.clone())
        else {
            return true;
        };
        let op = data.operator;
        let operands = data.operands.clone();
        let ranges = data.ranges.clone();
        let mut correct = true;
        for &o in operands.iter().flatten() {
            if !self.check(o, itr) {
                correct = false;
            }
        }
        for &r in &ranges {
            if !self.check(r, itr) {
                correct = false;
            }
        }
        let mut d = LevelData::default();
        if self.is_any_def(op) {
            let op_ok = self.check(op, itr);
            let opname = self.name(op);
            for (i, opd) in operands.iter().enumerate() {
                let Some(opd) = *opd else { continue };
                if self.level(opd) > self.max_level(op, i) {
                    if op_ok && self.check(opd, itr) {
                        self.err(
                            Code::ArgumentLevel,
                            id,
                            format!(
                                "level error in applying `{opname}`: the level of argument {} exceeds the maximum level the operator allows",
                                i + 1
                            ),
                        );
                    }
                    correct = false;
                }
                if let Some(od) = self.op_arg_target(opd)
                    && self.is_any_def(od)
                {
                    self.check(od, itr);
                    let alen = usize::try_from(self.arity(od)).unwrap_or(0);
                    for j in 0..alen {
                        if self.max_level(od, j) < self.min_max_level(op, i, j) {
                            if op_ok && self.check(opd, itr) {
                                self.err(
                                    Code::OperatorArgumentLevel,
                                    id,
                                    format!(
                                        "level error in applying `{opname}`: the permitted level of argument {} of operator argument {} must be at least {}",
                                        j + 1,
                                        i + 1,
                                        self.min_max_level(op, i, j)
                                    ),
                                );
                            }
                            correct = false;
                        }
                    }
                    for (j, oj) in operands.iter().enumerate() {
                        let Some(oj) = *oj else { continue };
                        for k in 0..alen {
                            if self.op_level_cond(op, i, j, k)
                                && self.level(oj) > self.max_level(od, k)
                            {
                                if self.check(opd, itr) && self.check(oj, itr) {
                                    self.err(
                                        Code::OperatorCoparameterLevel,
                                        id,
                                        format!(
                                            "level error in applying `{opname}`: the level of argument {} exceeds the maximum level the operator allows",
                                            j + 1
                                        ),
                                    );
                                }
                                correct = false;
                            }
                        }
                    }
                }
            }
            for &r in &ranges {
                let rok = self.check(r, itr);
                if self.level(r) > 2 {
                    if rok {
                        self.err(
                            Code::TemporalBound,
                            id,
                            format!("level error in applying `{opname}`: the range of a bound identifier is temporal"),
                        );
                    }
                    correct = false;
                }
            }
            d.level = self.data(op).level;
            for (i, o) in operands.iter().enumerate() {
                if let Some(o) = *o
                    && self.weight(op, i) == 1
                {
                    d.level = d.level.max(self.level(o));
                }
            }
            for &r in &ranges {
                d.level = d.level.max(self.level(r));
            }
            d.level_params.extend(self.lp(op));
            d.all_params.extend(self.ap(op));
            d.non_leibniz_params.extend(self.nlp(op));
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                if self.weight(op, i) == 1 {
                    d.level_params.extend(self.lp(o));
                }
                d.all_params.extend(self.ap(o));
                d.non_leibniz_params.extend(self.nlp(o));
                if !self.is_leibniz_arg(op, i) {
                    d.non_leibniz_params.extend(self.ap(o));
                }
            }
            for &r in &ranges {
                d.level_params.extend(self.lp(r));
                d.all_params.extend(self.ap(r));
                d.non_leibniz_params.extend(self.nlp(r));
            }
            let mut bound: Set = Set::new();
            if let Some(ub) = &data.unbounded {
                bound.extend(ub.iter().copied());
            }
            if let Some(b) = &data.bounded {
                bound.extend(b.iter().flatten().copied());
            }
            for b in &bound {
                d.level_params.remove(b);
                d.all_params.remove(b);
                d.non_leibniz_params.remove(b);
            }
            for (k, v) in self.lc(op) {
                lc_put(&mut d.level_constraints, k, v);
            }
            for o in operands.iter().flatten() {
                for (k, v) in self.lc(*o) {
                    if bound.is_empty() || !bound.contains(&k) {
                        lc_put(&mut d.level_constraints, k, v);
                    }
                }
            }
            for &r in &ranges {
                for (k, v) in self.lc(r) {
                    lc_put(&mut d.level_constraints, k, v);
                }
            }
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                let ml = self.max_level(op, i);
                for p in self.lp(o) {
                    lc_put(&mut d.level_constraints, p, ml);
                }
            }
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                let Some(arg_def) = self.op_arg_target(o).filter(|&a| self.is_any_def(a)) else {
                    continue;
                };
                self.check(arg_def, itr);
                let alen = usize::try_from(self.arity(arg_def)).unwrap_or(0);
                for (j, oj) in operands.iter().enumerate() {
                    let Some(oj) = *oj else { continue };
                    for k in 0..alen {
                        if self.op_level_cond(op, i, j, k) {
                            let ml = self.max_level(arg_def, k);
                            for p in self.lp(oj) {
                                lc_put(&mut d.level_constraints, p, ml);
                            }
                        }
                    }
                }
                if !self.is_leibniz(arg_def) {
                    for (j, oj) in operands.iter().enumerate() {
                        let Some(oj) = *oj else { continue };
                        for k in 0..alen {
                            if self.op_level_cond(op, i, j, k) && !self.is_leibniz_arg(arg_def, k) {
                                d.non_leibniz_params.extend(self.ap(oj));
                            }
                        }
                    }
                }
            }
            let alp_set = self.alp(op);
            for alp in &alp_set {
                let arg = self.get_arg(op, &operands, alp.0);
                if let Some(arg) = arg
                    && let Some(arg_def) = self.op_arg_target(arg).filter(|&a| self.is_any_def(a))
                {
                    self.check(arg_def, itr);
                    let ml = self.max_level(arg_def, alp.1 as usize);
                    lc_put(&mut d.level_constraints, alp.2, ml);
                }
            }
            for (k, v) in self.alc(op) {
                alc_put(&mut d.arg_level_constraints, k, v);
            }
            for o in operands.iter().flatten() {
                for (k, v) in self.alc(*o) {
                    alc_put(&mut d.arg_level_constraints, k, v);
                }
            }
            for &r in &ranges {
                for (k, v) in self.alc(r) {
                    alc_put(&mut d.arg_level_constraints, k, v);
                }
            }
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                let Some(op_arg) = self.op_arg_target(o).filter(|&a| self.is_param(a)) else {
                    continue;
                };
                let alen = usize::try_from(self.arity(op_arg)).unwrap_or(0);
                for j in 0..alen {
                    let v = self.min_max_level(op, i, j);
                    alc_put(&mut d.arg_level_constraints, (op_arg, u(j)), v);
                }
                for (j, oj) in operands.iter().enumerate() {
                    let Some(oj) = *oj else { continue };
                    for k in 0..alen {
                        if self.op_level_cond(op, i, j, k) {
                            let v = self.level(oj);
                            alc_put(&mut d.arg_level_constraints, (op_arg, u(k)), v);
                        }
                    }
                }
            }
            for alp in &alp_set {
                if let Some(arg) = self.get_arg(op, &operands, alp.0) {
                    self.check(arg, itr);
                    let v = self.level(arg);
                    alc_put(&mut d.arg_level_constraints, (alp.0, alp.1), v);
                }
            }
            for o in operands.iter().flatten() {
                for a in self.alp(*o) {
                    if bound.is_empty() || !bound.contains(&a.2) {
                        d.arg_level_params.insert(a);
                    }
                }
            }
            for &r in &ranges {
                d.arg_level_params.extend(self.alp(r));
            }
            for alp in &alp_set {
                match self.get_arg(op, &operands, alp.0) {
                    None => match self.get_arg(op, &operands, alp.2) {
                        None => {
                            d.arg_level_params.insert(*alp);
                        }
                        Some(a2) => {
                            self.check(a2, itr);
                            for p in self.lp(a2) {
                                d.arg_level_params.insert((alp.0, alp.1, p));
                            }
                        }
                    },
                    Some(arg) => {
                        if let Some(arg_op) = self.op_arg_target(arg).filter(|&a| self.is_param(a))
                        {
                            d.arg_level_params.insert((arg_op, alp.1, alp.2));
                        }
                    }
                }
            }
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                let Some(op_arg) = self.op_arg_target(o).filter(|&a| self.is_param(a)) else {
                    continue;
                };
                let alen = usize::try_from(self.arity(op_arg)).unwrap_or(0);
                for (j, oj) in operands.iter().enumerate() {
                    let Some(oj) = *oj else { continue };
                    for k in 0..alen {
                        if self.op_level_cond(op, i, j, k) {
                            for p in self.lp(oj) {
                                d.arg_level_params.insert((op_arg, u(k), p));
                            }
                        }
                    }
                }
            }
        } else {
            self.check(op, itr);
            d.level = self.data(op).level;
            for o in operands.iter().flatten() {
                self.check(*o, itr);
                d.level = d.level.max(self.level(*o));
            }
            d.level_params.insert(op);
            d.all_params.insert(op);
            for o in operands.iter().flatten() {
                d.level_params.extend(self.lp(*o));
                d.all_params.extend(self.ap(*o));
                d.non_leibniz_params.extend(self.nlp(*o));
            }
            for o in operands.iter().flatten() {
                for (k, v) in self.lc(*o) {
                    lc_put(&mut d.level_constraints, k, v);
                }
            }
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                let v = self.level(o);
                alc_put(&mut d.arg_level_constraints, (op, u(i)), v);
                for (k, v) in self.alc(o) {
                    alc_put(&mut d.arg_level_constraints, k, v);
                }
            }
            for (i, o) in operands.iter().enumerate() {
                let Some(o) = *o else { continue };
                for p in self.lp(o) {
                    d.arg_level_params.insert((op, u(i), p));
                }
                d.arg_level_params.extend(self.alp(o));
            }
        }
        // The special-case checks.
        let opname = self.name(op);
        let arg_level =
            |s: &Self, i: usize| operands.get(i).copied().flatten().map_or(0, |a| s.level(a));
        let arg_op_name = |s: &Self, i: usize| -> Option<String> {
            let a = operands.get(i).copied().flatten()?;
            match &s.m.node(a).kind {
                NodeKind::OpAppl(ad) => Some(s.name(ad.operator)),
                _ => None,
            }
        };
        if opname == "[]"
            && arg_level(self, 0) == 2
            && arg_op_name(self, 0).is_some_and(|n| n != "$SquareAct")
        {
            self.err(
                Code::AlwaysAction,
                id,
                "`[]` is applied to an action that is not of the form `[A]_v`; the formula would not be invariant under stuttering",
            );
            correct = false;
        }
        if opname == "<>"
            && arg_level(self, 0) == 2
            && arg_op_name(self, 0).is_some_and(|n| n != "$AngleAct")
        {
            self.err(
                Code::EventuallyAction,
                id,
                "`<>` is applied to an action that is not of the form `<<A>>_v`; the formula would not be invariant under stuttering",
            );
            correct = false;
        }
        if (opname == "~>" || opname == "-+->")
            && (arg_level(self, 0) == 2 || arg_level(self, 1) == 2)
        {
            self.err(
                Code::TemporalBinaryAction,
                id,
                format!("an action is used as an argument of `{opname}`, where only temporal formulas or state predicates are allowed"),
            );
            correct = false;
        }
        if matches!(
            opname.as_str(),
            "\\land" | "\\lor" | "=>" | "\\equiv" | "$ConjList" | "$DisjList"
        ) {
            let has_t = (0..operands.len()).any(|i| arg_level(self, i) == 3);
            let has_a = (0..operands.len()).any(|i| arg_level(self, i) == 2);
            if has_t && has_a {
                let what = match opname.as_str() {
                    "$ConjList" => "a conjunction list".to_owned(),
                    "$DisjList" => "a disjunction list".to_owned(),
                    other => format!("`{other}`"),
                };
                self.err(
                    Code::MixedActionTemporal,
                    id,
                    format!("{what} has both a temporal formula and an action as arguments"),
                );
                correct = false;
            }
        }
        if d.level == 3 && (opname == "$BoundedExists" || opname == "$BoundedForall") {
            for &r in &ranges {
                if self.level(r) == 2 {
                    self.err(
                        Code::TemporalQuantActionBound,
                        r,
                        "an action-level bound of a quantified temporal formula",
                    );
                    correct = false;
                }
            }
        }
        d.checked = itr;
        d.correct = correct;
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        correct
    }

    /// `OpArgNode.levelCheck`.
    fn check_op_arg(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let Some(op) = self.op_arg_target(id) else {
            return true;
        };
        let correct = self.check(op, itr);
        let d = LevelData {
            level: self.data(op).level,
            level_params: self.lp(op),
            all_params: self.ap(op),
            non_leibniz_params: self.data(id).non_leibniz_params.clone(),
            level_constraints: self.lc(op),
            arg_level_constraints: self.alc(op),
            arg_level_params: self.alp(op),
            checked: itr,
            correct,
        };
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        correct
    }

    /// `LetInNode.levelCheck`.
    fn check_let_in(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let (defs, insts, body) = match &self.m.node(id).kind {
            NodeKind::LetIn(d) => (d.defs.clone(), d.insts.clone(), d.body),
            _ => return true,
        };
        let mut correct = true;
        for &d in &defs {
            if self.kind(d) != Kind::ModuleInstance && !self.check(d, itr) {
                correct = false;
            }
        }
        let Some(body) = body else { return correct };
        if !self.check(body, itr) {
            correct = false;
        }
        for &i in &insts {
            if !self.check(i, itr) {
                correct = false;
            }
        }
        let mut d = self.data(id).clone();
        d.level = self.level(body);
        d.level_params = self.lp(body);
        d.all_params = self.ap(body);
        for (k, v) in self.lc(body) {
            lc_put(&mut d.level_constraints, k, v);
        }
        for &def in &defs {
            if self.kind(def) != Kind::ModuleInstance {
                for (k, v) in self.lc(def) {
                    lc_put(&mut d.level_constraints, k, v);
                }
            }
        }
        for (k, v) in self.alc(body) {
            alc_put(&mut d.arg_level_constraints, k, v);
        }
        for &def in &defs {
            if self.kind(def) != Kind::ModuleInstance {
                for (k, v) in self.alc(def) {
                    alc_put(&mut d.arg_level_constraints, k, v);
                }
            }
        }
        d.arg_level_params.extend(self.alp(body));
        for &def in &defs {
            if self.kind(def) == Kind::ModuleInstance {
                continue;
            }
            let params = if self.kind(def) == Kind::ThmOrAssumpDef {
                Vec::new()
            } else {
                self.params(def)
            };
            for a in self.alp(def) {
                if !params.contains(&a.0) && !params.contains(&a.2) {
                    d.arg_level_params.insert(a);
                }
            }
        }
        for &i in &insts {
            d.arg_level_params.extend(self.alp(i));
        }
        d.correct = correct;
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        correct
    }

    /// `AtNode.levelCheck`.
    fn check_at(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return true;
        }
        self.set_checked(id, itr);
        let (exc, comp) = match &self.m.node(id).kind {
            NodeKind::At { except, component } => (*except, *component),
            _ => return true,
        };
        let args: Vec<NodeId> = match &self.m.node(exc).kind {
            NodeKind::OpAppl(d) => d.operands.iter().flatten().copied().collect(),
            _ => Vec::new(),
        };
        let mut d = self.data(id).clone();
        let Some(&first) = args.first() else {
            return true;
        };
        self.check(first, itr);
        d.level = self.level(first);
        let mut used = vec![first];
        for &a in args.iter().skip(1) {
            self.check(a, itr);
            if a == comp {
                break;
            }
            d.level = d.level.max(self.level(a));
            used.push(a);
        }
        for &a in &used {
            d.level_params.extend(self.lp(a));
            d.all_params.extend(self.ap(a));
            for (k, v) in self.lc(a) {
                lc_put(&mut d.level_constraints, k, v);
            }
            for (k, v) in self.alc(a) {
                alc_put(&mut d.arg_level_constraints, k, v);
            }
            d.arg_level_params.extend(self.alp(a));
        }
        d.checked = itr;
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        true
    }

    /// `LabelNode.levelCheck`.
    fn check_label(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return true;
        }
        self.set_checked(id, itr);
        let (params, body) = match &self.m.node(id).kind {
            NodeKind::Label(d) => (d.params.clone(), d.body),
            _ => return true,
        };
        for p in params {
            self.check(p, itr);
        }
        body.is_none_or(|b| self.check(b, itr))
    }

    /// `Subst.paramSet` / `allParamSet`.
    fn param_set(&self, param: NodeId, substs: &[Subst], all: bool) -> Set {
        match substs.iter().find(|s| s.op == param) {
            Some(s) => {
                if all {
                    self.ap(s.expr)
                } else {
                    self.lp(s.expr)
                }
            }
            None => std::iter::once(param).collect(),
        }
    }

    fn get_sub(substs: &[Subst], param: NodeId) -> Option<NodeId> {
        substs.iter().find(|s| s.op == param).map(|s| s.expr)
    }

    /// `Subst.getSubLCSet`.
    fn sub_lc_set(&mut self, body: NodeId, substs: &[Subst], is_constant: bool, itr: u32) -> Lc {
        let mut res = Lc::new();
        for (param, mut plevel) in self.lc(body) {
            if !is_constant {
                match self.kind(param) {
                    Kind::ConstantDecl => plevel = 0,
                    Kind::VariableDecl => plevel = 1,
                    _ => {}
                }
            }
            for p in self.param_set(param, substs, false) {
                lc_put(&mut res, p, plevel);
            }
        }
        for alp in self.alp(body) {
            if let Some(sub) = Self::get_sub(substs, alp.0)
                && let Some(sub_op) = self.op_arg_target(sub)
                && matches!(self.m.node(sub_op).kind, NodeKind::OpDef(_))
            {
                self.check(sub_op, itr);
                let ml = self.max_level(sub_op, alp.1 as usize);
                for p in self.param_set(alp.2, substs, false) {
                    lc_put(&mut res, p, ml);
                }
            }
        }
        res
    }

    /// `Subst.getSubALCSet`.
    fn sub_alc_set(&mut self, body: NodeId, substs: &[Subst], itr: u32) -> Alc {
        let mut res = Alc::new();
        for ((param, pos), plevel) in self.alc(body) {
            match Self::get_sub(substs, param) {
                None => alc_put(&mut res, (param, pos), plevel),
                Some(sub) => {
                    if let Some(sub_op) = self.op_arg_target(sub)
                        && self.is_param(sub_op)
                    {
                        alc_put(&mut res, (sub_op, pos), plevel);
                    }
                }
            }
        }
        for alp in self.alp(body) {
            if let Some(sub_param) = Self::get_sub(substs, alp.2) {
                let op = match Self::get_sub(substs, alp.0) {
                    None => Some(alp.0),
                    Some(s) => self.op_arg_target(s),
                };
                if let Some(op) = op
                    && self.is_param(op)
                {
                    self.check(sub_param, itr);
                    let v = self.level(sub_param);
                    alc_put(&mut res, (op, alp.1), v);
                }
            }
        }
        res
    }

    /// `Subst.getSubALPSet`.
    fn sub_alp_set(&self, body: NodeId, substs: &[Subst]) -> Alp {
        let mut res = Alp::new();
        for alp in self.alp(body) {
            match Self::get_sub(substs, alp.0) {
                None => {
                    res.insert(alp);
                }
                Some(sub) => {
                    if let Some(sub_op) = self.op_arg_target(sub)
                        && self.is_param(sub_op)
                    {
                        for p in self.param_set(alp.2, substs, false) {
                            res.insert((sub_op, alp.1, p));
                        }
                    }
                }
            }
        }
        res
    }

    /// `ModuleNode.isConstant`.
    fn module_is_constant(&mut self, module: NodeId) -> bool {
        if !crate::generate::decls_in(self.m, module, true).is_empty() {
            return false;
        }
        self.check_module(module, 1);
        for d in self.module_op_defs(module) {
            if self.kind(d) != Kind::ModuleInstance
                && let NodeKind::OpDef(od) = &self.m.node(d).kind
                && od.body.is_some_and(|b| self.level(b) != 0)
            {
                return false;
            }
        }
        let theorems = match &self.m.node(module).kind {
            NodeKind::Module(md) => md.theorems.clone(),
            _ => Vec::new(),
        };
        theorems.into_iter().all(|t| self.level(t) == 0)
    }

    /// `SubstInNode.levelCheck` and `APSubstInNode.levelCheck`.
    fn check_subst_in(&mut self, id: NodeId, itr: u32, ap: bool) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let (substs, body, instantiated) = match &self.m.node(id).kind {
            NodeKind::SubstIn(d) | NodeKind::ApSubstIn(d) => {
                (d.substs.clone(), d.body, d.instantiated)
            }
            _ => return true,
        };
        let Some(body) = body else { return true };
        let mut correct = true;
        if !self.check(body, itr) {
            correct = false;
        }
        for s in &substs {
            if !self.check(s.expr, itr) {
                correct = false;
            }
        }
        let mut d = self.data(id).clone();
        d.level = self.level(body);
        let lp_body = self.lp(body);
        for s in &substs {
            if lp_body.contains(&s.op) {
                d.level = d.level.max(self.level(s.expr));
            }
        }
        for p in &lp_body {
            d.level_params.extend(self.param_set(*p, &substs, false));
        }
        if ap {
            for p in self.ap(body) {
                d.all_params.extend(self.param_set(p, &substs, true));
            }
        } else {
            d.all_params = self.ap(body);
            d.non_leibniz_params = self.nlp(body);
            for s in &substs {
                if d.all_params.remove(&s.op) {
                    d.all_params.extend(self.ap(s.expr));
                    d.non_leibniz_params.extend(self.nlp(s.expr));
                    if d.non_leibniz_params.remove(&s.op) {
                        d.non_leibniz_params.extend(self.ap(s.expr));
                    }
                }
            }
        }
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        let is_constant = self.module_is_constant(instantiated);
        let lc = self.sub_lc_set(body, &substs, is_constant, itr);
        let alc = self.sub_alc_set(body, &substs, itr);
        let alp = self.sub_alp_set(body, &substs);
        if let Some(d) = self.data_mut(id) {
            d.level_constraints = lc;
            d.arg_level_constraints = alc;
            d.arg_level_params = alp;
            d.correct = correct;
        }
        correct
    }

    /// `AssumeProveNode.levelCheck`.
    fn check_assume_prove(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let (assumes, prove) = match &self.m.node(id).kind {
            NodeKind::AssumeProve(d) => (d.assumes.clone(), d.prove),
            _ => return true,
        };
        let mut correct = true;
        for a in assumes.iter().flatten() {
            if !self.check(*a, itr) {
                correct = false;
            }
        }
        let Some(prove) = prove else { return correct };
        self.check(prove, itr);
        let mut d = self.data(id).clone();
        d.level = self.level(prove);
        for a in assumes.iter().flatten() {
            self.check(*a, itr);
            d.level = d.level.max(self.level(*a));
        }
        let mut parts = vec![prove];
        parts.extend(assumes.iter().flatten().copied());
        for &p in &parts {
            d.level_params.extend(self.lp(p));
            d.all_params.extend(self.ap(p));
            for (k, v) in self.lc(p) {
                lc_put(&mut d.level_constraints, k, v);
            }
            for (k, v) in self.alc(p) {
                alc_put(&mut d.arg_level_constraints, k, v);
            }
            d.arg_level_params.extend(self.alp(p));
        }
        if correct {
            self.add_temporal_level_constraint_to_constants(&mut d);
        }
        d.correct = correct;
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        correct
    }

    /// `LevelNode.addTemporalLevelConstraintToConstants`.
    fn add_temporal_level_constraint_to_constants(&self, d: &mut LevelData) {
        for &p in &d.level_params {
            if self.kind(p) == Kind::ConstantDecl {
                lc_put(&mut d.level_constraints, p, 2);
            }
        }
    }

    /// `NewSymbNode.levelCheck`.
    fn check_new_symb(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) < itr {
            self.set_checked(id, itr);
            let (decl, set) = match &self.m.node(id).kind {
                NodeKind::NewSymb { decl, set } => (*decl, *set),
                _ => return true,
            };
            let decl_ok = self.check(decl, itr);
            let mut d = self.data(id).clone();
            d.level = self.data(decl).level;
            if let Some(s) = set {
                d.correct = self.check(s, itr);
                d.level = d.level.max(self.level(s));
                if d.level == 3 {
                    d.correct = false;
                    let span = self.m.node(id).span;
                    *self.errors += 1;
                    self.diags.push(Diagnostic::new(
                        Code::NewTemporalBound,
                        span,
                        "level error: a temporal formula is used as the set of a NEW declaration",
                    ));
                }
            }
            d.correct = d.correct && decl_ok;
            if let Some(s) = set {
                d.level_params = self.lp(s);
                d.all_params = self.ap(s);
                d.level_constraints = self.lc(s);
                d.arg_level_constraints = self.alc(s);
                d.arg_level_params = self.alp(s);
            }
            if let Some(n) = self.m.node_mut(id) {
                n.level = d;
            }
        }
        self.correct(id)
    }

    /// `TheoremNode.levelCheck` (with its own iteration counter).
    fn check_theorem(&mut self, id: NodeId, itr: u32) -> bool {
        let (body, proof, def, own) = match &self.m.node(id).kind {
            NodeKind::Theorem(d) => (d.body, d.proof, d.def, d.checked),
            _ => return true,
        };
        if own >= itr {
            return true;
        }
        if let Some(NodeKind::Theorem(d)) = self.m.node_mut(id).map(|n| &mut n.kind) {
            d.checked = itr;
        }
        let mut subs = Vec::new();
        if let Some(df) = def {
            subs.push(df);
        } else if let Some(b) = body {
            subs.push(b);
        }
        if let Some(p) = proof {
            subs.push(p);
        }
        let ret = self.check_subnodes(id, itr, &subs);
        let Some(body) = body else { return ret };
        let body_level = self.data(body).level;
        if let NodeKind::OpAppl(bd) = &self.m.node(body).kind
            && self.name(bd.operator) == "$Pick"
            && body_level == 3
        {
            let ranges = bd.ranges.clone();
            for r in ranges {
                if self.level(r) != 0 {
                    self.err(
                        Code::TemporalPickBound,
                        r,
                        "a temporal PICK has a non-constant bound",
                    );
                }
            }
        }
        if body_level == 3 {
            self.check_temporal_proof(proof);
        }
        ret
    }

    /// `TheoremNode.LevelCheckTemporal`.
    fn check_temporal_proof(&mut self, proof: Option<NodeId>) {
        let Some(p) = proof else { return };
        let steps = match &self.m.node(p).kind {
            NodeKind::NonLeafProof { steps, .. } => steps.clone(),
            _ => return,
        };
        for s in steps {
            let NodeKind::Theorem(td) = &self.m.node(s).kind else {
                continue;
            };
            let sub_proof = td.proof;
            let Some(b) = td.body else { continue };
            let NodeKind::OpAppl(bd) = &self.m.node(b).kind else {
                continue;
            };
            let name = self.name(bd.operator);
            let level = self.level(b);
            match name.as_str() {
                "$Take" | "$Witness" | "$Have" => {
                    if level != 0 {
                        self.err(
                            Code::TemporalTakeWitnessHave,
                            b,
                            "a non-constant TAKE, WITNESS or HAVE in the proof of a temporal goal",
                        );
                    }
                }
                "$Pfcase" => {
                    if level != 0 {
                        self.err(
                            Code::TemporalCase,
                            b,
                            "a non-constant CASE in the proof of a temporal goal",
                        );
                    }
                    self.check_temporal_proof(sub_proof);
                }
                "$Qed" => self.check_temporal_proof(sub_proof),
                _ => {}
            }
        }
    }

    /// `AssumeNode.levelCheck` (with its own iteration counter).
    fn check_assume(&mut self, id: NodeId, itr: u32) -> bool {
        let (expr, def, own) = match &self.m.node(id).kind {
            NodeKind::Assume(d) => (d.expr, d.def, d.checked),
            _ => return true,
        };
        if own >= itr {
            return true;
        }
        if let Some(NodeKind::Assume(d)) = self.m.node_mut(id).map(|n| &mut n.kind) {
            d.checked = itr;
        }
        let Some(expr) = expr else { return true };
        let mut res = self.check(expr, itr);
        if let Some(df) = def {
            res = self.check(df, itr) && res;
        }
        let l = self.level(expr);
        if l != 0 {
            self.err(
                Code::AssumptionNotConstant,
                id,
                format!("an ASSUME must be constant level, but this one has level {l}"),
            );
        }
        res
    }

    /// `InstanceNode.levelCheck`.
    #[allow(
        clippy::too_many_lines,
        reason = "a direct port of InstanceNode.levelCheck"
    )]
    fn check_instance(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let (module, substs, params) = match &self.m.node(id).kind {
            NodeKind::Instance(d) => (d.module, d.substs.clone(), d.params.clone()),
            _ => return true,
        };
        let mname = self.name(module);
        let mut correct = true;
        if let Some(d) = self.data_mut(id) {
            d.level_params = Set::new();
        }
        if !self.check(module, itr) {
            correct = false;
        }
        for s in &substs {
            if !self.check(s.expr, itr) {
                correct = false;
            }
        }
        for s in &substs {
            let (mparam, mexp) = (s.op, s.expr);
            self.check(mexp, itr);
            self.check(mparam, itr);
            if !self.module_is_constant(module) && self.level(mexp) > self.data(mparam).level {
                if self.check(mexp, itr) && self.check(mparam, itr) {
                    let pn = self.name(mparam);
                    let pl = self.data(mparam).level;
                    self.err(
                        Code::SubstitutionLevel,
                        id,
                        format!("level error in instantiating module `{mname}`: what is substituted for `{pn}` must have level at most {pl}"),
                    );
                }
                correct = false;
            }
            if let Some(op) = self.op_arg_target(mexp)
                && matches!(self.kind(op), Kind::UserDefinedOp | Kind::BuiltIn)
                && !self.is_leibniz(op)
            {
                let pn = self.name(mparam);
                self.err(
                    Code::NonLeibnizSubstitution,
                    id,
                    format!("error in instantiating module `{mname}`: a non-Leibniz operator is substituted for `{pn}`"),
                );
            }
        }
        let lc_set = self.data(module).level_constraints.clone();
        let alc_set = self.data(module).arg_level_constraints.clone();
        for s in &substs {
            let (mparam, mexp) = (s.op, s.expr);
            if let Some(&pl) = lc_set.get(&mparam)
                && self.level(mexp) > pl
            {
                if self.check(mexp, itr) {
                    let pn = self.name(mparam);
                    self.err(
                        Code::SubstitutionLevel,
                        id,
                        format!("level error in instantiating module `{mname}`: what is substituted for `{pn}` must have level at most {pl}"),
                    );
                }
                correct = false;
            }
            let alen = usize::try_from(self.arity(mparam)).unwrap_or(0);
            if alen > 0
                && let Some(op_def) = self.op_arg_target(mexp)
                && matches!(self.m.node(op_def).kind, NodeKind::OpDef(_))
            {
                for j in 0..alen {
                    let alevel = alc_set.get(&(mparam, u(j))).copied();
                    let ok = self.check(op_def, itr);
                    if let Some(al) = alevel
                        && self.max_level(op_def, j) < al
                    {
                        if ok {
                            let on = self.name(op_def);
                            self.err(
                                Code::SubstitutionArgumentLevel,
                                id,
                                format!(
                                    "level error in instantiating module `{mname}`: argument {} of operator `{on}` must accept level {al}",
                                    j + 1
                                ),
                            );
                        }
                        correct = false;
                    }
                }
            }
        }
        for alp in self.data(module).arg_level_params.clone() {
            for si in &substs {
                for sj in &substs {
                    if alp.0 == si.op && alp.2 == sj.op {
                        let Some(op) = self.op_arg_target(si.expr) else {
                            continue;
                        };
                        let op_ok = self.check(op, itr);
                        if matches!(self.m.node(op).kind, NodeKind::OpDef(_))
                            && self.level(sj.expr) > self.max_level(op, alp.1 as usize)
                        {
                            if op_ok && self.check(sj.expr, itr) {
                                let pn = self.name(si.op);
                                self.err(
                                    Code::SubstitutionCoparameterLevel,
                                    id,
                                    format!(
                                        "level error in instantiating module `{mname}`: argument {} of `{pn}` must have level at most {}",
                                        alp.1,
                                        self.max_level(op, alp.1 as usize)
                                    ),
                                );
                            }
                            correct = false;
                        }
                    }
                }
            }
        }
        let is_constant = self.module_is_constant(module);
        let mut d = self.data(id).clone();
        let lc = self.sub_lc_set(module, &substs, is_constant, itr);
        for (k, v) in lc {
            if !params.contains(&k) {
                lc_put(&mut d.level_constraints, k, v);
            }
        }
        for s in &substs {
            for (k, v) in self.lc(s.expr) {
                if !params.contains(&k) {
                    lc_put(&mut d.level_constraints, k, v);
                }
            }
        }
        let alc = self.sub_alc_set(module, &substs, itr);
        for (k, v) in alc {
            if !params.contains(&k.0) {
                alc_put(&mut d.arg_level_constraints, k, v);
            }
        }
        for s in &substs {
            for (k, v) in self.alc(s.expr) {
                if !params.contains(&k.0) {
                    alc_put(&mut d.arg_level_constraints, k, v);
                }
            }
        }
        for a in self.sub_alp_set(module, &substs) {
            if !params.contains(&a.0) && !params.contains(&a.2) {
                d.arg_level_params.insert(a);
            }
        }
        for s in &substs {
            for a in self.alp(s.expr) {
                if !params.contains(&a.0) && !params.contains(&a.2) {
                    d.arg_level_params.insert(a);
                }
            }
        }
        d.correct = correct;
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        correct
    }

    /// `ModuleNode.getOpDefs`: the module's definitions (not built-ins or
    /// instance names), oldest first.
    fn module_op_defs(&self, module: NodeId) -> Vec<NodeId> {
        let NodeKind::Module(md) = &self.m.node(module).kind else {
            return Vec::new();
        };
        self.m
            .context(md.ctx)
            .entries()
            .filter(|(k, v)| {
                matches!(k, crate::model::Key::Sym(_))
                    && matches!(&self.m.node(*v).kind, NodeKind::OpDef(d)
                        if !matches!(d.kind, DefKind::BuiltIn | DefKind::ModuleInstance))
            })
            .map(|(_, v)| v)
            .collect()
    }

    fn module_thm_defs(&self, module: NodeId) -> Vec<NodeId> {
        let NodeKind::Module(md) = &self.m.node(module).kind else {
            return Vec::new();
        };
        self.m
            .context(md.ctx)
            .entries()
            .filter(|(_, v)| matches!(self.m.node(*v).kind, NodeKind::ThmOrAssumpDef(_)))
            .map(|(_, v)| v)
            .collect()
    }

    fn module_inner(&self, module: NodeId) -> Vec<NodeId> {
        let NodeKind::Module(md) = &self.m.node(module).kind else {
            return Vec::new();
        };
        self.m
            .context(md.ctx)
            .entries()
            .filter(|(k, _)| matches!(k, crate::model::Key::Module(_)))
            .map(|(_, v)| v)
            .collect()
    }

    fn set_opdef_flags(&mut self, id: NodeId, checked: u32) {
        self.set_checked(id, checked);
    }

    /// `ModuleNode.levelCheck`.
    #[allow(
        clippy::too_many_lines,
        reason = "a direct port of ModuleNode.levelCheck"
    )]
    pub(crate) fn check_module(&mut self, id: NodeId, itr: u32) -> bool {
        if self.checked(id) >= itr {
            return self.correct(id);
        }
        self.set_checked(id, itr);
        let (rec, top) = match &self.m.node(id).kind {
            NodeKind::Module(md) => (
                md.op_defs_in_recursive_section.clone(),
                md.top_level.clone(),
            ),
            _ => return true,
        };
        let mut first = 0;
        while first < rec.len() {
            let section = match &self.m.node(rec[first]).kind {
                NodeKind::OpDef(d) => d.recursive_section,
                _ => -1,
            };
            let mut end = first;
            while end < rec.len()
                && matches!(&self.m.node(rec[end]).kind, NodeKind::OpDef(d) if d.recursive_section == section)
            {
                end += 1;
            }
            let group: Vec<NodeId> = rec[first..end].to_vec();
            let in_rec = |s: &Self, n: NodeId| matches!(&s.m.node(n).kind, NodeKind::OpDef(d) if d.in_recursive);
            for &n in &group {
                if in_rec(self, n) {
                    self.set_opdef_flags(n, 1);
                    if let Some(l) = self.levels_mut(n) {
                        l.max_levels.fill(2);
                        l.weights.fill(1);
                    }
                } else {
                    self.set_opdef_flags(n, 0);
                }
            }
            let mut max_level = 0u8;
            let mut rec_lp = Set::new();
            let mut rec_ap = Set::new();
            for &n in &group {
                if in_rec(self, n) {
                    self.set_opdef_flags(n, 0);
                }
                self.check(n, 1);
                if in_rec(self, n) {
                    let arity = usize::try_from(self.arity(n)).unwrap_or(0);
                    for j in 0..arity {
                        if self
                            .levels(n)
                            .and_then(|l| l.max_levels.get(j).copied())
                            .unwrap_or(3)
                            < 2
                        {
                            let nn = self.name(n);
                            self.err(
                                Code::RecursivePrimesParameter,
                                n,
                                format!(
                                    "argument {} of recursive operator `{nn}` is primed",
                                    j + 1
                                ),
                            );
                        }
                    }
                    max_level = max_level.max(self.data(n).level);
                    rec_lp.extend(self.data(n).level_params.iter().copied());
                    rec_ap.extend(self.data(n).all_params.iter().copied());
                }
            }
            for &n in &group {
                if in_rec(self, n) {
                    self.set_opdef_flags(n, 2);
                }
                if let Some(d) = self.data_mut(n) {
                    d.level = d.level.max(max_level);
                    d.level_params.extend(rec_lp.iter().copied());
                    d.all_params.extend(rec_ap.iter().copied());
                }
            }
            for &n in &group {
                if in_rec(self, n) {
                    self.set_opdef_flags(n, 1);
                }
                self.check(n, 2);
            }
            first = end;
        }
        let mut correct = true;
        for m in self.module_inner(id) {
            if !self.check_module(m, 1) {
                correct = false;
            }
        }
        let op_defs = self.module_op_defs(id);
        for &d in &op_defs {
            if !self.check(d, 1) {
                correct = false;
            }
        }
        for d in self.module_thm_defs(id) {
            if !self.check(d, 1) {
                correct = false;
            }
        }
        for &t in &top {
            if !self.check(t, 1) {
                correct = false;
            }
        }
        let consts = crate::generate::decls_in(self.m, id, false);
        let mut d = self.data(id).clone();
        d.correct = correct;
        for &c in &consts {
            d.level_params.insert(c);
            d.all_params.insert(c);
        }
        if let Some(n) = self.m.node_mut(id) {
            n.level = d.clone();
        }
        if !self.module_is_constant(id) {
            for &c in &consts {
                lc_put(&mut d.level_constraints, c, 0);
            }
        }
        for &od in &op_defs {
            for (k, v) in self.lc(od) {
                lc_put(&mut d.level_constraints, k, v);
            }
            for (k, v) in self.alc(od) {
                alc_put(&mut d.arg_level_constraints, k, v);
            }
            let params = self.params(od);
            for a in self.alp(od) {
                if !params.contains(&a.0) && !params.contains(&a.2) {
                    d.arg_level_params.insert(a);
                }
            }
        }
        for &t in &top {
            for (k, v) in self.lc(t) {
                lc_put(&mut d.level_constraints, k, v);
            }
            for (k, v) in self.alc(t) {
                alc_put(&mut d.arg_level_constraints, k, v);
            }
            d.arg_level_params.extend(self.alp(t));
        }
        if let Some(n) = self.m.node_mut(id) {
            n.level = d;
        }
        correct
    }
}
