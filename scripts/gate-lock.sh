#!/bin/sh
# Usage: scripts/gate-lock.sh <command...>  -- serialises gate and corpus runs across the
# worktrees of one clone (state lives in the shared git directory, or DIVE_GATE_LOCK_DIR).
# Two lanes, each first-in first-out: `gate --fast` runs take the fast lane, everything
# else (full gates, corpus, shadow and harness runs) the heavy lane, so long runs never
# starve short fast gates. Waiters queue as ticket files named <epoch>-<pid>; only the
# queue head may reclaim or take the lock.
#
# A lock is stale, and the queue head reclaims it, when:
#   - its owner pid is gone, or the pid is alive but was started at a different time
#     (pid reuse; the owner file records `ps -o lstart=` of the owner);
#   - the lock directory has no owner file and is older than DIVE_GATE_LOCK_GRACE
#     seconds (default 60; a crash between mkdir and writing the owner file);
#   - DIVE_GATE_LOCK_MAX_AGE is set and the lock was taken more than that many seconds
#     ago, even if its owner is alive (an explicit override; it says so on stderr).
# DIVE_GATE_LOCK_POLL sets the wait interval in seconds (default 5).
D=${DIVE_GATE_LOCK_DIR:-$(git rev-parse --path-format=absolute --git-common-dir)/dive-gate-lock}; mkdir -p "$D"
case " $* " in *" --fast "*) LANE=fast; L=$D/lock-fast ;; *) LANE=heavy; L=$D/lock ;; esac
Q=$D/queue-$LANE
GRACE=${DIVE_GATE_LOCK_GRACE:-60}
MAX_AGE=${DIVE_GATE_LOCK_MAX_AGE:-}
POLL=${DIVE_GATE_LOCK_POLL:-5}
mkdir -p "$Q"

say() { echo "gate-lock: $*" >&2; }
# Start time of a pid, whitespace-normalised; empty when the pid does not exist.
pid_start() { ps -o lstart= -p "$1" 2>/dev/null | tr -s ' \t' ' ' | sed 's/^ //; s/ $//'; }
# A pid is alive when it exists and, if a start time was recorded, still matches it.
pid_alive() {
  kill -0 "$1" 2>/dev/null || return 1
  [ -z "$2" ] || [ "$(pid_start "$1")" = "$2" ]
}
# Modification time in epoch seconds (GNU stat first: BSD `stat -f` means something else there).
mtime() { stat -c %Y "$1" 2>/dev/null || stat -f %m "$1" 2>/dev/null; }
field() { sed -n "s/^$1=//p" "$2" 2>/dev/null | head -1; }

ME_START=$(pid_start $$)
T="$Q/$(date +%s)-$(printf '%010d' $$)"
printf '%s\n' "$ME_START" > "$T"
trap 'rm -f "$T"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Prints why the lock is stale and returns 0, or returns 1 when it is held.
lock_stale() {
  o=$L/owner
  if [ ! -f "$o" ]; then
    m=$(mtime "$L")
    case $m in ''|*[!0-9]*) return 1 ;; esac
    age=$(( $(date +%s) - m ))
    [ "$age" -gt "$GRACE" ] || return 1
    echo "lock directory has no owner file and is ${age}s old (grace ${GRACE}s)"; return 0
  fi
  opid=$(field pid "$o"); ostart=$(field start "$o"); oat=$(field acquired "$o")
  # Owner files from older versions of this script: "<pid> <date> <cwd> <lane>".
  [ -n "$opid" ] || opid=$(head -1 "$o" | cut -d' ' -f1)
  case $opid in ''|*[!0-9]*) echo "owner file names no pid"; return 0 ;; esac
  if ! kill -0 "$opid" 2>/dev/null; then echo "owner pid $opid is gone"; return 0; fi
  if [ -n "$ostart" ] && [ "$(pid_start "$opid")" != "$ostart" ]; then
    echo "owner pid $opid was reused (recorded start '$ostart', now '$(pid_start "$opid")')"; return 0
  fi
  case $MAX_AGE in ''|*[!0-9]*) return 1 ;; esac
  case $oat in ''|*[!0-9]*) return 1 ;; esac
  age=$(( $(date +%s) - oat ))
  [ "$age" -gt "$MAX_AGE" ] || return 1
  echo "owner pid $opid is alive but has held the lock for ${age}s, over DIVE_GATE_LOCK_MAX_AGE=${MAX_AGE}s"
}

# Move the stale lock aside, then check that it is still the one judged stale, so a
# racing waiter never removes a lock that was just taken.
reclaim() {
  seen=$(cat "$L/owner" 2>/dev/null)
  G=$D/.reclaim-$$-$(date +%s)
  mv "$L" "$G" 2>/dev/null || return 0
  if [ "$(cat "$G/owner" 2>/dev/null)" = "$seen" ]; then
    rm -rf "$G"; say "reclaimed $LANE lock: $1"
  else
    mv "$G" "$L" 2>/dev/null || rm -rf "$G"
  fi
}

waited=
while :; do
  for t in "$Q"/*; do
    [ -e "$t" ] || continue
    [ "$t" = "$T" ] && continue
    pid=$(basename "$t" | sed 's/^[0-9]*-0*//')
    pid_alive "$pid" "$(head -1 "$t" 2>/dev/null)" || rm -f "$t"
  done
  head=$(ls "$Q" | sort | head -1)
  if [ "$Q/$head" = "$T" ]; then
    if [ -d "$L" ] && why=$(lock_stale); then reclaim "$why"; fi
    if mkdir "$L" 2>/dev/null; then break; fi
  fi
  if [ -z "$waited" ]; then
    waited=1; say "waiting for the $LANE lock ($(tr '\n' ' ' < "$L/owner" 2>/dev/null))"
  fi
  sleep "$POLL"
done
printf 'pid=%s\nstart=%s\nacquired=%s\ndate=%s\ncwd=%s\nlane=%s\n' \
  $$ "$ME_START" "$(date +%s)" "$(date)" "$PWD" "$LANE" > "$L/owner.tmp"
mv "$L/owner.tmp" "$L/owner"
OWNER=$(cat "$L/owner")
rm -f "$T"
# Release only a lock that is still ours: a max-age override may have taken it over.
release() { [ "$(cat "$L/owner" 2>/dev/null)" = "$OWNER" ] && rm -rf "$L"; }
trap 'release' EXIT
"$@"
