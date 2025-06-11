#!/bin/bash
set -euo pipefail

# Script for building and deploying 9sdk-enclave to AWS Nitro Enclaves
# This script automates the entire deployment process with minimal manual intervention

# Configuration
ENCLAVE_NAME="9sdk-enclave"
ENCLAVE_CID=${ENCLAVE_CID:-16}  # Default CID for the enclave
VSOCK_PORT=${VSOCK_PORT:-5005}  # Default vsock port
MEMORY_MB=${MEMORY_MB:-512}     # Memory allocation for enclave
CPU_COUNT=${CPU_COUNT:-2}       # Number of CPUs for enclave

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running on an EC2 instance with Nitro Enclave support
check_nitro_support() {
    log_info "Checking Nitro Enclave support..."
    
    if ! command -v nitro-cli &> /dev/null; then
        log_error "nitro-cli not found. Please install AWS Nitro Enclaves CLI."
        log_info "Run: sudo amazon-linux-extras install aws-nitro-enclaves-cli"
        exit 1
    fi
    
    # Check if Nitro Enclaves is enabled
    if ! nitro-cli describe-enclaves &> /dev/null; then
        log_error "Nitro Enclaves not enabled. Please configure the instance."
        log_info "Run: sudo usermod -aG ne $USER && sudo systemctl start nitro-enclaves-allocator.service"
        exit 1
    fi
    
    log_info "Nitro Enclave support verified ✓"
}

# Build the Docker image for the enclave
build_docker_image() {
    log_info "Building Docker image for enclave..."
    
    # Build the enclave Docker image
    docker build -f Dockerfile.enclave -t ${ENCLAVE_NAME}:latest . || {
        log_error "Failed to build Docker image"
        exit 1
    }
    
    log_info "Docker image built successfully ✓"
}

# Convert Docker image to Enclave Image File (EIF)
build_enclave_image() {
    log_info "Converting Docker image to Enclave Image File..."
    
    # Create output directory
    mkdir -p build
    
    # Build EIF
    nitro-cli build-enclave \
        --docker-uri ${ENCLAVE_NAME}:latest \
        --output-file build/${ENCLAVE_NAME}.eif > build/enclave-build.json || {
        log_error "Failed to build enclave image"
        exit 1
    }
    
    # Extract PCR values for attestation
    PCR0=$(jq -r '.Measurements.PCR0' build/enclave-build.json)
    PCR1=$(jq -r '.Measurements.PCR1' build/enclave-build.json)
    PCR2=$(jq -r '.Measurements.PCR2' build/enclave-build.json)
    
    log_info "Enclave image built successfully ✓"
    log_info "PCR0: $PCR0"
    log_info "PCR1: $PCR1"
    log_info "PCR2: $PCR2"
    
    # Save PCR values for KMS policy
    cat > build/pcr-values.json <<EOF
{
    "PCR0": "$PCR0",
    "PCR1": "$PCR1",
    "PCR2": "$PCR2"
}
EOF
}

# Terminate any existing enclaves
terminate_existing_enclaves() {
    log_info "Checking for existing enclaves..."
    
    # Get list of running enclaves
    ENCLAVE_IDS=$(nitro-cli describe-enclaves | jq -r '.[] | .EnclaveID' 2>/dev/null || echo "")
    
    if [ -n "$ENCLAVE_IDS" ]; then
        for ENCLAVE_ID in $ENCLAVE_IDS; do
            log_warn "Terminating existing enclave: $ENCLAVE_ID"
            nitro-cli terminate-enclave --enclave-id "$ENCLAVE_ID" || true
        done
        sleep 2
    fi
}

# Run the enclave
run_enclave() {
    log_info "Starting the enclave..."
    
    # Start the enclave with vsock enabled
    ENCLAVE_ID=$(nitro-cli run-enclave \
        --cpu-count ${CPU_COUNT} \
        --memory ${MEMORY_MB} \
        --enclave-cid ${ENCLAVE_CID} \
        --eif-path build/${ENCLAVE_NAME}.eif \
        --debug-mode | jq -r '.EnclaveID')
    
    if [ "$ENCLAVE_ID" = "null" ] || [ -z "$ENCLAVE_ID" ]; then
        log_error "Failed to start enclave"
        exit 1
    fi
    
    log_info "Enclave started successfully ✓"
    log_info "Enclave ID: $ENCLAVE_ID"
    log_info "Enclave CID: $ENCLAVE_CID"
    log_info "VSock Port: $VSOCK_PORT"
    
    # Save enclave info
    cat > build/enclave-info.json <<EOF
{
    "EnclaveID": "$ENCLAVE_ID",
    "EnclaveCID": $ENCLAVE_CID,
    "VSockPort": $VSOCK_PORT,
    "MemoryMB": $MEMORY_MB,
    "CPUCount": $CPU_COUNT
}
EOF
}

# Setup vsock proxy for KMS (if needed)
setup_kms_proxy() {
    log_info "Setting up KMS proxy..."
    
    # Check if vsock-proxy is installed
    if ! command -v vsock-proxy &> /dev/null; then
        log_warn "vsock-proxy not found. KMS operations may not work."
        log_info "Install with: sudo yum install aws-nitro-enclaves-cli-devel"
        return
    fi
    
    # Start vsock proxy for KMS
    vsock-proxy 8000 kms.us-east-1.amazonaws.com 443 &
    PROXY_PID=$!
    
    log_info "KMS proxy started (PID: $PROXY_PID)"
    echo $PROXY_PID > build/kms-proxy.pid
}

# Create systemd service for automatic startup
create_systemd_service() {
    log_info "Creating systemd service for enclave..."
    
    cat > /tmp/${ENCLAVE_NAME}.service <<EOF
[Unit]
Description=9SDK Nitro Enclave
After=network.target nitro-enclaves-allocator.service

[Service]
Type=simple
ExecStartPre=/usr/bin/nitro-cli terminate-enclave --all
ExecStart=/usr/bin/nitro-cli run-enclave \\
    --cpu-count ${CPU_COUNT} \\
    --memory ${MEMORY_MB} \\
    --enclave-cid ${ENCLAVE_CID} \\
    --eif-path /opt/${ENCLAVE_NAME}/${ENCLAVE_NAME}.eif \\
    --debug-mode
ExecStop=/usr/bin/nitro-cli terminate-enclave --all
Restart=on-failure
User=root

[Install]
WantedBy=multi-user.target
EOF
    
    # Install the service
    sudo cp /tmp/${ENCLAVE_NAME}.service /etc/systemd/system/
    sudo mkdir -p /opt/${ENCLAVE_NAME}
    sudo cp build/${ENCLAVE_NAME}.eif /opt/${ENCLAVE_NAME}/
    
    log_info "Systemd service created ✓"
    log_info "Enable with: sudo systemctl enable ${ENCLAVE_NAME}"
    log_info "Start with: sudo systemctl start ${ENCLAVE_NAME}"
}

# Update parent application environment
update_parent_env() {
    log_info "Updating parent application environment..."
    
    # Create or update .env file for the parent application
    cat >> .env <<EOF

# Nitro Enclave Configuration
USE_VSOCK=true
ENCLAVE_CID=${ENCLAVE_CID}
VSOCK_PORT=${VSOCK_PORT}
EOF
    
    log_info "Environment updated ✓"
}

# Test vsock connection
test_vsock_connection() {
    log_info "Testing vsock connection..."
    
    # Create a simple test script
    cat > build/test-vsock.py <<'EOF'
#!/usr/bin/env python3
import socket
import sys

try:
    cid = int(sys.argv[1])
    port = int(sys.argv[2])
    
    # Create vsock socket
    sock = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    sock.settimeout(5)
    
    print(f"Connecting to CID {cid} port {port}...")
    sock.connect((cid, port))
    print("Connection successful!")
    
    sock.close()
except Exception as e:
    print(f"Connection failed: {e}")
    sys.exit(1)
EOF
    
    chmod +x build/test-vsock.py
    
    # Test the connection
    if python3 build/test-vsock.py ${ENCLAVE_CID} ${VSOCK_PORT}; then
        log_info "VSock connection test passed ✓"
    else
        log_warn "VSock connection test failed. The enclave might still be starting up."
    fi
}

# Main deployment process
main() {
    log_info "Starting Nitro Enclave deployment for ${ENCLAVE_NAME}"
    
    # Check if running with --local flag for local Docker testing
    if [ "${1:-}" = "--local" ]; then
        log_info "Running in local Docker mode (TCP)"
        build_docker_image
        
        # Run with TCP for local testing
        docker run -d \
            --name ${ENCLAVE_NAME} \
            -p 5005:5005 \
            -e RUST_LOG=debug \
            -e USE_VSOCK=false \
            -e TCP_ADDRESS=0.0.0.0:5005 \
            ${ENCLAVE_NAME}:latest
        
        log_info "Local Docker container started"
        exit 0
    fi
    
    # Full Nitro Enclave deployment
    check_nitro_support
    build_docker_image
    build_enclave_image
    terminate_existing_enclaves
    run_enclave
    setup_kms_proxy
    create_systemd_service
    update_parent_env
    
    # Wait a bit for enclave to fully start
    sleep 3
    
    test_vsock_connection
    
    log_info "Deployment completed successfully! 🎉"
    log_info ""
    log_info "Next steps:"
    log_info "1. Update your KMS key policy with the PCR values from build/pcr-values.json"
    log_info "2. Restart your parent application to use vsock"
    log_info "3. Monitor enclave logs with: nitro-cli console --enclave-id $ENCLAVE_ID"
}

# Run main function
main "$@"