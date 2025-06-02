use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use env_logger;
use log;
use nine_sdk::{EnclaveRequest, EnclaveResponse};
use rand_core::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
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
    nonce1: String,
    nonce2: String,
    double_encrypted_private_key: String,
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
        let db = Connection::open("/data/enclave.sqlite")
            .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS user_configs (
                user_id TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                salt1 TEXT NOT NULL,
                salt2 TEXT NOT NULL,
                nonce1 TEXT NOT NULL,
                nonce2 TEXT NOT NULL,
                double_encrypted_private_key TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
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

            // Debug log: print raw received data
            log::info!("Received raw: {}", String::from_utf8_lossy(&buffer));

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
                    Ok((salt1, salt2, password_hash)) => EnclaveResponse::ConfigSetup {
                        salt1,
                        salt2,
                        password_hash,
                    },
                    Err(e) => EnclaveResponse::Error {
                        message: e.to_string(),
                    },
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
            EnclaveRequest::LoadConfig { user_id } => match self.get_config(&user_id) {
                Ok(config_opt) => EnclaveResponse::Config { config: config_opt },
                Err(e) => EnclaveResponse::Error {
                    message: e.to_string(),
                },
            },
            EnclaveRequest::VerifyUserId { user_id } => {
                // Check if user_id exists in the DB
                let exists = self
                    .db
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM user_configs WHERE user_id = ?1)",
                        params![user_id],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0)
                    == 1;
                EnclaveResponse::UserIdVerified { verified: exists }
            }
            EnclaveRequest::StoreEncryptedConfig {
                user_id,
                nonce1,
                nonce2,
                double_encrypted_private_key,
            } => {
                match self.store_encrypted_config(
                    &user_id,
                    &nonce1,
                    &nonce2,
                    &double_encrypted_private_key,
                ) {
                    Ok(_) => EnclaveResponse::ConfigStored,
                    Err(e) => EnclaveResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
        }
    }

    /// Sets up the enclave configuration with a password and stores it in the DB
    pub fn setup_config(
        &mut self,
        user_id: &str,
        password: &str,
    ) -> Result<(String, String, String), EnclaveError> {
        let password_hash = hash_password(password)?;
        let mut salt1 = [0u8; 16];
        let mut salt2 = [0u8; 16];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut salt1);
        rng.fill_bytes(&mut salt2);
        // Store in DB
        self.db.execute(
            "INSERT OR REPLACE INTO user_configs (user_id, password_hash, salt1, salt2, nonce1, nonce2, double_encrypted_private_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![user_id, password_hash, hex::encode(&salt1), hex::encode(&salt2), "", "", ""],
        ).map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        Ok((hex::encode(&salt1), hex::encode(&salt2), password_hash))
    }

    /// Verifies a password and derives encryption keys using the DB
    pub fn verify_and_derive_keys(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<([u8; 32], [u8; 32]), EnclaveError> {
        log::info!("[verify_and_derive_keys] user_id: {}", user_id);
        let row = self
            .db
            .query_row(
                "SELECT password_hash, salt1, salt2 FROM user_configs WHERE user_id = ?1",
                params![user_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        let (password_hash, salt1, salt2) = row.ok_or(EnclaveError::InvalidConfig)?;
        log::info!("[verify_and_derive_keys] loaded salt1: {}", salt1);
        log::info!("[verify_and_derive_keys] loaded salt2: {}", salt2);
        // Verify password
        if !verify_password(password, &password_hash) {
            log::info!("[verify_and_derive_keys] password verification failed");
            return Err(EnclaveError::AuthenticationFailed);
        }
        // Decode salts
        let salt1_bytes =
            hex::decode(&salt1).map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;
        let salt2_bytes =
            hex::decode(&salt2).map_err(|e| EnclaveError::KeyGenerationError(e.to_string()))?;
        log::info!(
            "[verify_and_derive_keys] salt1_bytes: {}",
            hex::encode(&salt1_bytes)
        );
        log::info!(
            "[verify_and_derive_keys] salt2_bytes: {}",
            hex::encode(&salt2_bytes)
        );
        // Derive keys
        let key1 = derive_key(password, &salt1_bytes)?;
        let key2 = derive_key(password, &salt2_bytes)?;
        log::info!("[verify_and_derive_keys] key1: {}", hex::encode(&key1));
        log::info!("[verify_and_derive_keys] key2: {}", hex::encode(&key2));
        Ok((key1, key2))
    }

    /// Fetches the enclave configuration for a user from the DB
    pub fn get_config(&self, user_id: &str) -> Result<Option<String>, EnclaveError> {
        log::info!("[get_config] user_id: {}", user_id);
        let row = self.db.query_row(
            "SELECT password_hash, salt1, salt2, nonce1, nonce2, double_encrypted_private_key FROM user_configs WHERE user_id = ?1",
            params![user_id],
            |row| {
                Ok(EnclaveConfig {
                    password_hash: row.get(0)?,
                    salt1: row.get(1)?,
                    salt2: row.get(2)?,
                    nonce1: row.get(3)?,
                    nonce2: row.get(4)?,
                    double_encrypted_private_key: row.get(5)?,
                })
            },
        ).optional().map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        if let Some(cfg) = &row {
            log::info!("[get_config] loaded nonce1: {}", cfg.nonce1);
            log::info!("[get_config] loaded nonce2: {}", cfg.nonce2);
            log::info!(
                "[get_config] loaded double_encrypted_private_key: {}",
                cfg.double_encrypted_private_key
            );
        }
        if let Some(cfg) = row {
            let config_json = serde_json::to_string_pretty(&cfg)?;
            Ok(Some(config_json))
        } else {
            Ok(None)
        }
    }

    /// Stores the encrypted key config in the DB
    pub fn store_encrypted_config(
        &mut self,
        user_id: &str,
        nonce1: &str,
        nonce2: &str,
        double_encrypted_private_key: &str,
    ) -> Result<(), EnclaveError> {
        log::info!("[store_encrypted_config] user_id: {}", user_id);
        log::info!("[store_encrypted_config] nonce1: {}", nonce1);
        log::info!("[store_encrypted_config] nonce2: {}", nonce2);
        log::info!(
            "[store_encrypted_config] double_encrypted_private_key: {}",
            double_encrypted_private_key
        );
        self.db.execute(
            "UPDATE user_configs SET nonce1 = ?1, nonce2 = ?2, double_encrypted_private_key = ?3 WHERE user_id = ?4",
            params![nonce1, nonce2, double_encrypted_private_key, user_id],
        ).map_err(|e| EnclaveError::SocketError(e.to_string()))?;
        Ok(())
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
