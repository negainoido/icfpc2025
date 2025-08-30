#!/bin/bash

# GCP VM deployment script for ICFPC 2025
set -euxo pipefail

cd "$(dirname "$0")/.."

# Configuration
DEPLOY_USER="$USER"
SERVER="${SERVER:-negainoido}"

echo "🚀 Starting deployment to GCP VM..."

# Build API Server
echo "📦 Building API Server..."
cd api-server
cargo build --release
cd ..

# Build WebApp
echo "📦 Building WebApp..."
cd webapp
npm install
npm run build
cd ..

# Transfer files to GCP VM
echo "📤 Transferring files to GCP VM..."

# Create directories and webapp user on VM
ssh "$SERVER" "
    # Create webapp system user if it doesn't exist
    if ! id webapp &>/dev/null; then
        sudo useradd --system --no-create-home --shell /usr/sbin/nologin webapp
    fi
    mkdir ~/{api-server,webapp}
    
    # Create directories
    sudo mkdir -p /opt/icfpc2025/{api-server,webapp}
    sudo chown -R webapp:webapp /opt/icfpc2025
"

# Transfer API Server binary
scp api-server/target/release/icfpc2025-api-server "$SERVER":~/api-server/

# Transfer API Server resources using rsync for efficiency
ssh "$SERVER" "mkdir -p ~/api-server"
rsync -avz --delete api-server/resources/ "$SERVER":~/api-server/resources/

# Transfer WebApp build using rsync for efficiency
ssh "$SERVER" "mkdir -p ~/webapp"
rsync -avz --delete -e "ssh -o StrictHostKeyChecking=no" \
    webapp/dist/ "$SERVER":~/webapp/dist/

# Transfer configuration files
scp ./deploy/nginx.conf "$SERVER":~/nginx.conf

scp ./deploy/icfpc2025-api.service "$SERVER":~/icfpc2025-api.service

scp ./deploy/.env.production "$SERVER":~/api-server/.env

# Setup services on VM
echo "🔧 Setting up services on VM..."
ssh "$SERVER" "
    # Move resources
    sudo rsync -avr ~/api-server /opt/icfpc2025/
    sudo rsync -avr ~/webapp /opt/icfpc2025/

    # Ensure proper ownership of all transferred files
    sudo mkdir -p /opt/icfpc2025
    sudo chown -R webapp:webapp /opt/icfpc2025


    # Make binary executable
    chmod +x /opt/icfpc2025/api-server/icfpc2025-api-server
    
    # Setup nginx
    sudo cp ~/nginx.conf /etc/nginx/sites-available/icfpc2025.conf
    sudo ln -sf /etc/nginx/sites-available/icfpc2025.conf /etc/nginx/sites-enabled/
    sudo rm -f /etc/nginx/sites-enabled/default
    sudo nginx -t && sudo systemctl reload nginx
    
    # Setup systemd service
    sudo cp ~/icfpc2025-api.service /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable icfpc2025-api
    sudo systemctl restart icfpc2025-api
    
    # Check status
    echo '✅ Service status:'
    sudo systemctl status icfpc2025-api --no-pager
    echo '✅ Nginx status:'
    sudo systemctl status nginx --no-pager
"

echo "🎉 Deployment complete!"
