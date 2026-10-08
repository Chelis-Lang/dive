#!/bin/bash
# Records TLC's action split for a list of models (see README.md).
#
#   dump_actions.sh <tla2tools.jar> <javac> <java> <classpath-libraries> < models.tsv > out.jsonl
#
# models.tsv has one model per line: <id> TAB <directory> TAB <Module.tla> TAB <Model.cfg>.
# <classpath-libraries> is the rest of the harness classpath after tla2tools.jar
# (the CommunityModules jars, the Apalache and TLAPS library directories),
# colon-separated. <javac> must be a JDK 11+ compiler (the pinned JRE has
# none); <java> should be the pinned JRE. Check the jar's SHA-256 against
# conformance/oracles.toml first. Each model runs in its own JVM, in its own
# directory, as the harness runs TLC.
set -euo pipefail
jar=$1 javac=$2 java=$3 libs=$4
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
"$javac" -nowarn -cp "$jar" -d "$work/classes" "$here/ActionDump.java"
while IFS=$'\t' read -r id dir module cfg; do
  [ -n "$id" ] || continue
  (cd "$dir" && "$java" -XX:+UseParallelGC -Xmx3g -Xss8m -Duser.language=en -Duser.country=US \
     -Dfile.encoding=UTF-8 -Djava.io.tmpdir="$work" \
     -cp "$jar:$libs:$work/classes" ActionDump "$id" "$module" "$cfg" 2> /dev/null) \
    || echo "{\"id\":\"$id\",\"error\":\"the JVM failed\"}"
done
