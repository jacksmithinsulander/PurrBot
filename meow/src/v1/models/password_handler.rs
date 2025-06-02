use nine_sdk::PrivateKeyManager;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("Key manager error: {0}")]
    KeyManagerError(#[from] nine_sdk::KeyManagerError),
    #[error("Invalid state")]
    InvalidState,
}

pub struct PasswordHandler {
    key_manager: Arc<Mutex<PrivateKeyManager>>,
}

impl PasswordHandler {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = PrivateKeyManager::new()?;
        Ok(Self {
            key_manager: Arc::new(Mutex::new(key_manager)),
        })
    }

    pub async fn sign_up(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut key_manager = self.key_manager.lock().await;
        key_manager.sign_up(user_id, password, None).map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    pub async fn login(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut key_manager = self.key_manager.lock().await;

        // First load the user's config
        key_manager.load_config(user_id).map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })?;

        // Verify the user_id matches the loaded config
        if !key_manager.verify_user_id(user_id).map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })? {
            return Err(Box::new(PasswordError::InvalidState));
        }

        // Now attempt to login with the password
        key_manager.login(user_id, password).map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    pub async fn get_private_key(
        &self,
    ) -> Result<Option<[u8; 32]>, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = self.key_manager.lock().await;
        Ok(key_manager.get_private_key())
    }

    pub async fn get_public_key(
        &self,
    ) -> Result<Option<[u8; 32]>, Box<dyn std::error::Error + Send + Sync>> {
        let key_manager = self.key_manager.lock().await;
        Ok(key_manager.get_public_key())
    }
}
