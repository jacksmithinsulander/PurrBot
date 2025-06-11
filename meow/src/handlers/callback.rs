use crate::processors::callback_processor::process_callback;
use crate::services::user_config_store::UserConfigStore;
use std::error::Error;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

pub async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("callback_handler called! q: {:?}", q);

    // Extract callback data for logging
    if let Some(data) = q.data.as_deref() {
        log::info!("Callback data received: {}", data);
    } else {
        log::warn!("Callback query without data");
    }

    // Extract message info for logging
    if let Some(msg) = &q.message {
        log::info!("Callback from message ID: {}", msg.id());
    }

    // Process the callback
    let result = process_callback(bot, q, config_store).await;

    // Log the result
    match &result {
        Ok(_) => log::info!("Callback processing completed successfully"),
        Err(e) => log::error!("Callback processing failed: {}", e),
    }

    result
}

fn log_callback_result<T, E: std::fmt::Display>(result: &Result<T, E>) {
    match result {
        Ok(_) => log::info!("Callback processed successfully"),
        Err(error) => log::error!("Callback processing failed: {}", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::{CallbackQuery, User, UserId};
    
    // Helper function to create a test user
    fn create_test_user() -> User {
        User {
            id: UserId(12345),
            is_bot: false,
            first_name: "Test".to_string(),
            last_name: Some("User".to_string()),
            username: Some("testuser".to_string()),
            language_code: Some("en".to_string()),
            is_premium: false,
            added_to_attachment_menu: false,
        }
    }
    
    // Helper function to create a test callback query
    fn create_test_callback_query(data: Option<String>) -> CallbackQuery {
        CallbackQuery {
            id: "test_callback_123".to_string(),
            from: create_test_user(),
            message: None,
            inline_message_id: None,
            chat_instance: "test_instance".to_string(),
            data,
            game_short_name: None,
        }
    }
    
    #[test]
    fn test_callback_handler_signature() {
        // Verify that the function exists and has the correct signature
        fn _check_signature(
            _handler: fn(Bot, CallbackQuery, Arc<UserConfigStore>) -> 
                std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>
        ) {}
        
        _check_signature(|bot, query, store| Box::pin(callback_handler(bot, query, store)));
    }
    
    #[test]
    fn test_log_callback_result_success() {
        let result: Result<(), Box<dyn Error + Send + Sync>> = Ok(());
        // This test verifies the function doesn't panic
        log_callback_result(&result);
    }
    
    #[test]
    fn test_log_callback_result_error() {
        let result: Result<(), Box<dyn Error + Send + Sync>> = 
            Err("Test error".into());
        // This test verifies the function doesn't panic
        log_callback_result(&result);
    }
    
    #[test]
    fn test_log_callback_result_custom_error() {
        #[derive(Debug)]
        struct CustomError;
        impl std::fmt::Display for CustomError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Custom error message")
            }
        }
        impl Error for CustomError {}
        
        let result: Result<(), Box<dyn Error + Send + Sync>> = 
            Err(Box::new(CustomError));
        // This test verifies the function doesn't panic
        log_callback_result(&result);
    }
    
    #[test]
    fn test_error_types() {
        // Verify that the error type is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn Error + Send + Sync>>();
    }
    
    #[test]
    fn test_callback_query_creation() {
        let query_with_data = create_test_callback_query(Some("test_data".to_string()));
        assert_eq!(query_with_data.data, Some("test_data".to_string()));
        assert_eq!(query_with_data.id, "test_callback_123");
        assert_eq!(query_with_data.from.id, UserId(12345));
        
        let query_without_data = create_test_callback_query(None);
        assert_eq!(query_without_data.data, None);
    }
}
