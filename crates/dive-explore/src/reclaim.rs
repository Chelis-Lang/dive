//! Completed-level live-root Store replacement for owned pure safety search.

use std::sync::Arc;

use dive_core::memo::Memo;
use dive_core::store::compact::Permanent;
use dive_core::{Error, Handle, Machine, Store};

/// Whether a default-trigger reclamation failure may leave the resident Store
/// in place: a named refusal or an allocation failure, never a stop or a bug.
pub(super) fn skippable(error: &Error) -> bool {
    error.kind() == dive_core::ErrorKind::Unsupported || error.message().starts_with("DIVE-S0203:")
}

pub(super) fn boundary(
    machines: &mut [Machine],
    ex: &mut super::Explorer<'_>,
    store: &mut Arc<Store>,
    permanent: Option<&Permanent>,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
) -> Result<(), Error> {
    match collect(machines, ex, store, permanent, control) {
        // Every replacement is built before anything is installed, so a failed
        // default collection leaves the original Store, Machines and frontier.
        Err(error) if ex.candidate_storage.store_compact_default && skippable(&error) => {
            ex.candidate_storage.store_compact_bytes = None;
            ex.metrics.storage.reclamation.skipped = Some(error.message().to_owned());
            Ok(())
        }
        other => other,
    }
}

fn collect(
    machines: &mut [Machine],
    ex: &mut super::Explorer<'_>,
    store: &mut Arc<Store>,
    permanent: Option<&Permanent>,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
) -> Result<(), Error> {
    let Some(trigger) = ex.candidate_storage.store_compact_bytes else {
        return Ok(());
    };
    let before = store
        .memory_usage()
        .known_retained_bytes()
        .ok_or_else(|| Error::internal("DIVE-S0203: Store byte accounting overflow"))?;
    if trigger > 0 && before <= trigger {
        return Ok(());
    }
    if (ex.poll)() {
        return Err(Error::stop());
    }
    let permanent = permanent
        .ok_or_else(|| Error::internal("missing permanent roots for Store reclamation"))?;
    let before_nodes = store.stats().nodes;
    let mut remap = permanent.begin(store, &|| !(ex.poll)())?;
    // Build every replacement while all original roots and ownership remain live.
    // No partially copied frontier is installed on any failure or cancellation.
    let copy = |rows: &[Handle], remap: &mut dive_core::store::compact::Remap| {
        let mut copied = Vec::new();
        copied
            .try_reserve_exact(rows.len())
            .map_err(|_| Error::internal("DIVE-S0203: live frontier allocation"))?;
        for &h in rows {
            copied.push(remap.copy(h, &|| !(ex.poll)())?);
        }
        Ok::<_, Error>(copied)
    };
    let next = copy(&ex.next, &mut remap)?;
    let originals = copy(&ex.next_originals, &mut remap)?;
    let frontier = ex
        .frontier
        .as_mut()
        .map(|frontier| frontier.remap(&mut remap, ex.poll))
        .transpose()?;
    let replacement = Arc::clone(remap.store());
    let after = replacement
        .memory_usage()
        .known_retained_bytes()
        .ok_or_else(|| Error::internal("DIVE-S0203: replacement byte accounting overflow"))?;
    let after_nodes = replacement.stats().nodes;
    for machine in machines {
        let (nodes, handles) = machine.heap.arena().peak();
        ex.metrics.arena_peak.0 = ex.metrics.arena_peak.0.max(nodes);
        ex.metrics.arena_peak.1 = ex.metrics.arena_peak.1.max(handles);
        // Replacing the complete Machine drops all persistent-handle owners:
        // Arena, frames, materialization cache, interning tables and certificates.
        // Memo statistics and externally observable warnings remain cumulative.
        let mut memo = std::mem::replace(&mut machine.memo, Memo::new(&[]));
        memo.clear();
        let mut fresh = Machine::new(
            Arc::clone(&replacement),
            ex.plan.vars.len(),
            machine.opts,
            memo,
        );
        fresh.stats = machine.stats;
        fresh.warn = machine.warn;
        fresh.warnings = std::mem::take(&mut machine.warnings);
        fresh.printed = std::mem::take(&mut machine.printed);
        fresh.level = machine.level;
        fresh.set_work_control(control.cloned());
        *machine = fresh;
    }
    ex.next = next;
    ex.next_originals = originals;
    if let Some(mut replacement_frontier) = frontier {
        if let Some(old) = &mut ex.frontier {
            replacement_frontier.adopt_independent_queues(old);
        }
        ex.frontier = Some(replacement_frontier);
    }
    *store = replacement;
    let usage = &mut ex.metrics.storage.reclamation;
    usage.collections += 1;
    usage.reclaimed_nodes += before_nodes.saturating_sub(after_nodes);
    usage.peak_before_bytes = usage.peak_before_bytes.max(before as u64);
    usage.peak_after_bytes = usage.peak_after_bytes.max(after as u64);
    usage.peak_after_nodes = usage.peak_after_nodes.max(after_nodes);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "owned Store lifetime and cancellation controls"
    )]
    use super::*;
    use crate::{Config, Verdict, explore_owned, storage};

    #[test]
    fn owned_search_releases_the_original_store_all_machines_and_frontier_codecs() {
        for exact in [false, true] {
            let mut store = Arc::new(Store::new(true));
            let plan = crate::tests::frontier_plan(&store, false).expect("plan");
            let old = Arc::downgrade(&store);
            let out = explore_owned(
                &plan,
                &mut store,
                Config {
                    opts: dive_core::Opts::default(),
                    max_states: None,
                    max_depth: None,
                    workers: 4,
                    warn: true,
                    sample: None,
                },
                &|| false,
                None,
                &storage::Storage {
                    exact,
                    store_compact_bytes: Some(0),
                    frontier_memory_bytes: Some(0),
                    ..storage::Storage::default()
                },
                None,
                None,
            );
            assert_eq!(out.verdict, Verdict::Success);
            assert!(out.metrics.storage.reclamation.collections >= 2);
            assert!(
                old.upgrade().is_none(),
                "no startup or worker may retain the old payload owner"
            );
        }
    }

    #[test]
    fn a_default_trigger_skips_instead_of_refusing_and_keeps_the_result() {
        let mut reference = None;
        for (trigger, default) in [(None, false), (Some(0), true), (Some(0), false)] {
            let mut store = Arc::new(Store::new(true));
            let plan = crate::tests::frontier_plan(&store, false).expect("plan");
            let out = explore_owned(
                &plan,
                &mut store,
                Config {
                    opts: dive_core::Opts::default(),
                    max_states: None,
                    max_depth: None,
                    workers: 1,
                    warn: true,
                    sample: Some(crate::sample::Sample { seed: 7, states: 2 }),
                },
                &|| false,
                None,
                &storage::Storage {
                    store_compact_bytes: trigger,
                    store_compact_default: default,
                    ..storage::Storage::default()
                },
                None,
                None,
            );
            let usage = &out.metrics.storage.reclamation;
            match (trigger, default) {
                (Some(_), false) => {
                    assert!(matches!(out.verdict, Verdict::Unsupported { .. }));
                }
                (Some(_), true) => {
                    assert_eq!(out.verdict, Verdict::Success);
                    assert_eq!(usage.trigger_bytes, Some(0));
                    assert!(usage.default_trigger);
                    assert!(
                        usage
                            .skipped
                            .as_deref()
                            .is_some_and(|why| why.contains("sampling"))
                    );
                    assert_eq!(usage.collections, 0);
                    assert_eq!(Some(out.stats), reference);
                }
                (None, _) => {
                    assert_eq!(out.verdict, Verdict::Success);
                    reference = Some(out.stats);
                }
            }
        }
    }

    #[test]
    fn preloaded_names_do_not_inherit_checkpoint_admission() {
        use dive_core::plan::{Code, GNode, Node};
        for names in [65_536, 65_537] {
            for tlc in [false, true] {
                for exact in [false, true] {
                    let mut store = Arc::new(Store::with_semantics(
                        true,
                        if tlc {
                            dive_core::Semantics::tlc()
                        } else {
                            dive_core::Semantics::default()
                        },
                    ));
                    for i in 0..names {
                        store
                            .strings()
                            .intern(&format!("s{i}"))
                            .expect("preloaded name");
                    }
                    let mut plan = crate::tests::frontier_plan(&store, false).expect("plan");
                    plan.code = Code {
                        nodes: vec![Node::Const(Handle::TRUE)],
                        gnodes: vec![GNode::Assign { var: 0, rhs: 0 }],
                        ..Code::default()
                    };
                    for action in &mut plan.next {
                        action.g = 0;
                    }
                    let wire = dive_core::store::snapshot::capture(&store, 16 << 20, &|| true);
                    if names == 65_536 {
                        assert!(
                            wire.is_ok(),
                            "checkpoint boundary still admits 65,536 names"
                        );
                    } else {
                        assert!(
                            wire.expect_err("checkpoint name bound")
                                .message()
                                .contains("name admission bound")
                        );
                    }
                    let mut reference = None;
                    for trigger in [None, Some(usize::MAX), Some(0)] {
                        let out = explore_owned(
                            &plan,
                            &mut store,
                            Config {
                                opts: dive_core::Opts::default(),
                                max_states: None,
                                max_depth: None,
                                workers: 1,
                                warn: true,
                                sample: None,
                            },
                            &|| false,
                            None,
                            &storage::Storage {
                                exact,
                                store_compact_bytes: trigger,
                                ..storage::Storage::default()
                            },
                            None,
                            None,
                        );
                        assert_eq!(
                            out.verdict,
                            Verdict::Success,
                            "{names}/{tlc}/{exact}/{trigger:?}"
                        );
                        assert_eq!(out.stats.generated, 2);
                        assert_eq!(out.stats.distinct, 1);
                        assert_eq!(out.stats.depth, 1);
                        assert_eq!(store.strings().len(), names);
                        assert_eq!(store.strings().text(0).as_deref(), Some("s0"));
                        assert_eq!(
                            store
                                .strings()
                                .text(u32::try_from(names - 1).expect("last token"))
                                .as_deref(),
                            Some(format!("s{}", names - 1).as_str())
                        );
                        if let Some(stats) = reference {
                            assert_eq!(out.stats, stats);
                        }
                        reference = Some(out.stats);
                        assert_eq!(
                            out.metrics.storage.reclamation.collections,
                            if trigger == Some(0) { 2 } else { 0 }
                        );
                    }
                }
            }
        }
    }
}
