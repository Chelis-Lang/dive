"""Build the exact column-code variant of the pinned Flint checker in a new directory."""

import argparse
import hashlib
from pathlib import Path
import shutil

from flint_overlap import counted_main, counted_print, once

FILES = Path(__file__).with_name("flint-exact")
PINS = {
    "cli.rs": "a605d40d4519e4b292e0eb9441dfecca3290d94ed5593293868035decb9f34b7",
    "check.rs": "7da374b8cf0c3f9fc2d241a1acabadc4610cc2666e0663428e98aeeb7dffad34",
    "eval.rs": "b64e50553795200cf3728bab2b451a640ae1b5cdc2095dc872b1fcd3b19d7d7b",
    "lib.rs": "b40d345c9e62f8ba37006d587ab80d13765a9c91d1fcb94b95461000ba543593",
    "store.rs": "9386771d674a7400f8363a0a08b5a43a8bdf0266d0db03a5d64784ab12ac9061",
    "value.rs": "4695819896313d0fd84609575a3d16efaea7b1a31c5cb13587961251dcd262cd",
}


def exact_search(text):
    # Reuse unchanged parent components. Both admission and later replay must
    # derive exactly the same state key. The only full-state hash is the audit.
    edits = (
        ("pub fn run(&self) -> Outcome {", """pub fn run(&self) -> Outcome {
        let refused = if self.disk.recover || self.disk.checkpoint_secs != 0 {
            Some("exact column keys are process-local; checkpoints and recovery are unsupported")
        } else if self.sk.is_some() {
            Some("exact column prototype refuses VIEW and SYMMETRY")
        } else {
            None
        };
        if let Some(reason) = refused {
            return Outcome { generated: 0, distinct: 0, depth: 0,
                failure: Some(Failure::Eval(reason.into(), None)) };
        }"""),
        ("hv.extend(st.iter().map(var_hash));",
         "for (i, v) in st.iter().enumerate() { hv.push(crate::state_codes::value(nvars, i, v)?); }"),
        ("let pfp = combine(hv);", "let pfp = crate::state_codes::row(hv)?;"),
        ("hs[i] = var_hash(v);", "hs[i] = crate::state_codes::value(nvars, i, v)?;"),
        ("let fp = (0..np).map(|k| combine(&hs[k * nvars..(k + 1) * nvars])).min().unwrap();",
         "let fp = crate::state_codes::row(&hs[..nvars])?;"),
        ("None => Ok(fingerprint(st)),", "None => crate::state_codes::state(st),"),
        ("Some(sk) => sk.key_alone(st, &mut Scratch::default()),",
         'Some(_) => Err("exact column prototype refuses VIEW and SYMMETRY".into()),'),
        ("if new {\n                    let idx = match log.append(meta, fp, NO_PARENT)",
         "if new {\n                    if let Err(e) = crate::state_codes::audit(&s, fp) {\n"
         "                        return done(generated, 0, Some(Failure::Eval(e, None)));\n"
         "                    }\n                    let idx = match log.append(meta, fp, NO_PARENT)"),
        ("let idx = match log.append(meta, fp, pidx)",
         "if let Err(e) = crate::state_codes::audit(&s, fp) {\n"
         "                                    return Some(Failure::Eval(e, None));\n"
         "                                }\n                                let idx = match log.append(meta, fp, pidx)"),
    )
    for before, after in edits:
        text = once(text, before, after)
    return text


def checker(source, output):
    for name, digest in PINS.items():
        if hashlib.sha256((source / "src" / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"expected the pinned Flint source and published level-writer patch: {name}")
    output.mkdir(parents=True, exist_ok=False)
    for name in ("Cargo.toml", "Cargo.lock"):
        shutil.copyfile(source / name, output / name)
    for name in ("src", "modules"):
        shutil.copytree(source / name, output / name)
    src = output / "src"
    cli = (src / "cli.rs").read_text()
    cli = once(cli, "(None, false, 30.0, 256)", "(None, false, 0.0, 256)")
    cli = once(cli, "    let t0 = Instant::now();", """    if recover || checkpoint_min != 0.0 {
        return Err("exact column keys are process-local; checkpoints and recovery are unsupported".into());
    }
    let t0 = Instant::now();""")
    (src / "cli.rs").write_text(cli)
    (src / "check.rs").write_text(exact_search((src / "check.rs").read_text()))
    (src / "eval.rs").write_text(counted_print((src / "eval.rs").read_bytes()))
    with (src / "lib.rs").open("a") as stream:
        stream.write("\npub mod state_codes;\n")
    shutil.copyfile(FILES / "state_codes.rs", src / "state_codes.rs")
    (output / "examples").mkdir()
    shutil.copyfile(FILES / "check_codes.rs", output / "examples/check_codes.rs")


def main_file(source, output):
    text = counted_main(source.read_bytes())
    text = once(text, 'eprintln!("GCAS_DUPLICATES',
                'tlc_rs::state_codes::describe(); tlc_rs::state_codes::describe_audit(); '
                'eprintln!("GCAS_DUPLICATES')
    with output.open("x") as stream:
        stream.write(text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=("checker", "main"))
    parser.add_argument("source", type=Path)
    parser.add_argument("new_output", type=Path)
    args = parser.parse_args()
    (checker if args.stage == "checker" else main_file)(args.source, args.new_output)
