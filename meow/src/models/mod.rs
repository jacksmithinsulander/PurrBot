pub mod buttons;
mod create_account;
pub mod log_in_state;
pub mod password_handler;

use once_cell::sync::Lazy;
use password_handler::PasswordHandler;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub static PASSWORD_HANDLERS: Lazy<Arc<Mutex<HashMap<i64, Option<PasswordHandler>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
