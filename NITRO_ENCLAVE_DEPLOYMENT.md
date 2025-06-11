# AWS Nitro Enclave Deployment Guide

This guide explains how to deploy the 9sdk-enclave to AWS Nitro Enclaves with vsock communication support.

## Overview

The 9sdk-enclave has been updated to support vsock (Virtual Socket) communication, which is the secure communication channel used in AWS Nitro Enclaves. This provides:

- **Isolation**: Complete isolation from the parent instance network
- **Security**: Direct, encrypted communication between parent and enclave
- **Performance**: Low-latency local communication

## Architecture

```
┌─────────────────────────┐     ┌─────────────────────────┐
│   Parent Instance       │     │    Nitro Enclave        │
│   (CID: 3)             │     │    (CID: 16)            │
│                        │     │                         │
│  ┌─────────────────┐  │     │  ┌─────────────────┐   │
│  │  meow (bot)     │  │ vsock│  │ 9sdk-enclave    │   │
│  │                 ├──┼─────┼──┤ (port: 5005)    │   │
│  └─────────────────┘  │     │  └─────────────────┘   │
│                        │     │                         │
└─────────────────────────┘     └─────────────────────────┘
```

## Prerequisites

1. **EC2 Instance with Nitro Enclave Support**:
   - Instance types: m5.xlarge or larger (with .metal variants for best performance)
   - Enable enclave support when launching the instance

2. **Install Nitro Enclaves CLI**:
   ```bash
   # Amazon Linux 2
   sudo amazon-linux-extras install aws-nitro-enclaves-cli
   sudo yum install aws-nitro-enclaves-cli-devel -y
   
   # Ubuntu
   wget https://github.com/aws/aws-nitro-enclaves-cli/releases/download/v1.2.0/aws-nitro-enclaves-cli_1.2.0_amd64.deb
   sudo dpkg -i aws-nitro-enclaves-cli_1.2.0_amd64.deb
   ```

3. **Configure Nitro Enclaves**:
   ```bash
   # Add user to ne group
   sudo usermod -aG ne $USER
   
   # Configure memory allocation (edit /etc/nitro_enclaves/allocator.yaml)
   sudo nano /etc/nitro_enclaves/allocator.yaml
   # Set: memory_mib: 512 (or desired amount)
   # Set: cpu_count: 2 (or desired count)
   
   # Start the service
   sudo systemctl enable nitro-enclaves-allocator.service
   sudo systemctl start nitro-enclaves-allocator.service
   
   # Verify
   nitro-cli describe-enclaves
   ```

## Deployment Options

### Option 1: Quick Deployment with Script

The `deploy-nitro-enclave.sh` script automates the entire deployment:

```bash
# For Nitro Enclave deployment
./deploy-nitro-enclave.sh

# For local Docker testing (TCP mode)
./deploy-nitro-enclave.sh --local
```

The script will:
- Build the Docker image
- Convert to Enclave Image File (EIF)
- Start the enclave with vsock
- Set up systemd service
- Configure environment variables
- Test the vsock connection

### Option 2: Manual Deployment

1. **Build the Docker image**:
   ```bash
   docker build -f Dockerfile.enclave -t 9sdk-enclave:latest .
   ```

2. **Convert to Enclave Image**:
   ```bash
   nitro-cli build-enclave \
     --docker-uri 9sdk-enclave:latest \
     --output-file 9sdk-enclave.eif
   ```

3. **Run the enclave**:
   ```bash
   nitro-cli run-enclave \
     --cpu-count 2 \
     --memory 512 \
     --enclave-cid 16 \
     --eif-path 9sdk-enclave.eif \
     --debug-mode
   ```

4. **Update parent application environment**:
   ```bash
   # Add to .env file
   USE_VSOCK=true
   ENCLAVE_CID=16
   VSOCK_PORT=5005
   ```

## Development Workflow

### Local Development (Docker with TCP)

For local development, use Docker Compose with TCP:

```bash
# Standard mode
docker-compose up

# Debug mode with extra logging
docker-compose --profile debug up
```

### Testing vsock Connection

Test the vsock connection manually:

```python
# test_vsock.py
import socket

# Connect to enclave
sock = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
sock.connect((16, 5005))  # CID 16, Port 5005

# Send test request
request = b'{"SetupConfig":{"password":"test"}}'
sock.send(len(request).to_bytes(4, 'big'))
sock.send(request)

# Read response
length = int.from_bytes(sock.recv(4), 'big')
response = sock.recv(length)
print(response)
```

### Monitoring Enclave

```bash
# View console output
nitro-cli console --enclave-id <enclave-id>

# Describe running enclaves
nitro-cli describe-enclaves

# Terminate enclave
nitro-cli terminate-enclave --enclave-id <enclave-id>
```

## Environment Variables

### Parent Application (meow)
- `USE_VSOCK`: Set to "true" to use vsock instead of TCP
- `ENCLAVE_CID`: CID of the enclave (default: 16)
- `VSOCK_PORT`: Port number for vsock (default: 5005)
- `ENCLAVE_ADDRESS`: TCP address when not using vsock

### Enclave (9sdk-enclave)
- `USE_VSOCK`: Set to "true" to listen on vsock
- `VSOCK_CID`: CID to bind to (enclave's own CID)
- `VSOCK_PORT`: Port to listen on
- `TCP_ADDRESS`: TCP address when not using vsock

## Security Considerations

1. **Attestation**: The enclave generates PCR values that can be used for attestation:
   - PCR0: Enclave image measurement
   - PCR1: Linux kernel measurement
   - PCR2: Application measurement

2. **KMS Integration**: Use the PCR values in KMS key policies:
   ```json
   {
     "Condition": {
       "StringEquals": {
         "kms:RecipientAttestation:PCR0": "<PCR0_VALUE>"
       }
     }
   }
   ```

3. **Network Isolation**: The enclave has no network access except through vsock to the parent

## Troubleshooting

### Common Issues

1. **"vsock feature not enabled" error**:
   - Ensure both applications are built with `--features vsock`
   - Check Cargo.toml has vsock in default features

2. **Connection refused on vsock**:
   - Verify enclave is running: `nitro-cli describe-enclaves`
   - Check CID and port match in both applications
   - Ensure allocator service is running

3. **Out of memory**:
   - Increase memory allocation in `/etc/nitro_enclaves/allocator.yaml`
   - Restart allocator service

### Debug Commands

```bash
# Check enclave status
nitro-cli describe-enclaves

# View enclave console
nitro-cli console --enclave-id <id>

# Check vsock module
lsmod | grep vsock

# Test vsock connectivity
python3 -c "import socket; s=socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM); s.connect((16,5005))"
```

## Performance Tuning

1. **CPU Allocation**: Allocate dedicated CPU cores for consistent performance
2. **Memory**: Start with 512MB and increase if needed
3. **Debug Mode**: Disable debug mode in production for better performance

## Next Steps

1. Deploy to production EC2 instance with Nitro Enclave support
2. Set up monitoring and logging
3. Configure KMS policies with PCR values
4. Implement health checks and auto-recovery
5. Set up CI/CD pipeline for enclave updates