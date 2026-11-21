//! Typed work metadata, separate from authenticated owned value DAGs.

use crate::ending::Site;
use crate::parallel::{Ending, Expanded, Inv, Occ};
use dive_core::{Error, ErrorKind, Handle, TlcClass};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fault {
    kind: u8,
    code: Option<(u32, bool)>,
    message: String,
}
impl Fault {
    fn from(error: &Error) -> Self {
        Self {
            kind: match error.kind() {
                ErrorKind::Unsupported => 0,
                ErrorKind::Eval => 1,
                ErrorKind::Internal => 2,
                ErrorKind::Stop => 3,
            },
            code: error.tlc().map(|c| (c.code, c.eval_exception)),
            message: error.message().into(),
        }
    }
    fn error(self) -> Result<Error, Error> {
        match (self.kind, self.code) {
            (0, None) => Ok(Error::unsupported(self.message)),
            (1, None) => Ok(Error::eval(self.message)),
            (1, Some((code, eval_exception))) => Ok(Error::eval_class(
                TlcClass {
                    code,
                    eval_exception,
                },
                self.message,
            )),
            (2, None) => Ok(Error::internal(self.message)),
            (3, None) if self.message.is_empty() => Ok(Error::stop()),
            _ => Err(super::failure("remote error class")),
        }
    }
}

#[derive(Serialize, Deserialize)]
enum Test {
    Holds,
    Violated(usize),
    Error(usize, Fault),
}
impl Test {
    fn from(inv: &Inv) -> Self {
        match inv {
            Inv::Holds => Self::Holds,
            Inv::Violated(n) => Self::Violated(*n),
            Inv::Error(n, e) => Self::Error(*n, Fault::from(e)),
        }
    }
    fn inv(self, bound: usize) -> Result<Inv, Error> {
        match self {
            Self::Holds => Ok(Inv::Holds),
            Self::Violated(n) if n < bound => Ok(Inv::Violated(n)),
            Self::Error(n, e) if n < bound => Ok(Inv::Error(n, e.error()?)),
            _ => Err(super::failure("remote property index")),
        }
    }
}

#[derive(Serialize, Deserialize)]
enum Event {
    Old {
        key: u64,
        inside: bool,
        at: Option<usize>,
    },
    Candidate {
        key: u64,
        inside: bool,
        inv: Test,
        imp: Test,
        at: usize,
        identity: (usize, usize),
    },
    Incomplete {
        at: usize,
    },
    Check {
        site: (u8, usize),
        error: Fault,
        at: usize,
    },
}

#[derive(Serialize, Deserialize)]
enum End {
    Done,
    Budget,
    Retention(Fault),
    Generator {
        action: usize,
        emitted: u64,
        error: Fault,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    events: Vec<(usize, Event)>,
    end: End,
    warnings: Vec<(u64, (u16, u32))>,
    values: usize,
    counts: Counts,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Counts {
    total: u64,
    skipped: u64,
    discarded: u64,
}
impl Counts {
    pub(super) fn apply(self, metrics: &mut crate::Metrics) -> Result<(), Error> {
        if self.skipped > self.total {
            return Err(super::failure("remote work counters"));
        }
        metrics.conjuncts_total = metrics
            .conjuncts_total
            .checked_add(self.total)
            .ok_or_else(|| super::failure("remote work counter overflow"))?;
        metrics.conjuncts_skipped = metrics
            .conjuncts_skipped
            .checked_add(self.skipped)
            .ok_or_else(|| super::failure("remote work counter overflow"))?;
        metrics.discarded_before_intern = metrics
            .discarded_before_intern
            .checked_add(self.discarded)
            .ok_or_else(|| super::failure("remote work counter overflow"))?;
        Ok(())
    }
}

fn site(site: Site) -> (u8, usize) {
    match site {
        Site::Init => (0, 0),
        Site::Next(n) => (1, n),
        Site::Constraint(n) => (2, n),
        Site::ActionConstraint(n) => (3, n),
        Site::Invariant(n) => (4, n),
        Site::ImpliedInit(n) => (5, n),
        Site::ImpliedAction(n) => (6, n),
        Site::Incomplete => (7, 0),
        Site::Fingerprint => (8, 0),
        Site::View => (9, 0),
    }
}
fn from_site(value: (u8, usize), plan: &dive_core::Plan) -> Result<Site, Error> {
    let (tag, n) = value;
    match (tag, n) {
        (1, n) if n < plan.next.len() => Ok(Site::Next(n)),
        (2, n) if n < plan.constraints.len() => Ok(Site::Constraint(n)),
        (3, n) if n < plan.action_constraints.len() => Ok(Site::ActionConstraint(n)),
        (4, n) if n < plan.invariants.len() => Ok(Site::Invariant(n)),
        (6, n) if n < plan.implied_actions.len() => Ok(Site::ImpliedAction(n)),
        (7, 0) => Ok(Site::Incomplete),
        (8, 0) => Ok(Site::Fingerprint),
        _ => Err(super::failure("remote check site")),
    }
}

pub(super) fn encode(
    expanded: &Expanded,
    metrics: &crate::Metrics,
    limit: usize,
) -> Result<(Vec<u8>, Vec<Handle>), Error> {
    if expanded.payloads.is_some() {
        return Err(super::failure("remote transient spool"));
    }
    let mut roots = crate::capture_copy(&expanded.values)?;
    let mut events = Vec::new();
    events
        .try_reserve_exact(expanded.occs.len())
        .map_err(|_| super::failure("remote event allocation"))?;
    for (action, occ) in &expanded.occs {
        let event = match occ {
            Occ::Old { key, inside, at } => Event::Old {
                key: *key,
                inside: *inside,
                at: *at,
            },
            Occ::Cand {
                key,
                identity,
                in_model,
                inv,
                imp,
                at,
            } => {
                let first = roots.len();
                crate::capture_reserve(&mut roots, identity.len())?;
                roots.extend_from_slice(identity);
                Event::Candidate {
                    key: *key,
                    inside: *in_model,
                    inv: Test::from(inv),
                    imp: Test::from(imp),
                    at: *at,
                    identity: (first, identity.len()),
                }
            }
            Occ::Incomplete { at } => Event::Incomplete { at: *at },
            Occ::CheckErr {
                site: where_,
                error,
                at,
            } => Event::Check {
                site: site(*where_),
                error: Fault::from(error),
                at: *at,
            },
            Occ::CaptureLimit => return Err(super::failure("per-source successor admission")),
            Occ::CaptureBudget => {
                return Err(super::failure("remote work unit stopped incompletely"));
            }
        };
        events.push((*action, event));
    }
    let end = match &expanded.end {
        Ending::Done => End::Done,
        Ending::Budget => End::Budget,
        Ending::RetentionError(e) => End::Retention(Fault::from(e)),
        Ending::GenError {
            action,
            emitted,
            error,
        } => End::Generator {
            action: *action,
            emitted: *emitted,
            error: Fault::from(error),
        },
    };
    let metadata = Metadata {
        events,
        end,
        warnings: expanded.warnings.clone(),
        values: expanded.values.len(),
        counts: Counts {
            total: metrics.conjuncts_total,
            skipped: metrics.conjuncts_skipped,
            discarded: metrics.discarded_before_intern,
        },
    };
    let bytes = serde_json::to_vec(&metadata).map_err(super::failure)?;
    if bytes.len() > limit {
        return Err(super::failure("remote metadata admission"));
    }
    Ok((bytes, roots))
}

#[allow(
    clippy::too_many_lines,
    reason = "one result's ordered structural and authority checks reject atomically"
)]
pub(super) fn decode(
    bytes: &[u8],
    roots: &[Handle],
    plan: &dive_core::Plan,
    max: usize,
) -> Result<(Expanded, Counts), Error> {
    let metadata: Metadata = serde_json::from_slice(bytes).map_err(super::failure)?;
    let width = plan.vars.len();
    if width == 0
        || metadata.events.len() > max
        || metadata.values > roots.len()
        || metadata.events.len().checked_mul(width) != Some(metadata.values)
    {
        return Err(super::failure("remote result dimensions"));
    }
    let row = |at: usize| -> Result<usize, Error> {
        at.checked_add(width)
            .filter(|&end| end <= metadata.values && at.is_multiple_of(width))
            .map(|_| at)
            .ok_or_else(|| super::failure("remote occurrence row"))
    };
    let mut occs = Vec::new();
    occs.try_reserve_exact(metadata.events.len())
        .map_err(|_| super::failure("remote event allocation"))?;
    let mut suffix = metadata.values;
    let mut last_action = 0;
    let mut action_emitted = 0;
    for (action, event) in metadata.events {
        if action >= plan.next.len() || action < last_action {
            return Err(super::failure("remote action index/order"));
        }
        if action != last_action {
            action_emitted = 0;
        }
        last_action = action;
        action_emitted += 1;
        let expected = occs.len() * width;
        let occ = match event {
            Event::Old { .. } => {
                return Err(super::failure(
                    "remote membership result lacks exact row authority",
                ));
            }
            Event::Candidate {
                key,
                inside,
                inv,
                imp,
                at,
                identity: (start, len),
            } => {
                if len != width || start != suffix || at != expected {
                    return Err(super::failure("remote exact identity width/position"));
                }
                let end = start
                    .checked_add(len)
                    .ok_or_else(|| super::failure("remote identity arithmetic"))?;
                suffix = end;
                let identity = roots
                    .get(start..end)
                    .ok_or_else(|| super::failure("remote identity suffix"))?;
                Occ::Cand {
                    key,
                    in_model: inside,
                    inv: inv.inv(plan.invariants.len())?,
                    imp: imp.inv(plan.implied_actions.len())?,
                    at: row(at)?,
                    identity: crate::capture_copy(identity)?,
                }
            }
            Event::Incomplete { at } if at == expected => Occ::Incomplete { at: row(at)? },
            Event::Check { site, error, at } if at == expected => Occ::CheckErr {
                site: from_site(site, plan)?,
                error: error.error()?,
                at: row(at)?,
            },
            _ => return Err(super::failure("remote row order")),
        };
        occs.push((action, occ));
    }
    if suffix != roots.len() {
        return Err(super::failure("remote unused identity suffix"));
    }
    let end = match metadata.end {
        End::Done => Ending::Done,
        End::Budget => return Err(super::failure("remote work unit incomplete")),
        End::Retention(e) => return Err(super::failure(e.error()?.message())),
        End::Generator {
            action,
            emitted,
            error,
        } if action < plan.next.len()
            && action >= last_action
            && emitted
                == if action == last_action {
                    action_emitted
                } else {
                    0
                } =>
        {
            Ending::GenError {
                action,
                emitted,
                error: error.error()?,
            }
        }
        End::Generator { .. } => return Err(super::failure("remote generator ending")),
    };
    if metadata.warnings.iter().any(|(before, (var, _))| {
        *before > occs.len() as u64 || usize::from(*var) >= plan.vars.len()
    }) || metadata
        .warnings
        .windows(2)
        .any(|pair| pair[0].0 > pair[1].0)
    {
        return Err(super::failure("remote warning position"));
    }
    let values = crate::capture_copy(&roots[..metadata.values])?;
    Ok((
        Expanded {
            occs,
            values,
            payloads: None,
            end,
            deferred_print: false,
            warnings: metadata.warnings,
        },
        metadata.counts,
    ))
}

pub(super) fn join(meta: &[u8], dag: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let len = 8usize
        .checked_add(meta.len())
        .and_then(|n| n.checked_add(dag.len()))
        .filter(|&n| n <= limit)
        .ok_or_else(|| super::failure("remote frame payload admission"))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(super::failure)?;
    bytes.extend_from_slice(&(meta.len() as u64).to_le_bytes());
    bytes.extend_from_slice(meta);
    bytes.extend_from_slice(dag);
    Ok(bytes)
}
pub(super) fn split(bytes: &[u8]) -> Result<(&[u8], &[u8]), Error> {
    let word: [u8; 8] = bytes
        .get(..8)
        .ok_or_else(|| super::failure("remote payload header"))?
        .try_into()
        .map_err(super::failure)?;
    let end = usize::try_from(u64::from_le_bytes(word))
        .ok()
        .and_then(|n| n.checked_add(8))
        .filter(|&n| n <= bytes.len())
        .ok_or_else(|| super::failure("remote payload section"))?;
    Ok((&bytes[8..end], &bytes[end..]))
}
