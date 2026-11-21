#!/bin/bash
# sim-chain.sh [simbench arguments...]: S4 simulation throughput on a bench node, as user
# bench. Writes ~/sim/runs.jsonl and ~/sim/hostinfo.txt, then touches ~/sim/DONE.
set -u
C=$HOME/.cache/dive
JAVA=$(find $C/jre -name java -type f | head -1)
S=$C/sha256
CP=$S/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar
CP=$CP:$S/b7fcd727fc5135a628e56d5015651c74ba70681bbaf88df1cd59e9afa64215ad/CommunityModules-202610040242.jar
CP=$CP:$S/dddb19c3d7596913d92c43073b60d1408ac8f6d86716af4d19dd6861a95bfb7d/CommunityModules-deps-202610040242.jar
EX=$(ls -d $C/git/examples/*/ | head -1)
LIB=$(dirname "$(find $C/git -path '*tlapm-library*' -name TLAPS.tla | head -1)")
mkdir -p ~/sim ~/sim/tmp
rm -f ~/sim/DONE
{ uname -sr; lscpu | grep -E 'Model name|^CPU\(s\)|NUMA node\(s\)'; cat /sys/devices/system/cpu/smt/control
  cat /sys/devices/system/cpu/intel_pstate/no_turbo; cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq
  sha256sum ~/bins/dive; (cd ~/dive && git rev-parse HEAD); "$JAVA" -version 2>&1 | head -1; } > ~/sim/hostinfo.txt
python3 ~/tools/simbench.py --dive ~/bins/dive --java "$JAVA" --cp "$CP" --examples "$EX" \
  --library "$LIB" --runs ~/sim/runs.jsonl --tmp ~/sim/tmp "$@" > ~/sim/console.txt 2>&1
touch ~/sim/DONE
