# Use the official Rust image as base
FROM rust:1.82-slim-bullseye as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a new empty shell project
WORKDIR /usr/src/purrbot

# Copy the entire project
COPY . .

# Copy the .env file from parent directory
COPY ../.env .

# Remove existing Cargo.lock files to force regeneration
RUN rm -f 9sdk/Cargo.lock 9sdk-enclave/Cargo.lock meow/Cargo.lock

# Build the enclave first
WORKDIR /usr/src/purrbot/9sdk-enclave
RUN cargo build --release
RUN ls -la target/release/

# Build the meow crate
WORKDIR /usr/src/purrbot/meow
RUN cargo build --release

# Create a smaller runtime image
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binaries from the builder stage
COPY --from=builder /usr/src/purrbot/meow/target/release/meow /usr/local/bin/meow
COPY --from=builder /usr/src/purrbot/9sdk-enclave/target/release/nine_sdk_enclave /usr/local/bin/enclave
COPY --from=builder /usr/src/purrbot/.env /usr/local/bin/.env

# Set the working directory
WORKDIR /usr/local/bin

# Run the application
CMD ["./meow"] 