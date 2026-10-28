#!/bin/sh
# Usage: scripts/gate-lock.sh <command...>  -- serialises gate and corpus runs across the
# worktrees of one clone (state lives in the shared git directory, or DIVE_GATE_LOCK_DIR).
# Two lanes, each first-in first-out: `gate --fast` runs take the fast lane, everything
# else (full gates, corpus, shadow and harness runs) the heavy lane, so long runs never
# starve short fast gates. Waiters queue as ticket files; dead waiters' tickets are dropped.
D=${DIVE_GATE_LOCK_DIR:-$(git rev-parse --path-format=absolute --git-common-dir)/dive-gate-lock}; mkdir -p "$D"
case " $* " in *" --fast "*) LANE=fast; L=$D/lock-fast ;; *) LANE=heavy; L=$D/lock ;; esac
Q=$D/queue-$LANE
mkdir -p "$Q"
T="$Q/$(date +%s)-$(printf '%010d' $$)"
: > "$T"
cleanup_ticket() { rm -f "$T"; }
trap 'cleanup_ticket' EXIT INT TERM
while :; do
  for t in "$Q"/*; do
    [ -e "$t" ] || continue
    pid=$(basename "$t" | sed 's/^[0-9]*-0*//')
    kill -0 "$pid" 2>/dev/null || rm -f "$t"
  done
  if [ -d "$L" ]; then
    opid=$(cut -d' ' -f1 "$L/owner" 2>/dev/null)
    [ -n "$opid" ] && ! kill -0 "$opid" 2>/dev/null && rm -rf "$L"
  fi
  head=$(ls "$Q" | sort | head -1)
  if [ "$Q/$head" = "$T" ] && mkdir "$L" 2>/dev/null; then break; fi
  sleep 5
done
echo "$$ $(date) $PWD $LANE" > "$L/owner"
rm -f "$T"
trap 'rm -rf "$L"' EXIT INT TERM
"$@"
