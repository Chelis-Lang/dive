#!/bin/zsh
# Usage: DIVE_GATE_NODE=<instance-id> scripts/aws/remote-gate.sh <pushed-branch-or-sha> <fast|full>
# Runs `cargo xtask gate --<mode>` for a revision already pushed to origin on the gate
# node started by scripts/aws/gate-node.sh. Prints the gate report (and the log tail
# on failure) and exits with the gate's status; rc 99 means the checkout failed. A
# finished result is cached on the node, so rerunning the same revision and mode only
# reprints it. Push the branch first; a draft PR is fine. Label the report in PR
# bodies "full gate (remote Linux node)".
: ${AWS_PROFILE:?set AWS_PROFILE}; : ${DIVE_GATE_NODE:?set DIVE_GATE_NODE}
export AWS_REGION=${AWS_REGION:-us-east-1} AWS_PAGER=""
I=$DIVE_GATE_NODE; REF=$1; MODE=$2
case $MODE in fast|full) ;; *) echo "mode must be fast or full"; exit 2;; esac
if [[ $REF =~ '^[0-9a-f]{40}$' ]]; then SHA=$REF; else
  SHA=$(git ls-remote https://github.com/Chelis-Lang/dive.git "refs/heads/$REF" | cut -f1)
  [ -n "$SHA" ] || { echo "branch $REF not found on origin; push it first"; exit 2; }
fi
R=/home/gate/reports/$SHA-$MODE
run() { # run a short command on the node and print its stdout
  local P=$(jq -nc --arg c "$1" '{commands:[$c],executionTimeout:["600"]}')
  local ID=$(aws ssm send-command --instance-ids $I --document-name AWS-RunShellScript --parameters "$P" --query Command.CommandId --output text 2>/dev/null) || return 1
  local st; for i in $(seq 1 60); do st=$(aws ssm get-command-invocation --command-id $ID --instance-id $I --query Status --output text 2>/dev/null); case $st in Success|Failed|Cancelled|TimedOut) break;; esac; sleep 5; done
  aws ssm get-command-invocation --command-id $ID --instance-id $I --query StandardOutputContent --output text 2>/dev/null
}
until [ "$(run 'test -f /home/gate/SETUP_DONE && echo y')" = y ]; do echo "gate node still setting up"; sleep 120; done
echo "remote gate --$MODE for $SHA on $I"
run "test -f $R.rc || systemd-run --unit=gate-$SHA-$MODE --uid=gate --setenv=HOME=/home/gate --working-directory=/home/gate /home/gate/gate.sh $SHA $MODE >/dev/null 2>&1; echo queued" >/dev/null
until rc=$(run "cat $R.rc 2>/dev/null") && [ -n "$rc" ]; do sleep 120; done
rc=$(echo $rc | tr -d '[:space:]')
run "head -c 20000 $R.md 2>/dev/null"
[ "$rc" = 0 ] || { echo "--- log tail"; run "grep -vE '^\s+PASS ' $R.log | tail -c 12000"; }
echo "remote gate rc=$rc"
exit $rc
