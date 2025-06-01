mod create_account;
pub mod log_in_state;
pub mod logged_out_buttons;
pub mod password_handler;

use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex;
use password_handler::PasswordHandler;

pub static PASSWORD_HANDLER: Lazy<Arc<Mutex<Option<PasswordHandler>>>> = Lazy::new(|| {
    Arc::new(Mutex::new(None))
});
