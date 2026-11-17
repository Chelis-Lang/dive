#!/bin/zsh
# Usage: scripts/aws/gate-node.sh
# Launches a Linux gate node (c7i.16xlarge) that runs `cargo xtask gate --fast|--full`
# for revisions pushed to origin, one at a time, each in a fresh worktree and target
# directory that is removed afterwards. Use it when local gates are slow, for example
# while macOS scans every newly linked test binary. Drive it with
# scripts/aws/remote-gate.sh. Prints the instance id; export it as DIVE_GATE_NODE.
# Setup takes about 10 minutes. Terminate the node when done.
# DRY_RUN=1 prints the generated user data instead of launching.
# Required environment: AWS_PROFILE, DIVE_AWS_AMI (AMI id), DIVE_AWS_INSTANCE_PROFILE
# (IAM instance profile) and DIVE_AWS_GH_TOKEN_PARAM (SSM SecureString parameter
# name holding a GitHub token). The token is never printed.
: ${AWS_PROFILE:?set AWS_PROFILE}
: ${DIVE_AWS_AMI:?set DIVE_AWS_AMI to an Ubuntu x86_64 AMI id}
: ${DIVE_AWS_INSTANCE_PROFILE:?set DIVE_AWS_INSTANCE_PROFILE to an IAM instance profile that can read the token parameter}
: ${DIVE_AWS_GH_TOKEN_PARAM:?set DIVE_AWS_GH_TOKEN_PARAM to the SSM SecureString parameter holding a GitHub token}
export AWS_REGION=${AWS_REGION:-us-east-1} AWS_PAGER=""
UD=$(mktemp)
cat > $UD <<OUTER
#!/bin/bash
AWS_REGION=${(q)AWS_REGION}
TOKEN_PARAM=${(q)DIVE_AWS_GH_TOKEN_PARAM}
OUTER
cat >> $UD <<'OUTER'
exec > /var/log/dive-bootstrap.log 2>&1
set -eu
export DEBIAN_FRONTEND=noninteractive HOME=/root
apt-get update -q && apt-get install -yq build-essential pkg-config git curl jq unzip python3
systemctl stop snapd unattended-upgrades apt-daily.timer apt-daily-upgrade.timer || true
(cd /tmp && curl -sSfo awscliv2.zip https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip && unzip -qo awscliv2.zip && ./aws/install --update)
useradd -m -s /bin/bash gate || true
/usr/local/bin/aws ssm get-parameter --region "$AWS_REGION" --name "$TOKEN_PARAM" --with-decryption --query Parameter.Value --output text > /home/gate/.gh-token
chmod 600 /home/gate/.gh-token; chown gate: /home/gate/.gh-token
cat > /home/gate/setup.sh <<'INNER'
#!/bin/bash
set -eu
cd ~
curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.0 --profile minimal -c clippy,rustfmt >/dev/null
source ~/.cargo/env
curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ~/.cargo/bin
cargo install -q --locked cargo-deny
T=$(cat ~/.gh-token)
git clone -q https://x-access-token:$T@github.com/Chelis-Lang/dive.git ~/dive
cd ~/dive
GH_TOKEN=$T cargo run -q -p dive-conformance --release -- oracle fetch >/dev/null
GH_TOKEN=$T cargo run -q -p dive-conformance --release -- corpus fetch >/dev/null
mkdir -p ~/reports ~/wt
touch ~/SETUP_DONE
INNER
cat > /home/gate/gate.sh <<'INNER'
#!/bin/bash
# gate.sh <sha> <fast|full>: writes ~/reports/<sha>-<mode>.{log,md,rc}; rc is written last.
set -u
source ~/.cargo/env
SHA=$1; MODE=$2; R=~/reports/$SHA-$MODE
exec 9>~/gate.lock; flock 9
W=~/wt/$SHA
cleanup() { cd ~; rm -rf "$W"; git -C ~/dive worktree prune; }
trap cleanup EXIT
rm -rf ~/wt/*; git -C ~/dive worktree prune
cd ~/dive && git fetch -q origin '+refs/heads/*:refs/remotes/origin/*' '+refs/pull/*/head:refs/remotes/pr/*'
git worktree add -q --detach "$W" "$SHA" > $R.log 2>&1 || { echo 99 > $R.rc; exit 0; }
cd "$W"
export CARGO_TARGET_DIR=$W/target GH_TOKEN=$(cat ~/.gh-token)
cargo xtask gate --$MODE >> $R.log 2>&1; rc=$?
cp $W/target/gate-report.md $R.md 2>/dev/null || sed -n '/^### .cargo xtask gate/,$p' $R.log > $R.md
echo $rc > $R.rc
INNER
chmod 755 /home/gate/setup.sh /home/gate/gate.sh; chown gate: /home/gate/setup.sh /home/gate/gate.sh
systemd-run --unit=dive-gate-setup --uid=gate --setenv=HOME=/home/gate --working-directory=/home/gate /home/gate/setup.sh
OUTER
[ -n "${DRY_RUN:-}" ] && { cat $UD; rm -f $UD; exit 0; }
aws ec2 run-instances --image-id "$DIVE_AWS_AMI" --instance-type c7i.16xlarge \
  --iam-instance-profile Name="$DIVE_AWS_INSTANCE_PROFILE" \
  --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=200,VolumeType=gp3}' \
  --user-data file://$UD \
  --tag-specifications 'ResourceType=instance,Tags=[{Key=project,Value=dive},{Key=Name,Value=dive-gate}]' \
  --query 'Instances[0].InstanceId' --output text
rm -f $UD
