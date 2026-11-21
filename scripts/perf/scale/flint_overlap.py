"""Reproduce the released Flint compiled checker's successful GCas overlap counter."""

import argparse
import hashlib
from pathlib import Path
import re

from flint_counts import SOURCE


def once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"expected one occurrence of {old!r}")
    return text.replace(old, new)


def weighted_module(raw):
    if hashlib.sha256(raw).hexdigest() != SOURCE:
        raise ValueError("the released LeanP1.tla is required")
    text = once(raw.decode(), "MODULE LeanP1 ", "MODULE WeightedFlint ")
    text = once(text, "EXTENDS Naturals, FiniteSets", "EXTENDS Naturals, FiniteSets, TLC")
    if "PrintT" in text:
        raise ValueError("unexpected preexisting PrintT")
    start = text.index("GatewayStep ==")
    end = text.index("Spec == Init", start)
    selection, count = re.subn(r"\bGCas\(p\)", "CountGCas(p)", text[start:end])
    if count != 1:
        raise ValueError("expected one GCas call in the gateway selection")
    wrapper = ('CountGCas(p) == /\\ GCas(p)\n'
               '                /\\ IF GatewayIgnoresLease /\\ holder = "none" THEN PrintT(TRUE) ELSE TRUE\n\n')
    return text[:start] + wrapper + selection + text[end:]


def counted_print(raw):
    text = raw.decode()
    enum_start = text.index("pub enum Bi")
    start = text.rindex("\n\n", 0, enum_start)
    counter = ("\n\npub static G_CAS_DUPLICATES: std::sync::atomic::AtomicU64 = "
               "std::sync::atomic::AtomicU64::new(0);")
    text = text[:start] + counter + text[start:]
    return once(text, 'Bi::PrintT => {\n            println!("{}", v[0]);',
                'Bi::PrintT => {\n            G_CAS_DUPLICATES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);')


def counted_main(raw):
    text = raw.decode()
    start = text.index("fn main() -> std::process::ExitCode { tlc_rs::cli::main(")
    body = once(text[start:], "ExitCode { tlc_rs", "ExitCode { let exit = tlc_rs")
    body = once(body, 'make })) }', 'make })); eprintln!("GCAS_DUPLICATES {}", '
                'tlc_rs::eval::G_CAS_DUPLICATES.load(std::sync::atomic::Ordering::Relaxed)); exit }')
    return text[:start] + body


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    stages = {"module": weighted_module, "evaluator": counted_print, "main": counted_main}
    parser.add_argument("stage", choices=stages)
    parser.add_argument("source", type=Path)
    parser.add_argument("new_file", type=Path)
    args = parser.parse_args()
    output = stages[args.stage](args.source.read_bytes())
    with args.new_file.open("xb") as stream:
        stream.write(output.encode())
