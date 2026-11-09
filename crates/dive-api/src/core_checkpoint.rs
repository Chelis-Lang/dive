//! Core safety checkpoints and recovery, behind the public
//! `dive_runtime::checkpoint` request and result types.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use dive_ir::Ir;
use dive_runtime::checkpoint::{self, Action, Counters, Receipt, Request};
use dive_runtime::{CheckOptions, Verdict};

use crate::core_engine::{self, CoreRun, Prepared, Search};

const MAGIC: &[u8; 8] = b"DIVECC01";
/// The framing of archives the removed reference engine wrote. Recovery
/// rejects them by name (DIVE-S0400).
const REFERENCE_MAGIC: &[u8; 8] = b"DIVECP02";
const PARTS: usize = 4;
type Parts<'a> = [&'a [u8]; PARTS];

fn split(bytes: &[u8]) -> Result<Parts<'_>, String> {
    let mut at = 0usize;
    let mut parts = [&[][..]; PARTS];
    for part in &mut parts {
        let end = at
            .checked_add(8)
            .filter(|&n| n <= bytes.len())
            .ok_or("truncated Core archive section")?;
        let mut word = [0; 8];
        word.copy_from_slice(&bytes[at..end]);
        at = end;
        let n = usize::try_from(u64::from_le_bytes(word)).map_err(|e| e.to_string())?;
        let end = at
            .checked_add(n)
            .filter(|&n| n <= bytes.len())
            .ok_or("Core archive section bound")?;
        *part = &bytes[at..end];
        at = end;
    }
    if at != bytes.len() {
        return Err("Core archive trailing bytes".into());
    }
    Ok(parts)
}
fn join(parts: Parts<'_>) -> Result<Vec<u8>, String> {
    let size = parts
        .iter()
        .try_fold(PARTS * 8usize, |n, p| n.checked_add(p.len()))
        .filter(|&n| n <= checkpoint::MAX_PAYLOAD_BYTES)
        .ok_or("Core archive payload bound")?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|e| e.to_string())?;
    for part in parts {
        bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
        bytes.extend_from_slice(part);
    }
    Ok(bytes)
}

/// Saved seed and fingerprint defaults of a Core archive, without model
/// evaluation. `None` is an archive the removed reference engine wrote.
pub(crate) fn metadata(path: &Path) -> Result<Option<(i64, usize)>, String> {
    use std::io::Read;
    let mut magic = [0; 8];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .map_err(|e| e.to_string())?;
    if &magic == MAGIC {
        let (bytes, _, _) = checkpoint::read_payload(path, MAGIC)?;
        let parts = split(&bytes)?;
        let (seed, fp) = checkpoint::owned_identity_seed_fp(parts[0])?;
        Ok(Some((seed, fp)))
    } else {
        // Frames the reference archive (bounds, length and checksum), so a
        // damaged file still reports its framing error rather than the engine.
        checkpoint::read_payload(path, REFERENCE_MAGIC)?;
        Ok(None)
    }
}

fn refused(ir: &Ir, options: &CheckOptions, verdict: Verdict) -> checkpoint::Result {
    checkpoint::Result {
        check: core_engine::result(verdict, options, ir),
        receipt: None,
    }
}
fn failure(message: impl Into<String>) -> Verdict {
    Verdict::ResourceLimit {
        message: format!("DIVE-S0400: {}", message.into()),
    }
}
fn core_error(error: &dive_core::Error) -> Verdict {
    if error.message().contains("deferred provider") {
        Verdict::Unsupported {
            feature: error.message().into(),
        }
    } else {
        failure(error.message())
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered native checkpoint transaction from preflight to durable commit"
)]
pub(crate) fn check(
    ir: &Arc<Ir>,
    cfg: &dive_config::ModelConfig,
    options: &CheckOptions,
    request: &Request,
    profile_tlc: bool,
) -> (checkpoint::Result, CoreRun) {
    let started = Instant::now();
    let opts = match std::env::var("DIVE_CORE_OPT").ok().map_or_else(
        || Ok(dive_core::Opts::default()),
        |s| dive_core::Opts::parse(&s),
    ) {
        Ok(opts) => opts,
        Err(e) => {
            return (
                refused(
                    ir,
                    options,
                    Verdict::Unsupported {
                        feature: format!("DIVE_CORE_OPT: {e}"),
                    },
                ),
                CoreRun::default(),
            );
        }
    };
    let mut run = CoreRun {
        opts: opts.describe(),
        ..CoreRun::default()
    };
    let mut attempt = || -> Result<checkpoint::Result, Verdict> {
        if request.path.as_os_str().is_empty() {
            return Err(failure("empty checkpoint path"));
        }
        if let Some(feature) = core_engine::option_refusal(options, profile_tlc) {
            return Err(Verdict::Unsupported { feature });
        }
        if options.store_compact_bytes.is_some() && request.action != Action::Recover {
            return Err(Verdict::Unsupported {
                feature:
                    "Store reclamation with checkpoint pause (discovery residency is unsupported)"
                        .into(),
            });
        }
        // Static source and semantic identity admission precedes all binding/ASSUME work.
        let local_classes = dive_model::local_java_class_digests(ir)
            .map_err(|feature| Verdict::Unsupported { feature })?;
        let identity = checkpoint::owned_identity(ir, cfg, options, request, &opts.describe())?;
        let imported = if request.action == Action::Recover {
            let (bytes, sha, len) =
                checkpoint::read_payload(&request.path, MAGIC).map_err(failure)?;
            let sections = split(&bytes).map_err(failure)?;
            if sections[0] != identity {
                return Err(failure(
                    "Core source/configuration/executable/policy identity mismatch",
                ));
            }
            Some((bytes, sha, len))
        } else {
            if request
                .path
                .try_exists()
                .map_err(|e| failure(e.to_string()))?
            {
                return Err(failure("checkpoint destination already exists"));
            }
            None
        };
        let opts = dive_core::Opts {
            memo_cap: core_engine::memo_cap(options.budget.memory, options.workers.max(1)),
            ..opts
        };
        let Prepared {
            model,
            host: _host,
            start,
            mut store,
            plan,
            compile_time,
        } = core_engine::prepare(ir, cfg, options, profile_tlc, opts, false)
            .map_err(|r| r.verdict)?;
        if dive_model::local_java_class_digests(ir)
            .map_err(|feature| Verdict::Unsupported { feature })?
            != local_classes
        {
            return Err(failure("local Java class identity changed during startup"));
        }
        if let Some(verdict) = core_engine::startup_stop(options) {
            return Err(verdict);
        }
        let poll = || !matches!(options.budget.poll(), Ok(None));
        let proceed = || !poll();
        let startup =
            dive_core::store::snapshot::capture(&store, checkpoint::MAX_PAYLOAD_BYTES, &proceed)
                .map_err(|e| core_error(&e))?;
        let permanent = options
            .store_compact_bytes
            .map(|_| dive_core::store::compact::Permanent::capture(&store, &proceed))
            .transpose()
            .map_err(|e| core_error(&e))?;
        let mut recovered_receipt = None;
        let mut session = if let Some((bytes, sha, len)) = imported {
            let parts = split(&bytes).map_err(failure)?;
            store = dive_core::store::snapshot::restore(
                &store,
                parts[1],
                parts[2],
                checkpoint::MAX_PAYLOAD_BYTES,
                &proceed,
            )
            .map_err(|e| core_error(&e))?;
            let snapshot =
                dive_explore::checkpoint::Snapshot::decode(parts[3], Arc::clone(&store), &proceed)
                    .map_err(|e| core_error(&e))?;
            if snapshot
                .warning_sites()
                .any(|id| id as usize >= ir.nodes().len())
            {
                return Err(failure("Core warning source location"));
            }
            recovered_receipt = Some(Receipt {
                status: "recovered",
                payload_sha256: sha,
                transient_storage_cleanup_completed: false,
                archive_bytes: len,
                next_level: snapshot.next_level(),
                discovered: snapshot.discovered() as u64,
                queued: snapshot.queued() as u64,
                counters: request.counters,
            });
            dive_explore::checkpoint::Session::recover(
                snapshot,
                request.counters == Counters::Retain,
            )
        } else {
            let Action::PauseAfterLevels(n) = request.action else {
                return Err(failure("missing Core recovery archive"));
            };
            dive_explore::checkpoint::Session::pause(n).map_err(|e| core_error(&e))?
        };
        let workers = if options.seen_disk.is_some() {
            1
        } else {
            options.workers.max(1)
        };
        let explore_started = Instant::now();
        run.searched = true;
        let out = dive_explore::explore_owned(
            &plan,
            &mut store,
            dive_explore::Config {
                opts,
                max_states: options.max_states,
                max_depth: options.max_depth,
                workers,
                warn: options.eval.warnings,
                sample: None,
            },
            &poll,
            Some(&core_engine::search_work_control(&options.budget)),
            &dive_explore::storage::Storage {
                exact: options.store == dive_store::StoreMode::Exact,
                predecessor_memory_bytes: options.predecessor_memory_bytes,
                predecessor_directory: options.predecessor_directory.clone(),
                frontier_memory_bytes: options.frontier_memory_bytes,
                frontier_directory: options.frontier_directory.clone(),
                candidate_memory_bytes: options.candidate_memory_bytes,
                candidate_directory: options.candidate_directory.clone(),
                store_compact_bytes: options.store_compact_bytes,
                store_compact_default: false,
                seen_disk: options
                    .seen_disk
                    .as_ref()
                    .map(|o| dive_explore::storage::DiskOptions {
                        limits: o.limits,
                        directory: o.directory.clone(),
                    }),
            },
            Some(&mut session),
            permanent.as_ref(),
        );
        run.metrics = core_engine::metrics_json(&plan, &out.metrics, compile_time);
        let initial_pause = matches!(
            out.verdict,
            dive_explore::Verdict::CheckpointPaused { next_level: 1 }
        );
        let paused = matches!(out.verdict, dive_explore::Verdict::CheckpointPaused { .. });
        let disk_usage = out.metrics.storage.seen.clone();
        let mut check = core_engine::safety_result(
            &Search {
                ir,
                config: cfg,
                options,
                profile_tlc,
                workers,
                explore_started,
            },
            &model,
            &store,
            &plan,
            start,
            out,
        );
        if initial_pause {
            check.workers.used = 1;
            check.workers.reason =
                Some("checkpoint paused after Init, before parallel source-level expansion".into());
        }
        if let Some(usage) = disk_usage {
            check.seen = Some(core_engine::native_seen_usage(&usage));
            check.workers.used = usize::from(
                usage
                    .first_failure
                    .as_ref()
                    .is_none_or(|f| f.phase != "Creation"),
            );
            check.workers.reason=Some("fallible Core disk membership executes inline; requested width retained separately".into());
        }
        let mut receipt = if session.recovered() {
            recovered_receipt
        } else {
            None
        };
        if paused {
            let saved = (|| -> Result<Receipt, Verdict> {
                let snapshot = session
                    .paused()
                    .ok_or_else(|| failure("missing completed-level snapshot"))?;
                let image = dive_core::store::snapshot::capture(
                    &store,
                    checkpoint::MAX_PAYLOAD_BYTES,
                    &proceed,
                )
                .map_err(|e| core_error(&e))?;
                dive_core::store::snapshot::validate_extension(&startup, &image, &proceed)
                    .map_err(|e| core_error(&e))?;
                let search = snapshot.encode(&proceed).map_err(|e| core_error(&e))?;
                let bytes = join([&identity, &startup, &image, &search]).map_err(failure)?;
                if poll() {
                    return Err(failure("checkpoint cancelled before commit"));
                }
                checkpoint::write_payload(&request.path, MAGIC, &bytes).map_err(failure)?;
                let (_, sha, len) =
                    checkpoint::read_payload(&request.path, MAGIC).map_err(failure)?;
                Ok(Receipt {
                    status: "paused-incomplete",
                    payload_sha256: sha,
                    transient_storage_cleanup_completed: false,
                    archive_bytes: len,
                    next_level: snapshot.next_level(),
                    discovered: snapshot.discovered() as u64,
                    queued: snapshot.queued() as u64,
                    counters: request.counters,
                })
            })();
            match saved {
                Ok(saved) => receipt = Some(saved),
                Err(verdict) => check.verdict = verdict,
            }
        } else if matches!(request.action, Action::PauseAfterLevels(_))
            && check.verdict == Verdict::Success
        {
            check.notes.push("Search completed before the requested checkpoint boundary; no snapshot was committed.".into());
        }
        if let Some(receipt) = &mut receipt {
            receipt.finish_storage(&check.frontier, &check.predecessor);
            if check
                .seen
                .as_ref()
                .is_some_and(|seen| seen.first_failure.is_some())
            {
                receipt.transient_storage_cleanup_completed = false;
            }
        }
        check.elapsed = started.elapsed();
        Ok(checkpoint::Result { check, receipt })
    };
    let result = attempt().unwrap_or_else(|verdict| refused(ir, options, verdict));
    (result, run)
}
