use rand_core::{OsRng, RngCore};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Key, Nonce};
use chacha20poly1305::aead::Aead;
use serde::{Serialize, Deserialize};
use std::convert::TryInto;
use thiserror::Error;
use rand::Rng;
use std::sync::Mutex;

#[derive(Error, Debug)]
pub enum KeyManagerError {
    #[error("Invalid configuration")]
    InvalidConfig,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    #[error("Key generation error: {0}")]
    KeyGenerationError(String),
}

/// Configuration structure to store encrypted key data
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncryptedKeyConfig {
    password_hash: String,
    salt1: String,
    salt2: String,
    nonce1: String,
    nonce2: String,
    double_encrypted_private_key: String,
}

/// Manager for handling private key operations
pub struct PrivateKeyManager {
    decrypted_private_key: Mutex<Option<[u8; 32]>>,
    config: Option<EncryptedKeyConfig>,
}

impl PrivateKeyManager {
    /// Creates a new instance of the manager
    pub fn new() -> Self {
        Self {
            decrypted_private_key: Mutex::new(None),
            config: None,
        }
    }

    /// Sets up the configuration with a default password
    pub fn setup_config(&mut self) -> Result<String, KeyManagerError> {
        let mut private_key = [0u8; 32];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut private_key);
        let user_password = "double_encryption_pw";

        let password_hash = hash_password(user_password)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

        let mut salt1 = [0u8; 16];
        let mut salt2 = [0u8; 16];
        rng.fill_bytes(&mut salt1);
        rng.fill_bytes(&mut salt2);

        let key1 = derive_key(user_password, &salt1)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;
        let key2 = derive_key(user_password, &salt2)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

        let (ciphertext1, nonce1) = encrypt_chacha20(&key1, &private_key)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
        let (ciphertext2, nonce2) = encrypt_chacha20(&key2, &ciphertext1)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

        let config = EncryptedKeyConfig {
            password_hash: password_hash.clone(),
            salt1: hex::encode(&salt1),
            salt2: hex::encode(&salt2),
            nonce1: hex::encode(&nonce1),
            nonce2: hex::encode(&nonce2),
            double_encrypted_private_key: hex::encode(&ciphertext2),
        };

        self.config = Some(config.clone());
        Ok(serde_json::to_string_pretty(&config)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?)
    }

    /// Signs up a user with an optional password; returns the password and configuration
    pub fn sign_up(&mut self, password: Option<String>) -> Result<(String, String), KeyManagerError> {
        let final_password = password.unwrap_or_else(|| generate_random_password());
        let mut private_key = [0u8; 32];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut private_key);

        let password_hash = hash_password(&final_password)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

        let mut salt1 = [0u8; 16];
        let mut salt2 = [0u8; 16];
        rng.fill_bytes(&mut salt1);
        rng.fill_bytes(&mut salt2);

        let key1 = derive_key(&final_password, &salt1)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;
        let key2 = derive_key(&final_password, &salt2)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

        let (ciphertext1, nonce1) = encrypt_chacha20(&key1, &private_key)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
        let (ciphertext2, nonce2) = encrypt_chacha20(&key2, &ciphertext1)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

        let config = EncryptedKeyConfig {
            password_hash: password_hash.clone(),
            salt1: hex::encode(&salt1),
            salt2: hex::encode(&salt2),
            nonce1: hex::encode(&nonce1),
            nonce2: hex::encode(&nonce2),
            double_encrypted_private_key: hex::encode(&ciphertext2),
        };

        self.config = Some(config.clone());
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

        Ok((final_password, config_json))
    }

    /// Logs in a user by verifying the password and decrypting the private key
    pub fn login(&mut self, password: &str) -> Result<bool, KeyManagerError> {
        let cfg = self.config.as_ref().ok_or(KeyManagerError::InvalidConfig)?;
        
        if !verify_password(password, &cfg.password_hash) {
            return Ok(false);
        }

        let salt1 = hex::decode(&cfg.salt1)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
        let salt2 = hex::decode(&cfg.salt2)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
        let nonce1: [u8; 12] = hex::decode(&cfg.nonce1)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?
            .try_into()
            .map_err(|_| KeyManagerError::DecryptionError("Invalid nonce1 length".to_string()))?;
        let nonce2: [u8; 12] = hex::decode(&cfg.nonce2)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?
            .try_into()
            .map_err(|_| KeyManagerError::DecryptionError("Invalid nonce2 length".to_string()))?;
        let ciphertext2 = hex::decode(&cfg.double_encrypted_private_key)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;

        let key1 = derive_key(password, &salt1)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;
        let key2 = derive_key(password, &salt2)
            .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

        let decrypted_layer1 = decrypt_chacha20(&key2, &ciphertext2, &nonce2)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
        let decrypted_private_key = decrypt_chacha20(&key1, &decrypted_layer1, &nonce1)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;

        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&decrypted_private_key);
        *self.decrypted_private_key.lock().unwrap() = Some(private_key);
        Ok(true)
    }

    /// Retrieves the decrypted private key, if available
    pub fn get_private_key(&self) -> Option<[u8; 32]> {
        self.decrypted_private_key.lock().unwrap().clone()
    }
}

/// Generates a random password using a cryptographically secure RNG
fn generate_random_password() -> String {
    let mut rng = rand::thread_rng();
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?";
    (0..24)  // Increased length for better security
        .map(|_| charset[rng.gen_range(0..charset.len())] as char)
        .collect()
}

/// Hashes a password using Argon2
fn hash_password(password: &str) -> Result<String, KeyManagerError> {
    let salt = SaltString::generate(&mut rand::thread_rng());
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

/// Derives a 256-bit key from a password and salt using Argon2
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], KeyManagerError> {
    let mut key = [0u8; 32];
    let argon2 = Argon2::default();

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| KeyManagerError::KeyGenerationError(e.to_string()))?;

    Ok(key)
}

/// Encrypts plaintext using ChaCha20Poly1305
fn encrypt_chacha20(key: &[u8], plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), KeyManagerError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
    let mut nonce_bytes = [0u8; 12];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

    Ok((ciphertext, nonce_bytes))
}

/// Decrypts ciphertext using ChaCha20Poly1305
fn decrypt_chacha20(key: &[u8], ciphertext: &[u8], nonce: &[u8; 12]) -> Result<Vec<u8>, KeyManagerError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))
}
