"""Transfer finite induction obligations certified on the loaded source."""

from hashlib import file_digest, sha256
import hmac
import json
import os
from pathlib import Path
import sys
from time import monotonic
from types import SimpleNamespace

from induction import MAX_BYTES, read, write_atomic
from obligation_receipt import implementation, local_key, pack
from worker_process import Stopped, limit_memory, run_helper


FORMAT = "dive-finite-induction-certificate-v1"


class Refusal(ValueError):
    pass


class Job:
    def __init__(self, args):
        self.args = args
        self.module = Path(args.module).resolve()
        self.config = Path(args.config).resolve()
        self.key = local_key()
        self.started = monotonic()
        self.row = {"format": "dive-finite-induction-report-v1", "module": str(self.module),
                    "config": str(self.config), "encoding": "finite-induction"}

    def command(self, command, label):
        return run_helper(command, label, self.args.max_memory)

    def native(self, flag, *values):
        result = self.command([self.args.exporter, flag, self.module, self.config, *values], "finite induction")
        if result.returncode:
            reason = result.stderr.strip().removeprefix("Error: ").strip('"')
            raise Refusal(reason or "finite obligation was not established")
        return json.loads(result.stdout)

    def seal(self, body):
        with Path(__file__).open("rb") as worker:
            worker_id = file_digest(worker, "sha256").hexdigest()
        return hmac.digest(self.key, pack([FORMAT, implementation(self.args.exporter), worker_id, body]), "sha256").hex()

    def report(self, **fields):
        print(json.dumps(dict(self.row, **fields, seconds=round(monotonic() - self.started, 6))), flush=True)

    def proved(self, manifest, reused):
        actions = [row["name"] for row in manifest["actions"]]
        self.report(proof="inductive-safety", failed_obligation=None, transition_obligations=len(actions),
                    reused_obligations=len(actions) if reused else 0, action_inventory=actions,
                    freshly_checked=["target-on-prestates-and-boundary"] if reused else
                    ["Init", "target-on-prestates-and-boundary", *actions],
                    authorized_closure=reused)
        return 0

    @staticmethod
    def context(document):
        return sha256(pack(document)).hexdigest()

    def verify(self, path, scratch):
        artifact = read(Path(path))
        if not isinstance(artifact, dict) or artifact.get("format") != FORMAT:
            raise Refusal("unexpected portable finite predicate format")
        if artifact.get("boundary") != "check-target-before-filter" or not isinstance(artifact.get("actions"), list):
            raise Refusal("invalid finite predicate boundary or action inventory")
        table = artifact.get("predicate")
        if not isinstance(table, dict) or set(table) != {"columns", "inside", "boundary"}:
            raise Refusal("missing portable finite predicate")
        signature = artifact.pop("seal", None)
        signed = (self.key is not None and isinstance(signature, str) and len(signature) == 64
                  and signature.isascii() and hmac.compare_digest(signature, self.seal(artifact)))
        data = scratch / "predicate.json"
        data.write_bytes(pack(table))
        if signed and not self.args.write_predicate:
            # The digest and exact property results come from the same bound model.
            try:
                bound = self.native("--finite-targets", data)["binding"]
            except Refusal:
                # A changed relation may no longer generate the old boundary
                # row that failed. Establish the current obligations from scratch.
                pass
            else:
                if (self.context(bound) == artifact.get("binding") and
                        artifact.get("variables") == bound["variables"] and artifact.get("actions") == bound["actions"]):
                    return self.proved(bound, True)
        flag = "--finite-freeze" if self.args.write_predicate else "--finite-check"
        result = self.native(flag, data)
        bound = result["binding"]
        if artifact.get("variables") != bound["variables"]:
            raise Refusal("finite predicate variable layout")
        if self.context(bound) == artifact.get("binding") and artifact.get("actions") != bound["actions"]:
            raise Refusal("missing or altered original action inventory")
        if self.args.write_predicate:
            self.save(result)
        return self.proved(bound, False)

    def save(self, result):
        bound = result["binding"]
        artifact = {"format": FORMAT, "binding": self.context(bound), "variables": bound["variables"],
                    "actions": bound["actions"], "boundary": bound["boundary"], "predicate": result["table"],
                    "producer_targets": result["producer_targets"]}
        if self.key is not None:
            artifact["seal"] = self.seal(artifact)
        bytes_out = pack(artifact) + b"\n"
        if len(bytes_out) > MAX_BYTES:
            raise Stopped("finite predicate file limit")
        write_atomic(Path(self.args.write_predicate), bytes_out)

    def producer(self, scratch):
        try:
            result = self.native("--finite-produce")
        except Refusal as error:
            if "false predicate " in str(error):
                return self.reachable_failure(scratch)
            if "induction state limit" in str(error) or "row limit" in str(error) or "generated-state limit" in str(error):
                raise Stopped(str(error)) from error
            raise
        if self.args.write_predicate:
            self.save(result)
        return self.proved(result["binding"], False)

    def reachable_failure(self, scratch):
        # Candidate failure alone can be a CTI. Search the original initializer
        # and independently replay the resulting concrete word before reporting it.
        command = [self.args.dive, "check", self.module, "--config", self.config, "--store", "exact",
                   "--seed", "0", "--fp", "0", "--workers", "1", "--exit-codes", "native", "--output", "json"]
        result = self.command(command, "original finite search")
        report = json.loads(result.stdout)
        if report["verdict"] == "stopped":
            raise Stopped(report.get("message") or "original finite search stopped")
        if result.returncode != 10 or report["verdict"] != "invariant-violation":
            raise Refusal("no independently replayable reachable invariant failure")
        trace = report["counterexample"]["trace"]
        witness = scratch / "witness.json"
        witness.write_bytes(pack(trace))
        validation = self.command([self.args.dive, "validate-trace", self.module, "--config", self.config,
                                   "--trace", witness, "--output", "json"], "independent replay")
        if validation.returncode or json.loads(validation.stdout)["result"] != "valid":
            raise Refusal("independent finite witness replay failed")
        self.report(verdict="invariant-violation", ending=trace["ending"], depth=len(trace["states"]),
                    trace=trace, trace_validated=True)
        if self.args.trace:
            write_atomic(Path(self.args.trace), pack(trace) + b"\n")
        return 11


def main(args):
    limit_memory(args.max_memory)
    job = Job(args)
    if os.environ.get("DIVE_COMPAT", "dive") != "dive":
        job.report(verdict="unsupported", reason="finite induction supports the Dive compatibility profile")
        return 2
    try:
        scratch = Path(args.scratch)
        if args.verify_predicate:
            try:
                return job.verify(args.verify_predicate, scratch)
            except (OSError, ValueError, KeyError, TypeError) as error:
                job.report(proof="inconclusive", failed_obligation=str(error))
                return 2
        return job.producer(scratch)
    except Stopped as error:
        job.report(verdict="stopped", reason=str(error))
        return 4
    except (OSError, ValueError, KeyError, TypeError) as error:
        job.report(verdict="unsupported", reason=str(error))
        return 2


if __name__ == "__main__":
    sys.exit(main(SimpleNamespace(**json.loads(sys.argv[1]))))
