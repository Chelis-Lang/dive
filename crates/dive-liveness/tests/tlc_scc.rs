//! Final SCC order and independent accepting-cycle existence controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::CheckOptions;
use dive_liveness::{Graph, product::Limits, search::Acceptance, tlc_scc};
use std::sync::Arc;

fn labels(edges: &[Vec<usize>]) -> Acceptance {
    Acceptance {
        recurrent: edges.iter().map(|row| vec![true; row.len()]).collect(),
        states: vec![],
        actions: vec![],
    }
}
#[test]
fn reverse_successor_order_and_immediate_acceptance_preserve_root_fifo() {
    let edges = vec![vec![1, 2], vec![1], vec![2]];
    let graph = Graph::new(edges.clone(), vec![0, 0]).expect("graph");
    let mut a = labels(&edges);
    let run = tlc_scc::check(&graph, &a, Limits::default()).expect("check");
    assert_eq!(run.discovery, [0, 2]);
    assert_eq!(run.components[0].anchor, 2);
    a.recurrent[0][1] = false;
    let run = tlc_scc::check(&graph, &a, Limits::default()).expect("check");
    assert_eq!(run.discovery, [0, 1]);
    assert_eq!(run.components[0].anchor, 1);
    a.states = vec![vec![false, false, true]];
    let run = tlc_scc::check(&graph, &a, Limits::default()).expect("EA-false root");
    assert_eq!(run.discovery, [0, 1, 2]);
    assert_eq!(run.components.last().expect("component").anchor, 2);
    assert_eq!(run.accepting, Some(2));
}

// Floyd reachability and set-based acceptance deliberately do not use the
// implementation's Tarjan stack, low links, queue or component visitation.
fn independently_accepts(edges: &[Vec<usize>], roots: &[usize], a: &Acceptance) -> bool {
    let n = edges.len();
    let closure = |restricted: bool| {
        let mut reach = vec![vec![false; n]; n];
        for i in 0..n {
            reach[i][i] = true;
            for (e, &target) in edges[i].iter().enumerate() {
                reach[i][target] |= !restricted || a.recurrent[i][e];
            }
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        reach
    };
    let full = closure(false);
    let restricted = closure(true);
    for anchor in 0..n {
        if !roots.iter().any(|r| full[*r][anchor]) {
            continue;
        }
        let members: Vec<_> = (0..n)
            .filter(|i| restricted[anchor][*i] && restricted[*i][anchor])
            .collect();
        let inside = |id| members.contains(&id);
        let cyclic = members.len() > 1
            || edges[anchor]
                .iter()
                .enumerate()
                .any(|(e, t)| *t == anchor && a.recurrent[anchor][e]);
        if !cyclic {
            continue;
        }
        let states = a.states.iter().all(|row| members.iter().any(|id| row[*id]));
        let actions = a.actions.iter().all(|rows| {
            members.iter().any(|id| {
                edges[*id]
                    .iter()
                    .enumerate()
                    .any(|(e, target)| inside(*target) && a.recurrent[*id][e] && rows[*id][e])
            })
        });
        if states && actions {
            return true;
        }
    }
    false
}
#[test]
fn complete_small_graph_census_matches_independent_cycle_acceptance() {
    let mut total = 0;
    for mask in 0u32..512 {
        let edges: Vec<Vec<usize>> = (0..3)
            .map(|i| (0..3).filter(|j| mask & (1 << (i * 3 + j)) != 0).collect())
            .collect();
        for sample in 0..8 {
            let mut a = labels(&edges);
            for (i, row) in edges.iter().enumerate() {
                for (e, &target) in row.iter().enumerate() {
                    a.recurrent[i][e] = (i + target + sample) % 3 != 0;
                }
            }
            a.states = vec![(0..3).map(|i| (i + sample) % 2 == 0).collect()];
            a.actions = vec![
                edges
                    .iter()
                    .enumerate()
                    .map(|(i, row)| row.iter().map(|t| (i + t + sample) % 2 == 1).collect())
                    .collect(),
            ];
            let roots = if sample % 2 == 0 {
                vec![0, 1, 0]
            } else {
                vec![2]
            };
            let graph = Graph::new(edges.clone(), roots.clone()).expect("graph");
            let run = tlc_scc::check(&graph, &a, Limits::default()).expect("bounded complete");
            assert_eq!(
                run.accepting.is_some(),
                independently_accepts(&edges, &roots, &a),
                "mask={mask} sample={sample}"
            );
            let identities: Vec<_> = (0u64..3)
                .map(|i| dive_liveness::tlc_witness::Identity {
                    fingerprint: i,
                    tableau: -1,
                })
                .collect();
            let selected =
                dive_liveness::tlc_witness::select(&graph, &a, &identities, Limits::default())
                    .expect("bounded witness selection");
            if let Some(candidate) = selected.candidate {
                let mut cycle = candidate.cycle_stack.clone();
                cycle.push(candidate.postfix_end);
                let mut closing = candidate
                    .closure_reverse
                    .iter()
                    .rev()
                    .map(|fp| usize::try_from(*fp).expect("small identity"))
                    .collect::<Vec<_>>();
                if closing.first() == Some(&candidate.postfix_end) {
                    closing.remove(0);
                }
                cycle.extend(closing);
                cycle.push(candidate.anchor);
                let selected_edges = cycle
                    .windows(2)
                    .map(|pair| {
                        let edge = edges[pair[0]]
                            .iter()
                            .position(|target| *target == pair[1])
                            .expect("actual cycle edge");
                        assert!(a.recurrent[pair[0]][edge], "mask={mask} sample={sample}");
                        (pair[0], edge)
                    })
                    .collect::<Vec<_>>();
                assert!(a.states.iter().all(|row| cycle.iter().any(|id| row[*id])));
                assert!(
                    a.actions
                        .iter()
                        .all(|rows| selected_edges.iter().any(|(id, e)| rows[*id][*e]))
                );
                let prefix = candidate
                    .prefix_reverse
                    .iter()
                    .rev()
                    .copied()
                    .collect::<Vec<_>>();
                assert!(roots.contains(&prefix[0]));
                assert!(
                    prefix
                        .windows(2)
                        .all(|pair| edges[pair[0]].contains(&pair[1]))
                );
            }
            let mut seen = run.discovery.clone();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), run.discovery.len());
            total += 1;
        }
    }
    assert_eq!(total, 4096);
}
#[test]
fn malformed_and_bounded_inputs_never_become_completed_empty_language() {
    let edges = vec![vec![0]];
    let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
    let mut a = labels(&edges);
    for limit in 0..12 {
        let options = Limits {
            steps: limit,
            ..Limits::default()
        };
        assert!(tlc_scc::check(&graph, &a, options).is_err());
    }
    a.actions = vec![vec![vec![]]];
    assert_eq!(
        tlc_scc::check(&graph, &a, Limits::default())
            .expect_err("shape")
            .code(),
        "DIVE-L0060"
    );
    a = labels(&edges);
    a.states = vec![vec![]];
    assert!(tlc_scc::check(&graph, &a, Limits::default()).is_err());
    a = labels(&edges);
    assert!(
        tlc_scc::check(
            &graph,
            &a,
            Limits {
                acceptance_cells: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    bind(source, config, opts).expect("binding")
}
fn bind(
    source: &str,
    config: &str,
    opts: &CheckOptions,
) -> Result<dive_model::Model, Box<dive_model::Failure>> {
    let name = source.split_whitespace().nth(2).expect("module");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config)
        .into_config()
        .expect("configuration");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "compare all selected pinned SCC/witness fields without dropping models or identities"
)]
fn actual_pinned_worker_anchor_pop_order_and_first_visits_match() {
    use dive_liveness::phase_execution;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probes");
    let capture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(base.join("tlc-final-check/capture.json")).expect("capture"),
    )
    .expect("JSON");
    let mut completed = 0;
    for row in capture["rows"].as_array().expect("complete selection") {
        if row["exit"] != 0 {
            continue;
        }
        completed += 1;
        let name = row["name"].as_str().expect("model name");
        let raw =
            std::fs::read_to_string(base.join("tlc-final-check/raw").join(name).join("stdout"))
                .expect("actual worker projection");
        let fields: std::collections::BTreeMap<_, _> = raw
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .collect();
        let models = base.join("tlc-final-check/models").join(name);
        let source = std::fs::read_to_string(models.join(format!("{name}.tla"))).expect("source");
        let config = std::fs::read_to_string(models.join(format!("{name}.cfg"))).expect("config");
        for workers in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(&source, &config, &opts);
            let run =
                phase_execution::execute(&m, &opts, dive_liveness::phase_graph::Limits::default())
                    .expect("ordered graph");
            for (group, g) in run.groups.iter().enumerate() {
                let identity =
                    |id: usize| format!("{}:{}", g.nodes[id].fingerprint, g.nodes[id].tableau);
                let count = fields
                    .keys()
                    .filter(|k| {
                        k.starts_with(&format!("scc.g{group}.pem")) && k.ends_with(".components")
                    })
                    .count();
                assert!(count > 0);
                for pem in 0..count {
                    let (graph, labels) = run.product_inputs(group, pem).expect("product inputs");
                    let actual = tlc_scc::check(&graph, &labels, Limits::default())
                        .expect("native pinned SCC");
                    let prefix = format!("scc.g{group}.pem{pem}");
                    assert_eq!(
                        actual.components.len().to_string(),
                        fields[format!("{prefix}.components").as_str()],
                        "{name}"
                    );
                    for (c, component) in actual.components.iter().enumerate() {
                        assert_eq!(
                            identity(component.anchor),
                            fields[format!("{prefix}.component.{c}.anchor").as_str()],
                            "{name}"
                        );
                        assert_eq!(
                            component
                                .members
                                .iter()
                                .map(|id| identity(*id))
                                .collect::<Vec<_>>()
                                .join(";"),
                            fields[format!("{prefix}.component.{c}.pop_order").as_str()],
                            "{name}"
                        );
                        assert_eq!(
                            component.accepting.to_string(),
                            fields[format!("{prefix}.component.{c}.accepting").as_str()],
                            "{name}"
                        );
                    }
                    let discoveries: Vec<_> = fields[format!("{prefix}.events").as_str()]
                        .split(';')
                        .filter_map(|item| {
                            let mut parts = item.split(':');
                            if parts.next()? != "put" {
                                return None;
                            }
                            let fp = parts.next()?;
                            let t = parts.next()?;
                            (parts.next()? == "undone").then(|| format!("{fp}:{t}"))
                        })
                        .collect();
                    let names: Vec<_> = g
                        .nodes
                        .iter()
                        .map(|n| dive_liveness::tlc_witness::Identity {
                            fingerprint: n.fingerprint,
                            tableau: n.tableau,
                        })
                        .collect();
                    let selection = dive_liveness::tlc_witness::select(
                        &graph,
                        &labels,
                        &names,
                        Limits::default(),
                    )
                    .expect("pinned selection");
                    if let Some(candidate) = selection.candidate {
                        let c = actual.accepting.expect("accepting index");
                        let cp = format!("{prefix}.component.{c}");
                        assert_eq!(
                            identity(candidate.postfix_end),
                            fields[format!("{cp}.postfix_end").as_str()],
                            "{name}"
                        );
                        assert_eq!(
                            candidate
                                .cycle_stack
                                .iter()
                                .map(|id| identity(*id))
                                .collect::<Vec<_>>()
                                .join(";"),
                            fields[format!("{cp}.cycle_stack_forward").as_str()],
                            "{name}"
                        );
                        assert_eq!(
                            candidate
                                .closure_reverse
                                .iter()
                                .map(u64::to_string)
                                .collect::<Vec<_>>()
                                .join(";"),
                            fields[format!("{cp}.closure_reverse").as_str()],
                            "{name}"
                        );
                        assert_eq!(
                            candidate
                                .prefix_reverse
                                .iter()
                                .map(|id| g.nodes[*id].fingerprint.to_string())
                                .collect::<Vec<_>>()
                                .join(";"),
                            fields[format!("{cp}.prefix_reverse").as_str()],
                            "{name}"
                        );
                    }
                    assert_eq!(
                        actual
                            .discovery
                            .iter()
                            .map(|id| identity(*id))
                            .collect::<Vec<_>>(),
                        discoveries,
                        "{name}"
                    );
                }
            }
        }
    }
    assert_eq!(completed, 19);
}
#[test]
fn exact_budget_edge_and_identity_admission_never_certify_a_cut() {
    use dive_liveness::tlc_witness::{self, Identity};
    let edges = vec![vec![1], vec![0]];
    let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
    let mut a = labels(&edges);
    a.states = vec![vec![false, true]];
    let names = vec![
        Identity {
            fingerprint: 10,
            tableau: -1,
        },
        Identity {
            fingerprint: 20,
            tableau: -1,
        },
    ];
    let result = tlc_witness::select(&graph, &a, &names, Limits::default()).expect("selection");
    let work = result
        .traversal
        .inspections
        .max(result.candidate.expect("candidate").inspections);
    for steps in 0..work {
        assert!(
            tlc_witness::select(
                &graph,
                &a,
                &names,
                Limits {
                    steps,
                    ..Limits::default()
                }
            )
            .is_err(),
            "cut={steps}"
        );
    }
    assert!(
        tlc_witness::select(
            &graph,
            &a,
            &names,
            Limits {
                steps: work,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    assert!(tlc_witness::select(&graph, &a, &[names[0]], Limits::default()).is_err());
    assert!(tlc_witness::select(&graph, &a, &[names[0], names[0]], Limits::default()).is_err());
    assert!(
        tlc_witness::select(
            &graph,
            &a,
            &[
                names[0],
                Identity {
                    tableau: -2,
                    ..names[1]
                }
            ],
            Limits::default()
        )
        .is_err()
    );
}
