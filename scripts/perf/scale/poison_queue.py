"""Prepare original and payload-projected configurations of the pinned poison queue."""

from argparse import ArgumentParser
from hashlib import sha256
from pathlib import Path


MODULE = "BlockingQueuePoisonApple"
SOURCE_HASH = "1695bd49adcc3414ed7f1c5feb23eed29c36d8cca1ee106fb75d7d59c7ccea09"
CONFIG_HASH = "af6cf7f7a62e43194fa83ec60056a72a44ee112d121abdbdeb34aa0e0450d22f"


def pinned(path, digest):
    data = path.read_bytes()
    if sha256(data).hexdigest() != digest:
        raise ValueError(f"{path.name} differs from the pinned holdout")
    return data.decode("utf-8")


def main():
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("source_dir", type=Path)
    parser.add_argument("destination", type=Path)
    for name in ("producers", "consumers", "capacity"):
        parser.add_argument(f"--{name}", type=int, default=5)
    parser.add_argument("--project-payload", action="store_true",
                        help="write a single producer identity into ordinary queue entries")
    args = parser.parse_args()
    if min(args.producers, args.consumers, args.capacity) < 1:
        parser.error("producers, consumers and capacity must be positive")
    module = pinned(args.source_dir / f"{MODULE}.tla", SOURCE_HASH)
    config = pinned(args.source_dir / f"{MODULE}.cfg", CONFIG_HASH)
    if args.project_payload:
        # Next, Spec's WF and SSpec's SF keep p as the acting producer.
        if module.count("Put(p, p)") != 3:
            parser.error("unexpected call sites in the pinned source")
        module = module.replace("Put(p, p)", "Put(p, (CHOOSE ordinary \\in Producers : TRUE))")
    for prefix, count in (("p", args.producers), ("c", args.consumers)):
        original = ",".join(f"{prefix}{index}" for index in range(1, 6))
        members = ",".join(f"{prefix}{index}" for index in range(1, count + 1))
        config = config.replace("{" + original + "}", "{" + members + "}")
    config = config.replace("BufCapacity = 5", f"BufCapacity = {args.capacity}")
    args.destination.mkdir()
    (args.destination / f"{MODULE}.tla").write_bytes(module.encode())
    (args.destination / f"{MODULE}.cfg").write_bytes(config.encode())


if __name__ == "__main__":
    main()
