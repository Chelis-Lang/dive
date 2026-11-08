#!/bin/bash
# Records how TLC binds a list of models (see README.md).
#
#   dump_models.sh <tla2tools.jar> <javac> <java> <classpath-libraries> [dumptrace] < models.tsv > out.jsonl
#
# models.tsv has one model per line: <id> TAB <directory> TAB <Module.tla> TAB <Model.cfg>
# (the format of crates/dive-ir/acceptance/models.py). <classpath-libraries> is
# the rest of the harness classpath after tla2tools.jar (the CommunityModules
# jars, the Apalache and TLAPS library directories), colon-separated. <javac>
# must be a JDK 17+ compiler (the pinned JRE 17 has none; classes are compiled with --release 17 so it can load them); <java> should be the
# pinned JRE. Check the jar's SHA-256 against conformance/oracles.toml first.
# With `dumptrace`, TLC is set up as `-dumpTrace json <file>` sets it up, as the
# conformance harness runs it. Each model runs in its own JVM, in its own
# directory, as the harness runs TLC, and is stopped after DUMP_TIMEOUT
# seconds (default 120): TLC evaluates constant definitions at start-up, and
# some test models define sets too big to evaluate.
set -euo pipefail
jar=$1 javac=$2 java=$3 libs=$4 mode=${5:-}
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
"$javac" --release 17 -nowarn -cp "$jar" -d "$work/classes" "$here/ModelDump.java"
while IFS=$'\t' read -r id dir module cfg; do
  [ -n "$id" ] || continue
  (cd "$dir" && perl -e 'alarm shift; exec @ARGV' "${DUMP_TIMEOUT:-120}" "$java" -XX:+UseParallelGC -Xmx3g -Xss8m -Duser.language=en -Duser.country=US \
     -Dfile.encoding=UTF-8 -Djava.io.tmpdir="$work" \
     -cp "$jar:$libs:$work/classes" ModelDump "$id" "$module" "$cfg" $mode 2> /dev/null) \
    || echo "{\"id\":\"$id\",\"error\":{\"code\":-1,\"class\":\"jvm\",\"message\":\"the JVM failed\"}}"
done
