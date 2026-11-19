#!/bin/bash
# spot-prep.sh <dive-url> <bench-url> <rev>: install the release binaries built on the first node
# (presigned GET URLs from ops/presign.py), check the checkout is <rev>, record host facts, warm cache.
set -eu
cd ~
curl -sSfo ~/bins/dive "$1"; curl -sSfo ~/bins/dive-bench "$2"; chmod +x ~/bins/dive ~/bins/dive-bench
cd ~/dive; test "$(git rev-parse HEAD)" = "$3"
{
  echo "source $(git rev-parse HEAD)"
  ~/bins/dive --version
  sha256sum ~/bins/dive ~/bins/dive-bench
  lscpu | grep -E 'Model name|^CPU\(s\)|Thread|NUMA node'
  numactl --hardware | head -3
  echo "governor $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
  echo "freq $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq) $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq)"
  echo "smt $(cat /sys/devices/system/cpu/smt/control) no_turbo $(cat /sys/devices/system/cpu/intel_pstate/no_turbo)"
  echo "thp $(cat /sys/kernel/mm/transparent_hugepage/enabled)"
  echo "kernel $(uname -r)"
  grep PRETTY /etc/os-release
} > ~/hostinfo.txt 2>&1
find ~/.cache/dive ~/bins ~/dive/conformance/golden -type f -exec cat {} + > /dev/null 2>&1 || true
touch ~/PREP_DONE
