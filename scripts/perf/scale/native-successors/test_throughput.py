"""Counter provenance and training on the immutable property-bearing source."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import prepare


class ThroughputTests(unittest.TestCase):
    def test_only_the_model_process_contributes_to_the_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            tool = root / "llvm-profdata"
            tool.write_bytes(b"tool")
            binary = root / "worker"
            builds = []
            inputs = {"root": "Original.tla", "config_sha256": "actual cfg"}

            def build(_src, _target, dest, _jobs, settings):
                builds.append(settings)
                dest.write_bytes(b"instrumented" if len(builds) == 1 else b"optimized")
                if len(builds) == 1:
                    # A build script can create its own counters.
                    Path(settings["LLVM_PROFILE_FILE"].replace("%m-%p", "build")).write_bytes(b"unrelated")

            def sample(exe, args, cwd, env):
                self.assertEqual(exe.read_bytes(), b"instrumented")
                self.assertEqual(cwd, root)
                self.assertEqual(args[:4], ["check", "Original.tla", "--config", "native-study.cfg"])
                for flag in ["--compat=tlc", "--max-states", "--max-depth", "--time-limit", "--max-memory"]:
                    self.assertIn(flag, args)
                self.assertNotIn("DIVE_DISABLE_NATIVE_SUCCESSORS", env)
                Path(env["LLVM_PROFILE_FILE"].replace("%m-%p", "worker")).write_bytes(b"sample")
                return 5, {"verdict": "stopped", "statistics": {"generated": 28}}

            def merge(cmd, *, check):
                self.assertTrue(check)
                self.assertEqual(cmd[:3], [tool, "merge", "-o"])
                self.assertEqual([p.read_bytes() for p in cmd[4:]], [b"sample"])
                cmd[3].write_bytes(b"merged model sample")

            with patch.dict(os.environ, DIVE_DISABLE_NATIVE_SUCCESSORS="1"), \
                 patch.object(prepare, "build", build), patch.object(prepare, "cli", sample), \
                 patch.object(prepare.subprocess, "run", merge):
                receipt = prepare.throughput_build(root, root, root, binary, 2, ["--compat=tlc"], inputs, tool, 2)
            self.assertEqual(binary.read_bytes(), b"optimized")
            self.assertEqual(receipt["training_inputs"], inputs)
            self.assertEqual(receipt["training_verdict"], "stopped")
            self.assertEqual(receipt["profile_sha256"], prepare.digest(b"merged model sample"))
            self.assertIn(receipt["profile_sha256"], builds[1]["CARGO_ENCODED_RUSTFLAGS"])
            self.assertEqual(set(root.iterdir()), {binary, tool})

    def test_missing_model_sample_cannot_install_a_worker(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            binary = root / "worker"
            with patch.object(prepare, "build") as build, \
                 patch.object(prepare, "cli", return_value=(3, {"verdict": "unsupported"})), \
                 self.assertRaisesRegex(ValueError, "no native checker training sample"):
                prepare.throughput_build(root, root, root, binary, 2, [], {"root": "M.tla"}, root / "llvm-profdata", 2)
            self.assertEqual(build.call_count, 1)
            self.assertFalse(binary.exists())
