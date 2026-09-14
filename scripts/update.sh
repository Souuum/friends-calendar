#!/bin/bash
set -e

cd terraform
CONTAINER_IP=$(terraform output -raw container_ip 2>/dev/null)

if [ -z "$CONTAINER_IP" ]; then
    echo "Error: Could not get container IP"
    exit 1
fi

echo "Updating application on $CONTAINER_IP..."

# Copy new files
echo "Copying files..."
rsync -avz --delete \
    --exclude 'target' \
    --exclude '.git' \
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