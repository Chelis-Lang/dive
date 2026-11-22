"""Establish and authenticate source-bound exact carrier permutation proofs."""
import hashlib
import hmac
import json
import os
from pathlib import Path
import platform
import resource
import shutil
import stat
import subprocess
import sys
import tempfile
import time


def pack(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def authority():
    key = os.environ.get("DIVE_ORBIT_PROOF_KEY")
    if not key:
        return None
    path = Path(key).expanduser()
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as f:
        mode = os.fstat(f.fileno())
        if not stat.S_ISREG(mode.st_mode) or mode.st_uid != os.getuid() or stat.S_IMODE(mode.st_mode) not in (0o400, 0o600):
            raise ValueError("orbit proof key must be a private owner-controlled regular file")
        secret = f.read(33)
        if len(secret) != 32:
            raise ValueError("orbit proof key must contain exactly 32 bytes")
        return secret


def run_checked(command, folder, seconds, memory_mib, source=None):
    def limit():
        cap = memory_mib << 20
        _, hard = resource.getrlimit(resource.RLIMIT_AS)
        if hard != resource.RLIM_INFINITY:
            cap = min(cap, hard)
        resource.setrlimit(resource.RLIMIT_AS, (cap, cap))
    completed = subprocess.run(command, cwd=folder, input=source, capture_output=True, timeout=seconds,
                               env={"HOME": str(folder), "PATH": os.environ.get("PATH", ""), "LC_ALL": "C"},
                               preexec_fn=limit)
    if completed.returncode != 0:
        error = completed.stderr.decode(errors="replace").strip()[-700:]
        raise ValueError(f"certificate helper exit {completed.returncode}: {error}")
    return completed.stdout


class OriginalCounterexample(ValueError):
    """The fresh ordinary run found a failure before any graph proof is issued."""

    def __init__(self, result):
        super().__init__("original input has a counterexample")
        self.result = result


def counts(row):
    return tuple(row[name] for name in ("distinct", "generated", "depth"))


def census(reduced):
    r = reduced["census"]
    return (int(r["labeled_orbit_union"]), int(r["weighted_generated"]), reduced["statistics"]["depth"])


def establish(args, folder, original, identity, reduced, ordinary):
    started = time.monotonic()
    pins = identity["inputs"]
    finite = args.certify_equivariance == "finite"
    helper = folder / ".orbit-proof-helper"
    shutil.copyfile((args.exporter if finite else args.graph_auditor).resolve(), helper)
    helper.chmod(0o500)
    def run(command, data=None):
        return run_checked(command, folder, args.seconds, args.proof_memory_mib, data)
    namespace = dict(version=1, method=args.certify_equivariance, inputs=pins, checker=identity["binary_sha256"],
                     driver=identity["driver_sha256"], helper=digest(helper), verifier=digest(Path(__file__)),
                     layout=args.type_invariant if finite else None, workers=args.workers, platform=platform.machine(),
                     profile="tlc-exact-seed0-fp0")
    if finite:
        proof = Path(__file__).resolve().parents[2] / "symbolic" / "orbit_proof.py"
        namespace["finite_implementation"] = json.loads(run([sys.executable, proof, "--identity"]))
    context = hashlib.sha256(pack(namespace)).hexdigest()
    cache = args.certificate_cache
    secret = authority() if cache else None
    if cache and secret is None:
        raise ValueError("a shared certificate cache requires DIVE_ORBIT_PROOF_KEY")
    if cache:
        path = cache / (context + ".json")
        try:
            with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
                raw = stream.read(65537) if stat.S_ISREG(os.fstat(stream.fileno()).st_mode) else b""
            if len(raw) <= 65536:
                envelope = json.loads(raw)
                prior, mac = envelope["proof"], envelope["mac"]
                if (prior["context"] == context and isinstance(mac, str)
                        and hmac.compare_digest(mac, hmac.digest(secret, pack(prior), "sha256").hex())
                        and tuple(prior["ordinary_counts"]) == census(reduced)):
                    return dict(**prior, reuse="authenticated", seconds=round(time.monotonic() - started, 6))
        except (OSError, ValueError, KeyError, TypeError):
            pass  # A cache miss or damaged receipt always requires a new proof.

    # Both verifiers require a successful ordinary no-sink run on the complete
    # original input. This also covers evaluation-order diagnostics that a
    # mathematical relation can erase and Core's retained representations.
    full = ordinary()
    if full["verdict"] != "success":
        raise OriginalCounterexample(full)
    if counts(full["statistics"]) != census(reduced):
        raise ValueError("ordinary original safety disagrees with reduced counts or depth")
    if finite:
        doc = run([helper, "--orbit-proof", original, "input.cfg"])
        report = json.loads(run([sys.executable, proof, "--layout", args.type_invariant,
                                 "--memory-mib", str(args.proof_memory_mib), *pins["atoms"]], doc))
        if report.get("method") != "finite-role-conjugacy":
            raise ValueError("incomplete finite proof")
    else:
        report = json.loads(run([helper, original, "input.cfg", str(args.workers), *pins["atoms"]]))
        if report.get("method") != "reachable-graph" or counts(report) != counts(full["statistics"]):
            raise ValueError("ordinary original safety result disagrees with graph capture")
    result = dict(context=context, evidence=report, ordinary_counts=list(counts(full["statistics"])),
                  ordinary_seconds=full["seconds"])
    if cache:
        cache.mkdir(parents=True, exist_ok=True)
        signature = hmac.digest(secret, pack(result), "sha256").hex()
        with tempfile.NamedTemporaryFile(mode="wb", dir=cache, delete=False) as staged:
            staged.write(pack({"proof": result, "mac": signature}))
        try:
            os.replace(staged.name, path)
        finally:
            Path(staged.name).unlink(missing_ok=True)
    return dict(**result, reuse="fresh", seconds=round(time.monotonic() - started, 6))
