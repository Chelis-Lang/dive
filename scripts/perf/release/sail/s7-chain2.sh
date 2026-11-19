#!/bin/bash
# s7-chain2.sh: the second S7 chain, after the Sail #765 fix: builds the final and lto Sail
# revisions, then reruns latency, first-job, loop, throughput and Sail-only parts.
source ~/tools/s7-env.sh
L=~/s7chain.log
rm -f ~/st/s7-loop-*.jsonl ~/st/s7-through-*.jsonl
echo "build start $(date -u +%FT%TZ)" >> $L
bash ~/tools/s7-build.sh final ce94df74 > ~/build-final.log 2>&1
bash ~/tools/s7-build.sh lto 529bb6d7 > ~/build-lto.log 2>&1
echo "build end $(date -u +%FT%TZ)" >> $L
cat ~/inst-*.rev > ~/revs.txt
find ~/inst-* ~/bins ~/st/fixtures "$(dirname $(dirname $JAVA))" "$TLA2TOOLS" -type f -exec cat {} + > /dev/null 2>&1
sleep 30
run() { echo "start $1 $2 $(date -u +%FT%TZ)" >> $L; python3 -I ~/tools/s7.py $1 $2 >> ~/s7-$1-$2.err 2>&1; echo "end $1 $2 rc=$? $(date -u +%FT%TZ)" >> $L; }
run latency final
run first final
run loop before; run loop final
run through before; run through final
run only final
run latency lto
echo "chain2 done $(date -u +%FT%TZ)" >> $L
