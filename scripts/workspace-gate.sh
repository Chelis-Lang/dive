#!/usr/bin/env bash
# Fail closed on every incomplete or unsuccessful prerequisite.
set -euo pipefail
if [ "$#" -ne 3 ]; then
  echo "Expected authorship, workspace checks and workspace test results." >&2
  exit 1
fi
for result in "$@"; do
  if [ "$result" != success ]; then
    echo "Workspace prerequisite did not pass: ${result:-missing}" >&2
    exit 1
  fi
done
