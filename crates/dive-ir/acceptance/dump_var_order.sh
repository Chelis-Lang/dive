#!/bin/bash
# Records TLC's state-variable order for a list of modules (see README.md).
#
#   dump_var_order.sh <tla2tools.jar> <javac> <java> <classpath-libraries> < modules.tsv > out.jsonl
#
# modules.tsv has one module per line: <id> TAB <directory> TAB <Module.tla>
# [TAB <Model.cfg>]. With a cfg the order is read from TLC's FastTool, without
# one from SANY (VarOrderDump.java). <classpath-libraries> is the rest of the
# harness classpath after tla2tools.jar, colon-separated. <javac> must be a JDK
# 17+ compiler (the pinned JRE 17 has none; classes are compiled with
# --release 17 so that it can load them); <java> should be the pinned JRE.
# Check the jar's SHA-256 against conformance/oracles.toml first. Each module
# runs in its own JVM, in its own directory, as the harness runs TLC.
set -euo pipefail
jar=$1 javac=$2 java=$3 libs=$4
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
"$javac" --release 17 -nowarn -cp "$jar" -d "$work/classes" "$here/VarOrderDump.java"
while IFS=$'\t' read -r id dir module cfg; do
  [ -n "$id" ] || continue
  # shellcheck disable=SC2086
  (cd "$dir" && "$java" --add-opens java.base/java.util=ALL-UNNAMED -Xss8m \
     -Duser.language=en -Duser.country=US -Dfile.encoding=UTF-8 -Djava.io.tmpdir="$work" \
     -cp "$jar:$libs:$work/classes" VarOrderDump "$id" "$module" ${cfg:+"$cfg"} 2> /dev/null < /dev/null) \
    || echo "{\"id\":\"$id\",\"error\":\"the JVM failed\"}"
done
