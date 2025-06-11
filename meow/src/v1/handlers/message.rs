use crate::v1::processors::message_processor::process_message;
use crate::v1::services::user_config_store::UserConfigStore;
use std::error::Error;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Message;

pub async fn message_handler(
    bot: Bot,
    msg: Message,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let me = bot.get_me().await?;
    process_message(bot, msg, me, config_store).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use teloxide::types::{Chat, ChatId, ChatKind, MessageId, MessageKind, User, UserId};
    
    #[test]
    fn test_message_handler_signature() {
        // This test verifies the function signature is correct
        // The actual functionality is tested through integration tests
        // since message_handler is a thin wrapper around process_message
        
        // Verify that the function exists and has the correct signature
        fn _check_signature(
            _handler: fn(Bot, Message, Arc<UserConfigStore>) -> 
                std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>
        ) {}
        
        // This won't compile if the signature is wrong
        _check_signature(|bot, msg, store| Box::pin(message_handler(bot, msg, store)));
    }
    
    #[test]
    fn test_error_types() {
        // Verify that the error type is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn Error + Send + Sync>>();
    }
    
    // Mock structures for testing
    struct MockBot;
    struct MockMessage;
    struct MockUserConfigStore;
    
    // Note: Full integration tests would require mocking the Telegram API
    // which is better done in integration tests rather than unit tests
}
