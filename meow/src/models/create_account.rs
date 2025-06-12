use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountResponse {
    pub success: bool,
    pub message: String,
}

impl CreateAccountResponse {
    pub fn success(message: String) -> Self {
        Self {
            success: true,
            message,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            message,
        }
    }
}
