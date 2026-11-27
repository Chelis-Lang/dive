#!/bin/bash
# s7-chain-0.2.sh <sail-ref> <dive-rev>: the dive 0.2.0 / Sail 0.3.1 S7 chain. Builds Sail
# into ~/inst-rc and the dive CLI at <dive-rev>, records that Sail links <dive-rev>, builds
# the fixtures, then runs every part one measured process at a time. The `rc` rows are
# fresh checks (S7_FRESH=1: no receipt replay); the `rcreplay` rows rerun latency, loop and
# throughput with the jobs exactly as S7 builds them, so repeated jobs replay receipts.
set -u
REF=$1; DREV=$2
source ~/tools/s7-env.sh
L=~/s7chain.log
echo "build start $(date -u +%FT%TZ)" >> $L
bash ~/tools/s7-build.sh rc "$REF" > ~/build-rc.log 2>&1 || { echo "build failed" >> $L; touch ~/CHAIN_FAILED; exit 1; }
( source ~/.cargo/env; cd ~/dive && git checkout -q "$DREV" && cargo build -q --release --locked -p dive-cli --bins && mkdir -p ~/bins && cp target/release/dive ~/bins/dive ) > ~/build-dive.log 2>&1 || { echo "dive build failed" >> $L; touch ~/CHAIN_FAILED; exit 1; }
echo "build end $(date -u +%FT%TZ)" >> $L
ln -sfn ~/inst-rc ~/inst-rcreplay; ln -sfn ~/src-rc ~/src-rcreplay
{
  echo "sail $(cat ~/inst-rc.rev)"
  grep -A2 'name = "dive-api"' ~/src-rc/Cargo.lock
  (source ~/.cargo/env; cd ~/src-rc && cargo tree --locked -q -i dive-api -e normal --depth 0)
  strings ~/inst-rc/bin/sail-runtime-worker | grep -o 'checkouts/dive-[0-9a-f]*/[0-9a-f]\{7\}' | sort | uniq -c
  ~/inst-rc/bin/sail runtime capabilities | head -c 3000; echo
  ~/bins/dive --version; git -C ~/dive rev-parse HEAD
} > ~/link.txt 2>&1
{
  lscpu | grep -E 'Model name|^CPU\(s\)|Thread|NUMA node'
  echo "governor $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null)"
  echo "thp $(cat /sys/kernel/mm/transparent_hugepage/enabled)"; echo "kernel $(uname -r)"; grep PRETTY /etc/os-release
  free -g | head -2; "$JAVA" -version 2>&1 | head -1; sha256sum "$TLA2TOOLS" "$JAVA" ~/bins/dive
  cat ~/inst-rc.sha; sha256sum ~/tools/*
} > ~/hostinfo.txt 2>&1
rm -rf ~/st /tmp/sail-runtime-images-*
python3 -I ~/tools/s7.py fixtures rc > ~/s7-fixtures.err 2>&1 || { echo "fixtures failed" >> $L; touch ~/CHAIN_FAILED; exit 1; }
find ~/inst-rc/ ~/bins ~/st/fixtures "$(dirname $(dirname $JAVA))" "$TLA2TOOLS" -type f -exec cat {} + > /dev/null 2>&1
sleep 30
run() { echo "start $1 $2 $(date -u +%FT%TZ)" >> $L; env $3 python3 -I ~/tools/s7.py $1 $2 >> ~/s7-$1-$2.err 2>&1; echo "end $1 $2 rc=$? $(date -u +%FT%TZ)" >> $L; }
for p in latency first loop through only ratio; do run $p rc S7_FRESH=1; done
for p in latency loop through; do run $p rcreplay S7_FRESH=0; done
echo "chain done $(date -u +%FT%TZ)" >> $L
touch ~/CHAIN_DONE
