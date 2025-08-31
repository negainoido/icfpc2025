#!/bin/bash
# File transfer script for deployment

set -euo pipefail

# Check required parameters
if [ $# -ne 1 ]; then
    echo "Usage: $0 <server>"
    echo "Example: $0 negainoido"
    exit 1
fi

SERVER="$1"
SCRIPT_DIR="$(dirname "$0")"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "📤 Transferring files to GCP VM..."

# Transfer API Server binary
echo "Transferring API Server binary..."
scp "$PROJECT_ROOT/api-server/target/release/icfpc2025-api-server" "$SERVER":~/api-server/

# Transfer API Server resources using rsync for efficiency
echo "Transferring API Server resources..."
ssh "$SERVER" "mkdir -p ~/api-server"
rsync -avz --delete -e "ssh -o StrictHostKeyChecking=no" \
    "$PROJECT_ROOT/api-server/resources/" "$SERVER":~/api-server/resources/

# Transfer WebApp build using rsync for efficiency
echo "Transferring WebApp build..."
ssh "$SERVER" "mkdir -p ~/webapp"
rsync -avz --delete -e "ssh -o StrictHostKeyChecking=no" \
    "$PROJECT_ROOT/webapp/dist/" "$SERVER":~/webapp/dist/

# Transfer configuration files
echo "Transferring configuration files..."
scp "$SCRIPT_DIR/nginx.conf" "$SERVER":~/nginx.conf
scp "$SCRIPT_DIR/icfpc2025-api.service" "$SERVER":~/icfpc2025-api.service
scp "$SCRIPT_DIR/.env.production" "$SERVER":~/api-server/.env

echo "✅ File transfer completed"