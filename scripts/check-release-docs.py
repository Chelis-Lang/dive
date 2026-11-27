#!/usr/bin/env python3
"""Check that a release updated every docs surface.

Run as a step of docs/developer/releasing.md, not on every PR. It reads files
only (no network, no git) and fails when:

  (a) the CHANGELOG has no section for --version, or that section still holds
      placeholder text ("re-measure", "after RC", "TBD", "TODO");
  (b) --previous still appears in an install command, dependency line,
      download URL or similar version-bearing line of a current-state file
      (STALE_VERSION_TABLE below);
  (c) a user-facing file names a feature the release does not ship
      (UNSHIPPED_FEATURES), outside the preview pages in PREVIEW_ALLOWLIST;
  (d) with the optional paths: the mesite Dive book pin is not --tag-commit,
      the mesite or website Sail book pin is not --sail-tag-commit, or Sail's
      README, install guide, crate READMEs or GitHub Action default do not name
      --sail-version. Sail's CHANGELOG section and user-facing files get the
      (a) and (c) checks too, and Sail's current-state pages must not name
      --previous as the dive release Sail links (SAIL_DIVE_VERSION_TABLE).
      --sail needs no tag commit, so it can run before the Sail tag.

Usage:
  check-release-docs.py --version X.Y.Z --previous A.B.C [--tag-commit SHA]
      [--mesite PATH] [--sail PATH] [--sail-version X.Y.Z]
      [--sail-tag-commit SHA] [--website PATH] [--root PATH]

  --sail needs --sail-version. --website, and --mesite with --sail-version,
  need --sail-tag-commit.

Exit status: 0 clean, 1 findings, 2 usage error. Standard library only; run
with `python3 -I`.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# --- Configuration --------------------------------------------------------

# (b) Current-state files and the version-bearing patterns checked in them.
# In each pattern, {prev} is the previous version (A.B.C) and {prev_minor} its
# A.B prefix, both regex-escaped. Prose history ("0.1.0 covered 169") is not
# matched; only install commands, dependency lines, URLs and version output.
# A pattern whose name ends in "(lead)" is checked only in the page's lead
# paragraph (the first paragraph after the title).
VERSION_PATTERNS: dict[str, str] = {
    "install --version": r"\binstall\b.*--version[ =][=^~]?{prev}\b",
    "release download URL": r"releases/(?:download|tag)/v{prev}\b",
    "archive name": r"\bdive-v{prev}-",
    "dependency line": (
        r"^\s*dive-[\w-]+\s*=\s*(?:\{{[^}}]*\bversion\s*=\s*)?\"[=^~]?{prev_minor}(?:\.\d+)?\""
    ),
    "docs.rs version link": r"docs\.rs/[\w-]+/{prev}\b",
    "version output": r"\bdive {prev} \(",
    "git describe tag": r"\bv{prev}-\d+-g",
    "release named (lead)": r"\bdive {prev}\b",
}
ALL_VERSION_PATTERNS = tuple(k for k in VERSION_PATTERNS if not k.endswith("(lead)"))
STALE_VERSION_TABLE: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("README.md", ALL_VERSION_PATTERNS),
    ("crates/*/README.md", ALL_VERSION_PATTERNS),
    ("docs/user/install.md", ALL_VERSION_PATTERNS),
    ("docs/user/rust-api.md", ALL_VERSION_PATTERNS),
    ("skills/dive/SKILL.md", ALL_VERSION_PATTERNS),
    ("docs/developer/release-performance-results.md", ("release named (lead)",)),
)

# (a) Text that marks an unfinished release section.
PLACEHOLDERS: dict[str, re.Pattern[str]] = {
    "re-measure": re.compile(r"\bre-?measure", re.I),
    "after RC": re.compile(r"\bafter (?:the )?RC\b", re.I),
    "TBD": re.compile(r"\bTBD\b"),
    "TODO": re.compile(r"\bTODO\b"),
}

# (c) Features the release does not ship. dive 0.2.0 leaves out the symbolic
# worker and its strategies (they ship in 0.3); Sail 0.3.1 hides the command
# that drives them.
UNSHIPPED_FEATURES: dict[str, re.Pattern[str]] = {
    "symbolic strategy": re.compile(
        r"--strategy[ =](?:symbolic-path|symbolic|compiled|occupancy|finite-induction|auto)\b"
    ),
    "symbolic worker": re.compile(r"libexec/dive-symbolic\b"),
    "sail runtime symbolic-check": re.compile(r"\bsymbolic-check\b"),
    "source-obligations strategy": re.compile(r"\bsource-obligations\b"),
}
# A line that says the feature is left out may name it ("Symbolic proofs
# (`--strategy symbolic`) are not part of the installed 0.2.0").
NOT_SHIPPED_NOTE = re.compile(
    r"\b(?:not (?:part of|in|included in|listed in|shipped)|no longer listed|unlisted"
    r"|left out of|planned for)\b",
    re.I,
)

# User-facing files: these globs, the book pages under book/src with their
# {{#include}} targets, and the CHANGELOG section of the release (older
# sections are history and are not checked).
USER_FACING_GLOBS = (
    "README.md",
    "crates/*/README.md",
    "skills/*/SKILL.md",
    "docs/user/**/*.md",
    "docs/install.md",
    "docs/tlc-differences.md",
)
# Preview pages may name unshipped features.
PREVIEW_ALLOWLIST: frozenset[str] = frozenset(
    {
        "docs/developer/symbolic-preview.md",  # dive
        "docs/design/symbolic-check.md",  # Sail
    }
)

# (d) Sail files that must name the Sail release, and the version-bearing
# patterns in them. Each pattern captures the version it names.
SAIL_VERSION_FILES = ("README.md", "docs/install.md", "crates/*/README.md")
SAIL_REQUIRED_MENTION = ("README.md", "docs/install.md")
SAIL_VERSION_PATTERNS: dict[str, re.Pattern[str]] = {
    "install --version": re.compile(r"\binstall\s+sail-solve\b.*--version[ =][=^~]?(\d+\.\d+\.\d+)"),
    "docs.rs version link": re.compile(r"docs\.rs/sail-[\w-]+/(\d+\.\d+\.\d+)"),
    "VERSION=": re.compile(r"\bVERSION=(\d+\.\d+\.\d+)"),
    "stable sail-solve": re.compile(r"\bStable `sail-solve (\d+\.\d+\.\d+)`"),
    "install Sail": re.compile(r"\bInstall Sail (\d+\.\d+\.\d+)"),
    "release download URL": re.compile(r"sail/releases/(?:download|tag)/v(\d+\.\d+\.\d+)"),
}
# (d) Sail pages that state which dive release Sail links, and the patterns
# (VERSION_PATTERNS syntax) that catch --previous there. Release notes and
# older CHANGELOG sections are history and are not checked.
SAIL_DIVE_VERSION_PATTERNS: dict[str, str] = {
    "D.I.V.E. release": r"D\.I\.V\.E\.(?:\]\([^)\s]*\))?\s+{prev}\b",
    "dive crate version": r"\bdive-(?:api|ir|value|trace)\b.*\b{prev}\b",
}
SAIL_DIVE_VERSION_TABLE = ("README.md", "docs/install.md", "docs/temporal-jobs.md", "docs/spec/temporal-jobs.md")
SAIL_ACTION = "action/action.yml"
MESITE_PINS = "src/data/docs.json"
WEBSITE_PIN = "apps/cproof-site/src/data/sail-book.json"

# --- Checks ---------------------------------------------------------------

VERSION = re.compile(r"^\d+\.\d+\.\d+$")
COMMIT = re.compile(r"^[0-9a-f]{7,40}$")
INCLUDE = re.compile(r"\{\{#include\s+([^}\s:]+)")


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    kind: str
    message: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.kind}: {self.message}"


def rel(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def changelog_section(text: str, version: str) -> tuple[int, list[str]] | None:
    """Return (first line number, lines) of the `## version` section."""
    lines = text.splitlines()
    heading = re.compile(rf"^##\s+v?{re.escape(version)}(?:\s|$)")
    for i, line in enumerate(lines):
        if heading.match(line):
            body = []
            for line2 in lines[i + 1 :]:
                if re.match(r"^##\s", line2):
                    break
                body.append(line2)
            return i + 2, body
    return None


def check_changelog(root: Path, version: str, label: str) -> list[Finding]:
    path = root / "CHANGELOG.md"
    name = f"{label}CHANGELOG.md"
    if not path.is_file():
        return [Finding(name, 1, "changelog", "file is missing")]
    section = changelog_section(read(path), version)
    if section is None:
        return [Finding(name, 1, "changelog", f"no `## {version}` section")]
    start, body = section
    found = []
    for offset, line in enumerate(body):
        for what, pattern in PLACEHOLDERS.items():
            if pattern.search(line):
                found.append(
                    Finding(name, start + offset, "changelog placeholder", f"{what}: {line.strip()[:120]}")
                )
    if not any(line.strip() for line in body):
        found.append(Finding(name, start - 1, "changelog", f"`## {version}` section is empty"))
    return found


def lead_paragraph(lines: list[str]) -> range:
    """Line indexes of the first paragraph after the title."""
    i = 0
    while i < len(lines) and (not lines[i].strip() or lines[i].lstrip().startswith("#")):
        i += 1
    j = i
    while j < len(lines) and lines[j].strip():
        j += 1
    return range(i, j)


def compile_version_pattern(template: str, previous: str) -> re.Pattern[str]:
    minor = previous.rsplit(".", 1)[0]
    source = template.replace("{prev_minor}", re.escape(minor)).replace("{prev}", re.escape(previous))
    source = source.replace("{{", "{").replace("}}", "}")
    return re.compile(source)


def check_stale_versions(root: Path, previous: str) -> list[Finding]:
    patterns = {k: compile_version_pattern(v, previous) for k, v in VERSION_PATTERNS.items()}
    found = []
    for glob, names in STALE_VERSION_TABLE:
        for path in sorted(root.glob(glob)):
            if not path.is_file():
                continue
            lines = read(path).splitlines()
            lead = lead_paragraph(lines)
            for n, line in enumerate(lines):
                for name in names:
                    if name.endswith("(lead)") and n not in lead:
                        continue
                    if patterns[name].search(line):
                        found.append(
                            Finding(
                                rel(path, root),
                                n + 1,
                                "stale version",
                                f"{name} names {previous}: {line.strip()[:120]}",
                            )
                        )
    return found


def user_facing_files(root: Path) -> list[Path]:
    files: set[Path] = set()
    for glob in USER_FACING_GLOBS:
        files.update(p for p in root.glob(glob) if p.is_file())
    book = root / "book" / "src"
    for page in book.glob("**/*.md") if book.is_dir() else ():
        files.add(page)
        for target in INCLUDE.findall(read(page)):
            included = (page.parent / target).resolve()
            if included.is_file():
                files.add(included)
    changelog = (root / "CHANGELOG.md").resolve()
    return sorted(
        p
        for p in (f.resolve() for f in files)
        if p != changelog and rel(p, root) not in PREVIEW_ALLOWLIST
    )


def scan_unshipped(name: str, numbered: list[tuple[int, str]]) -> list[Finding]:
    found = []
    for n, line in numbered:
        if NOT_SHIPPED_NOTE.search(line):
            continue
        for what, pattern in UNSHIPPED_FEATURES.items():
            if pattern.search(line):
                found.append(Finding(name, n, "unshipped feature", f"{what}: {line.strip()[:120]}"))
    return found


def check_unshipped(root: Path, version: str, label: str) -> list[Finding]:
    found = []
    for path in user_facing_files(root):
        lines = read(path).splitlines()
        found += scan_unshipped(label + rel(path, root), list(enumerate(lines, 1)))
    changelog = root / "CHANGELOG.md"
    if changelog.is_file():
        section = changelog_section(read(changelog), version)
        if section is not None:
            start, body = section
            found += scan_unshipped(
                f"{label}CHANGELOG.md", [(start + i, line) for i, line in enumerate(body)]
            )
    return found


def load_json(path: Path, label: str) -> tuple[object, list[Finding]]:
    try:
        return json.loads(read(path)), []
    except (OSError, ValueError) as err:
        return None, [Finding(label, 1, "pin", f"cannot read pins: {err}")]


def check_pin(label: str, pins: object, key: str | None, expected: str, what: str) -> list[Finding]:
    entry = pins.get(key) if key is not None and isinstance(pins, dict) else pins
    revision = entry.get("revision") if isinstance(entry, dict) else None
    if not isinstance(revision, str):
        return [Finding(label, 1, "pin", f"no {what} revision")]
    if not revision.startswith(expected) and not expected.startswith(revision):
        return [Finding(label, 1, "pin", f"{what} pins {revision}, expected {expected}")]
    return []


def check_mesite(mesite: Path, tag_commit: str | None, sail_tag_commit: str | None) -> list[Finding]:
    label = f"mesite/{MESITE_PINS}"
    pins, found = load_json(mesite / MESITE_PINS, label)
    if found:
        return found
    if tag_commit:
        found += check_pin(label, pins, "dive", tag_commit, "Dive book")
    if sail_tag_commit:
        found += check_pin(label, pins, "sail", sail_tag_commit, "Sail book")
    return found


def check_website(website: Path, sail_tag_commit: str) -> list[Finding]:
    label = f"website/{WEBSITE_PIN}"
    pins, found = load_json(website / WEBSITE_PIN, label)
    return found or check_pin(label, pins, None, sail_tag_commit, "Sail book")


def action_default(text: str) -> tuple[int, str] | None:
    """The `default:` of the `sail-version` input of an action.yml."""
    lines = text.splitlines()
    in_inputs = False
    for i, line in enumerate(lines):
        if re.match(r"^inputs:\s*$", line):
            in_inputs = True
            continue
        if in_inputs and re.match(r"^\S", line):
            in_inputs = False
        if in_inputs and re.match(r"^  sail-version:\s*$", line):
            for j in range(i + 1, len(lines)):
                if re.match(r"^ {0,2}\S", lines[j]):
                    break
                m = re.match(r"^\s+default:\s*[\"']?([^\"'\s#]+)", lines[j])
                if m:
                    return j + 1, m.group(1)
            return None
    return None


def check_sail_dive_versions(sail: Path, previous: str) -> list[Finding]:
    patterns = {k: compile_version_pattern(v, previous) for k, v in SAIL_DIVE_VERSION_PATTERNS.items()}
    found = []
    for name in SAIL_DIVE_VERSION_TABLE:
        path = sail / name
        if not path.is_file():
            continue
        for n, line in enumerate(read(path).splitlines(), 1):
            for what, pattern in patterns.items():
                if pattern.search(line):
                    found.append(
                        Finding(
                            "sail/" + name,
                            n,
                            "stale version",
                            f"{what} names dive {previous}: {line.strip()[:120]}",
                        )
                    )
    return found


def check_sail_versions(sail: Path, sail_version: str) -> list[Finding]:
    found = []
    for glob in SAIL_VERSION_FILES:
        for path in sorted(sail.glob(glob)):
            name = "sail/" + rel(path, sail)
            seen = False
            for n, line in enumerate(read(path).splitlines(), 1):
                for what, pattern in SAIL_VERSION_PATTERNS.items():
                    for m in pattern.finditer(line):
                        seen = True
                        if m.group(1) != sail_version:
                            found.append(
                                Finding(
                                    name,
                                    n,
                                    "sail version",
                                    f"{what} names {m.group(1)}, expected {sail_version}",
                                )
                            )
            if not seen and rel(path, sail) in SAIL_REQUIRED_MENTION:
                found.append(Finding(name, 1, "sail version", f"never names Sail {sail_version}"))
    for required in SAIL_REQUIRED_MENTION:
        if not (sail / required).is_file():
            found.append(Finding("sail/" + required, 1, "sail version", "file is missing"))
    action = sail / SAIL_ACTION
    name = "sail/" + SAIL_ACTION
    if not action.is_file():
        found.append(Finding(name, 1, "sail version", "file is missing"))
    else:
        default = action_default(read(action))
        if default is None:
            found.append(Finding(name, 1, "sail version", "no `sail-version` input default"))
        elif default[1] != sail_version:
            found.append(
                Finding(name, default[0], "sail version", f"default is {default[1]}, expected {sail_version}")
            )
    return found


def run(args: argparse.Namespace) -> list[Finding]:
    root = Path(args.root)
    found = check_changelog(root, args.version, "")
    found += check_stale_versions(root, args.previous)
    found += check_unshipped(root, args.version, "")
    if args.mesite:
        found += check_mesite(Path(args.mesite), args.tag_commit, args.sail_tag_commit)
    if args.sail:
        sail = Path(args.sail)
        found += check_changelog(sail, args.sail_version, "sail/")
        found += check_unshipped(sail, args.sail_version, "sail/")
        found += check_sail_versions(sail, args.sail_version)
        found += check_sail_dive_versions(sail, args.previous)
    if args.website:
        found += check_website(Path(args.website), args.sail_tag_commit)
    return found


def parse(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--version", required=True, help="the release, X.Y.Z")
    parser.add_argument("--previous", required=True, help="the previous release, A.B.C")
    parser.add_argument("--tag-commit", help="commit of the dive release tag")
    parser.add_argument("--mesite", help="checkout of jeffreykentjr/mesite")
    parser.add_argument("--sail", help="checkout of Chelis-Lang/sail at the paired release")
    parser.add_argument("--sail-version", help="the paired Sail release, X.Y.Z")
    parser.add_argument("--sail-tag-commit", help="commit of the Sail release tag")
    parser.add_argument("--website", help="checkout of Chelis-Lang/website")
    parser.add_argument("--root", default=str(ROOT), help="dive checkout (default: this one)")
    args = parser.parse_args(argv)
    for name in ("version", "previous", "sail_version"):
        value = getattr(args, name)
        if value is not None and not VERSION.match(value):
            parser.error(f"--{name.replace('_', '-')} must be X.Y.Z, got {value!r}")
    for name in ("tag_commit", "sail_tag_commit"):
        value = getattr(args, name)
        if value is not None:
            value = value.lower()
            if not COMMIT.match(value):
                parser.error(f"--{name.replace('_', '-')} must be a hex commit, got {value!r}")
            setattr(args, name, value)
    if args.version == args.previous:
        parser.error("--version and --previous are the same")
    if args.mesite and not args.tag_commit:
        parser.error("--mesite needs --tag-commit")
    if args.sail and not args.sail_version:
        parser.error("--sail needs --sail-version")
    if args.mesite and args.sail_version and not args.sail_tag_commit:
        parser.error("--mesite with --sail-version needs --sail-tag-commit")
    if args.website and not args.sail_tag_commit:
        parser.error("--website needs --sail-tag-commit")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse(sys.argv[1:] if argv is None else argv)
    found = run(args)
    for finding in found:
        print(finding)
    if found:
        print(f"check-release-docs: {len(found)} finding(s)", file=sys.stderr)
        return 1
    print("check-release-docs: clean", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
