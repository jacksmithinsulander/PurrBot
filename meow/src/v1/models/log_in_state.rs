use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwaitingState {
    None,
    AwaitingSignUpPassword,
    AwaitingLoginPassword,
}

pub static USER_STATES: Lazy<Mutex<HashMap<i64, AwaitingState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
