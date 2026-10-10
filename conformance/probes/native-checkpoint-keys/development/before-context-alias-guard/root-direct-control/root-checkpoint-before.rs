//! Native completed-level safety snapshots (ADR 0033).
//!
//! This format restores a trusted local search, not an independent certificate.
//! Checksums detect corruption, not malicious rewriting. Java checkpoint bytes,
//! temporal graphs, runtime effects and parallel recovery are not admitted.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dive_config::ModelConfig;
use dive_eval::next::StateVals;
use dive_ir::{Effects, Ir};
use dive_model::Model;
use dive_store::state::{StateDecoder, encode_state};
use dive_store::trace::StateId;
use dive_value::{Interner, codec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::{Keyed, Search};
use crate::{CheckOptions, CheckResult, Verdict};

const MAGIC: &[u8; 8] = b"DIVECP01";
const SEMANTICS: &str = "tlc-94d0c50/native-checkpoint-0033-v1";
const HEADER: usize = 48;
/// Maximum complete native payload: 64 MiB. Codec/writer workspaces and ordinary
/// search retention remain outside this archive bound.
pub const MAX_PAYLOAD_BYTES: usize = 64 << 20;
/// Maximum discovered states admitted by the first snapshot slice.
pub const MAX_STATES: usize = 65_536;
/// Maximum lossless state/key record and inspected representation: 1 MiB.
pub const MAX_STATE_BYTES: usize = 1 << 20;
/// Maximum native restored BFS depth. Parents must precede their children.
pub const MAX_LEVEL: u64 = 65_536;
/// Largest positive TLC `long` counter admitted by this native schema.
pub const MAX_COUNTER: u64 = 9_223_372_036_854_775_807;
static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Recovery counting policy. Both profiles retain TLC's default unless explicitly
/// changed; `Retain` is a reported result-changing native option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Counters {
    /// TLC resets generated and initial counters to recovered distinct states.
    #[default]
    Tlc,
    /// Preserve original initial/generated totals, matching uninterrupted search.
    Retain,
}

/// When to write or load a native safety snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Stop before the next level after `n` complete expansion levels. Zero
    /// pauses immediately after initial-state enumeration. Never a model verdict.
    PauseAfterLevels(u64),
    /// Continue from the actual stored frontier, without enumerating Init again.
    Recover,
}

/// A separate opt-in native checkpoint request; ordinary `check` is unchanged.
///
/// ```
/// use dive_check::checkpoint::{Action, Counters, Request};
/// let request = Request {
///     path: "counter.cp".into(),
///     action: Action::PauseAfterLevels(2),
///     counters: Counters::Tlc,
///     config_source_sha256: None,
/// };
/// assert_eq!(request.action, Action::PauseAfterLevels(2));
/// ```
#[derive(Clone, Debug)]
pub struct Request {
    /// One native archive. Writers create a new file and never overwrite one.
    pub path: PathBuf,
    /// Pausing or recovery.
    pub action: Action,
    /// Resolved recovery counter behavior, reported even for a pause.
    pub counters: Counters,
    /// Optional exact `.cfg` source-byte hash supplied by a file loader. Direct
    /// programmatic `ModelConfig` callers instead pin its complete binding structure.
    pub config_source_sha256: Option<String>,
}

/// Native execution receipt. Serialization alone never proves model correctness.
#[derive(Clone, Debug, Serialize)]
pub struct Receipt {
    /// `paused-incomplete` or `recovered`; a paused run is not exhaustive success.
    pub status: &'static str,
    /// Payload SHA-256, not an authenticity guarantee.
    pub payload_sha256: String,
    /// Encoded native archive bytes.
    pub archive_bytes: u64,
    /// Next frontier's BFS level, where actual recovery starts.
    pub next_level: u64,
    /// Concrete discovery records persisted.
    pub discovered: u64,
    /// Concrete ordered queue entries persisted.
    pub queued: u64,
    /// Explicit counter policy.
    pub counters: Counters,
}

/// A check plus its native checkpoint evidence. Failed admission has no receipt.
#[derive(Clone, Debug)]
pub struct Result {
    /// Ordinary safety result and truthful actual workers. Pauses are resource stops.
    pub check: CheckResult,
    /// The actual committed/imported archive, if any.
    pub receipt: Option<Receipt>,
}

type Frontier = Vec<(StateVals, StateId)>;
type Restored = Option<(Frontier, u64)>;
type Fallible<T> = std::result::Result<T, String>;

/// Checks or resumes a pure safety model with a native snapshot.
///
/// This first slice runs the level engine inline with one actual worker even if
/// more were requested. Runtime effects, names, evaluator-driven deferred state values,
/// VIEW/SYMMETRY, temporal properties and transient-spill options refuse by name.
/// Identity and archive structure are validated before evaluation. Recovery
/// restores names before binding and resumes the saved frontier without regenerating
/// the search from Init. Ordinary concrete witness replay remains unchanged.
#[must_use]
pub fn check(ir: &Arc<Ir>, cfg: &ModelConfig, options: &CheckOptions, request: &Request) -> Result {
    if let Err(error) = dive_eval::validate_ir(ir) {
        return Result {
            check: crate::bare(crate::context_verdict(&error), options, ir),
            receipt: None,
        };
    }
    let started = std::time::Instant::now();
    let run = || {
        let mut session = match Session::prepare(ir, cfg, options, request) {
            Ok(s) => s,
            Err(verdict) => {
                return Result {
                    check: crate::bare(verdict, options, ir),
                    receipt: None,
                };
            }
        };
        let mut check = crate::run_check_with_checkpoint(ir, cfg, options, Some(&mut session));
        if session.receipt.is_none()
            && matches!(request.action, Action::PauseAfterLevels(_))
            && check.verdict == Verdict::Success
        {
            check.notes.push("Search completed before the requested checkpoint boundary; no snapshot was committed.".into());
        }
        check.elapsed = started.elapsed();
        Result {
            check,
            receipt: session.receipt,
        }
    };
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, run)
        {
            Ok(thread) => thread.join().unwrap_or_else(|_| Result {
                check: crate::bare(
                    Verdict::InternalError {
                        message: "native checkpoint checker panicked".into(),
                    },
                    options,
                    ir,
                ),
                receipt: None,
            }),
            Err(e) => Result {
                check: crate::bare(
                    failure(format!("cannot start checkpoint thread: {e}")),
                    options,
                    ir,
                ),
                receipt: None,
            },
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    schema: u32,
    semantics: String,
    executable_sha256: String,
    ir_sha256: String,
    config_sha256: String,
    config_source_sha256: Option<String>,
    seed: i64,
    fp: usize,
    options: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Discovery {
    parent: Option<StateId>,
    ordinal: u32,
    #[serde(deserialize_with = "bounded_bytes")]
    key: Vec<u8>,
    #[serde(deserialize_with = "bounded_bytes")]
    state: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    identity: Identity,
    #[serde(deserialize_with = "bounded_names")]
    names: Vec<String>,
    next_level: u64,
    generated: u64,
    depth: u64,
    initial_generated: u64,
    #[serde(deserialize_with = "bounded_discoveries")]
    discoveries: Vec<Discovery>,
    #[serde(deserialize_with = "bounded_frontier")]
    frontier: Vec<StateId>,
}

pub(crate) struct Session {
    request: Request,
    identity: Identity,
    imported: Option<Snapshot>,
    discoveries: Vec<Discovery>,
    encoded_bytes: usize,
    pub(crate) receipt: Option<Receipt>,
    pending_receipt: Option<Receipt>,
}

pub(crate) fn failure(message: impl Into<String>) -> Verdict {
    Verdict::ResourceLimit {
        message: format!("DIVE-S0400: native checkpoint {}", message.into()),
    }
}
fn unsupported(feature: impl Into<String>) -> Verdict {
    Verdict::Unsupported {
        feature: format!("native checkpoint {}", feature.into()),
    }
}
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| {
            [
                char::from(HEX[usize::from(b >> 4)]),
                char::from(HEX[usize::from(b & 15)]),
            ]
        })
        .collect()
}
fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn executable_hash() -> Fallible<String> {
    let mut file = File::open(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut bytes = [0u8; 8192];
    loop {
        let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&bytes[..n]);
    }
    Ok(hex(&digest.finalize()))
}

impl Session {
    #[allow(
        clippy::too_many_lines,
        reason = "ordered pre-effect admission and identity checks share one checkpoint transaction"
    )]
    fn prepare(
        ir: &Ir,
        cfg: &ModelConfig,
        opts: &CheckOptions,
        request: &Request,
    ) -> std::result::Result<Self, Verdict> {
        if ir.vars().len() > 1024 {
            return Err(unsupported("state variable arity above 1024"));
        }
        if request.path.as_os_str().is_empty() {
            return Err(failure("empty path"));
        }
        if matches!(request.action, Action::PauseAfterLevels(n) if n >= MAX_LEVEL) {
            return Err(failure("pause level exceeds native depth bound"));
        }
        if opts.frontier_memory_bytes.is_some()
            || opts.frontier_directory.is_some()
            || opts.predecessor_memory_bytes.is_some()
            || opts.predecessor_directory.is_some()
        {
            return Err(unsupported(
                "with transient predecessor spill is not implemented",
            ));
        }
        if cfg.view.is_some()
            || cfg.symmetry.is_some()
            || cfg.specification.is_some()
            || !cfg.properties.is_empty()
            || cfg.periodic.is_some()
            || cfg.rl_reward.is_some()
            || !cfg.possible.is_empty()
        {
            return Err(unsupported(
                "first slice accepts INIT/NEXT safety without VIEW/SYMMETRY/PROPERTY/SPECIFICATION/periodic hooks",
            ));
        }
        // Before binding: even unused constant definitions can execute at startup.
        for d in ir.defs().iter().filter(|d| {
            !ir.modules()
                .get(d.module.index())
                .is_some_and(|m| m.standard)
        }) {
            if d.effects.intersects(
                Effects::IMPURE
                    .union(Effects::LIVE_TLCGET)
                    .union(Effects::TEMPORAL),
            ) {
                return Err(unsupported(format!(
                    "runtime/temporal effects in {}",
                    d.name
                )));
            }
            if let Some(op) = d.std
                && op.creates_runtime_names()
            {
                return Err(unsupported(format!("runtime names in {}", d.name)));
            }
        }
        // Conservative across all source nodes, including unused definitions and
        // redirects. Std constructors can create names even in canonical profile.
        for node in ir.nodes() {
            for op in node.expr.op_refs() {
                let op = match op {
                    dive_ir::OpRef::Std(op) => Some(*op),
                    dive_ir::OpRef::Def(id) => ir.defs().get(id.index()).and_then(|d| d.std),
                    _ => None,
                };
                if op.is_some_and(|op| {
                    op.creates_runtime_names()
                        || matches!(
                            op,
                            dive_ir::StdOp::TlcTLCGet
                                | dive_ir::StdOp::TlcTLCSet
                                | dive_ir::StdOp::TlcExtTLCGetOrDefault
                        )
                }) {
                    return Err(unsupported(
                        "runtime name constructors or checker/register reads are not recoverable in this slice",
                    ));
                }
            }
        }
        let ir_bytes = serde_json::to_vec(ir).map_err(|e| failure(e.to_string()))?;
        let identity = Identity {
            schema: 1,
            semantics: SEMANTICS.into(),
            executable_sha256: executable_hash().map_err(failure)?,
            ir_sha256: hash(&ir_bytes),
            config_sha256: hash(format!("{cfg:?}").as_bytes()),
            config_source_sha256: request.config_source_sha256.clone(),
            seed: opts.seed,
            fp: opts.fp_index,
            // Worker/chaos are execution choices, excluded from semantic identity.
            options: format!(
                "{:?}/{:?}/{:?}/{}/{}/{}/{}/{}/{}/{}",
                opts.profile,
                opts.eval.ctx,
                opts.eval.exploration,
                opts.eval.max_depth,
                opts.eval.warnings,
                opts.fp_index,
                opts.tlc_fingerprints,
                opts.seed,
                opts.no_deadlock,
                format_args!("{:?}/{:?}", opts.store, opts.eval.comparisons)
            ),
        };
        let mut session = Self {
            request: request.clone(),
            identity,
            imported: None,
            discoveries: Vec::new(),
            encoded_bytes: 0,
            receipt: None,
            pending_receipt: None,
        };
        match request.action {
            Action::PauseAfterLevels(_) => {
                if request
                    .path
                    .try_exists()
                    .map_err(|e| failure(e.to_string()))?
                {
                    return Err(failure("destination exists; refusing overwrite"));
                }
            }
            Action::Recover => {
                let (snapshot, payload_sha256, archive_bytes) =
                    read(&request.path).map_err(failure)?;
                if snapshot.identity != session.identity {
                    return Err(failure(
                        "identity mismatch: executable/schema/oracle/IR/config/profile/options",
                    ));
                }
                validate_structure(&snapshot).map_err(failure)?;
                session.pending_receipt = Some(Receipt {
                    status: "recovered",
                    payload_sha256,
                    archive_bytes,
                    next_level: snapshot.next_level,
                    discovered: snapshot.discoveries.len() as u64,
                    queued: snapshot.frontier.len() as u64,
                    counters: request.counters,
                });
                session.imported = Some(snapshot);
            }
        }
        Ok(session)
    }

    pub(crate) fn names(&self) -> Option<&[String]> {
        self.imported.as_ref().map(|s| s.names.as_slice())
    }
    pub(crate) fn admit_model(&mut self, m: &Model) -> std::result::Result<(), Verdict> {
        if !m.sequential.is_empty()
            || m.runtime_name_order
            || !m.temporals.is_empty()
            || !m.implied_temporals.is_empty()
        {
            return Err(unsupported("runtime effects/names or temporal state"));
        }
        let names = names(&m.names).map_err(failure)?;
        self.encoded_bytes = names.iter().map(|name| name.len() + 8).sum();
        if let Some(snapshot) = &self.imported
            && names != snapshot.names
        {
            return Err(failure(
                "restored ordered name table does not match admitted startup",
            ));
        }
        Ok(())
    }

    pub(crate) fn record<S: Keyed>(
        &mut self,
        key: &S::Key,
        state: &StateVals,
        parent: Option<StateId>,
        ordinal: u32,
        ctx: dive_value::Ctx,
    ) -> std::result::Result<(), Verdict> {
        if self.discoveries.len() >= MAX_STATES {
            return Err(failure("65536 discovery limit"));
        }
        let mut bound = 0usize;
        let values: Vec<_> = state
            .iter()
            .cloned()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| failure("incomplete state in discovery"))?;
        for value in &values {
            bound = bound
                .checked_add(
                    codec::frontier_size_bound(value, MAX_STATE_BYTES.saturating_sub(bound))
                        .map_err(|e| unsupported(e.to_string()))?,
                )
                .ok_or_else(|| failure("state byte bound overflow"))?;
            if bound > MAX_STATE_BYTES {
                return Err(failure("state record limit"));
            }
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(bound)
            .map_err(|e| failure(e.to_string()))?;
        encode_state(&values, ctx, &mut bytes).map_err(|e| failure(e.to_string()))?;
        let key = S::checkpoint_key(key);
        if key.len() > MAX_STATE_BYTES {
            return Err(failure("key record limit"));
        }
        let extra = bytes.len().saturating_add(key.len()).saturating_add(64);
        self.encoded_bytes = self
            .encoded_bytes
            .checked_add(extra)
            .ok_or_else(|| failure("payload size overflow"))?;
        if self.encoded_bytes > MAX_PAYLOAD_BYTES / 6 {
            return Err(failure("snapshot representation admission bound"));
        }
        self.discoveries
            .try_reserve(1)
            .map_err(|e| failure(e.to_string()))?;
        self.discoveries.push(Discovery {
            parent,
            ordinal,
            key,
            state: bytes,
        });
        Ok(())
    }

    pub(crate) fn restore<S: Keyed>(
        &mut self,
        search: &mut Search<'_, S>,
    ) -> std::result::Result<Restored, Verdict> {
        let Some(snapshot) = self.imported.take() else {
            return Ok(None);
        };
        let mut decoder = StateDecoder::new(&search.m.names);
        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(snapshot.frontier.len())
            .map_err(|e| failure(e.to_string()))?;
        let first_frontier = snapshot.frontier.first().copied().unwrap_or(u64::MAX);
        for (i, discovery) in snapshot.discoveries.iter().enumerate() {
            encoded_shape(
                &discovery.state,
                search.m.ir.vars().len(),
                search.m.names.len(),
            )
            .map_err(failure)?;
            let state = decoder
                .decode(&discovery.state)
                .map_err(|e| failure(e.to_string()))?;
            if state.len() != search.m.ir.vars().len() {
                return Err(failure("state variable arity mismatch"));
            }
            let key = S::key(search.keyer, &state, false, search.ctx)
                .map_err(|e| failure(e.to_string()))?;
            if S::checkpoint_key(&key) != discovery.key || !search.set.insert(key) {
                return Err(failure(
                    "discovery state/key mismatch or duplicate visited key",
                ));
            }
            let id = match discovery.parent {
                None => search.log.push_initial(discovery.ordinal),
                Some(parent) => search.log.push(parent, discovery.ordinal),
            }
            .map_err(|e| failure(e.to_string()))?;
            if id != i as u64 {
                return Err(failure("discovery/log ID mismatch"));
            }
            if id >= first_frontier {
                frontier.push((
                    StateVals::from(state.into_iter().map(Some).collect::<Vec<_>>()),
                    id,
                ));
            }
        }
        search.generated = match self.request.counters {
            Counters::Tlc => search.set.len(),
            Counters::Retain => snapshot.generated,
        };
        search.initial_generated = match self.request.counters {
            Counters::Tlc => search.set.len(),
            Counters::Retain => snapshot.initial_generated,
        };
        search.depth = snapshot.depth;
        self.discoveries = snapshot.discoveries;
        self.encoded_bytes = self
            .discoveries
            .iter()
            .map(|d| d.key.len() + d.state.len() + 64)
            .sum::<usize>()
            + snapshot
                .names
                .iter()
                .map(|name| name.len() + 8)
                .sum::<usize>();
        self.receipt = self.pending_receipt.take();
        Ok(Some((frontier, snapshot.next_level)))
    }

    pub(crate) fn pause_due(&self, level: u64) -> bool {
        matches!(self.request.action, Action::PauseAfterLevels(n) if level > n)
    }
    pub(crate) fn pause<S: Keyed>(
        &mut self,
        search: &Search<'_, S>,
        frontier: &crate::frontier::Queue,
        level: u64,
    ) -> std::result::Result<(), Verdict> {
        if frontier.len() > MAX_STATES {
            return Err(failure("frontier state count admission limit"));
        }
        let mut ids = Vec::new();
        ids.try_reserve_exact(frontier.len())
            .map_err(|e| failure(e.to_string()))?;
        for i in 0..frontier.len() {
            let (_, id) = frontier.read(i, &search.m.names)?;
            ids.push(id);
        }
        let snapshot = Snapshot {
            identity: self.identity.clone(),
            names: names(&search.m.names).map_err(failure)?,
            next_level: level,
            generated: search.generated,
            depth: search.depth,
            initial_generated: search.initial_generated,
            discoveries: std::mem::take(&mut self.discoveries),
            frontier: ids,
        };
        validate_structure(&snapshot).map_err(failure)?;
        let payload = serde_json::to_vec(&snapshot).map_err(|e| failure(e.to_string()))?;
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(failure("payload byte limit"));
        }
        let sha = hash(&payload);
        write(&self.request.path, &payload).map_err(failure)?;
        self.receipt = Some(Receipt {
            status: "paused-incomplete",
            payload_sha256: sha,
            archive_bytes: (HEADER + payload.len()) as u64,
            next_level: level,
            discovered: snapshot.discoveries.len() as u64,
            queued: snapshot.frontier.len() as u64,
            counters: self.request.counters,
        });
        Ok(())
    }
}

fn names(names: &Interner) -> Fallible<Vec<String>> {
    if names.len() > MAX_STATES {
        return Err("name table limit".into());
    }
    let mut total = 0usize;
    let mut result = Vec::new();
    result
        .try_reserve_exact(names.len())
        .map_err(|e| e.to_string())?;
    for i in 0..names.len() {
        let atom = names
            .resolve(u32::try_from(i).map_err(|e| e.to_string())?)
            .ok_or_else(|| "missing intern token".to_owned())?;
        total = total
            .checked_add(atom.as_str().len() + 8)
            .ok_or_else(|| "name byte bound overflow".to_owned())?;
        if atom.as_str().len() > MAX_STATE_BYTES || total > MAX_PAYLOAD_BYTES / 6 {
            return Err("name byte admission limit".into());
        }
        result.push(atom.as_str().to_owned());
    }
    Ok(result)
}

fn validate_structure(s: &Snapshot) -> Fallible<()> {
    let count = s.discoveries.len();
    if count == 0
        || count > MAX_STATES
        || s.names.len() > MAX_STATES
        || s.next_level == 0
        || s.next_level > MAX_LEVEL
        || s.depth != s.next_level
        || s.generated > MAX_COUNTER
        || s.generated < count as u64
        || s.initial_generated == 0
        || s.initial_generated > s.generated
    {
        return Err("invalid snapshot count/depth/admission bounds".into());
    }
    let mut unique_names = std::collections::BTreeSet::new();
    let mut representation_bytes = 0usize;
    for name in &s.names {
        representation_bytes += name.len() + 8;
        if representation_bytes > MAX_PAYLOAD_BYTES / 6
            || name.len() > MAX_STATE_BYTES
            || !unique_names.insert(name)
        {
            return Err("duplicate/oversized intern name".into());
        }
    }
    let mut depths = Vec::with_capacity(count);
    let mut last = 1;
    let mut last_parent: Option<u64> = None;
    let mut last_ordinal: Option<u32> = None;
    let mut roots = 0u64;
    let mut first_frontier = None;
    for (id, d) in s.discoveries.iter().enumerate() {
        representation_bytes = representation_bytes
            .saturating_add(d.state.len())
            .saturating_add(d.key.len())
            .saturating_add(64);
        if representation_bytes > MAX_PAYLOAD_BYTES / 6
            || d.state.len() > MAX_STATE_BYTES
            || d.key.is_empty()
            || d.key.len() > MAX_STATE_BYTES
        {
            return Err("record size bound".into());
        }
        if let Some(parent) = d.parent {
            if last_parent.is_some_and(|previous| parent < previous)
                || (last_parent == Some(parent)
                    && last_ordinal.is_some_and(|ordinal| d.ordinal <= ordinal))
            {
                return Err("parent/ordinal discovery ordering inconsistent".into());
            }
            last_parent = Some(parent);
        } else {
            roots += 1;
            if last_parent.is_some() || last_ordinal.is_some_and(|ordinal| d.ordinal <= ordinal) {
                return Err("initial ordinal ordering inconsistent".into());
            }
        }
        last_ordinal = Some(d.ordinal);
        let level = if let Some(parent) = d.parent {
            let parent = usize::try_from(parent).map_err(|e| e.to_string())?;
            if parent >= id {
                return Err("parent must be an earlier discovery".into());
            }
            depths[parent] + 1
        } else {
            1
        };
        if level < last || level > s.next_level {
            return Err("discovery order/depth inconsistent".into());
        }
        if level == s.next_level && first_frontier.is_none() {
            first_frontier = Some(id);
        }
        depths.push(level);
        last = level;
    }
    let minimum_generated = s
        .initial_generated
        .checked_add(count as u64 - roots)
        .ok_or_else(|| "initial/generated counter overflow".to_owned())?;
    if roots > s.initial_generated || s.generated < minimum_generated {
        return Err("initial/generated counter consistency".into());
    }
    let first = first_frontier.ok_or_else(|| "no next-level discoveries".to_owned())?;
    if s.frontier.len() != count - first
        || s.frontier
            .iter()
            .enumerate()
            .any(|(i, id)| *id != (first + i) as u64)
    {
        return Err("frontier must be the complete ordered discovery suffix at next level".into());
    }
    Ok(())
}

fn read(path: &Path) -> Fallible<(Snapshot, String, u64)> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    if len < HEADER as u64 || len > (HEADER + MAX_PAYLOAD_BYTES) as u64 {
        return Err("archive byte bound".into());
    }
    let mut header = [0u8; HEADER];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..8] != MAGIC {
        return Err("native magic/schema mismatch".into());
    }
    let mut n = [0u8; 8];
    n.copy_from_slice(&header[8..16]);
    let size = usize::try_from(u64::from_le_bytes(n)).map_err(|e| e.to_string())?;
    if size > MAX_PAYLOAD_BYTES || size as u64 + HEADER as u64 != len {
        return Err("payload length/trailing bytes mismatch".into());
    }
    let mut payload = Vec::new();
    payload.try_reserve_exact(size).map_err(|e| e.to_string())?;
    payload.resize(size, 0);
    file.read_exact(&mut payload).map_err(|e| e.to_string())?;
    let mut trailing = [0];
    if file.read(&mut trailing).map_err(|e| e.to_string())? != 0 {
        return Err("archive changed or has trailing bytes".into());
    }
    let sha = Sha256::digest(&payload);
    if sha.as_slice() != &header[16..48] {
        return Err("payload checksum mismatch".into());
    }
    let snapshot = serde_json::from_slice(&payload).map_err(|e| e.to_string())?;
    Ok((snapshot, hex(&sha), len))
}

fn write(path: &Path, payload: &[u8]) -> Fallible<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .ok_or_else(|| "checkpoint needs a file name".to_owned())?
        .to_string_lossy();
    let staging = parent.join(format!(
        ".{stem}.{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&staging).map_err(|e| e.to_string())?;
        file.write_all(MAGIC)
            .and_then(|()| file.write_all(&(payload.len() as u64).to_le_bytes()))
            .and_then(|()| file.write_all(&Sha256::digest(payload)))
            .and_then(|()| file.write_all(payload))
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        // hard_link is an atomic no-replace commit on the same filesystem; the
        // single complete immutable archive becomes visible at once.
        fs::hard_link(&staging, path).map_err(|e| e.to_string())?;
        fs::remove_file(&staging)
            .map_err(|e| format!("committed but staging cleanup failed: {e}"))?;
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| format!("committed but directory sync failed: {e}"))?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(original) => {
            failed_staging_cleanup(original, staging.try_exists(), || fs::remove_file(&staging))
        }
    }
}

fn failed_staging_cleanup(
    original: String,
    inspection: std::io::Result<bool>,
    cleanup: impl FnOnce() -> std::io::Result<()>,
) -> Fallible<()> {
    match inspection {
        Ok(false) => Err(original),
        Ok(true) => match cleanup() {
            Ok(()) => Err(original),
            Err(error) => Err(format!(
                "original failure: {original}; staging cleanup failed: {error}"
            )),
        },
        Err(error) => Err(format!(
            "original failure: {original}; staging inspection failed: {error}"
        )),
    }
}

/// Refuses oversized arrays before enqueueing their next element. JSON's overall
/// byte and recursive-descent bounds are independent of these retained vectors.
fn bounded<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>, const N: usize>(
    d: D,
) -> std::result::Result<Vec<T>, D::Error> {
    struct Limited<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Limited<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "at most {N} native items")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut input: A,
        ) -> std::result::Result<Vec<T>, A::Error> {
            let mut result = Vec::new();
            while let Some(value) = input.next_element()? {
                if result.len() == N {
                    return Err(serde::de::Error::custom("native array admission limit"));
                }
                result.try_reserve(1).map_err(serde::de::Error::custom)?;
                result.push(value);
            }
            Ok(result)
        }
    }
    d.deserialize_seq(Limited::<T, N>(std::marker::PhantomData))
}
fn bounded_bytes<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
    bounded::<_, _, MAX_STATE_BYTES>(d)
}
fn bounded_names<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<String>, D::Error> {
    bounded::<_, _, MAX_STATES>(d)
}
fn bounded_discoveries<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<Discovery>, D::Error> {
    bounded::<_, _, MAX_STATES>(d)
}
fn bounded_frontier<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<StateId>, D::Error> {
    bounded::<_, _, MAX_STATES>(d)
}

/// Inspects native encoded representation without constructing values. This
/// prevents hostile declared counts/nesting from creating large decoder stacks
/// before refusal. Every scalar/name/container is accounted before allocation.
#[allow(
    clippy::too_many_lines,
    reason = "one bounded non-evaluating native codec tag inspection"
)]
fn encoded_shape(bytes: &[u8], variables: usize, names: usize) -> Fallible<()> {
    let mut input = bytes;
    let mut stack = vec![(variables, false)];
    let mut nodes = 0usize;
    let read_byte = |input: &mut &[u8]| -> Fallible<u8> {
        let (b, rest) = input
            .split_first()
            .ok_or_else(|| "truncated native value".to_owned())?;
        *input = rest;
        Ok(*b)
    };
    let number = |input: &mut &[u8]| -> Fallible<usize> {
        usize::try_from(codec::read_varint(input).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    let token = |input: &mut &[u8]| -> Fallible<()> {
        if number(input)? >= names {
            return Err("unknown native intern token".into());
        }
        Ok(())
    };
    let integer = |input: &mut &[u8], tag: u8| -> Fallible<()> {
        match tag {
            2 => {
                number(input)?;
            }
            3 => {
                if read_byte(input)? > 1 {
                    return Err("invalid integer sign".into());
                }
                let n = number(input)?;
                if n > input.len() {
                    return Err("truncated native integer".into());
                }
                *input = &input[n..];
            }
            _ => return Err("expected native integer".into()),
        }
        Ok(())
    };
    while let Some((left, named)) = stack.last_mut() {
        if *left == 0 {
            stack.pop();
            continue;
        }
        *left -= 1;
        if *named {
            token(&mut input)?;
        }
        nodes += 1;
        if nodes > codec::FRONTIER_MAX_NODES {
            return Err("native representation node admission limit".into());
        }
        let tag = read_byte(&mut input)?;
        match tag {
            0 | 1 => {}
            2 | 3 => integer(&mut input, tag)?,
            4 | 5 => token(&mut input)?,
            _ => {
                let t = tag & !1;
                let (children, fields) = match t {
                    0x14 => {
                        let t = read_byte(&mut input)?;
                        integer(&mut input, t)?;
                        let t = read_byte(&mut input)?;
                        integer(&mut input, t)?;
                        (0, false)
                    }
                    0x1E | 0x20 | 0x22 => (0, false),
                    0x16 | 0x24 => (1, false),
                    0x18 => (2, false),
                    0x08 | 0x10 | 0x12 | 0x1A | 0x1C | 0x26 | 0x28 | 0x2A => {
                        if t == 0x12 && read_byte(&mut input)? > 1 {
                            return Err("invalid raw-set normalization flag".into());
                        }
                        let n = number(&mut input)?;
                        let children = if t == 0x2A {
                            n.checked_mul(2)
                                .ok_or_else(|| "native count overflow".to_owned())?
                        } else {
                            n
                        };
                        (children, matches!(t, 0x1A | 0x26 | 0x28))
                    }
                    _ => return Err("unknown native value tag".into()),
                };
                if children > input.len() || children > codec::FRONTIER_MAX_NODES - nodes {
                    return Err("native count exceeds remaining bytes/node bound".into());
                }
                if children > 0 {
                    if stack.len() >= 128 {
                        return Err("native representation depth admission limit".into());
                    }
                    stack.push((children, fields));
                }
            }
        }
    }
    if !input.is_empty() {
        return Err("trailing native state bytes/arity mismatch".into());
    }
    Ok(())
}

/// SHA-256 of exact config source bytes, for file-based checkpoint identity.
#[must_use]
pub fn config_source_hash(bytes: &[u8]) -> String {
    hash(bytes)
}

/// Reads saved seed/fingerprint defaults without evaluating a model. Full input,
/// profile/executable/name/state identity is still required by [`check`].
///
/// # Errors
/// Explicit native format/checksum/admission failure; never a search result.
pub fn recovery_seed_fp(path: &Path) -> std::result::Result<(i64, usize), String> {
    let (snapshot, _, _) = read(path)?;
    validate_structure(&snapshot)?;
    Ok((snapshot.identity.seed, snapshot.identity.fp))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "negative native-format controls")]
    use super::*;
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "dive-cp-format-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).expect("dir");
        p
    }
    #[test]
    fn malformed_encoded_counts_depth_and_tokens_refuse_before_decoding() {
        assert!(encoded_shape(&[2, 0], 1, 1).is_ok());
        assert!(encoded_shape(&[0x08, 255, 255, 255, 255, 15], 1, 1).is_err());
        assert!(encoded_shape(&[4, 255, 255, 255, 255, 15], 1, 1).is_err());
        assert!(encoded_shape(&[0x12, 2, 0], 1, 1).is_err());
        assert!(encoded_shape(&[2, 0, 2, 0], 1, 1).is_err());
        let mut deep = vec![0x08, 1];
        for _ in 0..128 {
            deep.extend([0x08, 1]);
        }
        deep.push(0);
        assert!(encoded_shape(&deep, 1, 1).is_err());
    }
    #[test]
    fn failed_staging_inspection_and_cleanup_preserve_original_operation() {
        let calls = std::cell::Cell::new(0);
        let inspection = failed_staging_cleanup(
            "original commit failure".into(),
            Err(std::io::Error::other("inspection denied")),
            || {
                calls.set(calls.get() + 1);
                Ok(())
            },
        );
        let message = inspection.expect_err("inspection fails");
        assert!(
            message.contains("original commit failure") && message.contains("inspection denied")
        );
        assert_eq!(calls.get(), 0);
        let cleanup = failed_staging_cleanup("original write failure".into(), Ok(true), || {
            Err(std::io::Error::other("cleanup denied"))
        });
        let message = cleanup.expect_err("cleanup fails");
        assert!(message.contains("original write failure") && message.contains("cleanup denied"));
        let cleaned = failed_staging_cleanup("original sync failure".into(), Ok(true), || Ok(()));
        assert_eq!(
            cleaned.expect_err("original remains failure"),
            "original sync failure"
        );
    }
    #[test]
    fn corrupt_and_interrupted_files_never_become_native_snapshots() {
        let dir = dir();
        let p = dir.join("snapshot");
        for raw in [vec![], vec![0; HEADER], vec![0; HEADER + 1]] {
            fs::write(&p, raw).expect("write");
            assert!(read(&p).is_err());
        }
        fs::remove_file(&p).expect("remove");
        let staging = dir.join(".snapshot.interrupted.tmp");
        fs::write(&staging, b"DIVECP01\x01").expect("partial staged bytes");
        assert!(read(&p).is_err());
        assert!(read(&staging).is_err());
        let original = b"immutable existing checkpoint sentinel";
        fs::write(&p, original).expect("original");
        assert!(write(&p, b"new payload").is_err());
        assert_eq!(fs::read(&p).expect("old intact"), original);
        assert_eq!(
            fs::read_dir(&dir).expect("entries").count(),
            2,
            "failed staging removed, interrupted file retained"
        );
        assert!(write(&dir.join("missing-parent/snapshot"), b"payload").is_err());
        fs::remove_dir_all(dir).expect("cleanup");
    }
    fn valid_snapshot() -> Snapshot {
        Snapshot {
            identity: Identity {
                schema: 1,
                semantics: SEMANTICS.into(),
                executable_sha256: String::new(),
                ir_sha256: String::new(),
                config_sha256: String::new(),
                config_source_sha256: None,
                seed: 7,
                fp: 0,
                options: String::new(),
            },
            names: vec!["name".into()],
            next_level: 2,
            generated: 3,
            depth: 2,
            initial_generated: 1,
            discoveries: vec![
                Discovery {
                    parent: None,
                    ordinal: 0,
                    key: vec![1],
                    state: vec![2, 0],
                },
                Discovery {
                    parent: Some(0),
                    ordinal: 0,
                    key: vec![2],
                    state: vec![2, 2],
                },
                Discovery {
                    parent: Some(0),
                    ordinal: 1,
                    key: vec![3],
                    state: vec![2, 4],
                },
            ],
            frontier: vec![1, 2],
        }
    }
    #[test]
    fn checksummed_structural_mutants_refuse_and_commit_is_complete() {
        let directory = dir();
        let valid = valid_snapshot();
        assert!(validate_structure(&valid).is_ok());
        let file = directory.join("valid");
        write(&file, &serde_json::to_vec(&valid).expect("serialize")).expect("commit");
        let (recovered, _, bytes) = read(&file).expect("complete commit");
        assert!(validate_structure(&recovered).is_ok());
        assert_eq!(bytes, fs::metadata(&file).expect("metadata").len());
        assert_eq!(fs::read_dir(&directory).expect("entries").count(), 1);
        for kind in 0..10 {
            let mut snapshot = valid_snapshot();
            match kind {
                0 => snapshot.discoveries[1].parent = Some(1),
                1 => snapshot.discoveries[2].ordinal = 0,
                2 => snapshot.frontier.reverse(),
                3 => {
                    snapshot.frontier.pop();
                }
                4 => snapshot.names.push("name".into()),
                5 => snapshot.depth = 1,
                6 => snapshot.generated = 2,
                7 => {
                    snapshot.initial_generated = u64::MAX;
                    snapshot.generated = u64::MAX;
                }
                8 => snapshot.discoveries[0].parent = Some(2),
                _ => snapshot.discoveries[1].key.clear(),
            }
            let path = directory.join(format!("mutant-{kind}"));
            write(&path, &serde_json::to_vec(&snapshot).expect("serialize"))
                .expect("checksum commit");
            let (recovered, _, _) =
                read(&path).expect("checksum matches deliberately rewritten payload");
            assert!(validate_structure(&recovered).is_err(), "mutant {kind}");
        }
        fs::remove_dir_all(directory).expect("cleanup");
    }
    #[test]
    fn native_reader_checks_checksum_length_unknown_fields_and_array_limits() {
        #[derive(Deserialize)]
        struct Three {
            #[serde(deserialize_with = "small")]
            values: Vec<u8>,
        }
        fn small<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
            bounded::<_, _, 3>(d)
        }
        let dir = dir();
        let p = dir.join("snapshot");
        write(&p, b"{}").expect("commit");
        assert!(read(&p).is_err());
        let mut raw = fs::read(&p).expect("bytes");
        raw[HEADER] ^= 1;
        fs::write(&p, &raw).expect("corrupt");
        assert!(read(&p).is_err());
        raw.push(0);
        fs::write(&p, &raw).expect("extra");
        assert!(read(&p).is_err());
        let valid: Three = serde_json::from_str("{\"values\":[1,2,3]}").expect("three");
        assert_eq!(valid.values, [1, 2, 3]);
        assert!(serde_json::from_str::<Three>("{\"values\":[1,2,3,4]}").is_err());
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
