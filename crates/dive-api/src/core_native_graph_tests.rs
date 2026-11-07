//! Native rows, topology and failed admission through the actual explorer.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded capture controls"
)]

use std::sync::Arc;

use dive_core::node::flags;
use dive_runtime::capture::{CsrCapture, CsrComplete};

use super::*;

const ALIASES: &str = "VARIABLES k, x\n\
Init == k \\in 0..15 /\\ x = <<0>>\n\
A == k' = k /\\ x' = <<2>>\n\
Rejected == k' = k /\\ x' = <<9>>\n\
B == k' = k /\\ x' = [i \\in 1..1 |-> 4]\n\
Again == k' = k /\\ x' = <<2>>\n\
Literal == k' = k /\\ x' = <<4>>\n\
Next == A \\/ Rejected \\/ B \\/ Again \\/ Literal\n\
C == x[1] # 9\nV == k";
const ALIASES_CFG: &str = "INIT Init\nNEXT Next\nCONSTRAINT C\nVIEW V\n";

fn prepare(
    body: &str,
    cfg: &str,
    tlc: bool,
    opts: dive_core::Opts,
) -> crate::core_engine::Prepared {
    prepare_extending("Naturals", body, cfg, tlc, opts)
}

fn prepare_extending(
    extends: &str,
    body: &str,
    cfg: &str,
    tlc: bool,
    opts: dive_core::Opts,
) -> crate::core_engine::Prepared {
    let source = format!("---- MODULE Native ----\nEXTENDS {extends}\n{body}\n====\n");
    let analysis = dive_sema::analyze_source("Native.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(cfg).into_config().expect("config");
    let options = options(tlc);
    crate::core_engine::prepare(&ir, &cfg, &options, tlc, opts, false)
        .unwrap_or_else(|result| panic!("preparation: {:?}", result.verdict))
}

fn options(tlc: bool) -> CheckOptions {
    if tlc {
        CheckOptions::tlc(0, 0)
    } else {
        CheckOptions::dive(0)
    }
}

fn config(opts: dive_core::Opts, workers: usize) -> dive_explore::Config {
    dive_explore::Config {
        opts,
        workers,
        warn: true,
        max_states: None,
        max_depth: None,
        sample: None,
    }
}

fn shape(heap: &Heap, h: Handle) -> String {
    if !h.is_compound() {
        return format!("{h:?}");
    }
    let view = heap.view(h);
    format!(
        "{:?}/{}[{}]",
        view.kind,
        view.flags,
        view.data
            .iter()
            .map(|&child| shape(heap, child))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[test]
fn compound_native_sources_do_not_bind_the_optional_scalar_row_runtime() {
    for tlc in [false, true] {
        let opts = dive_core::Opts::default();
        let cfg = "INIT Init\nNEXT Next\n";
        let prepared = prepare(
            "VARIABLE x\nInit == x = <<0>>\nNext == UNCHANGED x",
            cfg,
            tlc,
            opts,
        );
        let options = options(tlc);
        let mut sink = NativeSink::new(
            &prepared.model,
            prepared.store.clone(),
            Limits::default(),
            &options,
            None,
        )
        .expect("sink");
        sink.prepare_source_rows(SourceRowSetup {
            ir: prepared.model.ir.clone(),
            config: dive_config::parse("INIT Missing\nNEXT Missing\nPROPERTY Missing\n")
                .into_config()
                .expect("valid syntax but binding must never be attempted"),
            options,
            limits: dive_liveness::analyze::Limits::default(),
            graph_labels: dive_liveness::analyze::GraphLabelPolicy::NoCellLimit,
        });
        let result = dive_explore::explore_with(
            &prepared.plan,
            &prepared.store,
            config(opts, 1),
            &|| false,
            Some(&mut sink),
        );
        assert_eq!(result.verdict, dive_explore::Verdict::Success);
        assert!(
            sink.source_setup.is_none(),
            "actual compound first source retires optional binding without attempting it"
        );
        assert!(sink.source_rows.is_none());
        assert!(sink.take_source_error().is_none());
        assert!(sink.take_error().is_none());
        assert_eq!((result.stats.generated, result.stats.distinct), (2, 1));
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered alias and original-row matrix"
)]
fn native_rows_keep_seen_originals_gaps_duplicates_and_stutters() {
    for tlc in [false, true] {
        for hashcons in [false, true] {
            for early_probe in [false, true] {
                let opts = dive_core::Opts {
                    hashcons,
                    early_probe,
                    fold: false,
                    ..Default::default()
                };
                for workers in [1, 2, 3, 8] {
                    let prepared = prepare(ALIASES, ALIASES_CFG, tlc, opts);
                    let options = options(tlc);
                    let mut native = NativeSink::new(
                        &prepared.model,
                        Arc::clone(&prepared.store),
                        Limits::default(),
                        &options,
                        None,
                    )
                    .expect("sink");
                    let out = dive_explore::explore_with(
                        &prepared.plan,
                        &prepared.store,
                        config(opts, workers),
                        &|| false,
                        Some(&mut native),
                    );
                    assert_eq!(out.verdict, dive_explore::Verdict::Success);
                    assert!(native.take_error().is_none());
                    let control = Arc::clone(&native.control);
                    let (graph, values) = native.finish();
                    assert_eq!(values.metrics()["admitted_rows"], 80);
                    assert_eq!(values.metrics()["cache_entries"], 0);
                    assert_eq!(values.metrics()["cache_bytes"], 0);
                    let csr =
                        CsrCapture::checked(graph.input(), graph.limits(), &control).expect("CSR");
                    assert_eq!((out.stats.generated, out.stats.distinct), (96, 16));
                    assert_eq!(csr.roots(), &(0..16).collect::<Vec<_>>());
                    assert_eq!(graph.retained_rows, 80, "no global semantic alias dedupe");
                    assert_eq!(graph.closed_rows, 16);
                    assert!(graph.memory.buffer_bytes > graph.rows.len() * 8);
                    assert!(
                        graph.memory.growth_overlap_upper_bound_bytes >= graph.memory.buffer_bytes
                    );
                    let heap = Heap::new(Arc::clone(&graph.store), false, false);
                    let variable = prepared
                        .plan
                        .vars
                        .iter()
                        .position(|name| name == "x")
                        .expect("x");
                    for source in 0..16 {
                        let representative = graph
                            .row(graph.representatives[source] as usize)
                            .expect("row");
                        assert_eq!(
                            heap.view(representative[variable]).flags & flags::TUPLE,
                            flags::TUPLE
                        );
                        let range = csr.outgoing_range(source).expect("source");
                        assert_eq!(range.len(), 5);
                        for (position, (ordinal, literal, number)) in
                            [(0, true, 2), (2, false, 4), (3, true, 2), (4, true, 4)]
                                .into_iter()
                                .enumerate()
                        {
                            let edge = range.start + position;
                            assert_eq!(csr.target(edge), Some(source));
                            assert_eq!(csr.ordinal(edge), Some(Some(ordinal)));
                            let original = graph
                                .row(csr.original_row(source, position).expect("original id"))
                                .expect("original row");
                            let view = heap.view(original[variable]);
                            assert_eq!(view.flags & flags::TUPLE != 0, literal);
                            assert_eq!(view.data[0].as_int(), Some(number));
                            assert_ne!(
                                shape(&heap, original[variable]),
                                shape(&heap, representative[variable]),
                                "representative substitution must fail"
                            );
                        }
                        let last = range.end - 1;
                        assert_eq!(csr.ordinal(last), Some(None));
                        assert_eq!(csr.label(last), Some(None));
                        assert_eq!(graph.edge_rows[last], graph.representatives[source]);
                    }
                    // Retained handles remain readable after all explorer arenas
                    // have reset and their worker heaps have been dropped.
                    drop(values);
                    drop(heap);
                    let store = Arc::downgrade(&prepared.store);
                    drop(prepared);
                    assert!(
                        store.upgrade().is_some(),
                        "native graph owns its Store through analysis"
                    );
                    let heap = Heap::new(Arc::clone(&graph.store), false, false);
                    for &handle in &graph.rows {
                        assert_ne!(shape(&heap, handle), "");
                    }
                }
            }
        }
    }
}

#[test]
fn native_new_targets_and_exact_seen_rows_reuse_first_discovery_rows() {
    let body = "VARIABLES n, x\nInit == n = 0 /\\ x = [a |-> <<0>>]\nNext == n' = (n + 1) % 3 /\\ UNCHANGED x";
    for tlc in [false, true] {
        for workers in [1, 2, 3, 8] {
            let opts = dive_core::Opts::default();
            let prepared = prepare(body, "INIT Init\nNEXT Next\n", tlc, opts);
            let options = options(tlc);
            let mut sink = NativeSink::new(
                &prepared.model,
                Arc::clone(&prepared.store),
                Limits::default(),
                &options,
                None,
            )
            .expect("sink");
            let out = dive_explore::explore_with(
                &prepared.plan,
                &prepared.store,
                config(opts, workers),
                &|| false,
                Some(&mut sink),
            );
            assert_eq!(out.verdict, dive_explore::Verdict::Success);
            let control = Arc::clone(&sink.control);
            let (graph, _) = sink.finish();
            let csr = CsrCapture::checked(graph.input(), graph.limits(), &control).expect("CSR");
            assert_eq!(graph.state_count(), 3);
            assert_eq!(graph.retained_rows, 3);
            assert_eq!(csr.targets(), &[1, 0, 2, 1, 0, 2]);
            for edge in 0..6 {
                assert_eq!(
                    graph.edge_rows[edge],
                    graph.representatives[graph.targets[edge] as usize]
                );
            }
        }
    }
}

#[test]
fn native_resource_refusals_do_not_produce_completed_graphs() {
    let opts = dive_core::Opts::default();
    let body = "VARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 3";
    for limits in [
        Limits {
            states: 0,
            ..Default::default()
        },
        Limits {
            edges: 0,
            ..Default::default()
        },
        Limits {
            generated: 0,
            ..Default::default()
        },
        Limits {
            successors_per_unit: 0,
            ..Default::default()
        },
    ] {
        for workers in [1, 2, 3, 8] {
            let prepared = prepare(body, "INIT Init\nNEXT Next\n", false, opts);
            let options = options(false);
            let mut sink = NativeSink::new(
                &prepared.model,
                Arc::clone(&prepared.store),
                limits,
                &options,
                None,
            )
            .expect("sink");
            let out = dive_explore::explore_with(
                &prepared.plan,
                &prepared.store,
                config(opts, workers),
                &|| false,
                Some(&mut sink),
            );
            assert_eq!(out.verdict, dive_explore::Verdict::GraphStop);
            let reason = sink.take_error().expect("stop reason");
            assert!(matches!(reason, Verdict::ResourceLimit { .. }));
            let control = Arc::clone(&sink.control);
            let (graph, _) = sink.finish();
            let safety = crate::core_engine::result(reason.clone(), &options, &prepared.model.ir);
            let complete = CsrComplete::from_producer(
                graph.input(),
                safety,
                options.clone(),
                limits,
                &control,
            );
            assert!(matches!(complete, Err(error) if error == reason));
        }
    }
}

#[test]
fn native_reservation_overflow_is_fallible_and_keeps_accounting() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let mut data = Vec::<Handle>::new();
    let mut memory = NativeMemory::default();
    assert!(reserve(&mut data, usize::MAX, &mut memory, &control).is_err());
    assert_eq!(data.capacity(), 0);
    assert_eq!(memory.buffer_bytes, 0);
    reserve(&mut data, 1, &mut memory, &control).expect("one slot");
    assert_eq!(
        memory.buffer_bytes,
        data.capacity() * std::mem::size_of::<Handle>()
    );
    data.push(Handle::EMPTY);
    let bytes = memory.buffer_bytes;
    assert!(reserve(&mut data, usize::MAX, &mut memory, &control).is_err());
    assert_eq!(memory.buffer_bytes, bytes);
}

#[test]
fn seen_initial_aliases_are_admitted_without_unused_row_retention() {
    let body = "VARIABLES n, x\nInit == n \\in 0..15 /\\ (x = <<0>> \\/ x = [i \\in 1..1 |-> 0])\nNext == UNCHANGED <<n, x>>";
    for tlc in [false, true] {
        for workers in [1, 2, 3, 8] {
            let opts = dive_core::Opts::default();
            let prepared = prepare(body, "INIT Init\nNEXT Next\n", tlc, opts);
            let options = options(tlc);
            let mut sink = NativeSink::new(
                &prepared.model,
                Arc::clone(&prepared.store),
                Limits::default(),
                &options,
                None,
            )
            .expect("sink");
            let out = dive_explore::explore_with(
                &prepared.plan,
                &prepared.store,
                config(opts, workers),
                &|| false,
                Some(&mut sink),
            );
            assert_eq!(out.verdict, dive_explore::Verdict::Success);
            let (graph, values) = sink.finish();
            assert_eq!(graph.state_count(), 16);
            assert_eq!(graph.retained_rows, 16);
            assert_eq!(values.metrics()["admitted_rows"], 48);
            let heap = Heap::new(Arc::clone(&graph.store), false, false);
            let x = prepared
                .plan
                .vars
                .iter()
                .position(|name| name == "x")
                .expect("x");
            for &row in &graph.representatives {
                let h = graph.row(row as usize).expect("representative")[x];
                assert_ne!(
                    heap.view(h).flags & flags::TUPLE,
                    0,
                    "first literal wins over a later general alias"
                );
            }
        }
    }
}

#[test]
fn native_conversion_precedes_state_and_edge_caps() {
    let opts = dive_core::Opts::default();
    let prepared = prepare(
        "VARIABLE x\nInit == x = 0\nNext == UNCHANGED x",
        "INIT Init\nNEXT Next\n",
        false,
        opts,
    );
    let options = options(false);
    let limits = Limits {
        states: 0,
        edges: 0,
        ..Default::default()
    };
    let mut sink = NativeSink::new(
        &prepared.model,
        Arc::clone(&prepared.store),
        limits,
        &options,
        None,
    )
    .expect("sink");
    let mut heap = Heap::new(Arc::clone(&prepared.store), false, false);
    let interval = heap.interval(1, 2);
    let original = heap.intern_exact(interval).expect("original");
    let canonical = heap.intern(interval).expect("identity");
    sink.emitted(1, None).expect("generated admission first");
    assert!(
        sink.state(
            &mut heap,
            Step {
                key: 1,
                new: Some((0, &[canonical])),
                original: &[original],
                parent: None,
                ordinal: 0,
                action: None
            }
        )
        .is_err()
    );
    let reason = sink.take_error().expect("conversion error");
    assert!(
        matches!(reason, Verdict::Unsupported { feature } if feature.contains("unmaterialized symbolic")),
        "conversion must precede the zero state/edge caps"
    );
    assert_eq!(sink.graph.state_count(), 0);
    assert_eq!(sink.graph.retained_rows, 0);
}

#[test]
fn native_completed_level_prefix_keeps_empty_frontier_rows() {
    let opts = dive_core::Opts::default();
    let prepared = prepare(
        "VARIABLE x\nInit == x = 0\nNext == x < 10 /\\ x' = x + 1",
        "INIT Init\nNEXT Next\n",
        false,
        opts,
    );
    let options = options(false);
    let control = EvaluationControl::new(&options.budget, Phase::Capture).expect("control");
    let mut on_level = |level, graph: &NativeGraph, _: &mut NativeValues<'_>, _: Stats| {
        CsrCapture::checked(graph.input(), graph.limits(), &control).expect("completed boundary");
        Ok(level == 2)
    };
    let mut sink = NativeSink::new(
        &prepared.model,
        Arc::clone(&prepared.store),
        Limits::default(),
        &options,
        Some(&mut on_level),
    )
    .expect("sink");
    let out = dive_explore::explore_with(
        &prepared.plan,
        &prepared.store,
        config(opts, 3),
        &|| false,
        Some(&mut sink),
    );
    assert_eq!(out.verdict, dive_explore::Verdict::PrefixStop { level: 2 });
    let (graph, _) = sink.finish();
    let csr = CsrCapture::checked(graph.input(), graph.limits(), &control).expect("prefix CSR");
    // There is no post-Init level callback. Level two has expanded x=0 and
    // x=1 and discovered the still-unexpanded x=2 frontier.
    assert_eq!(csr.state_count(), 3);
    assert_eq!(csr.closed_rows(), 2);
    assert_eq!(csr.offsets(), &[0, 2, 4, 4]);
    assert_eq!(csr.targets(), &[1, 0, 2, 1]);
    assert_eq!(csr.original_count(2), Some(0));
    let mut safety = crate::core_engine::result(Verdict::Success, &options, &prepared.model.ir);
    safety.stats = Some(Stats {
        generated: out.stats.generated,
        distinct: out.stats.distinct,
        depth: out.stats.depth,
        initial_generated: out.stats.initial_generated,
        left_on_queue: out.stats.left_on_queue,
    });
    assert!(
        CsrComplete::from_producer(
            graph.input(),
            safety,
            options.clone(),
            graph.limits(),
            &control
        )
        .is_err(),
        "prefix must not become complete"
    );
}

#[test]
fn native_callback_shape_failure_stops_without_selecting_a_prefix() {
    let opts = dive_core::Opts::default();
    let prepared = prepare(
        "VARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 3",
        "INIT Init\nNEXT Next\n",
        false,
        opts,
    );
    let options = options(false);
    let control = EvaluationControl::new(&options.budget, Phase::Capture).expect("control");
    let mut on_level = |_, graph: &NativeGraph, _: &mut NativeValues<'_>, _: Stats| {
        let mut malformed = graph.input();
        malformed.closed_rows = graph.state_count() + 1;
        CsrCapture::checked(malformed, graph.limits(), &control).map(|_| false)
    };
    let mut sink = NativeSink::new(
        &prepared.model,
        Arc::clone(&prepared.store),
        Limits::default(),
        &options,
        Some(&mut on_level),
    )
    .expect("sink");
    let out = dive_explore::explore_with(
        &prepared.plan,
        &prepared.store,
        config(opts, 2),
        &|| false,
        Some(&mut sink),
    );
    assert_eq!(out.verdict, dive_explore::Verdict::GraphStop);
    assert!(matches!(
        sink.take_error(),
        Some(Verdict::InternalError { .. })
    ));
}

/// Explores `body` with eager or deferred admission and then reads every
/// retained row, returning the names interned after start-up and the reads.
fn admitted_names(body: &str, deferred: bool, workers: usize) -> (Vec<String>, Vec<String>, u64) {
    let opts = dive_core::Opts::default();
    let prepared = prepare_extending("Naturals, TLC", body, "INIT Init\nNEXT Next\n", false, opts);
    let options = options(false);
    let startup = prepared.model.names.len();
    let mut sink = NativeSink::new(
        &prepared.model,
        Arc::clone(&prepared.store),
        Limits::default(),
        &options,
        None,
    )
    .expect("sink");
    if deferred {
        sink.defer_admission();
    }
    let out = dive_explore::explore_with(
        &prepared.plan,
        &prepared.store,
        config(opts, workers),
        &|| false,
        Some(&mut sink),
    );
    assert_eq!(out.verdict, dive_explore::Verdict::Success);
    let (graph, mut values) = sink.finish();
    let captured = prepared.model.names.len();
    let admitted = values.metrics()["admitted_rows"].as_u64().expect("count");
    let control = EvaluationControl::new(&options.budget, Phase::Temporal).expect("control");
    let mut reads = Vec::new();
    for row in (0..graph.retained_rows).rev() {
        let vals = values.read_retained(&graph, row, &control).expect("read");
        reads.push(format!("{vals:?}"));
    }
    if deferred {
        assert_eq!(captured, startup, "deferred capture interns no name");
    }
    let names = (startup..prepared.model.names.len())
        .map(|token| {
            let token = u32::try_from(token).expect("token");
            prepared
                .model
                .names
                .resolve(token)
                .expect("name")
                .as_str()
                .to_owned()
        })
        .collect();
    (names, reads, admitted)
}

#[test]
fn deferred_admission_interns_the_eager_names_in_the_same_order_at_its_first_read() {
    // Run-time strings in successor rows, and seen roots whose originals
    // differ from their representatives (a general function aliasing a tuple).
    let body = "VARIABLES n, x, s\n\
Init == n \\in 0..3 /\\ (x = <<0>> \\/ x = [i \\in 1..1 |-> 0]) /\\ s = \"init\"\n\
Next == n' = (n + 1) % 4 /\\ x' = x /\\ s' = ToString(<<n, \"z\", n * 7>>)";
    for workers in [1, 2, 8] {
        let (eager, eager_reads, eager_admitted) = admitted_names(body, false, workers);
        let (deferred, deferred_reads, deferred_admitted) = admitted_names(body, true, workers);
        assert!(!eager.is_empty(), "the fixture interns run-time names");
        assert_eq!(deferred, eager, "{workers} workers");
        assert_eq!(deferred_reads, eager_reads, "{workers} workers");
        assert!(
            deferred_admitted < eager_admitted,
            "seen successors stay unadmitted"
        );
    }
}

#[test]
fn deferred_admission_refuses_an_unconvertible_row_at_its_read() {
    let opts = dive_core::Opts::default();
    let prepared = prepare(
        "VARIABLE x\nInit == x = 0\nNext == UNCHANGED x",
        "INIT Init\nNEXT Next\n",
        false,
        opts,
    );
    let options = options(false);
    let mut sink = NativeSink::new(
        &prepared.model,
        Arc::clone(&prepared.store),
        Limits::default(),
        &options,
        None,
    )
    .expect("sink");
    sink.defer_admission();
    let mut heap = Heap::new(Arc::clone(&prepared.store), false, false);
    let interval = heap.interval(1, 2);
    let original = heap.intern_exact(interval).expect("original");
    let canonical = heap.intern(interval).expect("identity");
    sink.emitted(1, None).expect("generated admission first");
    sink.state(
        &mut heap,
        Step {
            key: 1,
            new: Some((0, &[canonical])),
            original: &[original],
            parent: None,
            ordinal: 0,
            action: None,
        },
    )
    .expect("a deferred row is retained without conversion");
    let (graph, mut values) = sink.finish();
    assert_eq!(graph.retained_rows, 1);
    assert_eq!(values.metrics()["admitted_rows"], 0);
    let control = EvaluationControl::new(&options.budget, Phase::Temporal).expect("control");
    assert!(matches!(
        values.read_retained(&graph, 0, &control),
        Err(AnalysisError::UnsupportedValue(reason)) if reason.contains("unmaterialized symbolic")
    ));
}
