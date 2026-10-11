//! Comparing dive's binding of a model with TLC's (`acceptance/ModelDump.java`).

#![allow(dead_code, reason = "shared by the corpus example and the tests")]
#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_model::{BindOptions, ErrorKind, Failure, Model};
use serde_json::Value as J;

/// TLC's binding of one model.
pub struct TlcDump {
    pub id: String,
    pub json: J,
}

impl TlcDump {
    pub fn parse(line: &str) -> Option<Self> {
        let json: J = serde_json::from_str(line).ok()?;
        Some(Self {
            id: json.get("id")?.as_str()?.to_owned(),
            json,
        })
    }

    pub fn error_code(&self) -> Option<i64> {
        self.json.get("error")?.get("code")?.as_i64()
    }

    fn strings(&self, key: &str) -> Vec<String> {
        self.json
            .get(key)
            .and_then(J::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// TLC's intern table, by token.
    pub fn interned(&self) -> (i64, Vec<String>) {
        let t = self.json.get("interned");
        let first = t
            .and_then(|t| t.get("first"))
            .and_then(J::as_i64)
            .unwrap_or(1);
        let names = t
            .and_then(|t| t.get("names"))
            .and_then(J::as_array)
            .map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or("\u{0}gap").to_owned())
                    .collect()
            })
            .unwrap_or_default();
        (first, names)
    }
}

/// How dive's binding compares with TLC's.
#[derive(Debug)]
pub enum Outcome {
    /// Both bind, and every role agrees.
    Bound,
    /// Both bind and agree, but the model uses features dive refuses.
    BoundUnsupported(Vec<String>),
    /// Both report the same error code.
    SameError(i64),
    /// dive refuses (unsupported), naming the feature.
    Unsupported(String),
    /// TLC rejects the model; dive's front end rejects it too.
    FrontEnd,
    /// Anything else.
    Wrong(String),
}

pub struct Setup {
    pub after: Vec<PathBuf>,
    pub dump_trace: bool,
    pub opts: BindOptions,
}

pub fn analyze(path: &Path, setup: &Setup) -> Result<Arc<dive_ir::Ir>, String> {
    let mut o = dive_sema::Options::default();
    o.search.after_standard.clone_from(&setup.after);
    if setup.dump_trace {
        o.extra_root_extendees = dive_model::DUMP_TRACE_JSON_MODULES
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
    }
    let a = dive_sema::analyze_file(path, &o).map_err(|e| format!("read: {e}"))?;
    if !a.accepted() {
        return Err("front end rejects".to_owned());
    }
    dive_ir::lower(&a).map(Arc::new).map_err(|e| e.to_string())
}

/// Binds on a thread with a large stack.
pub fn bind(
    ir: Arc<dive_ir::Ir>,
    cfg: dive_config::ModelConfig,
    opts: BindOptions,
) -> Result<Model, Box<Failure>> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || dive_model::bind(ir, &cfg, &opts))
        .expect("spawn")
        .join()
        .unwrap_or_else(|_| {
            Err(Box::new(Failure {
                error: dive_model::BindError::bug("the binding thread panicked"),
                output: Vec::new(),
            }))
        })
}

/// Runs and compares one model.
pub fn compare(
    dir: &Path,
    module: &str,
    cfg_name: &str,
    tlc: &TlcDump,
    setup: &Setup,
) -> (Outcome, Option<Model>) {
    let tlc_error = tlc.error_code();
    let ir = match analyze(&dir.join(module), setup) {
        Ok(ir) => ir,
        Err(e) => {
            return (
                if tlc_error.is_some() {
                    Outcome::FrontEnd
                } else {
                    Outcome::Wrong(format!("dive: {e}; TLC binds"))
                },
                None,
            );
        }
    };
    let text = std::fs::read(dir.join(cfg_name)).unwrap_or_default();
    let (_, parsed) = dive_config::parse_bytes(&text);
    let Some(cfg) = parsed.config().cloned() else {
        let code = parsed
            .error()
            .and_then(|e| e.tlc.as_ref())
            .map(|t| i64::from(t.code));
        return (
            match (code, tlc_error) {
                (Some(a), Some(b)) if a == b => Outcome::SameError(a),
                (a, b) => Outcome::Wrong(format!("cfg rejected by dive ({a:?}), TLC: {b:?}")),
            },
            None,
        );
    };
    match bind(ir, cfg, setup.opts.clone()) {
        Ok(model) => {
            if let Some(code) = tlc_error {
                let msg = tlc
                    .json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(J::as_str)
                    .unwrap_or("");
                return (
                    Outcome::Wrong(format!("dive binds; TLC fails with {code}: {msg}")),
                    Some(model),
                );
            }
            let diffs = role_diffs(&model, tlc);
            let out = if !diffs.is_empty() {
                Outcome::Wrong(diffs.join("; "))
            } else if model.unsupported.is_empty() {
                Outcome::Bound
            } else {
                Outcome::BoundUnsupported(
                    model
                        .unsupported
                        .iter()
                        .map(|u| u.feature.clone())
                        .collect(),
                )
            };
            (out, Some(model))
        }
        Err(f) => (
            match (&f.error.kind, tlc_error) {
                (ErrorKind::Unsupported { feature }, _) => Outcome::Unsupported(feature.clone()),
                (_, Some(c)) if f.error.code().map(i64::from) == Some(c) => Outcome::SameError(c),
                (_, c) => Outcome::Wrong(format!(
                    "dive: {}; TLC: {c:?} {}",
                    f.error,
                    tlc.json.get("error").map(J::to_string).unwrap_or_default()
                )),
            },
            None,
        ),
    }
}

fn names(v: &[dive_model::Named]) -> Vec<String> {
    v.iter().map(|n| n.name.clone()).collect()
}

/// The roles that differ from TLC's.
pub fn role_diffs(m: &Model, tlc: &TlcDump) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |what: &str, dive: Vec<String>, tlc: Vec<String>| {
        if dive != tlc {
            out.push(format!("{what}: dive {dive:?} TLC {tlc:?}"));
        }
    };
    let inits: Vec<String> = m
        .init
        .iter()
        .map(|i| {
            let name = i
                .def
                .map_or_else(|| "Action".to_owned(), |d| m.ir.def(d).name.clone());
            format!("<{name} {}>", m.location(&i.pred))
        })
        .collect();
    check("inits", inits, tlc.strings("inits"));
    check(
        "actions",
        m.actions.iter().map(|a| a.label().to_owned()).collect(),
        tlc.strings("actions"),
    );
    check(
        "invariants",
        names(&m.invariants),
        tlc.strings("invariants"),
    );
    check(
        "implied inits",
        names(&m.implied_inits),
        tlc.strings("impliedInits"),
    );
    check(
        "implied actions",
        names(&m.implied_actions),
        tlc.strings("impliedActions"),
    );
    check("temporals", names(&m.temporals), tlc.strings("temporals"));
    check(
        "implied temporals",
        names(&m.implied_temporals),
        tlc.strings("impliedTemporals"),
    );
    check(
        "constraints",
        names(&m.constraints),
        tlc.strings("constraints"),
    );
    check(
        "action constraints",
        names(&m.action_constraints),
        tlc.strings("actionConstraints"),
    );
    // Post conditions and alias: names or TLC's error code.
    let tlc_pcs = tlc.json.get("postconditions");
    let dive_pcs = match &m.postconditions {
        Ok(v) => serde_json::Value::from(v.iter().map(|n| n.name.clone()).collect::<Vec<_>>()),
        Err(e) => serde_json::json!({ "error": e.code() }),
    };
    if let Some(t) = tlc_pcs {
        // `-dumpTrace json` adds its own post condition, which dive does not model.
        let t = match t.as_array() {
            Some(a) => serde_json::Value::from(
                a.iter()
                    .filter(|v| v.as_str() != Some("_JsonTrace"))
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
            None => t.clone(),
        };
        if t != dive_pcs {
            out.push(format!("post conditions: dive {dive_pcs} TLC {t}"));
        }
    }
    if let Some(t) = tlc.json.get("aliasSpec") {
        let d = match &m.alias {
            None => serde_json::Value::Null,
            Some(Ok(n)) => serde_json::Value::from(m.location(&n.pred)),
            Some(Err(e)) => serde_json::json!({ "error": e.code() }),
        };
        if *t != d {
            out.push(format!("alias: dive {d} TLC {t}"));
        }
    }
    let tlc_view = tlc.json.get("view").is_some_and(|v| !v.is_null());
    if tlc_view != m.view.is_some() {
        out.push(format!("view: dive {} TLC {tlc_view}", m.view.is_some()));
    }
    let tlc_deadlock = tlc
        .json
        .get("deadlock")
        .and_then(J::as_bool)
        .unwrap_or(true);
    if tlc_deadlock != m.settings.check_deadlock {
        out.push(format!(
            "deadlock: dive {} TLC {tlc_deadlock}",
            m.settings.check_deadlock
        ));
    }
    out
}

/// How dive's name table compares with TLC's: model values with TLC's exact
/// token, and the names both tables hold that are out of TLC's relative
/// order (the complement of the longest common increasing subsequence).
pub struct InternStats {
    pub model_values: usize,
    pub model_values_exact: usize,
    pub common: usize,
    pub out_of_order: Vec<String>,
}

pub fn intern_stats(m: &Model, cfg: &dive_config::ModelConfig, tlc: &TlcDump) -> InternStats {
    let (first, tlc_names) = tlc.interned();
    let mut mv_exact = 0;
    for mv in &cfg.model_values {
        let tlc_tok = tlc_names
            .iter()
            .position(|n| n == mv)
            .map(|i| i64::try_from(i).unwrap_or(i64::MAX) + first);
        let dive_tok = m.names.get(mv).map(|a| i64::from(a.tok()));
        if tlc_tok.is_some() && tlc_tok == dive_tok {
            mv_exact += 1;
        }
    }
    // dive's names in token order.
    let mut dive_pos = std::collections::HashMap::new();
    for tok in 0..m.names.len() {
        if let Some(a) = m.names.resolve(u32::try_from(tok).unwrap_or(u32::MAX)) {
            dive_pos.entry(a.as_str().to_owned()).or_insert(tok);
        }
    }
    let seq: Vec<(usize, &String)> = tlc_names
        .iter()
        .filter_map(|n| dive_pos.get(n).map(|&p| (p, n)))
        .collect();
    // Longest increasing subsequence of dive positions along TLC's order.
    let mut tails: Vec<usize> = Vec::new();
    let mut tail_idx: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; seq.len()];
    for (i, (p, _)) in seq.iter().enumerate() {
        let k = tails.partition_point(|&t| t < *p);
        if k > 0 {
            prev[i] = Some(tail_idx[k - 1]);
        }
        if k == tails.len() {
            tails.push(*p);
            tail_idx.push(i);
        } else {
            tails[k] = *p;
            tail_idx[k] = i;
        }
    }
    let mut keep = vec![false; seq.len()];
    let mut cur = tail_idx.last().copied();
    while let Some(i) = cur {
        keep[i] = true;
        cur = prev[i];
    }
    let out_of_order = seq
        .iter()
        .zip(&keep)
        .filter(|(_, k)| !**k)
        .map(|((_, n), _)| (*n).clone())
        .collect();
    InternStats {
        model_values: cfg.model_values.len(),
        model_values_exact: mv_exact,
        common: seq.len(),
        out_of_order,
    }
}
