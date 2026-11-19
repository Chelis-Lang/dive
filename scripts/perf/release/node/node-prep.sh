#!/bin/bash
# node-prep.sh <sha>: build the release-candidate binaries on a bench node (as bench),
# record host facts in ~/hostinfo.txt and warm the page cache. Writes ~/PREP_DONE.
set -eu
SHA=$1
source ~/.cargo/env
rm -f ~/PREP_DONE
cd ~/dive
git fetch -q origin
git checkout -q "$SHA"
test "$(git rev-parse HEAD)" = "$SHA"
unset RUSTFLAGS
cargo build -q --release --locked -p dive-cli --bins
cp target/release/dive ~/bins/dive
cargo build -q --release --locked -p dive-conformance
cp target/release/dive-conformance ~/bins/dive-conformance
cargo build -q --release --locked -p dive-bench
cp target/release/dive-bench ~/bins/dive-bench
{
  echo "source $SHA"
  ~/bins/dive --version
  sha256sum ~/bins/dive ~/bins/dive-bench ~/bins/dive-conformance
  lscpu | grep -E 'Model name|^CPU\(s\)|Thread|NUMA node'
  numactl --hardware | head -3
  echo "governor $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
  echo "freq $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq) $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq)"
  echo "smt $(cat /sys/devices/system/cpu/smt/control) no_turbo $(cat /sys/devices/system/cpu/intel_pstate/no_turbo)"
  echo "thp $(cat /sys/kernel/mm/transparent_hugepage/enabled)"
  echo "kernel $(uname -r)"
  grep PRETTY /etc/os-release
} > ~/hostinfo.txt 2>&1
find ~/.cache/dive ~/bins -type f -exec cat {} + > /dev/null 2>&1 || true
touch ~/PREP_DONE
