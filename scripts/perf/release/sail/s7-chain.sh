#!/bin/bash
# chain.sh: every S7 part, one measured process at a time, nothing else running.
source ~/tools/s7-env.sh
L=~/s7chain.log
{
  lscpu | grep -E 'Model name|^CPU\(s\)|Thread|NUMA node'
  echo "governor $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null)"
  echo "thp $(cat /sys/kernel/mm/transparent_hugepage/enabled)"; echo "kernel $(uname -r)"; grep PRETTY /etc/os-release
  free -g | head -2; "$JAVA" -version 2>&1 | head -1; sha256sum "$TLA2TOOLS" "$JAVA" ~/bins/dive
  cat ~/inst-*.rev
} > ~/hostinfo.txt 2>&1
find ~/inst-* ~/bins ~/st/fixtures "$(dirname $(dirname $JAVA))" "$TLA2TOOLS" -type f -exec cat {} + > /dev/null 2>&1
run() { echo "start $1 $2 $(date -u +%FT%TZ)" >> $L; python3 -I ~/tools/s7.py $1 $2 >> ~/s7-$1-$2.err 2>&1; echo "end $1 $2 rc=$? $(date -u +%FT%TZ)" >> $L; }
rm -rf /tmp/sail-runtime-images-*
for lab in before cache mimalloc; do
  run latency $lab
done
run first cache
for lab in before cache mimalloc; do run loop $lab; run through $lab; done
run only mimalloc
echo "chain done $(date -u +%FT%TZ)" >> $L
