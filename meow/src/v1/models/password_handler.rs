use crate::v1::services::user_config_store::{UserConfigStore, UserConfigStoreError};
use hex;
use nine_sdk::{EncryptedKeyConfig, KeyManager};
use serde_json;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("Key manager error: {0}")]
    KeyManagerError(#[from] nine_sdk::KeyManagerError),
    #[error("Invalid state")]
    InvalidState,
    #[error("User config store error: {0}")]
    UserConfigStore(#[from] UserConfigStoreError),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct PasswordHandler {
    key_manager: Arc<Mutex<KeyManager>>,
    config_store: Arc<UserConfigStore>,
}

impl PasswordHandler {
    pub fn new(
        config_store: Arc<UserConfigStore>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = KeyManager::new();
        Ok(Self {
            key_manager: Arc::new(Mutex::new(key_manager)),
            config_store,
        })
    }

    pub async fn sign_up(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut key_manager = self.key_manager.lock().await;
        let config_json = key_manager.setup_config(password).await.map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })?;
        // Persist config JSON to DB
        self.config_store
            .insert_or_update_config(user_id, &config_json)
            .await?;
        Ok(config_json)
    }

    pub async fn login(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Load config from DB
        let config_json: String = self.config_store.get_config(user_id).await?;
        let config: EncryptedKeyConfig = serde_json::from_str(&config_json)?;
        let mut key_manager = self.key_manager.lock().await;
        // Set config in KeyManager
        key_manager.set_config(config);
        // Attempt to verify and derive keys
        match key_manager.verify_and_derive_keys(password).await {
            Ok(_) => Ok(true),
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
    ) -> Result<Option<[u8; 32]>, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = self.key_manager.lock().await;
        // For now, we'll return None since we need the password to derive the keys
        // TODO: Store the derived keys after login for later use
        Ok(None)
    }

    pub async fn get_public_key(
        &self,
    ) -> Result<Option<[u8; 32]>, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = self.key_manager.lock().await;
        // For now, we'll return None since we need the password to derive the keys
        // TODO: Store the derived keys after login for later use
        Ok(None)
    }
}
