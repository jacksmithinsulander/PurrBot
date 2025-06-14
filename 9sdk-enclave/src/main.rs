use nine_sdk::{KeyManager, EnclaveRequest, EnclaveResponse, Transport, listen};
use std::env;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use std::pin::Pin;
use argon2::Argon2;
use password_hash::{PasswordHash, SaltString, PasswordHasher, PasswordVerifier};
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnclaveError {
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Key generation error: {0}")]
    KeyGenerationError(String),
    #[error("Invalid configuration")]
    InvalidConfig,
    #[error("Socket error: {0}")]
    SocketError(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Configuration for the enclave
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnclaveConfig {
    password_hash: String,
    salt1: String,
    salt2: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EnclaveRequest {
    SetupConfig { password: String },
    VerifyAndDeriveKeys { password: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EnclaveResponse {
    ConfigSetup { config: String },
    Keys { key1: Vec<u8>, key2: Vec<u8> },
    Error { message: String },
}

/// Manager for handling sensitive operations in the enclave
pub struct EnclaveManager {
    config: Option<EnclaveConfig>,
    listener: TcpListener,
}

impl EnclaveManager {
    /// Creates a new instance of the enclave manager
    pub fn new() -> Result<Self, EnclaveError> {
        let listener = TcpListener::bind("0.0.0.0:8080")
            .map_err(|e| EnclaveError::SocketError(e.to_string()))?;

        Ok(Self {
            config: None,
            listener,
        })
    }

    /// Starts the enclave server
    pub fn run(&mut self) -> Result<(), EnclaveError> {
        loop {
            let mut stream = self
                .listener
                .accept()
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?
                .0;

            // Read request length first
            let mut length_buf = [0u8; 4];
            if let Err(e) = stream.read_exact(&mut length_buf) {
                log::warn!("Error reading message length: {}", e);
                continue; // Connection closed or error, try next connection
            }
            let length = u32::from_be_bytes(length_buf) as usize;

            // Then read the exact amount of bytes
            let mut buffer = vec![0u8; length];
            if let Err(e) = stream.read_exact(&mut buffer) {
                log::warn!("Error reading message body: {}", e);
                continue; // Error reading request
            }

            let request: EnclaveRequest = match serde_json::from_slice(&buffer) {
                Ok(req) => req,
                Err(e) => {
                    log::error!("Error deserializing request: {}", e);
                    continue;
                }
            };

            // Process request
            let response = self.handle_request(request)?;

            // Send response with length prefix
            let response_bytes = serde_json::to_vec(&response)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
            let length = (response_bytes.len() as u32).to_be_bytes();

            // Write length prefix first
            stream
                .write_all(&length)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
            // Then write the actual response
            stream
                .write_all(&response_bytes)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        }
    }

    /// Handles an incoming request
    fn handle_request(&mut self, request: EnclaveRequest) -> Result<EnclaveResponse, EnclaveError> {
        match request {
            EnclaveRequest::SetupConfig { password } => {
                let config = self.setup_config(&password)?;
                Ok(EnclaveResponse::ConfigSetup { config })
            }
            EnclaveRequest::VerifyAndDeriveKeys { password } => {
                match self.verify_and_derive_keys(&password) {
                    Ok((key1, key2)) => Ok(EnclaveResponse::Keys {
                        key1: key1.to_vec(),
                        key2: key2.to_vec(),
                    }),
                    Err(e) => Ok(EnclaveResponse::Error {
                        message: e.to_string(),
                    }),
                }
            }
        }
    }

    /// Sets up the enclave configuration with a password
    pub fn setup_config(&mut self, password: &str) -> Result<String, EnclaveError> {
        let password_hash = hash_password(password)?;

        let mut salt1 = [0u8; 16];
        let mut salt2 = [0u8; 16];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut salt1);
        rng.fill_bytes(&mut salt2);

        let config = EnclaveConfig {
            password_hash,
            salt1: hex::encode(&salt1),
            salt2: hex::encode(&salt2),
        };

        self.config = Some(config.clone());
        Ok(serde_json::to_string_pretty(&config)?)
    }

    /// Verifies a password and derives encryption keys
    pub fn verify_and_derive_keys(
        &self,
        password: &str,
    ) -> Result<([u8; 32], [u8; 32]), EnclaveError> {
        let cfg = self.config.as_ref().ok_or(EnclaveError::InvalidConfig)?;

        // Verify password
        if !verify_password(password, &cfg.password_hash) {
            return Err(EnclaveError::AuthenticationFailed);
        }

        // Decode salts
        let salt1 =
            hex::decode(&cfg.salt1).map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;
        let salt2 =
            hex::decode(&cfg.salt2).map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;

        // Derive keys
        let key1 = derive_key(password, &salt1)?;
        let key2 = derive_key(password, &salt2)?;

        Ok((key1, key2))
    }
}

/// Hashes a password using Argon2
fn hash_password(password: &str) -> Result<String, EnclaveError> {
    let mut rng = rand::thread_rng();
    let salt = SaltString::generate(&mut rng);
    let argon2 = Argon2::default();

    Ok(argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?
        .to_string())
}

/// Verifies a password against a stored hash
fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(hash) => hash,
        Err(_) => return false,
    };
    let argon2 = Argon2::default();

    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Derives a 256-bit key from a password and salt using Argon2
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], EnclaveError> {
    let mut key = [0u8; 32];
    let argon2 = Argon2::default();

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;

    Ok(key)
}

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
