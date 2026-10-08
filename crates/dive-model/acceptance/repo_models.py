"""Lists the repository's own models for dump_models.sh.

    python3 -I repo_models.py <repository>

Prints one model per line: id TAB directory TAB module TAB cfg, with the cfg
path relative to the repository as the id: dive-model's fixtures
(`crates/dive-model/tests/fixtures/specs/`), then the models
`crates/dive-ir/acceptance/models.py repo` lists (the split-rule fixtures,
the TLC probes and the harness fixtures).
"""
import glob
import os
import subprocess
import sys


def main():
    repo = os.path.abspath(sys.argv[1])
    pattern = os.path.join(repo, "crates/dive-model/tests/fixtures/specs/*/*.cfg")
    for cfg in sorted(glob.glob(pattern)):
        d = os.path.dirname(cfg)
        name = os.path.basename(d)
        if os.path.basename(cfg) != name + ".cfg":
            continue
        print("\t".join([os.path.relpath(cfg, repo), d, name + ".tla", name + ".cfg"]))
    out = subprocess.run(
        [sys.executable, "-I", os.path.join(repo, "crates/dive-ir/acceptance/models.py"), "repo", repo],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    sys.stdout.write(out)


if __name__ == "__main__":
    main()
