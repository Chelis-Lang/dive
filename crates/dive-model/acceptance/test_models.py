"""Lists tlaplus test-model configurations for dump_models.sh.

    python3 -I test_models.py <tlaplus-checkout>

Prints one model per line: id TAB directory TAB module TAB cfg, for the
`*.cfg` files under `tlatools/org.lamport.tlatools/test-model/`. A
configuration is paired with its module the way TLC's own tests pair them:
the `ModelCheckerTestCase` subclasses under `test/` name a specification, a
directory and, with `-config`, a configuration
(`super("Spec", "dir", new String[] {"-config", "Other.cfg"})`); otherwise a
configuration goes with the `.tla` of the same base name next to it. The id is
the cfg path relative to the checkout. Configurations that neither rule pairs
are listed on stderr and skipped.
"""
import os
import re
import sys

SUPER = re.compile(
    r'super\(\s*"([^"]+)"\s*,\s*(?:"([^"]*)"\s*,\s*)?(?:new\s+String\s*\[\s*\]\s*\{([^}]*)\})?'
)
CONFIG = re.compile(r'"-config"\s*,\s*"([^"]+)"')


def from_tests(root, base):
    """(directory, module, cfg) triples named by the Java tests."""
    out = set()
    for d, _, files in os.walk(os.path.join(root, "tlatools/org.lamport.tlatools/test")):
        for f in files:
            if not f.endswith(".java"):
                continue
            try:
                text = open(os.path.join(d, f), encoding="utf-8", errors="replace").read()
            except OSError:
                continue
            for m in SUPER.finditer(text):
                spec, path, extra = m.group(1), m.group(2) or "", m.group(3) or ""
                c = CONFIG.search(extra)
                cfg = c.group(1) if c else spec + ".cfg"
                if not cfg.endswith(".cfg"):
                    cfg += ".cfg"
                directory = os.path.normpath(os.path.join(base, path))
                module = spec if spec.endswith(".tla") else spec + ".tla"
                if os.path.exists(os.path.join(directory, module)) and os.path.exists(
                    os.path.join(directory, cfg)
                ):
                    out.add((directory, module, cfg))
    return out


def main():
    root = os.path.abspath(sys.argv[1])
    base = os.path.join(root, "tlatools/org.lamport.tlatools/test-model")
    pairs = {}
    for directory, module, cfg in from_tests(root, base):
        pairs.setdefault(os.path.join(directory, cfg), (directory, module, cfg))
    skipped = 0
    for d, _, files in sorted(os.walk(base)):
        for f in sorted(files):
            if not f.endswith(".cfg"):
                continue
            key = os.path.join(d, f)
            if key not in pairs:
                tla = f[: -len(".cfg")] + ".tla"
                if os.path.exists(os.path.join(d, tla)):
                    pairs[key] = (d, tla, f)
                else:
                    skipped += 1
                    print(os.path.relpath(key, root), file=sys.stderr)
    for key in sorted(pairs):
        d, module, cfg = pairs[key]
        print("\t".join([os.path.relpath(key, root), d, module, cfg]))
    print(f"{skipped} configurations no test or name pairs with a module", file=sys.stderr)


if __name__ == "__main__":
    main()
