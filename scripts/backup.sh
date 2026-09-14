set -e

BACKUP_DIR="./backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

cd terraform
CONTAINER_ID=$(terraform output -raw container_id 2>/dev/null)
PROXMOX_HOST=$(terraform output -json | jq -r '.proxmox_host.value // empty')

if [ -z "$CONTAINER_ID" ]; then
    echo "Error: Could not get container ID"
    exit 1
fi

mkdir -p $BACKUP_DIR

echo "Creating backup of container $CONTAINER_ID..."


ssh root@$PROXMOX_HOST << EOF
vzdump $CONTAINER_ID --mode snapshot --compress zstd --dumpdir /tmp
EOF

echo "Backup complete!"
echo "Location: Proxmox host /tmp/"