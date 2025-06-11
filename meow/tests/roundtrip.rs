use serde_json::json;
use std::process::Command;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};
use nine_sdk::{KeyManager, Transport};

#[tokio::test]
async fn test_enclave_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    // Start the enclave process
    let mut enclave = Command::new("cargo")
        .args(["run", "-p", "nine-sdk-enclave", "--bin", "nine-sdk-enclave"])
        .env("RUST_LOG", "debug")
        .spawn()?;

    // Give the enclave time to start up
    sleep(Duration::from_secs(1)).await;

    // Connect to the enclave
    let mut stream = TcpStream::connect("127.0.0.1:5005").await?;

    // Create a test request
    let request = json!({
        "SetupConfig": {
            "password": "test_password"
        }
    });

    // Send the request
    let request_bytes = serde_json::to_vec(&request)?;
    let length = (request_bytes.len() as u32).to_be_bytes();
    stream.write_all(&length).await?;
    stream.write_all(&request_bytes).await?;

    // Read the response length
    let mut length_buf = [0u8; 4];
    stream.read_exact(&mut length_buf).await?;
    let length = u32::from_be_bytes(length_buf) as usize;

    // Read the response
    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer).await?;
    let response: serde_json::Value = serde_json::from_slice(&buffer)?;

    // Verify the response
    assert!(response.get("ConfigSetup").is_some());

    // Clean up
    enclave.kill()?;
    Ok(())
}
