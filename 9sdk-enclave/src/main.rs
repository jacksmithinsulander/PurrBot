use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::thread_rng;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use std::pin::Pin;
use nine_sdk::{Transport, listen, KeyManager};
use log;
use std::error::Error;

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
}

impl EnclaveManager {
    /// Creates a new instance of the enclave manager
    pub fn new() -> Result<Self, EnclaveError> {
        Ok(Self {
            config: None,
        })
    }

    /// Starts the enclave server
    pub async fn run(&mut self) -> Result<(), EnclaveError> {
        // Determine transport based on environment
        let transport = if std::env::var("ENCLAVE_MODE").as_deref() == Ok("enclave") {
            #[cfg(feature = "vsock")]
            {
                Transport::Vsock(u32::MAX, 5005)
            }
            #[cfg(not(feature = "vsock"))]
            {
                return Err(EnclaveError::SocketError("vsock feature not enabled".to_string()));
            }
        } else {
            Transport::Tcp("0.0.0.0:5005".parse().unwrap())
        };

        log::info!("Enclave listening for connections...");

        loop {
            let mut stream = listen(transport.clone()).await
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;

            // Read request length first
            let mut length_buf = [0u8; 4];
            if let Err(e) = stream.read_exact(&mut length_buf).await {
                log::warn!("Error reading message length: {}", e);
                continue;
            }
            let length = u32::from_be_bytes(length_buf) as usize;

            // Then read the exact amount of bytes
            let mut buffer = vec![0u8; length];
            if let Err(e) = stream.read_exact(&mut buffer).await {
                log::warn!("Error reading message body: {}", e);
                continue;
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
            stream.write_all(&length).await
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
            // Then write the actual response
            stream.write_all(&response_bytes).await
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
        let mut rng = OsRng;
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
    let mut rng = thread_rng();
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
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    log::info!("Starting nine_sdk_enclave...");
    
    // Initialize the key manager
    let key_manager = KeyManager::new();
    log::info!("Key manager initialized");
    
    // Keep the enclave running
    tokio::signal::ctrl_c().await?;
    log::info!("Shutting down...");
    
    Ok(())
}
