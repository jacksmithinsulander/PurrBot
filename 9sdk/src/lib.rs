use chacha20poly1305::{
    aead::{Aead, AeadCore},
    ChaCha20Poly1305, KeyInit, Nonce,
};
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use thiserror::Error;

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
    #[error("Socket error: {0}")]
    SocketError(String),
}

/// Configuration structure to store encrypted key data
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncryptedKeyConfig {
    nonce1: String,
    nonce2: String,
    double_encrypted_private_key: String,
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

/// Manager for handling private key operations
pub struct PrivateKeyManager {
    decrypted_private_key: Mutex<Option<[u8; 32]>>,
    config: Option<EncryptedKeyConfig>,
}

impl PrivateKeyManager {
    /// Creates a new instance of the manager
    pub fn new() -> Result<Self, KeyManagerError> {
        Ok(Self {
            decrypted_private_key: Mutex::new(None),
            config: None,
        })
    }

    /// Sends a request to the enclave and receives the response
    fn send_request(&self, request: EnclaveRequest) -> Result<EnclaveResponse, KeyManagerError> {
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;

        // Add length prefix to the message
        let length = (request_bytes.len() as u32).to_be_bytes();

        let mut stream = TcpStream::connect("meow-enclave:8080")
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;
        // Write length prefix first
        stream
            .write_all(&length)
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;
        // Then write the actual message
        stream
            .write_all(&request_bytes)
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;
        stream
            .flush()
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;

        // Read response length first
        let mut length_buf = [0u8; 4];
        stream
            .read_exact(&mut length_buf)
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;
        let length = u32::from_be_bytes(length_buf) as usize;

        // Then read the exact amount of bytes
        let mut buffer = vec![0u8; length];
        stream
            .read_exact(&mut buffer)
            .map_err(|e| KeyManagerError::SocketError(e.to_string()))?;

        serde_json::from_slice(&buffer).map_err(|e| KeyManagerError::SocketError(e.to_string()))
    }

    /// Derives keys from the enclave using the given password
    fn derive_keys(&self, password: &str) -> Result<([u8; 32], [u8; 32]), KeyManagerError> {
        let response = self.send_request(EnclaveRequest::VerifyAndDeriveKeys {
            password: password.to_string(),
        })?;

        match response {
            EnclaveResponse::Keys { key1, key2 } => {
                if key1.len() != 32 || key2.len() != 32 {
                    return Err(KeyManagerError::KeyGenerationError(
                        "Invalid key length".to_string(),
                    ));
                }
                let mut k1 = [0u8; 32];
                let mut k2 = [0u8; 32];
                k1.copy_from_slice(&key1);
                k2.copy_from_slice(&key2);
                Ok((k1, k2))
            }
            EnclaveResponse::Error { message } => Err(KeyManagerError::KeyGenerationError(message)),
            _ => Err(KeyManagerError::InvalidConfig),
        }
    }

    /// Signs up a user with a password and optional private key; returns the configuration
    pub fn sign_up(
        &mut self,
        password: &str,
        private_key: Option<[u8; 32]>,
    ) -> Result<String, KeyManagerError> {
        // Setup enclave config with password
        let response = self.send_request(EnclaveRequest::SetupConfig {
            password: password.to_string(),
        })?;

        match response {
            EnclaveResponse::ConfigSetup { config } => config,
            EnclaveResponse::Error { message } => {
                return Err(KeyManagerError::KeyGenerationError(message))
            }
            _ => return Err(KeyManagerError::InvalidConfig),
        };

        let private_key = private_key.unwrap_or_else(|| {
            let mut key = [0u8; 32];
            let mut rng = rand::thread_rng();
            rng.fill_bytes(&mut key);
            key
        });

        // Get encryption keys from enclave
        let (key1, key2) = self.derive_keys(password)?;

        let mut rng = rand::thread_rng();
        let mut nonce1_bytes = [0u8; 12];
        let mut nonce2_bytes = [0u8; 12];
        rng.fill_bytes(&mut nonce1_bytes);
        rng.fill_bytes(&mut nonce2_bytes);

        let (ciphertext1, _) = encrypt_chacha20(&key1, &private_key)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
        let (ciphertext2, _) = encrypt_chacha20(&key2, &ciphertext1)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

        let config = EncryptedKeyConfig {
            nonce1: hex::encode(&nonce1_bytes),
            nonce2: hex::encode(&nonce2_bytes),
            double_encrypted_private_key: hex::encode(&ciphertext2),
        };

        self.config = Some(config.clone());
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

        Ok(config_json)
    }

    /// Logs in a user by verifying the password and decrypting the private key
    pub fn login(&mut self, password: &str) -> Result<bool, KeyManagerError> {
        let cfg = self.config.as_ref().ok_or(KeyManagerError::InvalidConfig)?;

        // Get decryption keys from enclave
        let (key1, key2) = self.derive_keys(password)?;

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

/// Encrypts plaintext using ChaCha20Poly1305
fn encrypt_chacha20(key: &[u8], plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), KeyManagerError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
    let nonce = ChaCha20Poly1305::generate_nonce(&mut rand::thread_rng());
    let nonce_bytes = nonce
        .as_slice()
        .try_into()
        .map_err(|_| KeyManagerError::EncryptionError("Invalid nonce length".to_string()))?;

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;

    Ok((ciphertext, nonce_bytes))
}

/// Decrypts ciphertext using ChaCha20Poly1305
fn decrypt_chacha20(
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
