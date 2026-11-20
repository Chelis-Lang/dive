#!/bin/sh
# Tests for scripts/gate-lock.sh: stale-lock reclaim and FIFO handoff.
# Usage: scripts/test-gate-lock.sh   (needs only a POSIX shell and ps)
set -u
here=$(cd "$(dirname "$0")" && pwd)
LOCK="$here/gate-lock.sh"
W=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$W"' EXIT
export DIVE_GATE_LOCK_POLL=1
fails=0

pass() { echo "ok - $1"; }
fail() { echo "not ok - $1"; fails=$((fails + 1)); }
# Run the lock in a fresh state directory with a time limit; prints nothing.
fresh() { D=$W/$1; mkdir -p "$D"; export DIVE_GATE_LOCK_DIR=$D; }
run_limited() { # <seconds> <command...>: run, kill after <seconds>, return its status
  limit=$1; shift
  "$@" & p=$!
  i=0
  while kill -0 "$p" 2>/dev/null && [ "$i" -lt "$((limit * 10))" ]; do sleep 0.1; i=$((i + 1)); done
  if kill -0 "$p" 2>/dev/null; then kill "$p" 2>/dev/null; wait "$p" 2>/dev/null; return 124; fi
  wait "$p"
}
dead_pid() { sh -c 'exit 0' & p=$!; wait "$p"; echo "$p"; }
start_of() { ps -o lstart= -p "$1" | tr -s ' \t' ' ' | sed 's/^ //; s/ $//'; }

# 1. Dead owner: the pid in the owner file no longer exists.
fresh dead
mkdir "$D/lock"
printf 'pid=%s\nstart=x\nacquired=%s\n' "$(dead_pid)" "$(date +%s)" > "$D/lock/owner"
if run_limited 10 "$LOCK" true 2> "$W/dead.err" && grep -q "is gone" "$W/dead.err" && [ ! -d "$D/lock" ]; then
  pass "dead owner is reclaimed"
else fail "dead owner is reclaimed"; cat "$W/dead.err"; fi

# 1b. Old-format owner file ("<pid> <date> <cwd> <lane>") with a dead pid.
fresh oldfmt
mkdir "$D/lock"
echo "$(dead_pid) $(date) /x heavy" > "$D/lock/owner"
if run_limited 10 "$LOCK" true 2>/dev/null; then pass "old-format dead owner is reclaimed"
else fail "old-format dead owner is reclaimed"; fi

# 2. Missing owner file: reclaimed once older than the grace period, not before.
fresh noowner
mkdir "$D/lock"
if run_limited 3 env DIVE_GATE_LOCK_GRACE=3600 "$LOCK" true 2>/dev/null; then
  fail "fresh owner-less lock is respected"
else pass "fresh owner-less lock is respected"; fi
touch -t 202001010000 "$D/lock"
if run_limited 10 env DIVE_GATE_LOCK_GRACE=60 "$LOCK" true 2> "$W/noowner.err" && grep -q "no owner file" "$W/noowner.err"; then
  pass "old owner-less lock is reclaimed"
else fail "old owner-less lock is reclaimed"; cat "$W/noowner.err"; fi

# 3. Reused pid: the pid is alive (this shell) but started at a different time.
fresh reused
mkdir "$D/lock"
printf 'pid=%s\nstart=Thu Jan 1 00:00:00 1970\nacquired=%s\n' $$ "$(date +%s)" > "$D/lock/owner"
if run_limited 10 "$LOCK" true 2> "$W/reused.err" && grep -q "was reused" "$W/reused.err"; then
  pass "reused pid is reclaimed"
else fail "reused pid is reclaimed"; cat "$W/reused.err"; fi

# 3b. A live owner with a matching start time is respected.
fresh live
mkdir "$D/lock"
printf 'pid=%s\nstart=%s\nacquired=%s\n' $$ "$(start_of $$)" "$(date +%s)" > "$D/lock/owner"
if run_limited 3 "$LOCK" true 2>/dev/null; then fail "live owner is respected"
else pass "live owner is respected"; fi

# 4. Max-age override reclaims a live owner and says so.
run_limited 10 env DIVE_GATE_LOCK_MAX_AGE=1 "$LOCK" true 2> "$W/maxage.err"
rc=$?
if [ "$rc" -eq 0 ] && grep -q "DIVE_GATE_LOCK_MAX_AGE" "$W/maxage.err"; then pass "max-age override reclaims"
else fail "max-age override reclaims"; cat "$W/maxage.err"; fi

# 5. Normal handoff in FIFO order, and the command's exit status is preserved.
fresh handoff
log=$W/handoff.log
"$LOCK" sh -c "echo A-start >> '$log'; sleep 3; echo A-end >> '$log'" 2>/dev/null & a=$!
sleep 1.5
"$LOCK" sh -c "echo B >> '$log'" 2>/dev/null & b=$!
sleep 1.5
"$LOCK" sh -c "echo C >> '$log'" 2>/dev/null & c=$!
wait "$a" "$b" "$c"
order=$(tr '\n' ' ' < "$log")
if [ "$order" = "A-start A-end B C " ] && [ ! -d "$D/lock" ] && [ -z "$(ls "$D/queue-heavy")" ]; then
  pass "handoff is FIFO and cleans up"
else fail "handoff is FIFO and cleans up (got: $order)"; fi
"$LOCK" sh -c 'exit 7' 2>/dev/null
if [ $? -eq 7 ]; then pass "exit status is preserved"; else fail "exit status is preserved"; fi

# 6. Fast and heavy lanes do not block each other.
fresh lanes
"$LOCK" sleep 4 & h=$!
sleep 1
if run_limited 3 "$LOCK" true --fast 2>/dev/null; then pass "fast lane is independent"
else fail "fast lane is independent"; fi
wait "$h"

[ "$fails" -eq 0 ] && echo "all gate-lock tests passed" || { echo "$fails gate-lock test(s) failed"; exit 1; }
