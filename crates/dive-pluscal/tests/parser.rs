//! Source AST contracts and deliberate rejection/resource controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test controls")]
use dive_pluscal::{Code, Options, ast::*, parse, parse_at};

#[test]
fn source_p_structures_are_not_expanded_or_dropped() {
    let src = r"--fair algorithm A
variables x = 0, y \in {1, 2};
define D == INSTANCE M WITH N <- 1, O <- 2 end define;
macro M(a) begin a := a + 1 end macro;
procedure P(a = 0, b) variables z = 1; begin L: return; end procedure;
begin
L: + while x < 3 do
  if x = 0 then x := 1 || y[1] := 2;
  elsif x = 1 then either skip or assert TRUE end either;
  else with a \in {1, 2}, b = a do print b; end with;
  end if;
  await TRUE; when TRUE; M(x); call P(x, y); goto Done;
end while;
end algorithm;";
    let parsed = parse(src, &Options::default()).expect("P grammar");
    assert_eq!(parsed.source, src);
    assert_eq!(parsed.algorithm.fairness, Fairness::Weak);
    assert_eq!(parsed.algorithm.variables.len(), 2);
    assert_eq!(parsed.algorithm.macros.len(), 1);
    assert_eq!(parsed.algorithm.procedures[0].parameters.len(), 2);
    assert!(
        parsed
            .text(parsed.algorithm.definitions.expect("define").span)
            .expect("slice")
            .contains("N <- 1, O <- 2")
    );
    assert!(
        parsed
            .statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::MacroCall { .. }))
    );
    assert!(
        parsed
            .statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::Call { .. }))
    );
    assert!(
        parsed
            .statements
            .iter()
            .any(|s| matches!(&s.kind, StatementKind::Assign(a) if a.len()==2))
    );
    for stmt in &parsed.statements {
        assert!(parsed.text(stmt.span).is_some());
    }
}

#[test]
fn c_process_procedure_and_nested_braces_preserve_source() {
    let src = r"--algorithm C {
variables x = 0;
procedure P(a = 1) variables z; { R: return; }
fair + process (P \in {1, 2}) variables a = [begin |-> 1, end |-> 2]; {
 L: - { while (x < 2) { x := x + 1; }; };
 either { skip; } or { call P(0); return; };
 with (b = 2, c \in {3, 4}) if (b = 2) print c; else assert TRUE;
}
process (Q = 3) { QL: await TRUE; }
}";
    let parsed = parse(src, &Options::default()).expect("C grammar");
    assert_eq!(parsed.algorithm.syntax, Syntax::C);
    let Body::Processes(processes) = &parsed.algorithm.body else {
        panic!("processes")
    };
    assert_eq!(processes.len(), 2);
    assert_eq!(processes[0].fairness, Fairness::Strong);
    assert!(
        parsed
            .statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::Block(_)))
    );
    assert_eq!(parsed.text(parsed.algorithm.span), Some(src));
}

#[test]
fn selected_region_ranges_point_into_original_source() {
    let src = "---- MODULE M ----\r\n(* --algorithm A { { L: skip; } } *)\n====";
    let start = src.find("--algorithm").expect("start");
    let end = src.find(" *)").expect("end");
    let parsed = parse_at(src, start..end, &Options::default()).expect("selected");
    assert_eq!(parsed.text(parsed.algorithm.name.span), Some("A"));
    assert_eq!(parsed.algorithm.span.start, start);
    assert_eq!(parsed.source, src);
    assert!(parse_at(src, usize::MAX..usize::MAX, &Options::default()).is_err());
}

#[test]
fn byte_ranges_and_display_columns_are_distinct() {
    let source = "α\r\n\t(* --algorithm A begin L: print \"ascii\"; end algorithm *)";
    let start = source.find("--algorithm").expect("marker");
    let end = source.find(" *)").expect("comment end");
    let parsed = dive_pluscal::parse_at(source, start..end, &Options::default()).expect("region");
    let position = parsed.position(start).expect("display");
    assert_eq!((position.line, position.column), (2, 12));
    assert!(parsed.position(1).is_none(), "inside UTF8 scalar");
    assert!(parsed.position(usize::MAX).is_none());
    let token = parsed
        .tokens
        .iter()
        .find(|t| t.text.starts_with('"'))
        .expect("string");
    assert_eq!(parsed.text(token.span), Some("\"ascii\""));
}

#[test]
fn expressions_keep_quantifier_commas_strings_comments_and_record_keywords() {
    let src = r#"--algorithm A variables x = 0; begin
L: x := \A a, b \in {1, 2}: a = b;
x := [end |-> ";end", begin |-> TRUE].end;
x := <<1, (* comment *) 2>>;
end algorithm"#;
    let parsed = parse(src, &Options::default()).expect("expressions");
    assert_eq!(parsed.statements.len(), 3);
    assert!(
        parsed
            .text(parsed.statements[0].span)
            .expect("original")
            .contains("a, b")
    );
}

#[test]
fn resource_and_invalid_controls_never_produce_an_ast() {
    for (source, code) in [
        (
            "--algorithm A begin either skip end either end algorithm",
            Code::Syntax,
        ),
        (
            "--algorithm A begin L: while TRUE do end while end algorithm",
            Code::Syntax,
        ),
        ("--algorithm A begin Done: skip end algorithm", Code::Label),
        (
            "--algorithm A macro M() begin L: skip end macro begin skip end algorithm",
            Code::Label,
        ),
        (
            "--algorithm A begin L: x := (1; end algorithm",
            Code::Expression,
        ),
        (
            "--algorithm A begin L: print \"bad; end algorithm",
            Code::Lexical,
        ),
        (
            "--algorithm A begin L: skip end algorithm junk",
            Code::Syntax,
        ),
    ] {
        let a = parse(source, &Options::default()).expect_err("negative");
        assert_eq!(a.code, code, "{source}: {a}");
        assert_eq!(parse(source, &Options::default()).expect_err("repeat"), a);
    }
    let source = "--algorithm A begin L: skip; end algorithm";
    for opts in [
        Options {
            max_bytes: 1,
            ..Options::default()
        },
        Options {
            max_tokens: 1,
            ..Options::default()
        },
        Options {
            max_nodes: 1,
            ..Options::default()
        },
        Options {
            max_depth: 0,
            ..Options::default()
        },
    ] {
        assert_eq!(
            parse(source, &opts).expect_err("limit").code,
            Code::Resource
        );
    }
}

#[test]
fn deep_input_is_bounded_on_an_ordinary_caller_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let source = format!(
                "--algorithm A {{ {{ {}skip;{} }} }}",
                "while (TRUE) {".repeat(1000),
                "};".repeat(1000)
            );
            let options = Options {
                max_depth: usize::MAX,
                ..Options::default()
            };
            assert_eq!(
                parse(&source, &options).expect_err("bounded").code,
                Code::Resource
            );
        })
        .expect("spawn bounded test")
        .join()
        .expect("no parser panic");
}

#[test]
fn all_arena_nodes_are_reachable_and_token_ranges_preserve_utf8() {
    use std::collections::BTreeSet;
    fn block(
        ids: &mut BTreeSet<usize>,
        parsed: &dive_pluscal::Parsed,
        body: &dive_pluscal::ast::Block,
    ) {
        for id in &body.statements {
            assert!(ids.insert(id.0), "each statement has exactly one parent");
            let statement = parsed.statement(*id).expect("known arena reference");
            match &statement.kind {
                StatementKind::Block(b)
                | StatementKind::While { body: b, .. }
                | StatementKind::With { body: b, .. } => block(ids, parsed, b),
                StatementKind::If {
                    branches,
                    otherwise,
                } => {
                    for branch in branches {
                        block(ids, parsed, &branch.body);
                    }
                    if let Some(b) = otherwise {
                        block(ids, parsed, b);
                    }
                }
                StatementKind::Either(bs) => {
                    for b in bs {
                        block(ids, parsed, b);
                    }
                }
                _ => {}
            }
        }
    }
    let source = "--fair algorithm A { variable x ∈ {1,2}; macro M(v) { x ≔ v; } procedure P() { return; } fair + process (N = 1) { L: - { with (a = 1) if (a = 1) { M(2); } else either skip; or print x; }; } }";
    let parsed = parse(source, &Options::default()).expect("full grammar");
    let mut ids = BTreeSet::new();
    for m in &parsed.algorithm.macros {
        block(&mut ids, &parsed, &m.body);
    }
    for p in &parsed.algorithm.procedures {
        block(&mut ids, &parsed, &p.body);
    }
    if let Body::Processes(ps) = &parsed.algorithm.body {
        for p in ps {
            block(&mut ids, &parsed, &p.body);
        }
    }
    assert_eq!(ids.len(), parsed.statements.len());
    for token in &parsed.tokens {
        assert_eq!(parsed.text(token.span), Some(token.text.as_str()));
    }
    let source = "é--algorithm A begin skip; end algorithm";
    for region in [
        1..source.len(),
        std::ops::Range { start: 2, end: 1 },
        usize::MAX..usize::MAX,
        0..usize::MAX,
    ] {
        let diagnostic =
            dive_pluscal::parse_at(source, region, &Options::default()).expect_err("bad selection");
        assert!(
            source
                .get(diagnostic.span.start..diagnostic.span.end)
                .is_some()
        );
    }
}

#[test]
fn pinned_pluscal_string_scanner_rejects_raw_unicode_and_controls() {
    for content in ["λ", "é", "🦀", "\u{1}", "\u{1f}", "\u{7f}"] {
        let source = format!("--algorithm A begin L: print \"{content}\"; end algorithm");
        assert!(matches!(parse(&source,&Options::default()),Err(e) if e.code==Code::Lexical));
    }
    assert!(
        parse(
            "--algorithm A (* 🦀 *) begin L: print \"ascii\"; end algorithm",
            &Options::default()
        )
        .is_ok()
    );
}

#[test]
fn printable_pluscal_string_endpoints_and_known_escapes_are_accepted() {
    for content in [" ", "~", r"\t", r"\n", r"\f", r"\r", r"\\", r#"\""#] {
        let source = format!("--algorithm A begin L: print \"{content}\"; end algorithm");
        assert!(parse(&source, &Options::default()).is_ok(), "{source}");
    }
    for content in [r"\u03bb", r"\x7f"] {
        let source = format!("--algorithm A begin L: print \"{content}\"; end algorithm");
        assert!(matches!(parse(&source,&Options::default()),Err(e) if e.code==Code::Lexical));
    }
}
