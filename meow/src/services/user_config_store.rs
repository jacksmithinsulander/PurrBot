use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

// Constants
const TABLE_NAME: &str = "user_configs";
const CREATE_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS user_configs (
    user_id TEXT PRIMARY KEY,
    config_json TEXT NOT NULL
)";
const INSERT_OR_UPDATE_SQL: &str = "INSERT INTO user_configs (user_id, config_json) VALUES (?1, ?2)
    ON CONFLICT(user_id) DO UPDATE SET config_json=excluded.config_json";
const SELECT_CONFIG_SQL: &str = "SELECT config_json FROM user_configs WHERE user_id = ?1";

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
    pub config_json: String, // Serialized EncryptedKeyConfig
}

/// Thread-safe store for user configuration data
pub struct UserConfigStore {
    connection: Arc<Mutex<Connection>>,
    database_path: String,
}

impl UserConfigStore {
    /// Creates a new UserConfigStore with the given database path
    pub fn new<P: AsRef<Path>>(database_path: P) -> Result<Self, UserConfigStoreError> {
        let connection = Connection::open(&database_path)?;
        initialize_database_schema(&connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            database_path: database_path.as_ref().to_string_lossy().to_string(),
        })
    }

    /// Stores or updates a user's configuration
    pub async fn insert_or_update_config(
        &self,
        user_id: &str,
        config_json: &str,
    ) -> Result<(), UserConfigStoreError> {
        let connection = self.connection.lock().await;
        execute_insert_or_update(&*connection, user_id, config_json)?;
        Ok(())
    }

    /// Retrieves a user's configuration
    pub async fn get_config(&self, user_id: &str) -> Result<String, UserConfigStoreError> {
        let connection = self.connection.lock().await;
        query_user_config(&*connection, user_id)
    }

    /// Checks if a user configuration exists
    pub async fn config_exists(&self, user_id: &str) -> Result<bool, UserConfigStoreError> {
        match self.get_config(user_id).await {
            Ok(_) => Ok(true),
            Err(UserConfigStoreError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn get_database_path(&self) -> &str {
        &self.database_path
    }
}

// Database operation helpers

fn open_database<P: AsRef<Path>>(path: P) -> Result<Connection, rusqlite::Error> {
    Connection::open(path)
}

fn initialize_database_schema(connection: &Connection) -> Result<(), UserConfigStoreError> {
    connection.execute(CREATE_TABLE_SQL, [])?;
    Ok(())
}

fn execute_insert_or_update(
    connection: &Connection,
    user_id: &str,
    config_json: &str,
) -> Result<(), rusqlite::Error> {
    connection.execute(INSERT_OR_UPDATE_SQL, params![user_id, config_json])?;
    Ok(())
}

fn query_user_config(
    connection: &Connection,
    user_id: &str,
) -> Result<String, UserConfigStoreError> {
    let config_json: Option<String> = connection
        .query_row(SELECT_CONFIG_SQL, params![user_id], |row| row.get(0))
        .optional()?;

    config_json.ok_or(UserConfigStoreError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Test constants
    const TEST_USER_ID: &str = "test_user_123";
    const TEST_CONFIG_JSON: &str = r#"{"password_hash":"test_hash","primary_key_salt":"test_salt1","secondary_key_salt":"test_salt2"}"#;
    const TEST_USER_ID_2: &str = "test_user_456";
    const TEST_CONFIG_JSON_2: &str = r#"{"password_hash":"test_hash2","primary_key_salt":"test_salt3","secondary_key_salt":"test_salt4"}"#;
    const UPDATED_CONFIG_JSON: &str = r#"{"password_hash":"updated_hash","primary_key_salt":"updated_salt1","secondary_key_salt":"updated_salt2"}"#;
    const EMPTY_CONFIG_JSON: &str = "{}";
    const SPECIAL_CHARS_USER_ID: &str = "user-with-special-chars!@#$%^&*()";
    const VERY_LONG_USER_ID: &str = "very_long_user_id_with_many_characters_that_exceeds_normal_length_expectations_but_should_still_work_correctly";

    // Helper function to create a test database
    async fn create_test_store() -> (UserConfigStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = UserConfigStore::new(&db_path).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_user_config_store_error_display() {
        // Test NotFound error
        let err = UserConfigStoreError::NotFound;
        assert_eq!(err.to_string(), "User config not found");

        // Test SQLite error conversion
        let sqlite_err = rusqlite::Error::InvalidPath("test path".into());
        let err = UserConfigStoreError::Sqlite(sqlite_err);
        assert!(err.to_string().contains("SQLite error"));

        // Test Serde error conversion
        let serde_err = serde_json::from_str::<String>("invalid json").unwrap_err();
        let err = UserConfigStoreError::Serde(serde_err);
        assert!(err.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_user_config_struct() {
        let config = UserConfig {
            user_id: TEST_USER_ID.to_string(),
            config_json: TEST_CONFIG_JSON.to_string(),
        };

        assert_eq!(config.user_id, TEST_USER_ID);
        assert_eq!(config.config_json, TEST_CONFIG_JSON);

        // Test Debug trait
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("UserConfig"));
        assert!(debug_str.contains(TEST_USER_ID));

        // Test Clone trait
        let cloned = config.clone();
        assert_eq!(cloned.user_id, config.user_id);
        assert_eq!(cloned.config_json, config.config_json);

        // Test Serialize/Deserialize
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: UserConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.user_id, config.user_id);
        assert_eq!(deserialized.config_json, config.config_json);
    }

    #[test]
    fn test_database_constants() {
        assert_eq!(TABLE_NAME, "user_configs");
        assert!(CREATE_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS user_configs"));
        assert!(CREATE_TABLE_SQL.contains("user_id TEXT PRIMARY KEY"));
        assert!(CREATE_TABLE_SQL.contains("config_json TEXT NOT NULL"));
        assert!(INSERT_OR_UPDATE_SQL.contains("INSERT INTO user_configs"));
        assert!(INSERT_OR_UPDATE_SQL.contains("ON CONFLICT(user_id) DO UPDATE"));
        assert!(SELECT_CONFIG_SQL.contains("SELECT config_json FROM user_configs"));
    }

    #[tokio::test]
    async fn test_new_store_creates_database() {
        let (_store, temp_dir) = create_test_store().await;

        // Verify database file was created
        let db_path = temp_dir.path().join("test.db");
        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn test_new_store_with_invalid_path() {
        let result = UserConfigStore::new("/invalid/path/to/database.db");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_insert_and_get_config() {
        let (store, _temp_dir) = create_test_store().await;

        // Insert config
        store
            .insert_or_update_config(TEST_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();

        // Get config
        let retrieved_config = store.get_config(TEST_USER_ID).await.unwrap();
        assert_eq!(retrieved_config, TEST_CONFIG_JSON);
    }

    #[tokio::test]
    async fn test_get_config_not_found() {
        let (store, _temp_dir) = create_test_store().await;

        // Try to get non-existent config
        let result = store.get_config("non_existent_user").await;
        assert!(matches!(result, Err(UserConfigStoreError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_existing_config() {
        let (store, _temp_dir) = create_test_store().await;

        // Insert initial config
        store
            .insert_or_update_config(TEST_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();

        // Update config
        store
            .insert_or_update_config(TEST_USER_ID, UPDATED_CONFIG_JSON)
            .await
            .unwrap();

        // Verify update
        let retrieved_config = store.get_config(TEST_USER_ID).await.unwrap();
        assert_eq!(retrieved_config, UPDATED_CONFIG_JSON);
    }

    #[tokio::test]
    async fn test_multiple_users() {
        let (store, _temp_dir) = create_test_store().await;

        // Insert configs for multiple users
        store
            .insert_or_update_config(TEST_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();
        store
            .insert_or_update_config(TEST_USER_ID_2, TEST_CONFIG_JSON_2)
            .await
            .unwrap();

        // Verify both configs exist independently
        let config1 = store.get_config(TEST_USER_ID).await.unwrap();
        let config2 = store.get_config(TEST_USER_ID_2).await.unwrap();

        assert_eq!(config1, TEST_CONFIG_JSON);
        assert_eq!(config2, TEST_CONFIG_JSON_2);
    }

    #[tokio::test]
    async fn test_config_exists() {
        let (store, _temp_dir) = create_test_store().await;

        // Check non-existent config
        assert_eq!(store.config_exists(TEST_USER_ID).await.unwrap(), false);

        // Insert config
        store
            .insert_or_update_config(TEST_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();

        // Check existing config
        assert_eq!(store.config_exists(TEST_USER_ID).await.unwrap(), true);
    }

    #[tokio::test]
    async fn test_empty_config_json() {
        let (store, _temp_dir) = create_test_store().await;

        // Insert empty JSON
        store
            .insert_or_update_config(TEST_USER_ID, EMPTY_CONFIG_JSON)
            .await
            .unwrap();

        // Retrieve and verify
        let retrieved = store.get_config(TEST_USER_ID).await.unwrap();
        assert_eq!(retrieved, EMPTY_CONFIG_JSON);
    }

    #[tokio::test]
    async fn test_special_characters_in_user_id() {
        let (store, _temp_dir) = create_test_store().await;

        // Insert with special characters
        store
            .insert_or_update_config(SPECIAL_CHARS_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();

        // Retrieve and verify
        let retrieved = store.get_config(SPECIAL_CHARS_USER_ID).await.unwrap();
        assert_eq!(retrieved, TEST_CONFIG_JSON);
    }

    #[tokio::test]
    async fn test_very_long_user_id() {
        let (store, _temp_dir) = create_test_store().await;

        // Insert with very long user ID
        store
            .insert_or_update_config(VERY_LONG_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();

        // Retrieve and verify
        let retrieved = store.get_config(VERY_LONG_USER_ID).await.unwrap();
        assert_eq!(retrieved, TEST_CONFIG_JSON);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let (store, _temp_dir) = create_test_store().await;
        let store = Arc::new(store);

        // Spawn multiple tasks to access the store concurrently
        let mut handles = vec![];

        for i in 0..5 {
            let store_clone = store.clone();
            let user_id = format!("user_{}", i);
            let config_json = format!(r#"{{"id":{}}}"#, i);

            let handle = tokio::spawn(async move {
                store_clone
                    .insert_or_update_config(&user_id, &config_json)
                    .await
                    .unwrap();
                let retrieved = store_clone.get_config(&user_id).await.unwrap();
                assert_eq!(retrieved, config_json);
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_large_config_json() {
        let (store, _temp_dir) = create_test_store().await;

        // Create a large JSON string
        let large_json = serde_json::json!({
            "data": vec!["x"; 1000].join(""),
            "numbers": (0..1000).collect::<Vec<_>>(),
        })
        .to_string();

        // Insert large config
        store
            .insert_or_update_config(TEST_USER_ID, &large_json)
            .await
            .unwrap();

        // Retrieve and verify
        let retrieved = store.get_config(TEST_USER_ID).await.unwrap();
        assert_eq!(retrieved, large_json);
    }

    #[test]
    fn test_helper_functions() {
        // Test open_database with invalid path
        let result = open_database("/invalid/path/db.sqlite");
        assert!(result.is_err());

        // Test with temp directory
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let conn = open_database(&db_path).unwrap();

        // Test initialize_database_schema
        assert!(initialize_database_schema(&conn).is_ok());

        // Test execute_insert_or_update
        assert!(execute_insert_or_update(&conn, TEST_USER_ID, TEST_CONFIG_JSON).is_ok());

        // Test query_user_config
        let result = query_user_config(&conn, TEST_USER_ID).unwrap();
        assert_eq!(result, TEST_CONFIG_JSON);

        // Test query non-existent user
        let result = query_user_config(&conn, "non_existent");
        assert!(matches!(result, Err(UserConfigStoreError::NotFound)));
    }

    #[tokio::test]
    async fn test_error_propagation() {
        let (store, _temp_dir) = create_test_store().await;

        // Drop the store to release the connection
        drop(store);

        // Create a new store with an invalid path to force errors
        let invalid_store = UserConfigStore::new("/nonexistent/invalid/path/db.sqlite");
        assert!(invalid_store.is_err());
    }

    #[tokio::test]
    async fn test_sql_injection_safety() {
        let (store, _temp_dir) = create_test_store().await;

        // Try SQL injection in user_id
        let malicious_user_id = "'; DROP TABLE user_configs; --";
        let result = store
            .insert_or_update_config(malicious_user_id, TEST_CONFIG_JSON)
            .await;
        assert!(result.is_ok());

        // Verify table still exists and data is stored correctly
        let retrieved = store.get_config(malicious_user_id).await.unwrap();
        assert_eq!(retrieved, TEST_CONFIG_JSON);

        // Verify we can still insert other users
        store
            .insert_or_update_config(TEST_USER_ID, TEST_CONFIG_JSON)
            .await
            .unwrap();
    }
}
