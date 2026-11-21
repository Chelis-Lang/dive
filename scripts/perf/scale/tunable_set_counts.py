"""Count states and successor occurrences of the accompanying FIFO history model."""

import argparse
import hashlib
import json
import math
from functools import cache
from itertools import accumulate
from pathlib import Path

MODULE_SHA256 = "c59b7a02af26badd2a522c8384ba0f99abb7dca58c342a5f2c507e157c2ce88f"


def local_histories(word, actor, replicas, limit, choices):
    """Count local histories, outgoing action witnesses and finished histories."""
    before_issue = [1]
    issued = available = 0
    for creator in word:
        if creator == actor:
            # Before this issue, any longer prefix already issued by others can arrive.
            before_issue.extend([0] * (available + 1 - len(before_issue)))
            before_issue = list(accumulate(before_issue))
            issued += 1
        else:
            available += 1
    before_issue.extend([0] * (available + 1 - len(before_issue)))
    states = outgoing = done = 0
    for received, histories in enumerate(accumulate(before_issue)):
        if issued + received == replicas * limit:
            # While, End and Done. Two local control transitions remain across them.
            states += 3 * histories
            outgoing += 2 * histories
            done += histories
        else:
            states += histories
            if issued < limit:
                # A receive on an empty inbox is an enabled stutter in this branch.
                outgoing += histories * (choices + 1)
            elif received < available:
                outgoing += histories
    return states, outgoing, done


def count(replicas, limit, elements, policies, symmetric, max_words=1_000_000):
    """Enumerate canonical creator prefixes, retaining all labeled action witnesses."""
    if min(replicas, limit, elements, policies, max_words) < 1:
        raise ValueError("counts and prefix budget must be positive")
    kinds = 2 * policies
    choices = kinds * elements

    @cache
    def payloads(length):
        if not symmetric:
            return choices**length
        # Element names in stamp order form a partition into at most E blocks.
        partitions = [1] + [0] * min(length, elements)
        for _ in range(length):
            partitions = [0] + [
                n * partitions[n] + partitions[n - 1] for n in range(1, len(partitions))
            ]
        return kinds**length * sum(partitions)

    distinct = generated = prefixes = 0
    stack = [((), 0)]
    while stack:
        word, used = stack.pop()
        prefixes += 1
        if prefixes > max_words:
            raise ValueError("creator-prefix budget exceeded")
        length = len(word)
        unused = replicas - used
        # An unused actor has received one of the length+1 FIFO prefixes.
        if symmetric:
            unused_states = math.comb(length + unused, unused)
            actor_names = 1
        else:
            unused_states = (length + 1) ** unused
            actor_names = math.perm(replicas, used)
        local = [local_histories(word, actor, replicas, limit, choices) for actor in range(used)]
        states = unused_states * math.prod(row[0] for row in local)
        outgoing = states * unused * (choices + 1)
        outgoing += sum((states // size) * degree for size, degree, _ in local)
        if unused == 0:
            outgoing += math.prod(row[2] for row in local)  # Complete PlusCal terminal self-loop.
        multiplicity = actor_names * payloads(length)
        distinct += states * multiplicity
        generated += outgoing * multiplicity
        # Order creator names by first issue. No group permutations are materialized.
        for actor in range(used):
            if word.count(actor) < limit:
                stack.append(((*word, actor), used))
        if unused:
            stack.append(((*word, used), used + 1))
    return dict(distinct=distinct, generated=1 + generated,
                depth=replicas * replicas * limit + 2 * replicas + 1, creator_prefixes=prefixes)


def configuration(replicas, operations, elements, policies, symmetric):
    """Emit a TLC/Core CFG for the same domains and the two reconstructed invariants."""
    def members(prefix, count):
        return "{" + ", ".join(f"{prefix}{n}" for n in range(1, count + 1)) + "}"

    lines = ["CONSTANTS", " Replicas = " + members("r", replicas),
             " Elements = " + members("e", elements), f" OpsPerReplica = {operations}",
             " Policies = " + members("", policies), "INIT Init", "NEXT Next",
             "INVARIANTS HistoryAgreement Convergence"]
    if symmetric:
        lines.append("SYMMETRY Symmetry")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("replicas", "operations", "elements", "policies"):
        parser.add_argument(arg, type=int)
    parser.add_argument("--symmetry", action="store_true")
    parser.add_argument("--emit-config", action="store_true", help="write the matching CFG to stdout")
    parser.add_argument("--max-words", type=int, default=1_000_000)
    args = parser.parse_args()
    module = Path(__file__).with_name("TunableSetHistory.tla")
    if hashlib.sha256(module.read_bytes()).hexdigest() != MODULE_SHA256:
        parser.error("the accompanying reconstructed transition system has changed")
    if min(args.replicas, args.operations, args.elements, args.policies) < 1:
        parser.error("all domains must be nonempty")
    if args.emit_config:
        print(configuration(args.replicas, args.operations, args.elements, args.policies,
                            args.symmetry), end="")
        return
    try:
        result = count(args.replicas, args.operations, args.elements, args.policies,
                       args.symmetry, args.max_words)
    except ValueError as exc:
        parser.error(str(exc))
    print(json.dumps(result))


if __name__ == "__main__":
    main()
