//! Completed-level live-root Store replacement for owned pure safety search.

use std::sync::Arc;

use dive_core::memo::Memo;
use dive_core::store::compact::Permanent;
use dive_core::{Error, Handle, Machine, Store};

pub(super) fn boundary(
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
    let mut views = Vec::new();
    views
        .try_reserve_exact(ex.next_views.len())
        .map_err(|_| Error::internal("DIVE-S0203: live view index allocation"))?;
    for (id, row) in &ex.next_views {
        views.push((*id, copy(row, &mut remap)?));
    }
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
    ex.next_views = views;
    if frontier.is_some() {
        ex.frontier = frontier;
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
}
