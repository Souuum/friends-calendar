#!/bin/bash
set -e

cd terraform
CONTAINER_IP=$(terraform output -raw container_ip 2>/dev/null)

if [ -z "$CONTAINER_IP" ]; then
    echo "Error: Could not get container IP. Is infrastructure deployed?"
    exit 1
fi

echo "Connecting to $CONTAINER_IP..."
ssh root@$CONTAINER_IP