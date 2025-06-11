use nine_sdk::{KeyManager, EnclaveRequest, EnclaveResponse, Transport, listen};
use std::env;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use std::pin::Pin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    log::info!("9SDK Enclave starting...");

    let key_manager = Arc::new(Mutex::new(KeyManager::new()));

    // Determine transport based on environment
    let transport = if env::var("USE_VSOCK").as_deref() == Ok("true") {
        // In a Nitro Enclave, the parent instance has CID 3
        // The enclave will use a specific port (e.g., 5005)
        let cid = env::var("VSOCK_CID")
            .unwrap_or_else(|_| "16".to_string())
            .parse::<u32>()
            .expect("Invalid VSOCK_CID");
        let port = env::var("VSOCK_PORT")
            .unwrap_or_else(|_| "5005".to_string())
            .parse::<u32>()
            .expect("Invalid VSOCK_PORT");
        
        log::info!("Using vsock transport: CID={}, Port={}", cid, port);
        
        #[cfg(feature = "vsock")]
        {
            Transport::Vsock(cid, port)
        }
        #[cfg(not(feature = "vsock"))]
        {
            log::error!("vsock feature not enabled!");
            return Err("vsock feature not enabled".into());
        }
    } else {
        let addr = env::var("TCP_ADDRESS")
            .unwrap_or_else(|_| "0.0.0.0:5005".to_string());
        log::info!("Using TCP transport: {}", addr);
        Transport::Tcp(addr.parse()?)
    };

    loop {
        log::info!("Waiting for connection...");
        match listen(transport.clone()).await {
            Ok(mut stream) => {
                log::info!("Connection established");
                let key_manager = Arc::clone(&key_manager);
                
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, key_manager).await {
                        log::error!("Error handling connection: {}", e);
                    }
                });
            }
            Err(e) => {
                log::error!("Failed to accept connection: {}", e);
                // Continue listening for new connections
            }
        }
    }
}

async fn handle_connection(
    mut stream: Pin<Box<dyn nine_sdk::transport::TransportStream>>,
    key_manager: Arc<Mutex<KeyManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Read message length
        let mut length_buf = [0u8; 4];
        match stream.read_exact(&mut length_buf).await {
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                log::info!("Client disconnected");
                break;
            }
            Err(e) => return Err(e.into()),
        }
        
        let length = u32::from_be_bytes(length_buf) as usize;
        
        // Read message body
        let mut buffer = vec![0u8; length];
        stream.read_exact(&mut buffer).await?;
        
        // Process request
        let request: EnclaveRequest = serde_json::from_slice(&buffer)?;
        log::info!("Received request: {:?}", request);
        
        let response = process_request(request, &key_manager).await;
        
        // Send response
        let response_bytes = serde_json::to_vec(&response)?;
        let length_bytes = (response_bytes.len() as u32).to_be_bytes();
        stream.write_all(&length_bytes).await?;
        stream.write_all(&response_bytes).await?;
        stream.flush().await?;
        
        log::info!("Sent response");
    }
    
    Ok(())
}

async fn process_request(
    request: EnclaveRequest,
    key_manager: &Arc<Mutex<KeyManager>>,
) -> EnclaveResponse {
    match request {
        EnclaveRequest::SetupConfig { password } => {
            let mut km = key_manager.lock().await;
            match km.setup_config(&password).await {
                Ok(config) => EnclaveResponse::ConfigSetup { config },
                Err(e) => EnclaveResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        EnclaveRequest::VerifyAndDeriveKeys { password } => {
            let km = key_manager.lock().await;
            match km.verify_and_derive_keys(&password).await {
                Ok((key1, key2)) => EnclaveResponse::Keys {
                    key1: key1.to_vec(),
                    key2: key2.to_vec(),
                },
                Err(e) => EnclaveResponse::Error {
                    message: e.to_string(),
                },
            }
        }
    }
}