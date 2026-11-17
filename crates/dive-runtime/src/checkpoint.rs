//! Native checkpoint framing, requests, receipts and the owned Core
//! backend's identity.
//!
//! Checksums detect corruption, not malicious rewriting.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dive_config::ModelConfig;
use dive_ir::{Effects, Ir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CheckOptions, CheckResult, Verdict};

/// Archive header bytes: magic, payload length and SHA-256.
#[doc(hidden)]
pub const HEADER: usize = 48;
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
/// use dive_runtime::checkpoint::{Action, Counters, Request};
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
    /// Whether configured transient queue/log cleanup completed without failure.
    /// A committed archive can exist despite a later storage failure; this is
    /// execution evidence, never an archive correctness certificate.
    pub transient_storage_cleanup_completed: bool,
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

impl Receipt {
    /// Records cleanup of the actual transient frontier and predecessor owners.
    pub fn finish_storage(
        &mut self,
        frontier: &crate::FrontierUsage,
        predecessor: &crate::result::PredecessorUsage,
    ) {
        self.transient_storage_cleanup_completed = frontier.failures.is_empty()
            && predecessor.failures.is_empty()
            && predecessor.cleanup_completed;
    }
}

/// A check plus its native checkpoint evidence. Failed admission has no receipt.
#[derive(Clone, Debug)]
pub struct Result {
    /// Ordinary safety result and truthful actual workers. Pauses are resource stops.
    pub check: CheckResult,
    /// The actual committed/imported archive, if any.
    pub receipt: Option<Receipt>,
}

/// A checkpoint operation failing with a message.
#[doc(hidden)]
pub type Fallible<T> = std::result::Result<T, String>;

/// Source, executable and policy identity of a native archive.
#[doc(hidden)]
#[allow(
    missing_docs,
    reason = "serialized identity fields named by the format"
)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub schema: u32,
    pub semantics: String,
    pub executable_sha256: String,
    pub ir_sha256: String,
    pub config_sha256: String,
    pub config_source_sha256: Option<String>,
    pub seed: i64,
    pub fp: usize,
    pub options: String,
}

/// A native checkpoint resource failure.
#[doc(hidden)]
pub fn failure(message: impl Into<String>) -> Verdict {
    Verdict::ResourceLimit {
        message: format!("DIVE-S0400: native checkpoint {}", message.into()),
    }
}
/// A native checkpoint refusal naming `feature`.
#[doc(hidden)]
pub fn unsupported(feature: impl Into<String>) -> Verdict {
    Verdict::Unsupported {
        feature: format!("native checkpoint {}", feature.into()),
    }
}
/// Lower-case hexadecimal text of `bytes`.
#[doc(hidden)]
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
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
/// SHA-256 of `bytes`, in hexadecimal.
#[doc(hidden)]
#[must_use]
pub fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
/// SHA-256 of the running executable.
///
/// # Errors
/// The executable cannot be located or read.
#[doc(hidden)]
pub fn executable_hash() -> Fallible<String> {
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

/// Conservative pre-binding reachability admission; `allow_temporal` admits
/// temporal effects.
///
/// # Errors
/// The named refusal of the first unrecoverable definition or operator.
#[doc(hidden)]
pub fn admit_source_inner(
    ir: &Ir,
    cfg: &ModelConfig,
    allow_temporal: bool,
) -> std::result::Result<(), Verdict> {
    let mut targets: std::collections::BTreeSet<&str> = cfg
        .constants
        .iter()
        .filter_map(|entry| {
            if let dive_config::ConstantEntry::Substitution { target, .. } = entry {
                Some(target.text.as_str())
            } else {
                None
            }
        })
        .collect();
    for name in [&cfg.init, &cfg.next, &cfg.view, &cfg.symmetry, &cfg.alias]
        .into_iter()
        .flatten()
    {
        targets.insert(name.text.as_str());
    }
    for names in [
        &cfg.invariants,
        &cfg.constraints,
        &cfg.action_constraints,
        &cfg.postconditions,
    ] {
        targets.extend(names.iter().map(|name| name.text.as_str()));
    }
    let mut roots: Vec<_> = ir
        .defs()
        .iter()
        .enumerate()
        .filter_map(|(index, def)| {
            let startup = !ir.module(def.module).standard
                || (def.params.is_empty() && matches!(def.scope, dive_ir::DefScope::TopLevel));
            let user = startup && def.std.is_none();
            (user || targets.contains(def.name.as_str()))
                .then(|| dive_ir::DefId(u32::try_from(index).unwrap_or(u32::MAX)))
        })
        .collect();
    // Configuration lookup resolves names in module contexts, including aliases
    // whose spelling differs from Def.name. Admit every possible matching
    // definition before Binder can precompute a constant replacement.
    roots.extend(ir.modules().iter().flat_map(|module| {
        module.context.iter().filter_map(|(name, symbol)| {
            if targets.contains(name.as_str())
                && let dive_ir::Symbol::Def(def) = symbol
            {
                Some(*def)
            } else {
                None
            }
        })
    }));
    let reached = ir.reachable_defs(&roots);
    let mut seen = std::collections::BTreeSet::new();
    for (index, def) in ir.defs().iter().enumerate() {
        if !reached[index] {
            continue;
        }
        if def
            .effects
            .intersects(
                Effects::IMPURE
                    .union(Effects::LIVE_TLCGET)
                    .union(if allow_temporal {
                        Effects::NONE
                    } else {
                        Effects::TEMPORAL
                    }),
            )
        {
            return Err(unsupported(format!(
                "runtime effects or temporal effects in {}",
                def.name
            )));
        }
        if let Some(op) = def.std {
            admit_native(op)?;
            continue;
        }
        let mut pending = vec![def.body];
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let expression = ir.expr(id);
            for reference in expression.op_refs() {
                if let dive_ir::OpRef::Std(op) = reference {
                    admit_native(*op)?;
                }
            }
            pending.extend(expression.children());
        }
    }
    Ok(())
}

fn admit_native(op: dive_ir::StdOp) -> std::result::Result<(), Verdict> {
    if op.impure()
        || matches!(
            op,
            dive_ir::StdOp::TlcTLCGet
                | dive_ir::StdOp::TlcTLCSet
                | dive_ir::StdOp::TlcExtTLCGetOrDefault
        )
    {
        Err(unsupported(format!(
            "runtime effects or checker/register reads in {} are not recoverable in this slice",
            op.name()
        )))
    } else {
        Ok(())
    }
}

/// Reads a bounded checksummed archive for its explicitly named backend format.
///
/// # Errors
/// Framing, checksum, allocation or filesystem failure; no evaluation occurs.
pub fn read_payload(path: &Path, magic: &[u8; 8]) -> Fallible<(Vec<u8>, String, u64)> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    if len < HEADER as u64 || len > (HEADER + MAX_PAYLOAD_BYTES) as u64 {
        return Err("archive byte bound".into());
    }
    let mut header = [0u8; HEADER];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..8] != magic {
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
    Ok((payload, hex(&sha), len))
}

/// Commits a bounded opaque backend payload atomically without replacing a file.
///
/// # Errors
/// Bounds, creation, write, durability or staging cleanup failure.
pub fn write_payload(path: &Path, magic: &[u8; 8], payload: &[u8]) -> Fallible<()> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err("archive payload byte bound".into());
    }

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
        file.write_all(magic)
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

/// The error of a failed commit after inspecting and cleaning its staging file.
///
/// # Errors
/// Always: the original failure, with any inspection or cleanup failure.
#[doc(hidden)]
pub fn failed_staging_cleanup(
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

/// SHA-256 of exact config source bytes, for file-based checkpoint identity.
#[must_use]
pub fn config_source_hash(bytes: &[u8]) -> String {
    hash(bytes)
}

/// Captures source, executable and semantic policy identity for the owned Core backend.
/// Actual temporal model roles must additionally be refused by Core binding before search.
///
/// # Errors
/// Impure source, unsupported hooks, identity hashing or serialization failure.
pub fn owned_identity(
    ir: &Ir,
    cfg: &ModelConfig,
    opts: &CheckOptions,
    request: &Request,
    core_options: &str,
) -> std::result::Result<Vec<u8>, Verdict> {
    if ir.vars().len() > 1024 {
        return Err(unsupported("state variable arity above 1024"));
    }
    if cfg.periodic.is_some() || cfg.rl_reward.is_some() || !cfg.possible.is_empty() {
        return Err(unsupported(
            "periodic/reward/possible hooks in owned Core recovery",
        ));
    }
    admit_source_inner(ir, cfg, true)?;
    let mut ir_bytes = serde_json::to_vec(ir).map_err(|e| failure(e.to_string()))?;
    let local_classes = dive_model::local_java_class_digests(ir).map_err(unsupported)?;
    ir_bytes.extend(serde_json::to_vec(&local_classes).map_err(|e| failure(e.to_string()))?);
    let identity = Identity {
        schema: 1,
        semantics: "owned-core-safety-0141-v1".into(),
        executable_sha256: executable_hash().map_err(failure)?,
        ir_sha256: hash(&ir_bytes),
        config_sha256: hash(format!("{cfg:?}").as_bytes()),
        config_source_sha256: request.config_source_sha256.clone(),
        seed: opts.seed,
        fp: opts.fp_index,
        options: format!(
            "{:?}/{:?}/{:?}/{}/{}/{}/{}/{}/{}/{:?}/{:?}/{core_options}",
            opts.profile,
            opts.eval.ctx,
            opts.eval.exploration,
            opts.eval.max_depth,
            opts.eval.warnings,
            opts.fp_index,
            opts.tlc_fingerprints,
            opts.seed,
            opts.no_deadlock,
            opts.store,
            opts.eval.comparisons
        ),
    };
    serde_json::to_vec(&identity).map_err(|e| failure(e.to_string()))
}

/// Reads seed/fingerprint metadata from a serialized owned backend identity.
///
/// # Errors
/// Unknown identity schema or semantics, invalid JSON or fingerprint index.
pub fn owned_identity_seed_fp(bytes: &[u8]) -> Fallible<(i64, usize)> {
    let identity: Identity = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if identity.schema != 1
        || identity.semantics != "owned-core-safety-0141-v1"
        || identity.fp >= dive_value::fp64::POLYS.len()
    {
        return Err("owned identity schema/semantics/fingerprint mismatch".into());
    }
    Ok((identity.seed, identity.fp))
}
