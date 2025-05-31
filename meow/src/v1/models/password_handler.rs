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
        password: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut key_manager = self.key_manager.lock().await;
        key_manager.sign_up(password, None).map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    pub async fn login(
        &self,
        password: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut key_manager = self.key_manager.lock().await;
        key_manager.login(password).map_err(|e| {
            Box::new(PasswordError::KeyManagerError(e)) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    // pub async fn get_private_key(
    // &self,
    // ) -> Result<Option<[u8; 32]>, Box<dyn std::error::Error + Send + Sync>> {
    // let key_manager = self.key_manager.lock().await;
    // Ok(key_manager.get_private_key())
    // }
}
