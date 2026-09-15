#!/bin/bash
# Manual fallback / one-off deploy. Normal deploys happen automatically via
# .github/workflows/cd.yml on every push to master (builds in CI, ships
# just the binary - see docs/deployment.md). This script still builds on
# the VPS itself, so it's slower and more memory-hungry than the CI path -
# useful for a first deploy or when you need to push a change without
# going through GitHub, not the everyday path.
set -e

cd terraform
CONTAINER_IP=$(terraform output -raw container_ip 2>/dev/null)

if [ -z "$CONTAINER_IP" ]; then
    echo "Error: Could not get container IP"
    exit 1
fi

echo "Updating application on $CONTAINER_IP..."

# Copy new files. --exclude '.env' is load-bearing, not cosmetic: without
# it this would overwrite the VPS's real production .env with whatever
# .env sits in the local working tree (a real file here in dev, per
# CLAUDE.md's Secrets note) on every run.
echo "Copying files..."
rsync -avz --delete \
    --exclude 'target' \
    --exclude '.git' \
    --exclude '.env' \
    ../backend/ root@$CONTAINER_IP:/opt/friends-calendar/

# Rebuild
echo "Building application..."
ssh root@$CONTAINER_IP << 'EOF'
cd /opt/friends-calendar
source $HOME/.cargo/env
cargo build --release
EOF

# Restart service
echo "Restarting service..."
ssh root@$CONTAINER_IP "systemctl restart friends-calendar"

echo "Update complete!"
echo "Check status: make status"