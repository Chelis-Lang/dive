#!/usr/bin/env bash
# Exercise the release layout without a checkout, system Python, or executables in PATH.
set -eu
dive=$(realpath "${1:?expected installed dive executable}")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
cat > Receipts.tla <<'TLA'
---- MODULE Receipts ----
CONSTANT IDs
VARIABLE got
TypeInvariant == got \subseteq IDs
Init == got = {}
Next == \/ \E i \in IDs \ got : got' = got \cup {i}
        \/ UNCHANGED got
Empty == got = {}
====
TLA
cat > Receipts.cfg <<'CFG'
CONSTANT IDs = {red, green, blue}
INIT Init
NEXT Next
INVARIANT TypeInvariant
CFG
run() { env -i PATH=/nonexistent "$dive" check Receipts.tla --strategy symbolic --output json "$@"; }
run --write-predicate closed.json > fresh
jq -e '.verdict=="success" and .distinct==8 and .generated==21 and .depth==4' fresh >/dev/null
run --verify-predicate closed.json --time-limit 10s > proof
jq -e '.proof=="inductive-safety" and (has("distinct")|not)' proof >/dev/null
cp Receipts.cfg OnlyEmpty.cfg
echo 'INVARIANT Empty' >> OnlyEmpty.cfg
rc=0; run --refine-domains --recheck Receipts.tla Receipts.cfg --recheck Receipts.tla OnlyEmpty.cfg > edit-session || rc=$?
test "$rc" = 1
jq -se 'length==3 and .[0].distinct==8 and .[1].reused and .[1].generated==21 and .[2].trace_validated and .[2].ending.name=="Empty" and (.[2].reused|not)' edit-session >/dev/null
rc=0; run --config OnlyEmpty.cfg --recheck Receipts.tla OnlyEmpty.cfg --trace Keep.json > saved-traces || rc=$?
test "$rc" = 1
jq -se 'length==2 and .[0].trace_file=="Keep.json" and (.[1]|has("trace_file")|not) and .[1].trace.ending.name=="Empty"' saved-traces >/dev/null
rc=0; run --max-memory 32m > limited || rc=$?
test "$rc" = 4
jq -e '.verdict=="stopped"' limited >/dev/null
cp Receipts.cfg before.cfg
rc=0; run --recheck Receipts.tla Receipts.cfg --trace Receipts.cfg >/dev/null 2>&1 || rc=$?
test "$rc" = 2 && cmp before.cfg Receipts.cfg
echo 'INVARIANT Empty' >> Receipts.cfg
rc=0; run --max-memory 1g > false-row || rc=$?
test "$rc" = 1
jq -e '.trace_validated and .depth==2 and .ending.name=="Empty"' false-row >/dev/null
cat > Machines.tla <<'TLA'
---- MODULE Machines ----
CONSTANT Ids
VARIABLE mode
TypeInvariant == mode \in [Ids -> {"off","on"}]
Init == mode = [i \in Ids |-> "off"]
Next == \E i \in Ids : mode' = [mode EXCEPT ![i] = "on"]
AllOff == \A i \in Ids : mode[i] = "off"
====
TLA
cat > Machines.cfg <<'CFG'
CONSTANT Ids = {a,b}
INIT Init NEXT Next INVARIANT TypeInvariant
CFG
occupancy() { env -i PATH=/nonexistent "$dive" check Machines.tla --strategy occupancy --output json "$@"; }
occupancy --time-limit 10s --write-predicate support.json > population
jq -e '.verdict=="success" and .distinct==4 and .generated==9 and .depth==3 and .occupancies==3' population >/dev/null
occupancy --verify-predicate support.json > verified-support
jq -e '.proof=="inductive-safety" and .failed_obligation==null and (has("distinct")|not)' verified-support >/dev/null
echo 'INVARIANT AllOff' >> Machines.cfg
rc=0; occupancy --verify-predicate support.json > unproved-support || rc=$?
test "$rc" = 2
jq -e '.proof=="inconclusive" and .failed_obligation=="AllOff" and (has("trace")|not)' unproved-support >/dev/null
rc=0; occupancy --trace population-trace.json > population-bad || rc=$?
test "$rc" = 1
jq -e '.trace_validated and .depth==2 and .ending.name=="AllOff" and .trace_file=="population-trace.json"' population-bad >/dev/null
