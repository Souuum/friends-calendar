set -e

cd terraform
CONTAINER_IP=$(terraform output -raw container_ip 2>/dev/null)

if [ -z "$CONTAINER_IP" ]; then
    echo "Error: Could not get container IP"
    exit 1
fi

echo "Following logs from $CONTAINER_IP (Ctrl+C to exit)..."
ssh root@$CONTAINER_IP "journalctl -u friends-calendar -f"