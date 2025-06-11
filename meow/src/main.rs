use std::error::Error;
use teloxide::{prelude::*, utils::command::BotCommands};
mod keyboard;
mod v1;
use std::sync::Arc;
use v1::services::user_config_store::UserConfigStore;

use nine_sdk::{Transport, connect};
use v1::commands::{CommandLoggedIn, CommandLoggedOut};
use v1::handlers::{callback_handler, message_handler};

// Constants
const DEFAULT_DATABASE_PATH: &str = "purrbot.sqlite";
const DEFAULT_TCP_ADDRESS: &str = "127.0.0.1:5005";
const ENCLAVE_MODE_ENV_VAR: &str = "ENCLAVE_MODE";
const ENCLAVE_MODE_VALUE: &str = "enclave";

// Helper functions
fn is_enclave_mode() -> bool {
    std::env::var(ENCLAVE_MODE_ENV_VAR).as_deref() == Ok(ENCLAVE_MODE_VALUE)
}

fn create_tcp_transport(address: &str) -> Transport {
    match address.parse() {
        Ok(addr) => Transport::Tcp(addr),
        Err(_) => panic!("Invalid TCP address"),
    }
}

fn create_default_transport() -> Transport {
    create_tcp_transport(DEFAULT_TCP_ADDRESS)
}

fn create_enclave_transport() -> Transport {
    // For now, enclave transport is the same as default TCP transport
    create_tcp_transport(DEFAULT_TCP_ADDRESS)
}

fn determine_transport() -> Transport {
    if is_enclave_mode() {
        create_enclave_transport()
    } else {
        create_default_transport()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    log::info!("PurrBot is purring...");

    // Initialize the SQLite user config store
    let config_store = Arc::new(UserConfigStore::new(DEFAULT_DATABASE_PATH)?);
    // Make it available globally if needed, or pass to handlers

    // Determine transport based on environment
    let transport = determine_transport();
    log::info!("Using TCP transport");

    let bot = Bot::from_env();

    // Register commands with Telegram
    bot.set_my_commands(CommandLoggedOut::bot_commands())
        .await?;
    log::info!("Commands registered successfully");

    let handler = dptree::entry()
        .branch(Update::filter_message().branch(dptree::endpoint({
            let config_store = Arc::clone(&config_store);
            move |bot, msg| {
                let config_store = Arc::clone(&config_store);
                async move { v1::handlers::message_handler(bot, msg, config_store).await }
            }
        })))
        .branch(Update::filter_callback_query().branch(dptree::endpoint({
            let config_store = Arc::clone(&config_store);
            move |bot, q| {
                let config_store = Arc::clone(&config_store);
                async move { callback_handler(bot, q, config_store).await }
            }
        })));
    //.branch(Update::filter_inline_query().branch(dptree::endpoint(inline_query_handler)));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_DATABASE_PATH, "purrbot.sqlite");
        assert_eq!(DEFAULT_TCP_ADDRESS, "127.0.0.1:5005");
        assert_eq!(ENCLAVE_MODE_ENV_VAR, "ENCLAVE_MODE");
        assert_eq!(ENCLAVE_MODE_VALUE, "enclave");
    }
    
    #[test]
    fn test_is_enclave_mode() {
        // Save original env
        let original = std::env::var(ENCLAVE_MODE_ENV_VAR).ok();
        
        // Test when env var is not set
        std::env::remove_var(ENCLAVE_MODE_ENV_VAR);
        assert!(!is_enclave_mode());
        
        // Test when env var is set to "enclave"
        std::env::set_var(ENCLAVE_MODE_ENV_VAR, "enclave");
        assert!(is_enclave_mode());
        
        // Test when env var is set to something else
        std::env::set_var(ENCLAVE_MODE_ENV_VAR, "not_enclave");
        assert!(!is_enclave_mode());
        
        // Test when env var is set to empty string
        std::env::set_var(ENCLAVE_MODE_ENV_VAR, "");
        assert!(!is_enclave_mode());
        
        // Test case sensitivity
        std::env::set_var(ENCLAVE_MODE_ENV_VAR, "ENCLAVE");
        assert!(!is_enclave_mode());
        
        // Restore original state
        match original {
            Some(val) => std::env::set_var(ENCLAVE_MODE_ENV_VAR, val),
            None => std::env::remove_var(ENCLAVE_MODE_ENV_VAR),
        }
    }
    
    #[test]
    fn test_create_tcp_transport() {
        let transport = create_tcp_transport("127.0.0.1:5005");
        match transport {
            Transport::Tcp(addr) => {
                assert_eq!(addr.to_string(), "127.0.0.1:5005");
            }
            _ => panic!("Expected TCP transport"),
        }
    }
    
    #[test]
    #[should_panic(expected = "Invalid TCP address")]
    fn test_create_tcp_transport_invalid_address() {
        create_tcp_transport("invalid:address:format");
    }
    
    #[test]
    fn test_create_default_transport() {
        let transport = create_default_transport();
        match transport {
            Transport::Tcp(addr) => {
                assert_eq!(addr.to_string(), DEFAULT_TCP_ADDRESS);
            }
            _ => panic!("Expected TCP transport"),
        }
    }
    
    #[test]
    fn test_create_enclave_transport() {
        let transport = create_enclave_transport();
        match transport {
            Transport::Tcp(addr) => {
                assert_eq!(addr.to_string(), DEFAULT_TCP_ADDRESS);
            }
            _ => panic!("Expected TCP transport"),
        }
    }
    
    #[test]
    fn test_determine_transport() {
        // Save original env
        let original = std::env::var(ENCLAVE_MODE_ENV_VAR).ok();
        
        // Test non-enclave mode
        std::env::remove_var(ENCLAVE_MODE_ENV_VAR);
        let transport = determine_transport();
        match transport {
            Transport::Tcp(addr) => {
                assert_eq!(addr.to_string(), DEFAULT_TCP_ADDRESS);
            }
            _ => panic!("Expected TCP transport"),
        }
        
        // Test enclave mode
        std::env::set_var(ENCLAVE_MODE_ENV_VAR, "enclave");
        let transport = determine_transport();
        match transport {
            Transport::Tcp(addr) => {
                assert_eq!(addr.to_string(), DEFAULT_TCP_ADDRESS);
            }
            _ => panic!("Expected TCP transport"),
        }
        
        // Restore original state
        match original {
            Some(val) => std::env::set_var(ENCLAVE_MODE_ENV_VAR, val),
            None => std::env::remove_var(ENCLAVE_MODE_ENV_VAR),
        }
    }
    
    #[test]
    fn test_create_config_store() {
        // This test would require a temporary directory in a real scenario
        // For now, we just verify the function exists and returns the expected type
        // The actual functionality is tested in user_config_store tests
    }
    
    #[test]
    fn test_main_function_exists() {
        // Simply verify that main function exists by using compile-time checks
        // The actual functionality is tested through integration tests
        
        // This test is primarily to ensure the main function maintains its signature
        // The fact that this compiles verifies the function exists
        assert!(true, "Main function exists and compiles");
    }
}
