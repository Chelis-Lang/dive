//! Streamed documents (`dive_trace::stream`): writing one state at a time
//! gives the bytes of the whole document, and validating a header with a
//! state source gives the verdict of validating the whole document.
#![allow(clippy::expect_used, clippy::panic, reason = "fixed test sources")]

use dive_trace::stream::{StateSource, Streamed, write_document};
use dive_trace::{Check, Ending, Label, Session, TraceDoc, Validation};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = "---- MODULE S ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == \\/ x < 3 /\\ x' = x + 1\n  \\/ \\E n \\in Nat : x' = x + n /\\ n = 10\nInv == x < 3\n====\n";

fn session(invariant: bool) -> Session {
    let analysis = dive_sema::analyze_source("S.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(if invariant {
        "INIT Init\nNEXT Next\nINVARIANT Inv\n"
    } else {
        "INIT Init\nNEXT Next\n"
    })
    .into_config()
    .expect("config");
    Session::new(
        dive_model::Recipe::new(ir, config, dive_eval::Options::dive(), false),
        None,
    )
}

fn word(xs: &[i64], ending: Ending) -> TraceDoc {
    let mut doc = TraceDoc::behavior(xs.iter().map(|x| json!({ "x": x })).collect())
        .expect("word")
        .with_ending(ending);
    for k in 1..xs.len() {
        doc.labels[k] = Some(Label {
            name: "Next".into(),
            location: Some(format!("<Next line {k}>")),
        });
    }
    doc
}

/// A source that produces its states from integers on demand.
struct Counting(Vec<i64>);

impl StateSource for Counting {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn states(&self, each: &mut dyn FnMut(Streamed) -> Result<(), String>) -> Result<(), String> {
        for (k, x) in self.0.iter().enumerate() {
            let serde_json::Value::Object(values) = json!({ "x": x }) else {
                unreachable!()
            };
            each(Streamed {
                values,
                label: (k > 0).then(|| Label {
                    name: "Next".into(),
                    location: Some(format!("<Next line {k}>")),
                }),
                witnesses: if x - self.0.get(k.wrapping_sub(1)).copied().unwrap_or(0) == 10 {
                    vec![json!(10)]
                } else {
                    Vec::new()
                },
                draws: Vec::new(),
            })?;
        }
        Ok(())
    }
}

#[test]
fn streamed_documents_are_the_document_bytes() {
    let mut big =
        word(&[0, 1, 2, 12], Ending::Invariant("Inv".into())).with_witnesses(3, vec![json!(10)]);
    big.length = Some(4);
    for doc in [
        word(&[0], Ending::Behavior),
        word(&[0, 1, 2], Ending::Deadlock),
        big,
        TraceDoc::behavior(Vec::new()).expect("empty"),
        word(&[0, 1], Ending::Behavior)
            .with_properties(vec!["P".into()])
            .with_concrete_closure(),
    ] {
        let mut text = Vec::new();
        write_document(&doc.header(), &doc, &mut text).expect("written");
        assert_eq!(
            String::from_utf8(text).expect("utf-8"),
            doc.to_json().to_string()
        );
        let mut measure = dive_trace::stream::Measure::default();
        write_document(&doc.header(), &doc, &mut measure).expect("measured");
        assert_eq!(measure.len, doc.to_json().to_string().len());
    }
}

#[test]
fn streamed_validation_gives_the_document_verdict() {
    let cases: [(&[i64], Ending, bool); 6] = [
        (&[0, 1, 2], Ending::Behavior, false),
        (&[0, 1, 11], Ending::Behavior, false),
        (&[0, 2], Ending::Behavior, false),
        (&[0, 1, 2, 3], Ending::Invariant("Inv".into()), true),
        (&[0, 1, 2], Ending::Invariant("Inv".into()), true),
        (&[0, 10], Ending::Behavior, false),
    ];
    for (xs, ending, invariant) in cases {
        let session = session(invariant);
        let source = Counting(xs.to_vec());
        let mut doc = TraceDoc::behavior(Vec::new())
            .expect("header")
            .with_ending(ending.clone());
        doc.variables = vec!["x".into()];
        let streamed = session
            .read_sampled_source(&doc, &source, None)
            .expect("completed")
            .validation;
        let mut whole = word(xs, ending.clone());
        for (k, x) in xs.iter().enumerate().skip(1) {
            if x - xs[k - 1] == 10 {
                whole = whole.with_witnesses(k, vec![json!(10)]);
            }
        }
        let expected = match ending {
            Ending::Behavior => session.read_sampled(&whole, None),
            _ => session.read_controlled(&whole, Arc::new(Continue)),
        }
        .expect("completed")
        .0
        .validation;
        assert_eq!(streamed, expected, "{xs:?}");
    }
    // The verdicts themselves: a valid word, a skipped step, and the
    // witness that proves the jump of ten.
    let session = session(false);
    let header = TraceDoc::behavior(Vec::new()).expect("header");
    let read = |xs: &[i64]| {
        session
            .read_sampled_source(&header, &Counting(xs.to_vec()), None)
            .expect("completed")
            .validation
    };
    assert_eq!(read(&[0, 1, 2]), Validation::Valid);
    assert_eq!(read(&[0, 10]), Validation::Valid);
    match read(&[0, 2]) {
        Validation::Invalid(failure) => {
            assert_eq!((failure.state, failure.check), (1, Check::Next));
        }
        other => panic!("{other:?}"),
    }
    // Endings simulation never produces are refused by name.
    let other = TraceDoc::behavior(Vec::new())
        .expect("header")
        .with_ending(Ending::Stuttering);
    assert!(matches!(
        session
            .read_sampled_source(&other, &Counting(vec![0]), None)
            .expect("completed")
            .validation,
        Validation::Unsupported(_)
    ));
}

#[derive(Debug)]
struct Continue;
impl dive_eval::HostControl for Continue {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        Ok(())
    }
}
