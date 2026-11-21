#!/usr/bin/env python3
"""Reconstruct the two model-checking modules from MSR-TR-2012-84, pp. 14–34."""

import argparse
from hashlib import sha256
from pathlib import Path
import re
import unicodedata

PDF_HASH = "316e298ec24a63eedb8e5d5e3d25cfc4da514cc007d99be5c6a82701fbdb342c"
MODULES = (
    ("PastureNode", 14, 32, "cd9a63087ae2786797a7a7857cbe1252879bfa139beb805195a01baf0df7c921"),
    ("PastureNodeModel", 33, 34, "329a07d39097209def8a709ecb2cba76012998952fc6b341a3eb691246397467"),
)
BOUNDS = ("MaxAppPcrLen", "MaxSemPcrLen", "MaxSealPcrLen", "MaxTsValues", "MaxBootCtr")
TOKENS = str.maketrans({
    "−": "-", "∧": "/\\", "∨": "\\/", "∈": "\\in ", "∃": "\\E ", "∀": "\\A ",
    "≤": "<=", "∪": "\\cup ", "∩": "\\cap ", "¬": "~", "̸": "/", "→": "->",
    "←": "<-", "⇒": "=>", "⟨": "<<", "⟩": ">>", "′": "'", "“": '"', "”": '"', "□": "[]",
})


def body_spans(page):
    # The report shades the full extent of each comment, including inline comments.
    shaded = [s["rect"] for s in page.get_drawings() if s["fill"] and 0.84 < s["fill"][0] < 0.86]
    for block in page.get_text("rawdict")["blocks"]:
        if block["type"]:
            continue
        for line in block["lines"]:
            for span in line["spans"]:
                font = span["font"]
                if "Medi" in font or "Slant" in font:
                    continue
                keep = []
                for char in span["chars"]:
                    _, y = char["origin"]
                    x = (char["bbox"][0] + char["bbox"][2]) / 2
                    if y >= 111 and not any(r.x0 - 0.2 < x < r.x1 + 0.2 and r.y0 < y - 2 < r.y1 for r in shaded):
                        keep.append(char)
                text = "".join(c["c"] for c in keep).strip()
                if text:
                    x, y = next(c["origin"] for c in keep if c["c"].strip())
                    yield x, y, font, text


def page_lines(page, name):
    rows = {}
    raised = []
    for x, y, font, text in body_spans(page):
        if font in ("CMR5", "CMSY7", "CMTI7"):
            raised.append((x, y, font, text))
        else:
            rows.setdefault(round(y, 1), []).append((x, font, text))
    margin = min(x for row in rows.values() for x, _, _ in row)
    for x, y, font, text in raised:
        # Put the definition triangle above its equals, a prime on its variable
        # or the subscript of [Next]_vars back on the original baseline.
        target = y + {"CMR5": 5.65, "CMSY7": 3.62, "CMTI7": -1.6}[font]
        baseline = min(rows, key=lambda b: abs(b - target))
        if abs(baseline - target) >= 2:
            raise ValueError("could not place a printed superscript or subscript")
        row = rows[baseline]
        if font == "CMR5":
            entry = min((e for e in row if "=" in e[2]), key=lambda e: abs(e[0] - x))
            row.remove(entry)
            row.append((*entry[:2], entry[2].replace("=", "==", 1)))
        else:
            row.append((x, font, "_" + text if font == "CMTI7" else text))

    for _, row in sorted(rows.items()):
        row.sort()
        pieces = [t.replace("�→", "|->").translate(TOKENS) for _, _, t in row]
        line = re.sub(r"/\s+=", "/=", " ".join(pieces))
        if "MODULE" in line:
            line = f"---- MODULE {name} ----"
        yield " " * round((row[0][0] - margin) / 2.75) + line


def model_text(document, name, first, last):
    lines = [line for page in document.pages(first - 1, last)
             for line in page_lines(page, name)]
    start = next(i for i, line in enumerate(lines) if "MODULE" in line)
    lines = lines[start:]
    lines[0] = lines[0].strip()
    text = "\n".join(lines) + "\n====\n"
    if name == "PastureNodeModel":
        return text
    text = unicodedata.normalize("NFKC", text)
    # Computer Modern prints negations as overlays, boxes as a special-font 2,
    # and the digits inside these identifiers in a different font.
    text = text.replace(" = / ", " /= ").replace("\\in  / ", "\\notin ").replace(" ≡ ", " <=> ")
    text = text.replace(" 2 [ Next ] _vars", " [][Next]_vars")
    text = re.sub(
        r"\b(NextSemChkpt|NextSemRecov|PcCHKPT|PcRECOV|bootCtr|nvappPcr|nvcurrent|pcr|sealPcr) ([0-9])(?: (WhenCorrect|WhenIncorrect|o|r)\b)?",
        lambda m: "".join(part for part in m.groups() if part), text,
    )
    lines = text.splitlines()
    nested = False
    for i, line in enumerate(lines):
        if not line.startswith(" "):
            nested = False
        if nested:
            lines[i] = "         " + line.lstrip()
        if line.lstrip().startswith("/\\ \\E") and line.endswith(":"):
            nested = True
        if line.endswith(" ELSE"):
            lines[i + 1] = " " * (len(line) - len(line.lstrip()) + 5) + lines[i + 1].lstrip()
        if ":: /\\" in line:
            lines[i + 1] = " " * line.rindex("/\\") + lines[i + 1].lstrip()
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pdf", type=Path)
    parser.add_argument("output", type=Path, help="new directory for modules and Pasture.cfg")
    parser.add_argument("--bounds", type=int, nargs=5, default=[1] * 5, metavar=BOUNDS)
    args = parser.parse_args()
    if min(args.bounds) < 1:
        parser.error("the published controls use positive bounds")
    if sha256(args.pdf.read_bytes()).hexdigest() != PDF_HASH:
        parser.error("expected the August 17, 2012 Microsoft Research technical report")
    try:
        import fitz
    except ImportError:
        parser.error("install PyMuPDF (validated with version 1.23.7)")
    files = {}
    with fitz.open(args.pdf) as document:
        for name, first, last, digest in MODULES:
            encoded = model_text(document, name, first, last).encode()
            if sha256(encoded).hexdigest() != digest:
                parser.error(f"{name} transcription differs from the checked reconstruction")
            files[name + ".tla"] = encoded
    cfg = ("SPECIFICATION Spec\nCONSTRAINT Constrain\n"
           "INVARIANTS InvType InvNvProtection InvVerifiableRevocation InvAccessUndeniability\n"
           "CONSTANT NullTs = [PastureNode] NullTs\n\nCONSTANTS\n")
    cfg += "".join(f"{key} = {n}\n" for key, n in zip(BOUNDS, args.bounds))
    files["Pasture.cfg"] = cfg.encode()
    args.output.mkdir()
    for filename, content in files.items():
        (args.output / filename).write_bytes(content)


if __name__ == "__main__":
    main()
