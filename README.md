# PurrBot

A secure Telegram bot with AWS Nitro Enclaves support.

## Required Environment Variables

- `RUST_LOG`: Logging level (e.g., "debug", "info")
- `ENCLAVE_MODE`: Set to "enclave" when running in AWS Nitro Enclave, empty otherwise
- `AWS_REGION`: AWS region for KMS operations (if used)
- `AWS_ACCESS_KEY_ID`: AWS access key for KMS operations (if used)
- `AWS_SECRET_ACCESS_KEY`: AWS secret key for KMS operations (if used)

## Local Development

1. Start the services using Docker Compose:
   ```bash
   docker-compose up
   ```

This will start:
- `meow` service on http://localhost:8080
- `nine_sdk_enclave` service on localhost:5005

The services will communicate over TCP in local development mode.

## Production Deployment

1. Build the enclave image:
   ```bash
   nitro-cli build-enclave --docker-dir . --output-file purrbot.eif
   ```

2. Run the enclave:
   ```bash
   nitro-cli run-enclave --eif-path purrbot.eif --memory 2048 --cpu-count 2
   ```

The parent service will communicate with the enclave over vsock in production mode.

## Building from Source

1. Install Rust 1.85 or later
2. Build with vsock support:
   ```bash
   cargo build --features vsock
   ```

## Testing

Run the test suite:
```bash
cargo test
```

The integration tests will run in TCP mode by default. To test vsock functionality, you'll need to run the tests on an EC2 instance with Nitro Enclaves support. 