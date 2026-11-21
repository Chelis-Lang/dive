//! Native rows, topology and failed admission through the actual explorer.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded capture controls"
)]

use std::sync::Arc;

use dive_core::node::flags;
use dive_runtime::capture::{
    CsrCapture, CsrCaptureColumns, CsrCaptureInput, CsrComplete, CsrStorage,
};

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
                    let csr = CsrCapture::checked_targets(graph.input(), graph.limits(), &control)
                        .expect("CSR");
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
                        assert_eq!(
                            graph.input().edge_row(last),
                            Some(graph.representatives[source])
                        );
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
            let csr =
                CsrCapture::checked_targets(graph.input(), graph.limits(), &control).expect("CSR");
            assert_eq!(graph.state_count(), 3);
            assert_eq!(graph.retained_rows, 3);
            assert_eq!(graph.edge_rows.rows.len(), 0);
            assert_eq!(csr.targets(), &[1, 0, 2, 1, 0, 2]);
            for edge in 0..6 {
                assert_eq!(
                    graph.input().edge_row(edge),
                    Some(graph.representatives[csr.target(edge).expect("target")])
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
            let complete = CsrComplete::from_target_producer(
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
                seen: None,
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
        CsrCapture::checked_targets(graph.input(), graph.limits(), &control)
            .expect("completed boundary");
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
    let csr =
        CsrCapture::checked_targets(graph.input(), graph.limits(), &control).expect("prefix CSR");
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
        CsrComplete::from_target_producer(
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
        CsrCapture::checked_targets(malformed, graph.limits(), &control).map(|_| false)
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
            seen: None,
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

/// One synthetic step with the forced key 7.
#[allow(
    clippy::too_many_arguments,
    reason = "every Step field the test varies"
)]
fn equal_key_step(
    sink: &mut NativeSink<'_, '_>,
    heap: &mut Heap,
    generated: u64,
    new: Option<u64>,
    seen: Option<u64>,
    row: Handle,
    parent: Option<u64>,
    ordinal: u64,
) -> Result<(), Verdict> {
    let canonical = [row];
    let emitted = sink.emitted(generated, parent.map(|_| ordinal));
    let stepped = emitted.and_then(|()| {
        sink.state(
            heap,
            Step {
                key: 7,
                new: new.map(|id| (id, &canonical[..])),
                seen,
                original: &canonical,
                parent,
                ordinal,
                action: parent.map(|_| 0),
            },
        )
    });
    stepped.map_err(|_| sink.take_error().expect("the sink keeps its reason"))
}

/// Two distinct roots with equal keys, then a source whose seen successors
/// name each of them by discovery id.
fn equal_key_steps(sink: &mut NativeSink<'_, '_>, heap: &mut Heap) -> Result<(), Verdict> {
    let zero = Handle::int(0).expect("small integer");
    let one = Handle::int(1).expect("small integer");
    equal_key_step(sink, heap, 1, Some(0), None, zero, None, 0)?;
    equal_key_step(sink, heap, 2, Some(1), None, one, None, 1)?;
    equal_key_step(sink, heap, 3, None, Some(1), one, Some(0), 0)?;
    equal_key_step(sink, heap, 4, None, Some(0), zero, Some(0), 1)
}

/// Equal keys of distinct states, as forced key collisions present them: the
/// exact sink keeps distinct vertices and resolves seen targets by their
/// discovery ids; the fingerprint sink cannot, and says so.
#[test]
fn exact_identities_keep_equal_keys_as_distinct_vertices() {
    let opts = dive_core::Opts::default();
    let prepared = prepare(
        "VARIABLE x\nInit == x \\in {0, 1}\nNext == x' = 1 - x",
        "INIT Init\nNEXT Next\n",
        false,
        opts,
    );
    let options = options(false);
    let sink = || {
        NativeSink::new(
            &prepared.model,
            Arc::clone(&prepared.store),
            Limits::default(),
            &options,
            None,
        )
        .expect("sink")
    };
    let mut heap = Heap::new(Arc::clone(&prepared.store), false, false);

    let mut exact = sink();
    exact.use_exact_identities();
    equal_key_steps(&mut exact, &mut heap).expect("exact identities");
    assert_eq!(exact.graph.state_count(), 2);
    assert_eq!(exact.graph.input().targets.get(0), Some(1));
    assert_eq!(exact.graph.input().targets.get(1), Some(0));
    assert_eq!(exact.graph.input().targets.get(2), None);
    assert_eq!(
        exact.graph.memory.key_entries, 0,
        "no key index in exact mode"
    );

    let mut fingerprint = sink();
    let error = equal_key_steps(&mut fingerprint, &mut heap).expect_err("one key, two states");
    assert!(
        matches!(&error, Verdict::InternalError { message } if message.contains("differ from trace log")),
        "{error:?}"
    );

    // A seen id in fingerprint mode, or none in exact mode, is a bug; so is
    // an id that names no discovered vertex.
    let zero = Handle::int(0).expect("small integer");
    for (exact_mode, seen) in [(false, Some(0)), (true, None), (true, Some(5))] {
        let mut sink = sink();
        if exact_mode {
            sink.use_exact_identities();
        }
        equal_key_step(&mut sink, &mut heap, 1, Some(0), None, zero, None, 0).expect("root");
        let error = equal_key_step(&mut sink, &mut heap, 2, None, seen, zero, Some(0), 0)
            .expect_err("an inconsistent seen id");
        assert!(
            matches!(&error, Verdict::InternalError { message } if message.contains("id")),
            "exact={exact_mode} seen={seen:?}: {error:?}"
        );
    }
}

#[test]
fn compact_csr_preserves_wide_ordinals_and_rejects_malformed_tables() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let limits = Limits {
        successors_per_unit: u64::MAX,
        ..Limits::default()
    };
    let escape = CsrOrdinals::EXTENDED;
    let large = [(1, u64::from(escape)), (2, u64::MAX - 1)];
    let words = [0, escape, escape, u16::MAX];
    let labels = [String::from("advance")];
    // External producers can hand their mutable slice directly to the public input.
    let actions: &mut [Option<u32>] = &mut [None, Some(0), None, None];
    let input = CsrCaptureInput {
        offsets: &[0, 4],
        targets: &[0; 4],
        ordinals: CsrOrdinals::Compact(&words, &large),
        actions,
        edge_rows: &[0; 4],
        representatives: &[0],
        roots: &[0],
        action_labels: &labels,
        retained_rows: 1,
        closed_rows: 1,
    };
    let checked = CsrCapture::checked(input, limits, &control).expect("valid CSR");
    let expanded = [Some(0), Some(u64::from(escape)), Some(u64::MAX - 1), None];
    let previous = CsrCapture::checked(
        CsrCaptureInput {
            ordinals: CsrOrdinals::Expanded(&expanded),
            ..input
        },
        limits,
        &control,
    )
    .expect("valid expanded CSR");
    for edge in 0..=4 {
        assert_eq!(checked.ordinal(edge), previous.ordinal(edge));
        assert_eq!(checked.label(edge), previous.label(edge));
    }
    assert!(
        matches!(input.ordinals.prefix(2), Some(CsrOrdinals::Compact(prefix, side)) if prefix == &words[..2] && side == &large[..1])
    );
    assert!(input.ordinals.prefix(5).is_none());
    for bad in [
        &large[..1],
        &[large[1], large[0]],
        &[large[0], large[0]],
        &[large[0], (3, u64::MAX - 1)],
        &[(1, 1), large[1]],
    ] {
        let malformed = CsrCaptureInput {
            ordinals: CsrOrdinals::Compact(&words, bad),
            ..input
        };
        assert!(CsrCapture::checked(malformed, limits, &control).is_err());
    }
}

#[test]
fn byte_action_ids_preserve_named_unnamed_and_stutter_edges() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let labels: Vec<_> = (0..255).map(|i| format!("action{i}")).collect();
    let mut ids = ActionIds::for_labels(labels.len());
    for id in [Some(254), None, None] {
        ids.push(id).expect("representable ID");
    }
    assert!(ids.push(Some(255)).is_err());
    let input = CsrCaptureColumns {
        offsets: &[0, 3],
        targets: &[0; 3],
        ordinals: &[Some(0), Some(1), None],
        actions: ids.view(),
        edge_rows: &[0; 3],
        representatives: &[0],
        roots: &[0],
        action_labels: &labels,
        retained_rows: 1,
        closed_rows: 1,
    };
    let checked = CsrCapture::checked(input, Limits::default(), &control).expect("valid graph");
    assert_eq!(checked.label(0), Some(Some("action254")));
    assert_eq!(checked.label(1), Some(Some("")));
    assert_eq!(checked.label(2), Some(None));
    assert_eq!(checked.label(3), None);
    assert!(
        matches!(input.actions.prefix(2), Some(CsrActions::Bytes(bytes)) if bytes == [254,255])
    );
    assert!(input.actions.prefix(4).is_none());
    for bad in [CsrActions::Bytes(&[0, 0]), CsrActions::Bytes(&[0, 0, 0])] {
        assert!(
            CsrCapture::checked(
                CsrCaptureColumns {
                    actions: bad,
                    ..input
                },
                Limits::default(),
                &control
            )
            .is_err()
        );
    }
    assert!(
        CsrCapture::checked(
            CsrCaptureColumns {
                action_labels: &labels[..254],
                ..input
            },
            Limits::default(),
            &control
        )
        .is_err()
    );
    let mut wide = ActionIds::for_labels(256);
    wide.push(Some(255)).expect("wide label");
    wide.push(None).expect("wide unnamed");
    assert!(matches!(wide.view(), CsrActions::Words(ids) if ids == [255,u16::MAX]));
    assert!(wide.push(Some(u32::from(u16::MAX))).is_err());
    let mut extended = ActionIds::for_labels(65536);
    extended.push(Some(65535)).expect("full-width label");
    assert!(matches!(extended.view(), CsrActions::Expanded(ids) if ids == [Some(65535)]));
}

#[test]
fn original_row_blocks_promote_late_and_decode_captured_prefixes() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let mut originals = OriginalRows::default();
    let mut targets = Vec::new();
    let mut expanded = Vec::new();
    let mut memory = NativeMemory::default();
    // A discovery ID and its retained row ID need not be equal.
    let representatives = [3];
    for edge in 0..1031 {
        let row = if [511, 512, 700, 1030].contains(&edge) {
            17
        } else {
            3
        };
        originals
            .append(
                (&targets).into(),
                &representatives,
                0,
                row,
                &mut memory,
                &control,
            )
            .expect("append");
        targets.push(0);
        expanded.push(row);
    }
    assert_eq!(originals.directory, [0, 1, 2, 2, 3]);
    assert_eq!(originals.rows.len(), 2 * CsrEdgeRows::BLOCK + 7);
    let view = originals.view(targets.len());
    for count in [
        0, 1, 255, 256, 257, 511, 512, 513, 700, 768, 769, 1024, 1030, 1031,
    ] {
        let prefix = view.prefix(count).expect("available prefix");
        for (edge, &row) in expanded[..count].iter().enumerate() {
            assert_eq!(prefix.row(edge, &targets, &representatives), Some(row));
        }
        assert_eq!(prefix.row(count, &targets, &representatives), None);
    }
    assert!(view.prefix(1032).is_none());
}

#[test]
fn checked_original_row_blocks_validate_directory_bounds_and_stutter() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let input = CsrStorage {
        offsets: &[0, 3],
        targets: &[0; 3],
        ordinals: &[Some(0), Some(1), None],
        actions: CsrActions::Bytes(&[255; 3]),
        edge_rows: CsrEdgeRows::Blocks {
            edges: 3,
            directory: &[1],
            rows: &[2, 3, 1],
        },
        representatives: &[1],
        roots: &[0],
        action_labels: &[],
        retained_rows: 4,
        closed_rows: 1,
    };
    let csr = CsrCapture::checked(input, Limits::default(), &control).expect("valid");
    assert_eq!(csr.original_row(0, 0), Some(2));
    assert_eq!(csr.original_row(0, 1), Some(3));
    assert_eq!(csr.original_row(0, 2), None);
    for (edges, directory, rows) in [
        (2, &[1][..], &[2, 3][..]),
        (3, &[], &[]),
        (3, &[usize::MAX], &[2, 3, 1]),
        (3, &[0], &[2, 3, 1]),
        (3, &[1], &[2, 3]),
        (3, &[1], &[2, 4, 1]),
        (3, &[1], &[2, 3, 2]),
    ] {
        assert!(
            CsrCapture::checked(
                CsrStorage {
                    edge_rows: CsrEdgeRows::Blocks {
                        edges,
                        directory,
                        rows
                    },
                    ..input
                },
                Limits::default(),
                &control
            )
            .is_err()
        );
    }
}

#[test]
fn dense_row_inputs_preserve_slice_coercions() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let mut rows = [7];
    let input = CsrCaptureInput {
        offsets: &[0, 1],
        targets: &[0],
        ordinals: &[None],
        actions: &mut [None],
        edge_rows: rows.as_mut_slice(),
        representatives: &[7],
        roots: &[0],
        action_labels: &[],
        retained_rows: 8,
        closed_rows: 1,
    };
    assert!(CsrCapture::checked(input, Limits::default(), &control).is_ok());
    let boxed: Box<[u32]> = vec![7].into_boxed_slice();
    let changed = CsrCaptureColumns {
        edge_rows: &boxed,
        ..input
    };
    assert!(CsrCapture::checked(changed, Limits::default(), &control).is_ok());
}

#[test]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the generated graph has fewer than 2,000 edges"
)]
fn action_blocks_preserve_random_runs_unnamed_edges_and_prefixes() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let lengths = [1, 254, 1, 255, 256, 257, 71, 103];
    let edges: usize = lengths.iter().sum();
    let mut offsets = vec![0];
    let mut targets = Vec::new();
    let mut expected = Vec::new();
    let mut ordinals = Vec::new();
    for (source, &length) in lengths.iter().enumerate() {
        for i in 0..length {
            let ordinal = (i + 1 < length).then_some(i as u64);
            let n = targets.len();
            let id = match n / 256 {
                0 => Some(0),
                1 => Some((n % 2) as u32),
                2 => Some(((n * 97 + 11) % 251) as u32),
                _ => (!(n / 17).is_multiple_of(3)).then_some(((n / 31) % 111) as u32),
            };
            expected.push(ordinal.and(id));
            ordinals.push(ordinal);
            targets.push(source as u32);
        }
        offsets.push(targets.len());
    }
    for names in [255, 292, 65535, 65536] {
        // A long run contains the largest actual ID at each width.
        let expected: Vec<_> = expected
            .iter()
            .map(|id| id.map(|n| if n == 0 { names as u32 - 1 } else { n }))
            .collect();
        let labels: Vec<_> = (0..names).map(|n| format!("A{n}")).collect();
        let mut ids = ActionIds::for_labels(names);
        let mut memory = NativeMemory::default();
        for &action in &expected {
            ids.reserve(&mut memory, &control).expect("reserve");
            ids.push(action).expect("append");
        }
        let acts = ids.view();
        let rows: Vec<_> = (0..lengths.len() as u32).collect();
        for (count, &end) in offsets.iter().enumerate().skip(1) {
            let graph = CsrCapture::checked(
                CsrCaptureColumns {
                    offsets: &offsets[..=count],
                    targets: &targets[..end],
                    ordinals: &ordinals[..end],
                    actions: acts.prefix(end).expect("prefix"),
                    edge_rows: &targets[..end],
                    representatives: &rows[..count],
                    roots: &[0],
                    action_labels: &labels,
                    retained_rows: count,
                    closed_rows: count,
                },
                Limits::default(),
                &control,
            )
            .expect("checked prefix");
            for edge in (0..end).rev() {
                let name = ordinals[edge]
                    .map(|_| expected[edge].map_or("", |n| labels[n as usize].as_str()));
                assert_eq!(graph.label(edge), Some(name));
            }
        }
        assert!(acts.prefix(edges + 1).is_none());
    }
}

#[test]
#[allow(
    clippy::cast_possible_truncation,
    reason = "three blocks of 256 edges bound each position"
)]
fn implicit_ordinal_blocks_preserve_gaps_full_width_and_stutters() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let edges = 3 * 256;
    let mut words = vec![u16::MAX; 256];
    let mut wide = Vec::new();
    let mut targets = Vec::new();
    let mut expected = Vec::new();
    for edge in 0..edges {
        let source = edge / 256;
        let pos = edge % 256;
        targets.push(source as u32);
        let mut ordinal = (pos != 255).then_some(pos as u64);
        if source == 1 && (40..255).contains(&pos) {
            if pos < 180 {
                words[pos] = 500 + pos as u16;
                ordinal = Some(u64::from(words[pos]));
            } else {
                words[pos] = CsrOrdinals::EXTENDED;
                ordinal = Some(u64::MAX - 256 + pos as u64);
                wide.push((edge, ordinal.expect("generated")));
            }
        }
        expected.push(ordinal);
    }
    let ords = CsrOrdinals::PositionBlocks {
        edges,
        sources: &[0, 1, 2],
        directory: &[0, 1, 1],
        words: &words,
        large: &wide,
    };
    for rows in 1..=3 {
        let end = rows * 256;
        let acts = vec![None; end];
        let graph = CsrCapture::checked(
            CsrCaptureColumns {
                offsets: &[0, 256, 512, 768][..=rows],
                targets: &targets[..end],
                ordinals: ords.prefix(end).expect("prefix"),
                actions: acts.as_slice(),
                edge_rows: &targets[..end],
                representatives: &[0, 1, 2][..rows],
                roots: &[0],
                action_labels: &[],
                retained_rows: rows,
                closed_rows: rows,
            },
            Limits {
                successors_per_unit: u64::MAX,
                ..Limits::default()
            },
            &control,
        )
        .expect("valid graph");
        for edge in (0..end).rev() {
            assert_eq!(graph.ordinal(edge), Some(expected[edge]));
            assert_eq!(
                graph.metadata(edge / 256, edge % 256).expect("metadata").0,
                expected[edge]
            );
        }
        assert_eq!(graph.ordinal(end), None);
    }
}

#[test]
fn checked_block_columns_reject_malformed_runs_unknown_ids_and_source_hints() {
    let control = EvaluationControl::new(&options(false).budget, Phase::Capture).expect("control");
    let offsets = [0, 5];
    let labels = ["left".to_owned(), "right".to_owned()];
    let positional = |sources, directory, words| CsrOrdinals::PositionBlocks {
        edges: 5,
        sources,
        directory,
        words,
        large: &[],
    };
    let runs = |encoded| {
        CsrActions::WordBlocks(CsrActionBlocks {
            edges: 5,
            directory: &[6],
            encoded,
            tail: &[],
        })
    };
    let input = CsrCaptureColumns {
        offsets: &offsets,
        targets: &[0; 5],
        ordinals: positional(&[0], &[0], &[]),
        actions: runs(&[1, 0, 3, 1, 255, u16::MAX]),
        edge_rows: &[0; 5],
        representatives: &[0],
        roots: &[0],
        action_labels: &labels,
        retained_rows: 1,
        closed_rows: 1,
    };
    let graph =
        CsrCapture::checked(input, Limits::default(), &control).expect("prefix inside a run block");
    assert_eq!(graph.label(1), Some(Some("left")));
    assert_eq!(graph.label(2), Some(Some("right")));
    assert_eq!(graph.label(4), Some(None));
    for actions in [
        runs(&[1, 0, 1, 1, 255, u16::MAX]), // repeated endpoint
        runs(&[1, 0, 3, 1, 254, u16::MAX]), // missing block end
        runs(&[1, 2, 3, 1, 255, u16::MAX]), // unknown label
        runs(&[1, 0, 3, 1, 255, 1]),        // labelled stutter
        runs(&[1, 0, 255, u16::MAX]),       // bad block extent
    ] {
        assert!(
            CsrCapture::checked(
                CsrCaptureColumns { actions, ..input },
                Limits::default(),
                &control
            )
            .is_err()
        );
    }
    for ordinals in [
        positional(&[], &[0], &[]),
        positional(&[1], &[0], &[]),
        positional(&[0], &[2], &[]),
        positional(&[0], &[1], &[0]),
        positional(&[0], &[1], &[0, 1, 2, 3, 4]), // explicit ordinal on the stutter
        positional(&[0], &[1], &[0, 2, 1, 3, u16::MAX]),
    ] {
        assert!(
            CsrCapture::checked(
                CsrCaptureColumns { ordinals, ..input },
                Limits::default(),
                &control
            )
            .is_err()
        );
    }
}
