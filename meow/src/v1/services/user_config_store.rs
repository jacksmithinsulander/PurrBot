use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum UserConfigStoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("User config not found")]
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub user_id: String,
    pub config_json: String, // serialized EncryptedKeyConfig
}

pub struct UserConfigStore {
    conn: Arc<Mutex<Connection>>,
}

impl UserConfigStore {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, UserConfigStoreError> {
        let conn = Connection::open(db_path)?;
        // Run migration synchronously before entering async context
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_configs (
                user_id TEXT PRIMARY KEY,
                config_json TEXT NOT NULL
            )",
            [],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn insert_or_update_config(&self, user_id: &str, config_json: &str) -> Result<(), UserConfigStoreError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO user_configs (user_id, config_json) VALUES (?1, ?2)
             ON CONFLICT(user_id) DO UPDATE SET config_json=excluded.config_json",
            params![user_id, config_json],
        )?;
        Ok(())
    }

    pub async fn get_config(&self, user_id: &str) -> Result<String, UserConfigStoreError> {
        let conn = self.conn.lock().await;
        let config_json: Option<String> = conn
            .query_row(
                "SELECT config_json FROM user_configs WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;
        config_json.ok_or(UserConfigStoreError::NotFound)
    }
} 