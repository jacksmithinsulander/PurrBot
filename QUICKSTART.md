# Quick Start Guide - 9SDK Enclave with Vsock

## Local Development (TCP Mode)

```bash
# 1. Start the services
docker-compose up

# 2. Test the connection
./test-vsock-enclave.py --mode tcp

# 3. View logs
docker-compose logs -f meow-enclave
```

## AWS Nitro Enclave Deployment

### Prerequisites
- EC2 instance with Nitro Enclave support (m5.xlarge or larger)
- Nitro Enclaves CLI installed
- Docker installed

### Deploy in 3 Steps

```bash
# 1. Clone and navigate to the project
git clone <your-repo>
cd <project-directory>

# 2. Run the deployment script
./deploy-nitro-enclave.sh

# 3. Test the vsock connection
./test-vsock-enclave.py --mode vsock
```

## Environment Configuration

### For TCP (Development)
```env
USE_VSOCK=false
ENCLAVE_ADDRESS=127.0.0.1:5005
```

### For Vsock (Production)
```env
USE_VSOCK=true
ENCLAVE_CID=16
VSOCK_PORT=5005
```

## Common Commands

```bash
# View enclave status
nitro-cli describe-enclaves

# Monitor enclave logs
nitro-cli console --enclave-id <id>

# Restart services
docker-compose restart

# Clean up
docker-compose down
nitro-cli terminate-enclave --all
```

## Troubleshooting

1. **Connection refused**: Check if enclave is running with `nitro-cli describe-enclaves`
2. **vsock not found**: Ensure you're on a Nitro-enabled EC2 instance
3. **Out of memory**: Edit `/etc/nitro_enclaves/allocator.yaml` and increase memory

## Need Help?

- Check `NITRO_ENCLAVE_DEPLOYMENT.md` for detailed documentation
- View logs with `docker-compose logs` or `nitro-cli console`
- Test connectivity with `./test-vsock-enclave.py`