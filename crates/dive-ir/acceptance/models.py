"""Lists models for dump_actions.sh, one per line: id TAB directory TAB module TAB cfg.

    python3 -I models.py goldens <repository> <examples-checkout>   # the small-tier goldens
    python3 -I models.py repo <repository>                          # the repository's own models

`goldens` lists every Examples model with a TLC golden under
conformance/golden/<oracle>/examples/, with its cfg path relative to the corpus
as the id. `repo` lists the split-rule fixtures, the TLC probes and the harness
fixtures, with repository-relative ids. UsesCommunityModules is left out: it
needs the CommunityModules on the module path, which the in-repository test
does not have.
"""
import glob
import json
import os
import sys

ORACLE = "tlc-2026.10.06.014338"


def goldens(repo, corpus):
    pattern = os.path.join(repo, "conformance/golden", ORACLE, "examples/specifications/**/*.json")
    for f in sorted(glob.glob(pattern, recursive=True)):
        with open(f) as fh:
            model = json.load(fh)["model"]
        module = os.path.join(corpus, model["module"])
        cfg = os.path.join(corpus, model["id"])
        d = os.path.dirname(module)
        yield model["id"], d, os.path.basename(module), os.path.relpath(cfg, d)


def repo_models(repo):
    for pattern in [
        "crates/dive-ir/tests/fixtures/actions/specs/*/*.cfg",
        "crates/dive-api/tests/fixtures/checker/tlc-probes/specs/*/*.cfg",
        "crates/dive-conformance/tests/fixtures/specs/*/*.cfg",
    ]:
        for cfg in sorted(glob.glob(os.path.join(repo, pattern))):
            d = os.path.dirname(cfg)
            name = os.path.basename(d)
            if name == "UsesCommunityModules" or not os.path.exists(os.path.join(d, name + ".tla")):
                continue
            yield os.path.relpath(cfg, repo), d, name + ".tla", os.path.basename(cfg)


def main():
    if sys.argv[1:2] == ["goldens"] and len(sys.argv) == 4:
        rows = goldens(os.path.abspath(sys.argv[2]), os.path.abspath(sys.argv[3]))
    elif sys.argv[1:2] == ["repo"] and len(sys.argv) == 3:
        rows = repo_models(os.path.abspath(sys.argv[2]))
    else:
        sys.exit(__doc__)
    for row in rows:
        print("\t".join(row))


main()
