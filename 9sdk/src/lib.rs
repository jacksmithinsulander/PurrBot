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
    SetupConfig {
        user_id: String,
        password: String,
    },
    VerifyAndDeriveKeys {
        user_id: String,
        password: String,
    },
    LoadConfig {
        user_id: String,
    },
    VerifyUserId {
        user_id: String,
    },
    StoreEncryptedConfig {
        user_id: String,
        nonce1: String,
        nonce2: String,
        double_encrypted_private_key: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EnclaveResponse {
    ConfigSetup {
        salt1: String,
        salt2: String,
        password_hash: String,
    },
    ConfigStored,
    Keys {
        key1: Vec<u8>,
        key2: Vec<u8>,
    },
    Config {
        config: Option<String>,
    },
    Error {
        message: String,
    },
    UserIdVerified {
        verified: bool,
    },
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

    /// Loads the configuration for a specific user
    pub fn load_config(&mut self, user_id: &str) -> Result<(), KeyManagerError> {
        let response = self.send_request(EnclaveRequest::LoadConfig {
            user_id: user_id.to_string(),
        })?;

        match response {
            EnclaveResponse::Config { config } => {
                if let Some(config_str) = config {
                    let config: EncryptedKeyConfig = serde_json::from_str(&config_str)
                        .map_err(|e| KeyManagerError::InvalidConfig)?;
                    self.config = Some(config);
                    Ok(())
                } else {
                    Err(KeyManagerError::InvalidConfig)
                }
            }
            EnclaveResponse::Error { message } => Err(KeyManagerError::KeyGenerationError(message)),
            _ => Err(KeyManagerError::InvalidConfig),
        }
    }

    /// Verifies if the loaded config matches the given user_id
    pub fn verify_user_id(&self, user_id: &str) -> Result<bool, KeyManagerError> {
        let response = self.send_request(EnclaveRequest::VerifyUserId {
            user_id: user_id.to_string(),
        })?;

        match response {
            EnclaveResponse::UserIdVerified { verified } => Ok(verified),
            EnclaveResponse::Error { message } => Err(KeyManagerError::KeyGenerationError(message)),
            _ => Err(KeyManagerError::InvalidConfig),
        }
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
    fn derive_keys(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<([u8; 32], [u8; 32]), KeyManagerError> {
        let response = self.send_request(EnclaveRequest::VerifyAndDeriveKeys {
            user_id: user_id.to_string(),
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
        user_id: &str,
        password: &str,
        private_key: Option<[u8; 32]>,
    ) -> Result<String, KeyManagerError> {
        // 1. Call SetupConfig to get salts
        let response = self.send_request(EnclaveRequest::SetupConfig {
            user_id: user_id.to_string(),
            password: password.to_string(),
        })?;

        let (salt1, salt2) = match response {
            EnclaveResponse::ConfigSetup { salt1, salt2, .. } => (salt1, salt2),
            EnclaveResponse::Error { message } => {
                return Err(KeyManagerError::KeyGenerationError(message))
            }
            _ => return Err(KeyManagerError::InvalidConfig),
        };
        println!("[sign_up] salt1: {}", salt1);
        println!("[sign_up] salt2: {}", salt2);

        let private_key = private_key.unwrap_or_else(|| {
            let mut key = [0u8; 32];
            let mut rng = rand::thread_rng();
            rng.fill_bytes(&mut key);
            key
        });
        println!("[sign_up] private_key: {}", hex::encode(private_key));

        // Get encryption keys from enclave
        let (key1, key2) = self.derive_keys(user_id, password)?;
        println!("[sign_up] key1: {}", hex::encode(key1));
        println!("[sign_up] key2: {}", hex::encode(key2));

        let mut rng = rand::thread_rng();
        let mut nonce1_bytes = [0u8; 12];
        let mut nonce2_bytes = [0u8; 12];
        rng.fill_bytes(&mut nonce1_bytes);
        rng.fill_bytes(&mut nonce2_bytes);
        println!("[sign_up] nonce1: {}", hex::encode(nonce1_bytes));
        println!("[sign_up] nonce2: {}", hex::encode(nonce2_bytes));

        let ciphertext1 = encrypt_chacha20(&key1, &private_key, &nonce1_bytes)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
        println!("[sign_up] ciphertext1: {}", hex::encode(&ciphertext1));
        let ciphertext2 = encrypt_chacha20(&key2, &ciphertext1, &nonce2_bytes)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
        println!("[sign_up] ciphertext2: {}", hex::encode(&ciphertext2));

        let config = EncryptedKeyConfig {
            nonce1: hex::encode(&nonce1_bytes),
            nonce2: hex::encode(&nonce2_bytes),
            double_encrypted_private_key: hex::encode(&ciphertext2),
        };

        // Store the encrypted config in the enclave DB
        let _ = self.send_request(EnclaveRequest::StoreEncryptedConfig {
            user_id: user_id.to_string(),
            nonce1: config.nonce1.clone(),
            nonce2: config.nonce2.clone(),
            double_encrypted_private_key: config.double_encrypted_private_key.clone(),
        });

        self.config = Some(config.clone());
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
        Ok(config_json)
    }

    /// Logs in a user by verifying the password and decrypting the private key
    pub fn login(&mut self, user_id: &str, password: &str) -> Result<bool, KeyManagerError> {
        let cfg = self.config.as_ref().ok_or(KeyManagerError::InvalidConfig)?;
        println!("[login] nonce1: {}", cfg.nonce1);
        println!("[login] nonce2: {}", cfg.nonce2);
        println!(
            "[login] double_encrypted_private_key: {}",
            cfg.double_encrypted_private_key
        );

        // Get decryption keys from enclave
        let (key1, key2) = self.derive_keys(user_id, password)?;
        println!("[login] key1: {}", hex::encode(key1));
        println!("[login] key2: {}", hex::encode(key2));

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
        println!("[login] ciphertext2: {}", hex::encode(&ciphertext2));

        let decrypted_layer1 = decrypt_chacha20(&key2, &ciphertext2, &nonce2)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
        println!(
            "[login] decrypted_layer1: {}",
            hex::encode(&decrypted_layer1)
        );
        let decrypted_private_key = decrypt_chacha20(&key1, &decrypted_layer1, &nonce1)
            .map_err(|e| KeyManagerError::DecryptionError(e.to_string()))?;
        println!(
            "[login] decrypted_private_key: {}",
            hex::encode(&decrypted_private_key)
        );

        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&decrypted_private_key);
        *self.decrypted_private_key.lock().unwrap() = Some(private_key);
        Ok(true)
    }

    /// Retrieves the decrypted private key, if available
    pub fn get_private_key(&self) -> Option<[u8; 32]> {
        self.decrypted_private_key.lock().unwrap().clone()
    }

    /// Retrieves the public key derived from the private key, if available
    pub fn get_public_key(&self) -> Option<[u8; 32]> {
        self.decrypted_private_key
            .lock()
            .unwrap()
            .map(|private_key| {
                let mut public_key = [0u8; 32];
                // For now, we'll just use a simple XOR with a constant as a placeholder
                // In a real implementation, this would use proper public key derivation
                for (i, byte) in private_key.iter().enumerate() {
                    public_key[i] = byte ^ 0xFF;
                }
                public_key
            })
    }
}

/// Encrypts plaintext using ChaCha20Poly1305
fn encrypt_chacha20(key: &[u8], plaintext: &[u8], nonce: &[u8; 12]) -> Result<Vec<u8>, KeyManagerError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| KeyManagerError::EncryptionError(e.to_string()))?;
    Ok(ciphertext)
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
