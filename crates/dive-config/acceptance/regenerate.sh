#!/bin/bash
# Regenerates dive-config's oracle files from TLC's own cfg reader.
#
#   regenerate.sh <tla2tools.jar> <javac> <java> <examples-checkout> <tlaplus-checkout>
#
# <javac> must be a JDK 11+ compiler (the pinned JRE has none); <java> should
# be the pinned JRE. Check the jar's SHA-256 against conformance/oracles.toml
# first. The checkouts must be at the pins in conformance/corpora.toml
# (Examples 6a7b26a, tlaplus 94d0c50). See README.md.
set -euo pipefail
jar=$1 javac=$2 java=$3 examples=$4 tlaplus=$5
here=$(cd "$(dirname "$0")" && pwd)
crate=$(dirname "$here")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

"$javac" -nowarn -cp "$jar" -d "$work/classes" "$here/CfgDump.java"

# dump <root> <out.jsonl>: every .cfg under <root>, sorted by path.
dump() {
  (cd "$1" && find . -name '*.cfg' -type f | sed 's|^\./||' | LC_ALL=C sort) > "$work/list"
  (cd "$1" && tr '\n' '\0' < "$work/list" | xargs -0 -n 200 \
    "$java" -Dfile.encoding=UTF-8 -Duser.language=en -Duser.country=US \
      -cp "$jar:$work/classes" CfgDump "$1") > "$2"
  echo "$(basename "$2"): $(wc -l < "$2" | tr -d ' ') cfg files" >&2
}

dump "$crate/tests/fixtures/specs" "$crate/tests/fixtures/specs.jsonl"
dump "$crate/tests/fixtures/modelconfig" "$crate/tests/fixtures/modelconfig.jsonl"
dump "$examples" "$crate/tests/corpus/examples.jsonl"
dump "$tlaplus" "$crate/tests/corpus/tlaplus.jsonl"
