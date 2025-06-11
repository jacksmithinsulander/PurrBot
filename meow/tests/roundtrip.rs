use serde_json::json;
use std::io::{Read, Write};
use std::process::{Command, Child};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

// Constants for better maintainability
const ENCLAVE_ADDRESS: &str = "127.0.0.1:5005";
const ENCLAVE_STARTUP_DELAY: Duration = Duration::from_secs(1);
const TEST_PASSWORD: &str = "test_password";
const MESSAGE_LENGTH_SIZE: usize = 4;

#[tokio::test]
#[ignore = "Requires nine-sdk-enclave binary from another package"]
async fn test_enclave_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let mut enclave_process = start_enclave_process()?;
    
    // Ensure cleanup happens even if test fails
    let result = run_enclave_test().await;
    
    cleanup_enclave_process(&mut enclave_process)?;
    
    result
}

async fn run_enclave_test() -> Result<(), Box<dyn std::error::Error>> {
    wait_for_enclave_startup().await;
    
    let mut connection = connect_to_enclave().await?;
    
    send_setup_config_request(&mut connection, TEST_PASSWORD).await?;
    
    let response = read_enclave_response(&mut connection).await?;
    
    verify_config_setup_response(&response)?;
    
    Ok(())
}

// Process management functions

fn start_enclave_process() -> Result<Child, Box<dyn std::error::Error>> {
    Command::new("cargo")
        .args(["run", "--bin", "nine-sdk-enclave"])
        .env("RUST_LOG", "debug")
        .spawn()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

async fn wait_for_enclave_startup() {
    sleep(ENCLAVE_STARTUP_DELAY).await;
}

fn cleanup_enclave_process(process: &mut Child) -> Result<(), Box<dyn std::error::Error>> {
    process.kill().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

// Network communication functions

async fn connect_to_enclave() -> Result<TcpStream, Box<dyn std::error::Error>> {
    TcpStream::connect(ENCLAVE_ADDRESS)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

async fn send_setup_config_request(
    stream: &mut TcpStream,
    password: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = create_setup_config_request(password);
    send_request(stream, &request).await
}

fn create_setup_config_request(password: &str) -> serde_json::Value {
    json!({
        "SetupConfig": {
            "password": password
        }
    })
}

async fn send_request(
    stream: &mut TcpStream,
    request: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_bytes = serialize_request(request)?;
    write_message_with_length(stream, &request_bytes).await
}

fn serialize_request(request: &serde_json::Value) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    serde_json::to_vec(request)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

async fn write_message_with_length(
    stream: &mut TcpStream,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let length_bytes = encode_message_length(data.len());
    
    stream.write_all(&length_bytes).await?;
    stream.write_all(data).await?;
    
    Ok(())
}

fn encode_message_length(length: usize) -> [u8; MESSAGE_LENGTH_SIZE] {
    (length as u32).to_be_bytes()
}

// Response handling functions

async fn read_enclave_response(
    stream: &mut TcpStream,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let message_length = read_message_length(stream).await?;
    let message_body = read_message_body(stream, message_length).await?;
    deserialize_response(&message_body)
}

async fn read_message_length(stream: &mut TcpStream) -> Result<usize, Box<dyn std::error::Error>> {
    let mut length_buffer = [0u8; MESSAGE_LENGTH_SIZE];
    stream.read_exact(&mut length_buffer).await?;
    Ok(decode_message_length(&length_buffer))
}

fn decode_message_length(buffer: &[u8; MESSAGE_LENGTH_SIZE]) -> usize {
    u32::from_be_bytes(*buffer) as usize
}

async fn read_message_body(
    stream: &mut TcpStream,
    length: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer).await?;
    Ok(buffer)
}

fn deserialize_response(data: &[u8]) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    serde_json::from_slice(data)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

// Verification functions

fn verify_config_setup_response(response: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    if response.get("ConfigSetup").is_some() {
        Ok(())
    } else {
        Err("Expected ConfigSetup response".into())
    }
}
