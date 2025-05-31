use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use rand::Rng;
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;
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
        let listener = TcpListener::bind("127.0.0.1:8000")
            .map_err(|e| EnclaveError::SocketError(e.to_string()))?;

        Ok(Self {
            config: None,
            listener,
        })
    }

    /// Starts the enclave server
    pub fn run(&mut self) -> Result<(), EnclaveError> {
        for stream in self.listener.incoming() {
            let mut stream = stream.map_err(|e| EnclaveError::SocketError(e.to_string()))?;

            // Read request
            let mut buffer = Vec::new();
            stream
                .read_to_end(&mut buffer)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;

            let request: EnclaveRequest = serde_json::from_slice(&buffer)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;

            // Process request
            let response = self.handle_request(request)?;

            // Send response
            let response_bytes = serde_json::to_vec(&response)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
            stream
                .write_all(&response_bytes)
                .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        }

        Ok(())
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
    let salt = SaltString::generate(&mut rand::thread_rng());
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

fn main() -> Result<(), EnclaveError> {
    let mut manager = EnclaveManager::new()?;
    manager.run()
}
