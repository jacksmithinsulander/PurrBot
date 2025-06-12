use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;

pub mod transport;

pub use transport::{connect, listen, Transport};

#[derive(Error, Debug)]
pub enum KeyManagerError {
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
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
}

/// Configuration for encrypted keys
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncryptedKeyConfig {
    pub password_hash: String,
    pub salt1: String,
    pub salt2: String,
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

/// Manager for handling sensitive operations
pub struct KeyManager {
    config: Mutex<Option<EncryptedKeyConfig>>,
}

impl KeyManager {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(None),
        }
    }

    pub async fn setup_config(&self, password: &str) -> Result<String, KeyManagerError> {
        let password_hash = hash_password(password)?;

        let mut salt1 = [0u8; 16];
        let mut salt2 = [0u8; 16];
        let mut rng = thread_rng();
        rng.fill_bytes(&mut salt1);
        rng.fill_bytes(&mut salt2);

        let config = EncryptedKeyConfig {
            password_hash,
            salt1: hex::encode(&salt1),
            salt2: hex::encode(&salt2),
        };

        let config_json = serde_json::to_string_pretty(&config)?;
        *self.config.lock().unwrap() = Some(config);
        Ok(config_json)
    }

    pub async fn verify_and_derive_keys(
        &self,
        password: &str,
    ) -> Result<([u8; 32], [u8; 32]), KeyManagerError> {
        let config = {
            let guard = self.config.lock().unwrap();
            guard
                .as_ref()
                .ok_or(KeyManagerError::InvalidConfig)?
                .clone()
        };

        // Verify password
        if !verify_password(password, &config.password_hash) {
            return Err(KeyManagerError::AuthenticationFailed);
        }

        // Decode salts
        let salt1 = hex::decode(&config.salt1)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;
        let salt2 = hex::decode(&config.salt2)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

        // Derive keys
        let key1 = derive_key(password, &salt1)?;
        let key2 = derive_key(password, &salt2)?;

        Ok((key1, key2))
    }

    pub fn set_config(&self, config: EncryptedKeyConfig) {
        let mut guard = self.config.lock().unwrap();
        *guard = Some(config);
    }
}

/// Hashes a password using Argon2
fn hash_password(password: &str) -> Result<String, KeyManagerError> {
    let mut rng = thread_rng();
    let salt = SaltString::generate(&mut rng);
    let argon2 = Argon2::default();

    Ok(argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?
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

/// Derives a key from a password and salt
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], KeyManagerError> {
    let mut key = [0u8; 32];
    let argon2 = Argon2::default();
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;
    Ok(key)
}

/// Encrypts plaintext using ChaCha20Poly1305
pub fn encrypt_chacha20(
    key: &[u8],
    plaintext: &[u8],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, KeyManagerError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
    Ok(ciphertext)
}

/// Decrypts ciphertext using ChaCha20Poly1305
pub fn decrypt_chacha20(
    key: &[u8],
    ciphertext: &[u8],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, KeyManagerError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))
}
