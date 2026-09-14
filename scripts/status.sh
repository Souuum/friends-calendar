#!/bin/bash
set -e

cd terraform
CONTAINER_IP=$(terraform output -raw container_ip 2>/dev/null)

if [ -z "$CONTAINER_IP" ]; then
    echo "Error: Could not get container IP"
    exit 1
fi

echo "=========================================="
echo "Service Status"
echo "=========================================="
ssh root@$CONTAINER_IP "systemctl status friends-calendar"

echo ""
echo "=========================================="
echo "Recent Logs"
echo "=========================================="
ssh root@$CONTAINER_IP "journalctl -u friends-calendar -n 20 --no-pager"