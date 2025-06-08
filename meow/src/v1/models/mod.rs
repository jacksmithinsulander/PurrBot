mod create_account;
pub mod log_in_state;
pub mod buttons;
pub mod password_handler;

use once_cell::sync::Lazy;
use password_handler::PasswordHandler;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

pub static PASSWORD_HANDLERS: Lazy<Arc<Mutex<HashMap<i64, Option<PasswordHandler>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
