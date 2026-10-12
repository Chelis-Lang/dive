//! Stored-only successor witnesses require fresh independent replay.
#![allow(clippy::expect_used, clippy::panic, reason = "adversarial controls")]
use dive_model::BindOptions;
use std::sync::Arc;
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "seven stored shapes and independent closing-edge words"
)]
fn stored_words_are_independently_evaluated_at_every_closing_position() {
    use dive_model::Pred;
    use dive_value::{Ctx, Value};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stored-successor-material/models");
    let mut cases = 0;
    for shape in [
        "Set",
        "Record",
        "Tuple",
        "General",
        "Nested",
        "Strings",
        "ModelAtoms",
    ] {
        let name = format!("Stored{shape}Holds");
        let dir = base.join(&name);
        let analysis = dive_sema::analyze_file(
            &dir.join(format!("{name}.tla")),
            &dive_sema::Options::default(),
        )
        .expect("analysis");
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse(
            &std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg"),
        )
        .into_config()
        .expect("parse");
        for (options, ctx) in [
            (BindOptions::tlc(), Ctx::tlc()),
            (BindOptions::dive(), Ctx::dive()),
        ] {
            let model = dive_model::bind(ir.clone(), &cfg, &options).expect("bind");
            let def = ir.root_def("Live").expect("property");
            let pred = Pred::new(ir.def(def).body, model.evaluator.scope(def).expect("scope"));
            let plan = dive_liveness::plan::lower(
                &model,
                &pred,
                dive_liveness::formula::Limits::default(),
            )
            .expect("plan");
            for n in 1..=4 {
                for mask in 0_usize..(1 << n) {
                    let truth: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
                    let states: Vec<dive_trace::State> = truth
                        .iter()
                        .map(|bit| {
                            let set = || {
                                Value::set(if *bit { vec![1.into()] } else { vec![] }, ctx)
                                    .expect("stored set")
                            };
                            let value = match shape {
                                "Set" => set(),
                                "Record" => Value::record(
                                    vec![
                                        (model.names.intern("flag"), (*bit).into()),
                                        (model.names.intern("bag"), set()),
                                    ],
                                    ctx,
                                )
                                .expect("record"),
                                "Tuple" => Value::tuple(vec![set(), (*bit).into()]),
                                "General" => Value::fcn(
                                    vec![
                                        (Value::string(&model.names, "alpha"), set()),
                                        (Value::string(&model.names, "beta"), set()),
                                    ],
                                    ctx,
                                )
                                .expect("function"),
                                "Nested" => Value::tuple(vec![
                                    Value::record(
                                        vec![
                                            (model.names.intern("flag"), (*bit).into()),
                                            (model.names.intern("bag"), set()),
                                        ],
                                        ctx,
                                    )
                                    .expect("record"),
                                ]),
                                "Strings" => Value::set(
                                    if *bit {
                                        vec![Value::string(&model.names, "alpha")]
                                    } else {
                                        vec![]
                                    },
                                    ctx,
                                )
                                .expect("string set"),
                                "ModelAtoms" => Value::set(
                                    if *bit {
                                        vec![Value::Model(model.names.intern("a"))]
                                    } else {
                                        vec![]
                                    },
                                    ctx,
                                )
                                .expect("model set"),
                                _ => panic!("shape"),
                            };
                            Arc::from([Some(value)])
                        })
                        .collect();
                    for back in 0..n {
                        cases += 1;
                        let independent =
                            dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                                .expect("independent word");
                        assert_eq!(
                            independent,
                            vec![truth[back..].iter().any(|b| *b); n],
                            "{shape}"
                        );
                        let bins: Vec<Vec<bool>> = (0..n)
                            .map(|i| {
                                let edge = dive_eval::St::action(
                                    states[i].clone(),
                                    states[if i + 1 == n { back } else { i + 1 }].clone(),
                                );
                                plan.atoms
                                    .iter()
                                    .map(|atom| atom.evaluate(&model, &edge).expect("atom"))
                                    .collect()
                            })
                            .collect();
                        assert_eq!(
                            plan.formula
                                .evaluate_word(&bins, &bins, back, 1_000_000)
                                .expect("kernel"),
                            independent
                        );
                    }
                }
            }
        }
    }
    assert_eq!(cases, 1372);
}
