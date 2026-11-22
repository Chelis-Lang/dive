#!/usr/bin/env python3
"""Build an optional source-driven native successor worker in a private Linux cache."""
import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from emitter import HERE, lower

REPO = HERE.parents[3]
GEN = Path("crates/dive-core/src/generate/native_study.rs")
STUB = """use super::{Code, Error, Machine, Sink};
pub(crate) fn matches(_: &Code) -> bool { false }
pub(super) fn run(m:&mut Machine,c:&Code,g:u32,s:&mut dyn Sink)->Result<(),Error>{m.run_action(c,g,s)}
"""
EXPORT_MARKER = "native successor study exported the plan before exploration"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def key(data):
    return digest(json.dumps(data, sort_keys=True, separators=(",", ":")).encode())


def record(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n")


def output(cmd, cwd, **kwargs):
    return subprocess.check_output(list(map(str, cmd)), cwd=cwd, **kwargs).decode().strip()


def replace_once(path, before, after):
    text = path.read_text()
    if text.count(before) != 1:
        raise ValueError(f"the committed Core hook changed in {path.name}")
    path.write_text(text.replace(before, after))


def extract(tree, destination):
    with subprocess.Popen(["git", "archive", tree], cwd=REPO, stdout=subprocess.PIPE) as source:
        subprocess.run(["tar", "-xf", "-", "-C", destination], stdin=source.stdout, check=True)
        if source.wait():
            raise ValueError("could not extract the committed Dive tree")
    core = destination / "crates/dive-core/src"
    replace_once(core / "plan.rs", "pub struct Code {", "pub struct Code {\n    /// Shape-bound native successor study worker.\n    pub native_worker: bool,")
    replace_once(core / "compile.rs", "    let mut plan = Plan {", (HERE / "export.rs").read_text() + "\n    let mut plan = Plan {")
    replace_once(core / "generate.rs", "mod composition;", "mod composition;\npub(crate) mod native_study;")
    replace_once(core / "generate.rs", "        let result = self.run_action(code, action.g, sink);", """
        let result = if code.native_worker && self.debugger.is_none() && self.fuel == u64::MAX
            && self.unbounded.is_none() && !self.heap.speculative && sink.exhaustive() && !sink.constructive() {
            self.clear_buffer();
            native_study::run(self,code,action.g,sink)
        } else { self.run_action(code, action.g, sink) };""")
    (destination / GEN).write_text(STUB)


@contextmanager
def locked(path):
    with path.open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def compiler_environment():
    names = {"HOME", "PATH", "RUSTUP_HOME", "CARGO_HOME", "TMPDIR", "SSL_CERT_FILE", "SSL_CERT_DIR"}
    return {name: value for name, value in os.environ.items() if name in names}


def build(source, target, to, jobs):
    env = compiler_environment()
    env.update(CARGO_TARGET_DIR=str(target), CARGO_PROFILE_RELEASE_LTO="off", CARGO_PROFILE_RELEASE_CODEGEN_UNITS="16")
    subprocess.run(["cargo", "build", "--release", "--locked", "-j", str(jobs), "-p", "dive-cli", "--bin", "dive"], cwd=source, env=env, check=True)
    shutil.copyfile(target / "release/dive", to)
    to.chmod(0o700)
    subprocess.run(["strip", to], check=True)


def cached_binary(folder):
    executable = folder / "dive"
    if not executable.is_file() or not (folder / "build.json").is_file():
        raise ValueError("incomplete cached build")
    manifest = json.loads((folder / "build.json").read_text())
    expected = manifest.get("binary_sha256") if isinstance(manifest, dict) else None
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError("invalid cached executable manifest")
    if file_digest(executable) != expected:
        raise ValueError("cached executable checksum changed")
    return executable


def cli(binary, args, cwd, env):
    run = subprocess.run([binary, *args, "--output", "json"], capture_output=True, cwd=cwd, env=env, timeout=90)
    report = json.loads(run.stdout)
    if not isinstance(report, dict):
        raise ValueError("checker returned an invalid response")
    return run.returncode, report


def require_export(status, report):
    if status != 3 or report.get("verdict") != "unsupported" or report.get("message") != EXPORT_MARKER:
        raise ValueError(f"plan unavailable: exit {status}, {report.get('verdict')}, {report.get('message') or report.get('refusal')}")


def snapshot(bootstrap, module, config, folder, env):
    status, analysis = cli(bootstrap, ["analyze", str(module)], module.parent, env)
    if status or not analysis.get("accepted") or analysis.get("module") != module.stem:
        raise ValueError("input did not pass analysis")
    files = {}
    for entry in analysis.get("files", []):
        if entry["origin"] == "standard":
            continue
        path = (module.parent / entry["path"]).resolve()
        if entry["origin"] != "file" or path.parent != module.parent or path.name in files:
            raise ValueError("only colocated source modules are supported")
        data = path.read_bytes()
        (folder / path.name).write_bytes(data)
        files[path.name] = digest(data)
    if module.name not in files:
        raise ValueError("resolved closure did not contain the root")
    cfg = config.read_bytes()
    (folder / "native-study.cfg").write_bytes(cfg)
    status, check = cli(bootstrap, ["analyze", module.name], folder, env)
    closure = {Path(e["path"]).name for e in check.get("files", []) if e["origin"] == "file"}
    if status or not check.get("accepted") or closure != set(files):
        raise ValueError("resolved source closure changed while copying")
    return {"root": module.name, "modules": files, "config_sha256": digest(cfg)}


def prepare(args):
    started = time.monotonic()
    module, config, cache = args.module.resolve(), args.config.resolve(), args.cache.resolve()
    if cache.is_relative_to(REPO) or REPO.is_relative_to(cache):
        raise ValueError("use a dedicated cache outside the source checkout")
    if args.jobs < 1:
        raise ValueError("--jobs must be positive")
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    tree = output(["git", "rev-parse", "HEAD^{tree}"], REPO)
    build_env = compiler_environment()
    compiler = output(["rustc", "-Vv"], REPO, env=build_env)
    tool_inputs = {path.name: file_digest(path) for path in sorted(HERE.iterdir()) if path.suffix in {".rs", ".py"}}
    identity = {"tree": tree, "rustc": compiler, "cargo": output(["cargo", "-V"], REPO, env=build_env), "study": tool_inputs}
    epoch = cache / key(identity)
    epoch.mkdir(exist_ok=True)
    with locked(epoch / "lock"):
        src, target = epoch / "source", epoch / "target"
        if not src.is_dir():
            with tempfile.TemporaryDirectory(dir=epoch) as tmp:
                checkout = Path(tmp) / "source"
                checkout.mkdir()
                extract(tree, checkout)
                checkout.rename(src)
        bootstrap_dir = epoch / "bootstrap"
        bootstrap_cold = not bootstrap_dir.exists()
        if bootstrap_cold:
            (src / GEN).write_text(STUB)
            with tempfile.TemporaryDirectory(dir=epoch) as tmp:
                path = Path(tmp) / "build"
                path.mkdir()
                build(src, target, path / "dive", args.jobs)
                record(path / "build.json", {"binary_sha256": file_digest(path / "dive")})
                path.rename(bootstrap_dir)
        bootstrap = cached_binary(bootstrap_dir)
        with tempfile.TemporaryDirectory(dir=epoch) as tmp:
            folder = Path(tmp)
            env = {"HOME": str(folder), "PATH": os.environ["PATH"], "LC_ALL": "C"}
            inputs = snapshot(bootstrap, module, config, folder, env)
            planfile = folder / "generator.plan"
            env["DIVE_EXPORT_SUCCESSORS"] = str(planfile)
            flags = ["--compat=tlc"] if args.profile == "tlc" else []
            status, report = cli(bootstrap, ["check", module.name, "--config", "native-study.cfg", *flags, "--workers", "1", "--seed", "0", "--fp", "0", "--max-states", "1", "--time-limit", "15s"], folder, env)
            require_export(status, report)
            rust, shape, counts = lower(planfile.read_text())
        kernel_key = key({"rust": digest(rust.encode()), "shape": digest(shape.encode()), "compiler": identity})
        installed = epoch / kernel_key
        new = not installed.exists()
        compile_seconds = 0
        if new:
            (src / GEN).write_text(rust)
            (src / GEN).with_name("native-study.shape").write_text(shape)
            with tempfile.TemporaryDirectory(dir=epoch) as tmp:
                path = Path(tmp) / "build"
                path.mkdir()
                t = time.monotonic()
                build(src, target, path / "dive", args.jobs)
                compile_seconds = time.monotonic() - t
                record(path / "build.json", {"binary_sha256": file_digest(path / "dive"), "control_shape_sha256": digest(shape.encode()), "code_sha256": digest(rust.encode()), "compiler": identity, "lowered": counts})
                path.rename(installed)
        binary = cached_binary(installed)
        return {"status": "prepared", "inputs": inputs, "profile": args.profile, "build_tree": tree, "kernel": kernel_key, "built": new, "bootstrap_built": bootstrap_cold, "compile_s": round(compile_seconds, 6), "prepare_s": round(time.monotonic() - started, 6), "binary": str(binary), "binary_sha256": file_digest(binary), "lowered": counts}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--cache", required=True, type=Path)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--profile", choices=("dive", "tlc"), default="dive")
    try:
        print(json.dumps(prepare(parser.parse_args()), indent=2))
    except (OSError, ValueError, RuntimeError, IndexError, AttributeError, subprocess.SubprocessError, SyntaxError) as error:
        print(json.dumps({"status": "refused", "reason": str(error)}))
        sys.exit(2)
