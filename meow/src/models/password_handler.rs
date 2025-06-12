use crate::services::user_config_store::{UserConfigStore, UserConfigStoreError};
use alloy_primitives::{Address, B256};
use alloy_signer_local::{LocalSigner, PrivateKeySigner};
use nine_sdk::{EncryptedKeyConfig, KeyManager};
use rand;
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserWalletConfig {
    pub encrypted_key_config: EncryptedKeyConfig,
    pub encrypted_ethereum_private_key: String,
    pub ethereum_public_key: String,
    pub ethereum_address: String,
    pub nonce: String, // For encryption
}

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("Key manager error: {0}")]
    KeyManagerError(#[from] nine_sdk::KeyManagerError),
    #[error("User config store error: {0}")]
    UserConfigStore(#[from] UserConfigStoreError),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Wallet generation error: {0}")]
    WalletError(String),
}

pub struct PasswordHandler {
    key_manager: Arc<Mutex<KeyManager>>,
    config_store: Arc<UserConfigStore>,
    ethereum_wallet: Arc<Mutex<Option<PrivateKeySigner>>>,
}

impl PasswordHandler {
    pub fn new(
        config_store: Arc<UserConfigStore>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = KeyManager::new();
        Ok(Self {
            key_manager: Arc::new(Mutex::new(key_manager)),
            config_store,
            ethereum_wallet: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn sign_up(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Generate encryption keys
        let mut key_manager = self.key_manager.lock().await;
        let config_json = key_manager.setup_config(password).await.map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })?;

        let encrypted_key_config: EncryptedKeyConfig = serde_json::from_str(&config_json)?;

        // Derive encryption keys to encrypt the Ethereum private key
        let (key1, _key2) = key_manager
            .verify_and_derive_keys(password)
            .await
            .map_err(|e| {
                Box::new(PasswordError::KeyManagerError(e))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;

        // Generate Ethereum wallet
        let wallet = PrivateKeySigner::random();
        let ethereum_private_key = hex::encode(wallet.to_bytes());
        let ethereum_public_key = hex::encode(wallet.address().as_slice());
        let ethereum_address = format!("{:?}", wallet.address());

        // Store wallet in memory
        {
            let mut eth_wallet = self.ethereum_wallet.lock().await;
            *eth_wallet = Some(wallet);
        }

        // Encrypt the private key using ChaCha20Poly1305
        let mut nonce = [0u8; 12];
        rand::Rng::fill(&mut rand::thread_rng(), &mut nonce);

        let encrypted_private_key =
            nine_sdk::encrypt_chacha20(&key1, ethereum_private_key.as_bytes(), &nonce).map_err(
                |e| {
                    Box::new(PasswordError::EncryptionError(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                },
            )?;

        // Create wallet config
        let wallet_config = UserWalletConfig {
            encrypted_key_config,
            encrypted_ethereum_private_key: hex::encode(&encrypted_private_key),
            ethereum_public_key,
            ethereum_address,
            nonce: hex::encode(&nonce),
        };

        let wallet_config_json = serde_json::to_string_pretty(&wallet_config)?;

        // Persist config JSON to DB
        self.config_store
            .insert_or_update_config(user_id, &wallet_config_json)
            .await?;

        Ok(wallet_config_json)
    }

    pub async fn login(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Load config from DB
        let config_json: String = self.config_store.get_config(user_id).await?;
        let wallet_config: UserWalletConfig = serde_json::from_str(&config_json)?;

        let mut key_manager = self.key_manager.lock().await;
        // Set config in KeyManager
        key_manager.set_config(wallet_config.encrypted_key_config.clone());

        // Attempt to verify and derive keys
        match key_manager.verify_and_derive_keys(password).await {
            Ok((key1, _key2)) => {
                // Decrypt the Ethereum private key
                let nonce_bytes = hex::decode(&wallet_config.nonce).map_err(|e| {
                    Box::new(PasswordError::EncryptionError(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(&nonce_bytes[..12]);

                let encrypted_key_bytes =
                    hex::decode(&wallet_config.encrypted_ethereum_private_key).map_err(|e| {
                        Box::new(PasswordError::EncryptionError(e.to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?;

                let decrypted_key = nine_sdk::decrypt_chacha20(&key1, &encrypted_key_bytes, &nonce)
                    .map_err(|e| {
                        Box::new(PasswordError::EncryptionError(e.to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?;

                let private_key_hex = String::from_utf8(decrypted_key).map_err(|e| {
                    Box::new(PasswordError::EncryptionError(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;

                let private_key_bytes = hex::decode(&private_key_hex).map_err(|e| {
                    Box::new(PasswordError::EncryptionError(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;

                // Convert to B256 for alloy
                let mut key_array = [0u8; 32];
                key_array.copy_from_slice(&private_key_bytes[..32]);
                let b256_key = B256::from(key_array);

                // Reconstruct the wallet from the private key
                let wallet = PrivateKeySigner::from_bytes(&b256_key).map_err(|e| {
                    Box::new(PasswordError::WalletError(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;

                // Store wallet in memory
                {
                    let mut eth_wallet = self.ethereum_wallet.lock().await;
                    *eth_wallet = Some(wallet);
                }

                Ok(true)
            }
            Err(e) => {
                if matches!(e, nine_sdk::KeyManagerError::AuthenticationFailed) {
                    Ok(false)
                } else {
                    Err(Box::new(PasswordError::KeyManagerError(e)))
                }
            }
        }
    }

    pub async fn get_private_key(
        &self,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let eth_wallet = self.ethereum_wallet.lock().await;
        if let Some(wallet) = eth_wallet.as_ref() {
            Ok(Some(wallet.to_bytes().to_vec()))
        } else {
            Ok(None)
        }
    }

    pub async fn get_public_key(
        &self,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let eth_wallet = self.ethereum_wallet.lock().await;
        if let Some(wallet) = eth_wallet.as_ref() {
            Ok(Some(wallet.address().as_slice().to_vec()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_ethereum_key_generation_and_recovery() {
        // Create a temporary database
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path().to_str().unwrap();

        // Create user config store
        let config_store = Arc::new(UserConfigStore::new(db_path).unwrap());

        // Create password handler
        let handler = PasswordHandler::new(config_store.clone()).unwrap();

        // Test user credentials
        let user_id = "test_user_123";
        let password = "strong_password_123!";

        // Sign up (generates Ethereum wallet)
        let config_json = handler.sign_up(user_id, password).await.unwrap();

        // Parse the config to verify it has the expected fields
        let wallet_config: UserWalletConfig = serde_json::from_str(&config_json).unwrap();
        assert!(!wallet_config.encrypted_ethereum_private_key.is_empty());
        assert!(!wallet_config.ethereum_public_key.is_empty());
        assert!(!wallet_config.ethereum_address.is_empty());
    }
}
