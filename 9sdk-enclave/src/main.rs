use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use env_logger;
use log;
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;
use thiserror::Error;
use rusqlite::{params, Connection, OptionalExtension};

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
    SetupConfig { user_id: String, password: String },
    VerifyAndDeriveKeys { user_id: String, password: String },
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
    db: Connection,
}

impl EnclaveManager {
    /// Creates a new instance of the enclave manager
    pub fn new() -> Result<Self, EnclaveError> {
        let listener = TcpListener::bind("0.0.0.0:8080")
            .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        // Open or create the SQLite DB file (in enclave, this should be on an encrypted volume)
        let db = Connection::open("/data/enclave.sqlite").map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS user_configs (
                user_id TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                salt1 TEXT NOT NULL,
                salt2 TEXT NOT NULL
            )",
            [],
        ).map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        Ok(Self {
            config: None,
            listener,
            db,
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

            log::info!("Accepted connection from {:?}", stream.peer_addr());

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

            log::info!("Received request: {:?}", request);

            // Process request
            let response = self.handle_request(request);

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
            stream.flush().ok();
            drop(stream); // Explicitly close the stream after response
        }
    }

    /// Handles an incoming request
    fn handle_request(&mut self, request: EnclaveRequest) -> EnclaveResponse {
        match request {
            EnclaveRequest::SetupConfig { user_id, password } => {
                match self.setup_config(&user_id, &password) {
                    Ok(config) => EnclaveResponse::ConfigSetup { config },
                    Err(e) => EnclaveResponse::Error { message: e.to_string() },
                }
            }
            EnclaveRequest::VerifyAndDeriveKeys { user_id, password } => {
                match self.verify_and_derive_keys(&user_id, &password) {
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

    /// Sets up the enclave configuration with a password and stores it in the DB
    pub fn setup_config(&mut self, user_id: &str, password: &str) -> Result<String, EnclaveError> {
        let password_hash = hash_password(password)?;
        let mut salt1 = [0u8; 16];
        let mut salt2 = [0u8; 16];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut salt1);
        rng.fill_bytes(&mut salt2);
        let config = EnclaveConfig {
            password_hash: password_hash.clone(),
            salt1: hex::encode(&salt1),
            salt2: hex::encode(&salt2),
        };
        // Store in DB
        self.db.execute(
            "INSERT OR REPLACE INTO user_configs (user_id, password_hash, salt1, salt2) VALUES (?1, ?2, ?3, ?4)",
            params![user_id, password_hash, hex::encode(&salt1), hex::encode(&salt2)],
        ).map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        Ok(serde_json::to_string_pretty(&config)?)
    }

    /// Verifies a password and derives encryption keys using the DB
    pub fn verify_and_derive_keys(&self, user_id: &str, password: &str) -> Result<([u8; 32], [u8; 32]), EnclaveError> {
        let row = self.db.query_row(
            "SELECT password_hash, salt1, salt2 FROM user_configs WHERE user_id = ?1",
            params![user_id],
            |row| {
                Ok(EnclaveConfig {
                    password_hash: row.get(0)?,
                    salt1: row.get(1)?,
                    salt2: row.get(2)?,
                })
            },
        ).optional().map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        let cfg = row.ok_or(EnclaveError::InvalidConfig)?;
        // Verify password
        if !verify_password(password, &cfg.password_hash) {
            return Err(EnclaveError::AuthenticationFailed);
        }
        // Decode salts
        let salt1 = hex::decode(&cfg.salt1).map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;
        let salt2 = hex::decode(&cfg.salt2).map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;
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
    env_logger::init(); // Initialize logger for log macros
    let mut manager = EnclaveManager::new()?;
    manager.run()
}
