#!/usr/bin/env python3
"""Check the prose of dive's user documentation.

User documentation teaches a TLA+ user how to run dive. It does not carry
maintainer material: issue numbers, design-record numbers, gate names, commit
hashes, process talk or status words. Two kinds of check run here.

* Literal checks gate (exit status 1): em dashes, spaced en dashes,
  "load-bearing", issue and pull-request references, ADR citations, bare
  divergence ids (a link into tlc-differences.md or an entry heading is fine),
  the release gate names R1 to R6, commit hashes, the words owner, campaign,
  agent, LLM, Claude and Codex, status words, and links into maintainer
  material (docs/survey, docs/internal). An unclosed code fence also gates.
* Phrase tics are reported as warnings and never gate: stock words such as
  "robust" or "leverage", self-narration ("this section explains"), forward
  promises, signposting, hedges and "not only" contrasts. They nominate a line
  for review; they are not a reason to rewrite it.

Fenced code blocks and HTML comments are skipped. Inline code is checked
except for the word rules, so a command flag in backticks does not trip them.

Usage: check-docs-prose.py [--warn-only] [--quiet] [PATH ...]

With no PATH it checks README.md, CHANGELOG.md, docs/user/**/*.md and
docs/tlc-differences.md under the repository root. A directory PATH is
searched for Markdown files. Finding no files at all is an error: a checker
that reads nothing must not report clean. --warn-only reports literal findings
but exits 0. Standard library only; run with `python3 -I`.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCOPE = ("README.md", "CHANGELOG.md", "docs/user", "docs/tlc-differences.md")

# Literal checks. Each pattern is searched in one prose line.
LITERAL: dict[str, re.Pattern[str]] = {
    "em dash": re.compile(r"—|&mdash;|&#8212;|&#x2014;", re.I),
    "en dash used as a dash": re.compile(r"\s–\s|&ndash;", re.I),
    "load-bearing": re.compile(r"\bload[- ]bearing\b", re.I),
    "issue or PR reference": re.compile(
        r"(?<![\w&/(\[])#\d+\b(?![-\w])"
        r"|\b[A-Za-z][\w.-]*/[\w.-]+#\d+\b(?![-\w])"
        r"|github\.com/[\w.-]+/[\w.-]+/(?:issues|pull)/\d+",
        re.I,
    ),
    "ADR citation": re.compile(r"\bADRs?[- ]?\d+"),
    "R-gate name": re.compile(r"(?<![\w-])R[1-6](?![\w-])"),
    "link into maintainer material": re.compile(
        r"\]\([^)]*\b(?:docs/)?(?:survey|internal|reviews)/[^)]*\)"
    ),
}

# Divergence ids are allowed in a heading (the entry itself) and inside a
# Markdown link (a citation the reader can follow).
DIVERGENCE_ID = re.compile(r"(?<![\w-])D-\d{3}(?!\d)")
LINK = re.compile(r"\[[^\]]*\]\([^)]*\)")
HEADING = re.compile(r"^\s{0,3}#{1,6}\s")

# A commit hash: 7 to 40 hex digits holding at least one digit and one
# letter, not part of a longer word, a path or a 0x literal, and not a
# constant padded with a long run of one character (7ff8000000000000).
HEX = re.compile(
    r"(?<![\w./-])(?=[0-9a-f]*[a-f])(?=[0-9a-f]*\d)(?![0-9a-f]*(\w)\1{5})"
    r"[0-9a-f]{7,40}(?![\w-])"
)

# Word rules apply outside inline code.
WORDS: dict[str, re.Pattern[str]] = {
    "process word": re.compile(
        r"\b(?:owners?|campaigns?|(?!(?-i:AGENTS\b))agents?|LLMs?|Claude|Codex|Copilot)\b",
        re.I,
    ),
    "status word": re.compile(
        r"\b(?:planned|coming soon|not yet (?:implemented|supported|available|wired|ported|done)"
        r"|roadmap|TODO|TBD|WIP|phase\s+\d+[a-z]?|milestones?|work in progress)\b",
        re.I,
    ),
    "see the source": re.compile(
        r"\b(?:see|read|check|consult|refer to|look at)\s+(?:the\s+)?source(?:\s+code)?\b"
        r"|\bin the source tree\b",
        re.I,
    ),
}

# Phrase tics: warnings only. "dive", "dive in" and "deep dive" are left out
# because dive is the product name.
TIC_WORDS = (
    "delve", "unlock", "unleash", "supercharge", "empower", "elevate", "seamless",
    "seamlessly", "robust", "cutting-edge", "state-of-the-art", "game-changer",
    "revolutionize", "plethora", "myriad", "tapestry", "realm", "testament to",
    "boasts", "ever-evolving", "embark", "honest", "honestly", "truly",
    "leverage", "leverages", "leveraged", "leveraging", "comprehensive",
    "comprehensively", "crucial", "crucially", "notably", "pivotal", "intricate",
    "meticulous", "meticulously", "foster", "showcase", "underscores",
    "paradigm", "groundbreaking", "holistic", "synergy", "profound",
    "transformative", "let's",
)
TIC_PHRASES = (
    # stock phrases
    "at its core", "it's worth noting", "it is worth noting", "worth noting",
    "it's important to note", "it is important to note", "the beauty of",
    "in today's",
    # "not X but Y" and its relatives
    "not just", "isn't just", "not only", "more than just", "not merely",
    "rather than merely",
    # hedges
    "strictly speaking", "to be clear", "needless to say", "for what it's worth",
    # self-narration and signposting
    "this section explains", "this section describes", "this section covers",
    "this page explains", "this page describes", "this document describes",
    "this document explains", "this guide explains", "in this section",
    "the rest of this", "as mentioned above", "as noted above",
    "as described above", "as we saw",
    # forward promises
    "we will see", "we'll see", "as we will", "later in this", "more on this later",
    # process narration
    "we originally", "in an earlier version", "we first tried", "we settled on",
)
TIC_WORD_RE = re.compile(
    r"(?<![\w-])(?:" + "|".join(re.escape(w) for w in TIC_WORDS) + r")(?![\w-])", re.I
)
TIC_PHRASE_RE = re.compile("|".join(re.escape(p) for p in TIC_PHRASES), re.I)

FENCE = re.compile(r"^\s*(```+|~~~+)")
COMMENT = re.compile(r"<!--.*?-->", re.S)
INLINE_CODE = re.compile(r"`+[^`]*`+")


def prose_lines(text: str) -> tuple[list[tuple[int, str]], list[tuple[int, str, str]]]:
    """Return the numbered lines outside fenced code, and fence defects."""
    text = COMMENT.sub(lambda m: "\n" * m.group(0).count("\n"), text)
    lines: list[tuple[int, str]] = []
    defects: list[tuple[int, str, str]] = []
    fence: str | None = None
    opened = 0
    for number, line in enumerate(text.splitlines(), 1):
        m = FENCE.match(line)
        if m:
            marker = m.group(1)
            if fence is None:
                fence, opened = marker[:3], number
            elif marker.startswith(fence) and not line.strip()[len(marker):].strip():
                fence = None
            continue
        if fence is None:
            lines.append((number, line))
    if fence is not None:
        defects.append((opened, "unclosed code fence", fence))
    return lines, defects


def check_text(text: str) -> tuple[list[tuple[int, str, str]], list[tuple[int, str, str]]]:
    """Return (literal findings, tic warnings) as (line, rule, match) triples."""
    lines, errors = prose_lines(text)
    warnings: list[tuple[int, str, str]] = []
    for number, line in lines:
        for rule, pattern in LITERAL.items():
            m = pattern.search(line)
            if m:
                errors.append((number, rule, m.group(0)))
        if not HEADING.match(line):
            m = DIVERGENCE_ID.search(LINK.sub("", line))
            if m:
                errors.append((number, "bare divergence id", m.group(0)))
        no_links = LINK.sub(lambda l: l.group(0).split("](")[0], line)
        m = HEX.search(no_links)
        if m:
            errors.append((number, "commit hash", m.group(0)))
        words = INLINE_CODE.sub("", line)
        words = LINK.sub(lambda l: l.group(0).split("](")[0], words)
        for rule, pattern in WORDS.items():
            m = pattern.search(words)
            if m:
                errors.append((number, rule, m.group(0)))
        for pattern in (TIC_WORD_RE, TIC_PHRASE_RE):
            for m in pattern.finditer(words):
                warnings.append((number, "phrase tic", m.group(0)))
    errors.sort()
    return errors, warnings


def collect(paths: list[Path]) -> list[Path]:
    files: set[Path] = set()
    for path in paths:
        if path.is_dir():
            files.update(p for p in path.rglob("*.md") if p.is_file())
        elif path.is_file():
            files.add(path)
        else:
            print(f"check-docs-prose: no such file: {path}", file=sys.stderr)
    return sorted(files)


def display(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("paths", nargs="*", type=Path)
    parser.add_argument(
        "--warn-only", action="store_true", help="report literal findings but exit 0"
    )
    parser.add_argument("--quiet", action="store_true", help="do not print phrase tics")
    args = parser.parse_args()
    paths = args.paths or [ROOT / p for p in DEFAULT_SCOPE]
    files = collect(paths)
    if not files:
        print("check-docs-prose: no Markdown files found", file=sys.stderr)
        return 2
    n_errors = n_warnings = 0
    for path in files:
        errors, warnings = check_text(path.read_text(encoding="utf-8"))
        name = display(path)
        for number, rule, hit in errors:
            print(f"{name}:{number}: {rule}: {hit}")
        if not args.quiet:
            for number, rule, hit in warnings:
                print(f"{name}:{number}: warning: {rule}: {hit}")
        n_errors += len(errors)
        n_warnings += len(warnings)
    mode = " (warn-only)" if args.warn_only else ""
    print(
        f"check-docs-prose: {len(files)} files, {n_errors} findings, "
        f"{n_warnings} phrase-tic warnings{mode}"
    )
    return 1 if n_errors and not args.warn_only else 0


if __name__ == "__main__":
    sys.exit(main())
