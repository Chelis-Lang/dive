"""Authenticate previously discharged closure images under an explicitly supplied local key."""

from functools import cache
from hashlib import file_digest, sha256
import hmac
import json
import os
from pathlib import Path
import platform
import stat
import sys

from dd import cudd


def pack(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True).encode("ascii")


def local_key():
    path = os.environ.get("DIVE_SYMBOLIC_PROOF_KEY")
    if not path:
        return None
    # Ignore a missing or unusable key. The ordinary proof remains available.
    try:
        flags = os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW
        with os.fdopen(os.open(path, flags), "rb") as stream:
            meta = os.fstat(stream.fileno())
            if (not stat.S_ISREG(meta.st_mode) or meta.st_uid != os.getuid()
                    or stat.S_IMODE(meta.st_mode) not in (0o400, 0o600)):
                return None
            secret = stream.read(33)
            return secret if len(secret) == 32 else None
    except OSError:
        return None


@cache
def implementation(exporter):
    # Includes the actual evaluator, binder and native BDD extension. A worker
    # installation is trusted and immutable while a check runs.
    files = [Path(__file__).with_name(name + ".py") for name in (
        "obligation_receipt", "property_proof", "induction", "engine", "finite_relation",
        "finite_values", "worker_process", "check",
    )]
    files.extend(map(Path, (exporter, cudd.__file__, sys.executable)))
    digests = []
    for path in files:
        with path.open("rb") as stream:
            digests.append(file_digest(stream, "sha256").hexdigest())
    return [sys.version, platform.machine(), digests]


def for_candidate(model, candidate, args):
    """An unavailable or overly complex optional receipt falls back to ordinary induction."""
    secret = local_key()
    if secret is None:
        return None
    try:
        return ObligationReceipt(model, candidate, args, secret)
    except (OSError, RecursionError, TypeError, OverflowError):
        return None


class ObligationReceipt:
    """One namespace per predicate, input encoding, admission and checking implementation."""

    def __init__(self, model, candidate, args, secret):
        self.secret = secret
        self.actions = {}
        # Opaque native addresses only serve as *local* memo-table keys. Every
        # persistent identity is derived from labeled Boolean DAG contents.
        @cache
        def identity(node):
            if node.negated:
                return sha256(b"N" + identity(~node)).digest()
            if node == model.b.true:
                return sha256(b"T").digest()
            return sha256(b"B" + node.var.encode("ascii") + identity(node.low) + identity(node.high)).digest()

        layout = [(path, [bit.var for bit in fields], choices) for path, (fields, choices) in model.layout.items()]
        bindings = [model.doc["constants"], model.doc["overrides"], sorted(model.counters)]
        self.context = sha256(pack([
            "dive-closure-obligations-v1", implementation(args.exporter), model.names, layout, bindings,
            args.property_proof, identity(candidate).hex(), identity(model.constraint).hex(),
            identity(model.initial).hex(), identity(model.invalid_sources).hex(),
            [(name, identity(node).hex()) for name, node in model.properties],
        ])).hexdigest()
        for guard, update in model.effects:
            self.actions[guard, update] = sha256(pack([identity(guard).hex(), sorted(
                (name, identity(expr).hex()) for name, expr in update
            )])).hexdigest()
        identity.cache_clear()

    def encode(self, checked):
        rows = sorted({self.actions[action] for action in checked})
        tag = hmac.digest(self.secret, pack([self.context, rows]), "sha256").hex()
        return {"context": self.context, "actions": rows, "mac": tag}

    def checked(self, receipt):
        if not isinstance(receipt, dict) or receipt.get("context") != self.context:
            return set()
        rows = receipt.get("actions")
        tag = receipt.get("mac")
        if (not isinstance(rows, list) or len(rows) > 100_000 or not isinstance(tag, str)
                or len(tag) != 64 or not tag.isascii()
                or any(not isinstance(row, str) or len(row) != 64 or not row.isascii() for row in rows)):
            return set()
        if not hmac.compare_digest(hmac.digest(self.secret, pack([self.context, rows]), "sha256").hex(), tag):
            return set()
        accepted = set(rows)
        return {action for action, digest in self.actions.items() if digest in accepted}
