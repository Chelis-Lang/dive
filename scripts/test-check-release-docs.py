#!/usr/bin/env python3
"""Each failure class of check-release-docs.py, on small temporary trees."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True

SPEC = importlib.util.spec_from_file_location(
    "check_release_docs", Path(__file__).with_name("check-release-docs.py")
)
CHECK = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECK  # dataclasses look the module up
SPEC.loader.exec_module(CHECK)

DIVE_SHA = "a" * 40
SAIL_SHA = "b" * 40

CLEAN_DIVE = {
    "CHANGELOG.md": "# Changelog\n\n## Unreleased\n\n## 0.2.0\n\nFaster checks.\n\n"
    "## 0.1.0\n\nRun `dive check --strategy symbolic` (history, TODO).\n",
    "README.md": "# dive\n\n```sh\ncargo install dive-cli --locked\n```\n\nSince 0.1.0, dive is faster.\n",
    "crates/dive-api/README.md": "# dive-api\n\n[docs](https://docs.rs/dive-api/0.2.0/)\n",
    "docs/user/install.md": "# Install\n\nbase=https://github.com/Chelis-Lang/dive/releases/download/v0.2.0\n"
    "name=dive-v0.2.0-x86_64-unknown-linux-gnu\n",
    "docs/user/rust-api.md": '# Rust API\n\n```toml\n[dependencies]\ndive-api = "0.2"\n```\n',
    "skills/dive/SKILL.md": "# dive\n\nInstall with `cargo install dive-cli --locked`.\n",
    "docs/developer/release-performance-results.md": "# Performance\n\nHow fast dive 0.2.0 is.\n\n"
    "Compared with dive 0.1.0 it is faster.\n",
    "docs/developer/symbolic-preview.md": "# Preview\n\nRun `dive check --strategy symbolic`.\n",
    "docs/tlc-differences.md": "# Differences\n\nSymbolic proofs (`--strategy symbolic`) are not part of 0.2.0.\n",
    "book/src/index.md": "{{#include ../../docs/user/README.md}}\n",
    "docs/user/README.md": "# Overview\n\nCheck a model with `dive check`.\n",
}

CLEAN_SAIL = {
    "CHANGELOG.md": "# Changelog\n\n## 0.3.1\n\nSail 0.3.1 links D.I.V.E. 0.2.0.\n",
    "README.md": "# Sail\n\n```sh\ncargo +1.99.1 install sail-solve --version 0.3.1 --locked\n```\n",
    "docs/install.md": "# Install\n\nStable `sail-solve 0.3.1` is on crates.io.\n\nVERSION=0.3.1\n",
    "crates/sail-api/README.md": "# sail-api\n\n[API](https://docs.rs/sail-api/0.3.1/)\n",
    "crates/sail-kani/README.md": "# sail-kani\n\nNo version here.\n",
    "action/action.yml": "name: Sail\ninputs:\n  sail-version:\n    description: >-\n"
    "      Sail release.\n    required: false\n    default: \"0.3.1\"\n  archive:\n"
    "    default: \"\"\noutputs:\n  sail-version:\n    value: x\n",
    "docs/design/symbolic-check.md": "# Symbolic\n\n`sail runtime symbolic-check` preview.\n",
}


def write_tree(root, files):
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)


class ReleaseDocsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.dive = base / "dive"
        self.sail = base / "sail"
        self.mesite = base / "mesite"
        self.website = base / "website"
        write_tree(self.dive, CLEAN_DIVE)
        write_tree(self.sail, CLEAN_SAIL)
        pins = {"dive": {"revision": DIVE_SHA}, "sail": {"revision": SAIL_SHA}}
        write_tree(self.mesite, {"src/data/docs.json": json.dumps(pins)})
        write_tree(self.website, {CHECK.WEBSITE_PIN: json.dumps({"revision": SAIL_SHA})})

    def tearDown(self):
        self.tmp.cleanup()

    def reset(self):
        self.tearDown()
        self.setUp()

    def run_check(self, *extra, full=True):
        argv = ["--root", str(self.dive), "--version", "0.2.0", "--previous", "0.1.0", *extra]
        if full:
            argv += [
                "--tag-commit", DIVE_SHA, "--mesite", str(self.mesite),
                "--sail", str(self.sail), "--sail-version", "0.3.1",
                "--sail-tag-commit", SAIL_SHA, "--website", str(self.website),
            ]  # fmt: skip
        return CHECK.run(CHECK.parse(argv))

    def kinds(self, findings):
        return sorted({(f.path, f.kind) for f in findings})

    def edit(self, root, name, old, new):
        path = root / name
        text = path.read_text()
        self.assertIn(old, text)
        path.write_text(text.replace(old, new))

    def test_clean_tree_passes(self):
        self.assertEqual(self.run_check(), [])
        self.assertEqual(self.run_check(full=False), [])
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            argv = ["--root", str(self.dive), "--version", "0.2.0", "--previous", "0.1.0"]
            self.assertEqual(CHECK.main(argv), 0)

    # (a)
    def test_missing_changelog_section(self):
        self.edit(self.dive, "CHANGELOG.md", "## 0.2.0", "## 0.2.1")
        self.assertEqual(self.kinds(self.run_check()), [("CHANGELOG.md", "changelog")])

    def test_changelog_placeholders(self):
        for text in ("(re-measure on RC)", "numbers after RC", "TBD", "TODO: curate", "re-measured later"):
            with self.subTest(text=text):
                self.reset()
                self.edit(self.dive, "CHANGELOG.md", "Faster checks.", f"Faster checks {text}.")
                self.assertEqual(
                    self.kinds(self.run_check()), [("CHANGELOG.md", "changelog placeholder")]
                )

    def test_sail_changelog_section(self):
        self.edit(self.sail, "CHANGELOG.md", "## 0.3.1", "## Unreleased")
        self.assertEqual(self.kinds(self.run_check()), [("sail/CHANGELOG.md", "changelog")])

    # (b)
    def test_stale_versions(self):
        cases = [
            ("README.md", "cargo install dive-cli --locked", "cargo install dive-cli --version 0.1.0 --locked"),
            ("docs/user/install.md", "download/v0.2.0", "download/v0.1.0"),
            ("docs/user/install.md", "dive-v0.2.0-x86", "dive-v0.1.0-x86"),
            ("docs/user/rust-api.md", 'dive-api = "0.2"', 'dive-api = "0.1"'),
            ("docs/user/rust-api.md", 'dive-api = "0.2"', 'dive-api = { version = "=0.1.0" }'),
            ("crates/dive-api/README.md", "dive-api/0.2.0/", "dive-api/0.1.0/"),
            ("skills/dive/SKILL.md", "`cargo install dive-cli --locked`", "`dive 0.1.0 (abc)`"),
            ("docs/developer/release-performance-results.md", "How fast dive 0.2.0", "How fast dive 0.1.0"),
        ]
        for name, old, new in cases:
            with self.subTest(file=name, new=new):
                self.reset()
                self.edit(self.dive, name, old, new)
                self.assertEqual(self.kinds(self.run_check()), [(name, "stale version")])

    def test_prose_history_is_not_stale(self):
        # CLEAN_DIVE already says "Since 0.1.0" in the README and names
        # dive 0.1.0 outside the results page's lead paragraph.
        self.assertEqual(self.run_check(full=False), [])

    # (c)
    def test_unshipped_features(self):
        cases = [
            ("README.md", "Since 0.1.0", "Use `dive check --strategy occupancy`. Since 0.1.0"),
            ("docs/user/README.md", "`dive check`", "`dive check --strategy=compiled`"),
            ("CHANGELOG.md", "Faster checks.", "New `--strategy finite-induction`."),
            ("skills/dive/SKILL.md", "Install", "Put the worker in libexec/dive-symbolic. Install"),
        ]
        for name, old, new in cases:
            with self.subTest(file=name):
                self.reset()
                self.edit(self.dive, name, old, new)
                self.assertEqual(self.kinds(self.run_check()), [(name, "unshipped feature")])
        self.reset()
        self.edit(self.sail, "README.md", "# Sail", "# Sail\n\nRun `sail runtime symbolic-check`.")
        self.assertEqual(self.kinds(self.run_check()), [("sail/README.md", "unshipped feature")])

    def test_preview_pages_and_notes_may_name_unshipped_features(self):
        # symbolic-preview.md and symbolic-check.md are allowlisted, the
        # tlc-differences line says the feature is not part of the release,
        # and the 0.1.0 CHANGELOG section is history.
        self.assertEqual(self.run_check(), [])

    # (d)
    def test_mesite_dive_pin(self):
        self.edit(self.mesite, "src/data/docs.json", DIVE_SHA, "c" * 40)
        self.assertEqual(self.kinds(self.run_check()), [("mesite/src/data/docs.json", "pin")])

    def test_mesite_sail_pin(self):
        self.edit(self.mesite, "src/data/docs.json", SAIL_SHA, "c" * 40)
        self.assertEqual(self.kinds(self.run_check()), [("mesite/src/data/docs.json", "pin")])

    def test_abbreviated_tag_commit_matches(self):
        self.assertEqual(self.run_check("--tag-commit", DIVE_SHA[:12]), [])

    def test_website_sail_pin(self):
        self.edit(self.website, CHECK.WEBSITE_PIN, SAIL_SHA, "c" * 40)
        self.assertEqual(self.kinds(self.run_check()), [("website/" + CHECK.WEBSITE_PIN, "pin")])

    def test_sail_versions(self):
        cases = [
            ("README.md", "--version 0.3.1", "--version 0.3.0"),
            ("docs/install.md", "`sail-solve 0.3.1`", "`sail-solve 0.3.0`"),
            ("docs/install.md", "VERSION=0.3.1", "VERSION=0.3.0"),
            ("crates/sail-api/README.md", "sail-api/0.3.1/", "sail-api/0.3.0/"),
            ("action/action.yml", 'default: "0.3.1"', 'default: "0.3.0"'),
        ]
        for name, old, new in cases:
            with self.subTest(file=name, new=new):
                self.reset()
                self.edit(self.sail, name, old, new)
                self.assertEqual(self.kinds(self.run_check()), [("sail/" + name, "sail version")])

    def test_sail_readme_must_name_the_release(self):
        self.edit(self.sail, "README.md", "cargo +1.99.1 install sail-solve --version 0.3.1 --locked", "cargo build")
        self.assertEqual(self.kinds(self.run_check()), [("sail/README.md", "sail version")])

    def test_usage_errors(self):
        bad = [
            ["--version", "0.2", "--previous", "0.1.0"],
            ["--version", "0.2.0", "--previous", "0.2.0"],
            ["--version", "0.2.0", "--previous", "0.1.0", "--mesite", "x"],
            ["--version", "0.2.0", "--previous", "0.1.0", "--sail", "x"],
            ["--version", "0.2.0", "--previous", "0.1.0", "--website", "x"],
            ["--version", "0.2.0", "--previous", "0.1.0", "--tag-commit", "v0.2.0"],
        ]
        for argv in bad:
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as caught:
                    CHECK.parse(argv)
                self.assertEqual(caught.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
