#!/bin/bash
# User data for the Sail Linux measurement node (S1-S6). Driven through SSM only.
# Prepend three lines before passing this as EC2 user data (as scripts/aws/bench-node.sh does):
#   AWS_REGION=us-east-1  TOKEN_PARAM=<SSM SecureString holding a GitHub token>  SAIL_REF=<sail ref>
# The 0.1.0 runs used SAIL_REF=origin/feat/native-temporal-dive-0.1.
exec > /var/log/sail-bootstrap.log 2>&1
set -eu
: "${AWS_REGION:?}" "${TOKEN_PARAM:?}" "${SAIL_REF:?}"
export DEBIAN_FRONTEND=noninteractive HOME=/root
apt-get update -q && apt-get install -yq build-essential pkg-config libssl-dev git curl jq time numactl unzip python3 rsync
systemctl stop snapd unattended-upgrades apt-daily.timer apt-daily-upgrade.timer || true
(cd /tmp && curl -sSfo awscliv2.zip https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip && unzip -qo awscliv2.zip && ./aws/install --update)
useradd -m -s /bin/bash bench || true
/usr/local/bin/aws ssm get-parameter --region "$AWS_REGION" --name "$TOKEN_PARAM" --with-decryption --query Parameter.Value --output text > /home/bench/.gh-token
chmod 600 /home/bench/.gh-token; chown bench: /home/bench/.gh-token
cat > /home/bench/setup.sh <<'INNER'
#!/bin/bash
set -eu
cd ~
T=$(cat ~/.gh-token)
git config --global url."https://x-access-token:$T@github.com/Chelis-Lang/dive".insteadOf "https://github.com/Chelis-Lang/dive"
git clone -q https://github.com/Chelis-Lang/sail.git ~/sail
cd ~/sail && git checkout -q "$1"
TC=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "$TC" --profile minimal >/dev/null
source ~/.cargo/env
export CARGO_NET_GIT_FETCH_WITH_CLI=true
cargo build -q --release --locked -p sail-solve --bins
git clone -q https://github.com/tlaplus/Examples.git ~/examples && git -C ~/examples checkout -q 6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f
git clone -q https://github.com/Chelis-Lang/dive.git ~/dive
touch ~/SETUP_DONE
INNER
chmod 755 /home/bench/setup.sh; chown bench: /home/bench/setup.sh
systemd-run --unit=sail-setup --uid=bench --setenv=HOME=/home/bench --working-directory=/home/bench /home/bench/setup.sh "$SAIL_REF"
