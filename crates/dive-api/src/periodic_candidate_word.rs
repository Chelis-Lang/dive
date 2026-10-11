//! Private reconstruction from the actual internally created service run.
use std::collections::BTreeMap;

use dive_eval::{St, next::StateVals};
use dive_liveness::{periodic_lifecycle, phase_graph, tlc_witness};
use dive_trace::{Ending, Trace};

use super::{Error, Origin};

pub(super) struct Selected {
    pub origin: Origin,
    pub order: (usize, usize),
    pub candidate: tlc_witness::Candidate,
    pub identities: Vec<tlc_witness::Identity>,
    pub written: Vec<bool>,
}
pub(super) fn selected(run: &periodic_lifecycle::Run) -> Result<Option<Selected>, Error> {
    let lifecycle = run;
    let backend = lifecycle.backend();
    let Some(stop) = backend.stop() else {
        return Ok(None);
    };
    if !matches!(stop.error, phase_graph::Error::PartialCandidate) {
        return Ok(None);
    }
    if let Some(service) = lifecycle.request_services().last().filter(|service| {
        matches!(
            service.outcome(),
            Ok(dive_liveness::periodic::Outcome::LivenessStopped(2116))
        )
    }) {
        if lifecycle.safety_eof().is_some() || service.resume_requested() {
            return Err(Error::Reconstruction(
                "request candidate has inconsistent EOF/resume".into(),
            ));
        }
        let checkpoint = backend
            .checkpoints()
            .last()
            .ok_or_else(|| Error::Reconstruction("missing request checkpoint".into()))?;
        let index = checkpoint
            .candidate_check()
            .ok_or_else(|| Error::Reconstruction("missing deciding partial selection".into()))?;
        let check = backend
            .checks()
            .get(index)
            .ok_or_else(|| Error::Reconstruction("deciding partial selection index".into()))?;
        let candidate = check.selection().candidate().ok_or_else(|| {
            Error::Reconstruction("deciding partial selection lacks candidate".into())
        })?;
        if check.sources() != lifecycle.concrete_sources().len()
            || checkpoint.sources() != check.sources()
            || checkpoint.stats() != service.stats()
        {
            return Err(Error::Reconstruction(
                "request source-prefix correspondence".into(),
            ));
        }
        return Ok(Some(Selected {
            origin: Origin::Request(service.clone()),
            order: (check.group(), check.pem()),
            candidate: candidate.clone(),
            identities: check.identities().to_vec(),
            written: check.written().to_vec(),
        }));
    }
    let Some(eof) = lifecycle.safety_eof() else {
        return Err(Error::Reconstruction(
            "candidate lacks actual request or EOF origin".into(),
        ));
    };
    let final_check = lifecycle
        .final_checks()
        .last()
        .ok_or_else(|| Error::Reconstruction("EOF lacks final selection".into()))?;
    let candidate = final_check
        .selection()
        .candidate
        .as_ref()
        .ok_or_else(|| Error::Reconstruction("EOF final selection lacks candidate".into()))?;
    let order = final_check.order();
    let group = stop
        .groups
        .get(order.0)
        .ok_or_else(|| Error::Reconstruction("final OOS identity".into()))?;
    Ok(Some(Selected {
        origin: Origin::Eof(eof),
        order,
        candidate: candidate.clone(),
        identities: group
            .nodes
            .iter()
            .map(|node| tlc_witness::Identity {
                fingerprint: node.fingerprint,
                tableau: node.tableau,
            })
            .collect(),
        written: group.nodes.iter().map(|node| node.written).collect(),
    }))
}

struct Meter {
    remaining: usize,
}
impl Meter {
    fn charge(&mut self, amount: usize) -> Result<(), Error> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or(Error::Limit("shared graph/reconstruction inspections"))?;
        Ok(())
    }
}
fn identity(selected: &Selected, id: usize) -> Result<tlc_witness::Identity, Error> {
    if selected.written.get(id) != Some(&true) {
        return Err(Error::Reconstruction(
            "selected concrete record was not written".into(),
        ));
    }
    selected
        .identities
        .get(id)
        .copied()
        .ok_or_else(|| Error::Reconstruction("selected product identity".into()))
}

pub(super) fn reconstruct(
    model: &dive_model::Model,
    run: &periodic_lifecycle::Run,
    selected: &Selected,
    limits: phase_graph::Limits,
) -> Result<Trace, Error> {
    let backend = run.backend();
    let mut meter = Meter {
        remaining: limits
            .graph
            .steps
            .checked_sub(backend.graph_inspections())
            .ok_or(Error::Limit("shared graph/reconstruction inspections"))?,
    };
    let candidate = &selected.candidate;
    identity(selected, candidate.postfix_end)?;
    let count = candidate
        .prefix_reverse
        .len()
        .checked_add(candidate.cycle_stack.len())
        .and_then(|n| n.checked_add(candidate.closure_reverse.len()))
        .filter(|n| *n <= limits.graph.vertices)
        .ok_or(Error::Limit("concrete witness positions"))?;
    meter.charge(count)?;
    let mut representatives: BTreeMap<u64, &StateVals> = BTreeMap::new();
    for (index, source) in run.concrete_sources().iter().enumerate() {
        meter.charge(
            source
                .values()
                .len()
                .checked_add(1)
                .ok_or(Error::Limit("concrete source inspection"))?,
        )?;
        if usize::try_from(source.identity()).ok() != Some(index) {
            return Err(Error::Reconstruction(
                "original source discovery identity".into(),
            ));
        }
        if let Some(previous) = representatives.insert(source.fingerprint(), source.values())
            && previous != source.values()
        {
            return Err(Error::Reconstruction(
                "distinct concrete sources share a fingerprint".into(),
            ));
        }
    }
    let mut prefix = Vec::with_capacity(candidate.prefix_reverse.len());
    for &id in candidate.prefix_reverse.iter().rev() {
        let fp = identity(selected, id)?.fingerprint;
        if prefix.last() != Some(&fp) {
            prefix.push(fp);
        }
    }
    let anchor = identity(selected, candidate.anchor)?.fingerprint;
    if prefix.last() != Some(&anchor) {
        return Err(Error::Reconstruction("prefix/anchor correspondence".into()));
    }
    let back = prefix
        .len()
        .checked_sub(1)
        .ok_or_else(|| Error::Reconstruction("empty actual prefix".into()))?;
    // LongVec.pack removes contiguous identical concrete fingerprints, without
    // discarding product identities before pinned candidate selection.
    let mut reverse = candidate.closure_reverse.clone();
    for &id in candidate.cycle_stack.iter().rev() {
        let fp = identity(selected, id)?.fingerprint;
        if reverse.last() != Some(&fp) {
            reverse.push(fp);
        }
    }
    reverse.dedup();
    if reverse.last() == Some(&anchor) {
        reverse.pop();
    }
    prefix.extend(reverse.into_iter().rev());
    let mut states = Vec::with_capacity(prefix.len());
    for fp in prefix {
        meter.charge(1)?;
        let original = representatives.get(&fp).ok_or_else(|| {
            Error::Reconstruction("missing original concrete source representative".into())
        })?;
        states.push((*original).clone());
    }
    // Every referenced material value was admitted by the producer Session;
    // nevertheless a collision/unknown recursive source remains a refusal.
    // Atom-cell reservations are cumulative, not restarted for each property.
    let properties = attribute(
        model,
        &states,
        back,
        limits.graph.acceptance_cells,
        &mut meter,
    )?;
    let ending = if back + 1 == states.len() {
        Ending::Stuttering
    } else {
        Ending::Lasso { back_to: back }
    };
    Ok(Trace {
        labels: vec![None; states.len()],
        length: Some(states.len()),
        states,
        ending,
        properties,
    })
}
fn attribute(
    model: &dive_model::Model,
    states: &[StateVals],
    back: usize,
    maximum: usize,
    meter: &mut Meter,
) -> Result<Vec<String>, Error> {
    let mut properties = Vec::new();
    let mut cells = 0usize;
    for claim in model.property_claims() {
        meter.charge(1)?;
        let plan = dive_liveness::plan::lower_violation(
            model,
            &claim.pred,
            dive_liveness::formula::Limits::default(),
        )
        .map_err(|e| Error::Attribution(e.to_string()))?;
        let count = states
            .len()
            .checked_mul(plan.atoms.len())
            .ok_or(Error::Limit("attribution atom cells"))?;
        cells = cells
            .checked_add(count)
            .filter(|n| *n <= maximum)
            .ok_or(Error::Limit("attribution atom cells"))?;
        meter.charge(
            count
                .checked_add(states.len())
                .ok_or(Error::Limit("attribution work"))?,
        )?;
        let mut rows = Vec::with_capacity(states.len());
        for (i, current) in states.iter().enumerate() {
            let next = if i + 1 == states.len() { back } else { i + 1 };
            let state = St::action(current.clone(), states[next].clone());
            rows.push(
                plan.atoms
                    .iter()
                    .map(|atom| {
                        atom.evaluate(model, &state)
                            .map_err(|e| Error::Attribution(e.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let truth = plan
            .formula
            .evaluate_word(&rows, &rows, back, maximum)
            .map_err(|e| Error::Attribution(e.to_string()))?;
        if truth[0] && !properties.contains(&claim.name) {
            properties.push(claim.name.clone());
        }
    }
    if properties.is_empty() {
        return Err(Error::Attribution(
            "selected word falsifies no original configured property".into(),
        ));
    }
    Ok(properties)
}
