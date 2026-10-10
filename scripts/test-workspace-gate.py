"""Exercise every combination of successful and incomplete CI prerequisites."""
import itertools
from pathlib import Path
import subprocess

script = Path(__file__).resolve().with_name("workspace-gate.sh")
for results in itertools.product(("success", "failure", "cancelled", "skipped", ""), repeat=3):
    run = subprocess.run(["bash", str(script), *results], capture_output=True)
    expected = all(result == "success" for result in results)
    if (run.returncode == 0) != expected:
        raise SystemExit(f"Incorrect required gate result for {results!r}")
for arguments in ([], ["success"], ["success"] * 2, ["success"] * 4):
    if subprocess.run(["bash", str(script), *arguments], capture_output=True).returncode == 0:
        raise SystemExit(f"Accepted incomplete prerequisite results: {arguments!r}")
print("125 prerequisite combinations and four malformed argument lists passed")
