#!/bin/zsh
# Usage: scripts/aws/ssm.sh <instance-id> <shell command...>
# Runs a shell command on a dive AWS node through SSM and prints status, stdout
# and stderr. SSM truncates output at about 24 KB: fetch bigger files as
# gzip+base64 chunks (`... | gzip -c | base64 -w0 | cut -c<from>-<to>`).
# Needs AWS_PROFILE (an SSO profile for the dive account); AWS_REGION defaults
# to us-east-1.
: ${AWS_PROFILE:?set AWS_PROFILE}; export AWS_REGION=${AWS_REGION:-us-east-1} AWS_PAGER=""
I=$1; shift
P=$(jq -nc --arg c "$*" '{commands:[$c],executionTimeout:["3600"]}')
ID=$(aws ssm send-command --instance-ids $I --document-name AWS-RunShellScript --parameters "$P" --query Command.CommandId --output text) || exit 1
for i in $(seq 1 720); do
  S=$(aws ssm get-command-invocation --command-id $ID --instance-id $I --query Status --output text 2>/dev/null)
  case $S in Success|Failed|Cancelled|TimedOut) break;; esac; sleep 5
done
aws ssm get-command-invocation --command-id $ID --instance-id $I --query '[Status,StandardOutputContent,StandardErrorContent]' --output text
