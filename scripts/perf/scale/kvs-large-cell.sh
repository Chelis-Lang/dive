#!/usr/bin/env bash
# Run one bounded cell in a new directory, inside the host's 24 GiB cgroup.
# Inputs and exact tool versions are described in the performance data index.
set -eu
if [[ $# -ne 4 && $# -ne 6 ]]; then
    echo "usage: $0 core|tlc|symbolic WORKERS INPUT-DIRECTORY NEW-RESULT-DIRECTORY [MODULE CONFIG]" >&2
    exit 2
fi
mode=$1
workers=$2
input=$(realpath "$3")
output=$(realpath -m "$4")
if [[ $workers != 1 && $workers != 8 ]]; then
    echo 'expected one or eight workers' >&2
    exit 2
fi
module=$(realpath "${5:-$input/KVS/MCKVS.tla}")
cfg=$(realpath "${6:-$input/KVS/MCKVSSafetyLarge.cfg}")
case $mode in
    core)
        cmd=("$input/dive" check "$module" --config "$cfg" --workers "$workers"
             --fp 0 --seed 0 --compat=tlc --time-limit 600s --max-memory 24g --output json)
        # The checker emits its final stopped report after its own deadline.
        timeout_secs=615
        ;;
    tlc)
        cmd=("$input/jre/bin/java" -Xmx16g -XX:+UseParallelGC -XX:ActiveProcessorCount=8
             -jar "$input/tla2tools.jar" -workers "$workers" -seed 0 -fp 0
             -metadir "$output/states" -config "$cfg" "$module")
        timeout_secs=600
        ;;
    symbolic)
        [[ $workers == 1 ]] || { echo 'symbolic is single-worker' >&2; exit 2; }
        cmd=("$input/python/bin/python3.11" "$input/symbolic/check.py" "$module" "$cfg"
             --dive "$input/dive" --exporter "$input/symbolic_input")
        export PYTHONPATH="$input/site"
        timeout_secs=600
        ;;
    *) echo "unknown checker $mode" >&2; exit 2 ;;
esac
mkdir "$output"
cd "$output"
rc=0
LC_ALL=C /usr/bin/time -f '%e %M' -o time-and-rss \
    timeout --kill-after=5s "${timeout_secs}s" \
    taskset -c 0-15 numactl --membind=0 "${cmd[@]}" >result 2>stderr || rc=$?
echo "$rc" > exit-code
case "$mode:$rc" in
    symbolic:0|core:0|core:5|tlc:0|tlc:124) exit 0 ;;
    *) echo "$mode failed with exit $rc; see $output" >&2; exit 1 ;;
esac
