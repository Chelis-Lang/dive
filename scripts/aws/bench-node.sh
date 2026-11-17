#!/bin/zsh
# Usage: scripts/aws/bench-node.sh <rev> <dive-bench arguments...>
# Launches a quiet x86 bench node (c7i.metal-24xl, SMT and turbo off, 2.4 GHz,
# pinned to CPUs 0-15, see docs/developer/core/09-performance-method.md §2).
# It builds <rev> (pushed to origin), fetches the oracle and corpora, writes
# ~/bins/dive plus the wrappers ~/bins/dive-core (DIVE_ENGINE=core),
# ~/bins/core-memo and ~/bins/core-nomemo, then runs
#   dive-bench <arguments> --runs ~/out/runs.jsonl --summary ~/out/summary.json
# as user `bench`, and touches ~/out/DONE. Prints the instance id.
# Example (core vs reference vs TLC):
#   scripts/aws/bench-node.sh <sha> --dive-bin ~/bins/dive-core --dive-revision <sha>+core \
#     --baseline-bin ~/bins/dive --baseline-revision <sha>+reference \
#     --profile default --workers 1,8 --reps 5 --tlc --timeout 900 --xmx 3g
# Fetch with scripts/aws/ssm.sh (as /home/bench/out/...), then terminate the node.
# DRY_RUN=1 prints the generated user data instead of launching.
# Required environment: AWS_PROFILE, DIVE_AWS_AMI (AMI id), DIVE_AWS_INSTANCE_PROFILE
# (IAM instance profile) and DIVE_AWS_GH_TOKEN_PARAM (SSM SecureString parameter
# name holding a GitHub token). The token is never printed.
: ${AWS_PROFILE:?set AWS_PROFILE}
: ${DIVE_AWS_AMI:?set DIVE_AWS_AMI to an Ubuntu x86_64 AMI id}
: ${DIVE_AWS_INSTANCE_PROFILE:?set DIVE_AWS_INSTANCE_PROFILE to an IAM instance profile that can read the token parameter}
: ${DIVE_AWS_GH_TOKEN_PARAM:?set DIVE_AWS_GH_TOKEN_PARAM to the SSM SecureString parameter holding a GitHub token}
export AWS_REGION=${AWS_REGION:-us-east-1} AWS_PAGER=""
REV=$1; shift
ARGS="$*"  # arguments must not contain spaces; ~ expands on the node
UD=$(mktemp)
cat > $UD <<OUTER
#!/bin/bash
REV=$REV
AWS_REGION=${(q)AWS_REGION}
TOKEN_PARAM=${(q)DIVE_AWS_GH_TOKEN_PARAM}
OUTER
cat >> $UD <<'OUTER'
exec > /var/log/dive-bootstrap.log 2>&1
set -eu
export DEBIAN_FRONTEND=noninteractive HOME=/root
apt-get update -q && apt-get install -yq build-essential pkg-config git curl jq time numactl unzip
echo off > /sys/devices/system/cpu/smt/control || true
echo 1 > /sys/devices/system/cpu/intel_pstate/no_turbo || true
for c in /sys/devices/system/cpu/cpu[0-9]*; do
  [ -f $c/cpufreq/scaling_governor ] && echo performance > $c/cpufreq/scaling_governor || true
  [ -f $c/cpufreq/scaling_max_freq ] && echo 2400000 > $c/cpufreq/scaling_max_freq || true
  [ -f $c/cpufreq/scaling_min_freq ] && echo 2400000 > $c/cpufreq/scaling_min_freq || true
done
systemctl stop snapd unattended-upgrades apt-daily.timer apt-daily-upgrade.timer || true
(cd /tmp && curl -sSfo awscliv2.zip https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip && unzip -qo awscliv2.zip && ./aws/install --update)
useradd -m -s /bin/bash bench || true
/usr/local/bin/aws ssm get-parameter --region "$AWS_REGION" --name "$TOKEN_PARAM" --with-decryption --query Parameter.Value --output text > /home/bench/.gh-token
chmod 600 /home/bench/.gh-token; chown bench: /home/bench/.gh-token
cat > /home/bench/run.sh <<'INNER'
#!/bin/bash
set -eu
cd ~
curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.0 --profile minimal >/dev/null
source ~/.cargo/env
T=$(cat ~/.gh-token)
git clone -q https://x-access-token:$T@github.com/Chelis-Lang/dive.git ~/dive
cd ~/dive && git checkout -q "$1"; shift
GH_TOKEN=$T cargo run -q -p dive-conformance --release -- oracle fetch >/dev/null
GH_TOKEN=$T cargo run -q -p dive-conformance --release -- corpus fetch >/dev/null
cargo build -q --release --locked -p dive-cli -p dive-bench
mkdir -p ~/bins ~/out
cp target/release/dive ~/bins/dive && cp target/release/dive-bench ~/bins/dive-bench
w() { printf '#!/bin/sh\n%s exec %s/bins/dive "$@"\n' "$2" "$HOME" > ~/bins/$1; chmod +x ~/bins/$1; }
w dive-core "DIVE_ENGINE=core"; w core-memo "DIVE_ENGINE=core DIVE_CORE_OPT=+memo"; w core-nomemo "DIVE_ENGINE=core DIVE_CORE_OPT=-memo"
touch ~/BOOTSTRAP_DONE
eval "set -- $*"
taskset -c 0-15 numactl --membind=0 ~/bins/dive-bench "$@" \
  --runs ~/out/runs.jsonl --summary ~/out/summary.json > ~/out/console.txt 2>&1 || true
touch ~/out/DONE
INNER
chmod 755 /home/bench/run.sh; chown bench: /home/bench/run.sh
OUTER
print -r -- "systemd-run --unit=dive-bench --uid=bench --setenv=HOME=/home/bench --working-directory=/home/bench /home/bench/run.sh \"\$REV\" ${(q)ARGS}" >> $UD
[ -n "${DRY_RUN:-}" ] && { cat $UD; rm -f $UD; exit 0; }
aws ec2 run-instances --image-id "$DIVE_AWS_AMI" --instance-type c7i.metal-24xl \
  --iam-instance-profile Name="$DIVE_AWS_INSTANCE_PROFILE" \
  --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=100,VolumeType=gp3}' \
  --user-data file://$UD \
  --tag-specifications 'ResourceType=instance,Tags=[{Key=project,Value=dive},{Key=Name,Value=dive-bench}]' \
  --query 'Instances[0].InstanceId' --output text
rm -f $UD
