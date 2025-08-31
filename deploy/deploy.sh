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

# Setup VM
echo "🔧 Setting up VM..."
scp ./deploy/setup-vm.sh "$SERVER":~/setup-vm.sh
ssh "$SERVER" "chmod +x ~/setup-vm.sh && ~/setup-vm.sh"

# Transfer files to GCP VM
./deploy/transfer-files.sh "$SERVER"

# Deploy services
echo "🔧 Deploying services..."
scp ./deploy/deploy-services.sh "$SERVER":~/deploy-services.sh
ssh "$SERVER" "chmod +x ~/deploy-services.sh && ~/deploy-services.sh"

echo "🎉 Deployment complete!"
