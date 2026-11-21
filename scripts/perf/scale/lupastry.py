"""Prepare an executable LuPastry+ experiment from the authors' pinned archive."""

import argparse
from hashlib import sha256
from pathlib import Path
from zipfile import ZipFile


ARCHIVE_SHA256 = "1919350f4103ea98141c7b2b1d3f7cfe14f2c96c1ff40a59e43299f2d58a0e81"
COUNTED = (
    b"SharedDigitsAux(i,j) ==\r\n  [n \\in RTRows |-> Cardinality({k \\in 1..n :\r\n"
    b"    IthDigit(i, RTRowCount-k+1) = IthDigit(j, RTRowCount-k+1)})]\r\n\r\n"
)


def routing_table(source, definition):
    if definition == "published":
        return source
    if definition == "bounded":
        return source.replace(
            b"CHOOSE g (*\\in [RTRows -> 0..RTRowCount]*):",
            b"CHOOSE g \\in [RTRows -> 0..RTRowCount]:",
            1,
        )
    start = source.index(b"SharedDigitsAux(i,j) ==")
    end = source.index(b"SharedDigits(i,j) ==", start)
    return source[:start] + COUNTED + source[end:]


def singleton_leaves(source):
    original = b"[node: I, left: SUBSET I, right: SUBSET I]"
    bound = b"(IF L = 1 THEN ({{}} \\cup {{n} : n \\in I}) ELSE SUBSET I)"
    if source.count(original) != 1:
        raise ValueError("expected the released LeafSet record bound exactly once")
    return source.replace(original, b"[node: I, left: " + bound + b", right: " + bound + b"]")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="lupastryplus.zip from the authors")
    parser.add_argument("destination", type=Path, help="new directory for the model")
    parser.add_argument("--definition", choices=("published", "bounded", "counted"),
                        default="published")
    parser.add_argument("--bits", type=int, default=2, help="M with B=1 (ring size 2^M)")
    parser.add_argument("--ready", type=int, nargs="+", default=[0], help="initial ready node IDs")
    parser.add_argument("--next", choices=("Next", "NextMC"), default="NextMC")
    parser.add_argument("--singleton-leaves", action="store_true",
                        help="enumerate at most one neighbor directly when L=1")
    args = parser.parse_args()
    if args.bits <= 0 or any(i < 0 or i.bit_length() > args.bits for i in args.ready):
        parser.error("M must be positive and every ready ID must fit in M bits")
    if sha256(args.archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        parser.error("archive digest differs from the released LuPastry+ archive")
    if args.destination.exists():
        parser.error("destination already exists")
    with ZipFile(args.archive) as archive:
        args.destination.mkdir()
        for name in archive.namelist():
            if name.endswith(".tla"):
                source = archive.read(name)
                if name == "RoutingTable.tla":
                    source = routing_table(source, args.definition)
                if name == "LeafSet.tla" and args.singleton_leaves:
                    source = singleton_leaves(source)
                (args.destination / name).write_bytes(source)
    ready = ", ".join(map(str, sorted(set(args.ready))))
    (args.destination / "Model.cfg").write_text(
        f"INIT Init\nNEXT {args.next}\nCONSTANTS\n A = {{{ready}}}\n NullNode = NullNode\n"
        f" B = 1\n M = {args.bits}\n L = 1\nINVARIANTS TypeInvariant CorrectDelivery\n"
        "CHECK_DEADLOCK FALSE\n"
    )


if __name__ == "__main__":
    main()
