use crate::processors::message_processor::{delete_all_messages, print_keys, logout, process_message};
use crate::models::{PASSWORD_HANDLERS, log_in_state};
use crate::services::user_config_store::UserConfigStore;
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{ChatId, Message, Me, MessageId},
};
use mockall::predicate::*;
use mockall::mock;
use std::error::Error;

// Mock Bot for testing
mock! {
    Bot {
        fn delete_message(&self, chat_id: ChatId, message_id: MessageId) -> ResponseResult<bool>;
        fn send_message(&self, chat_id: ChatId, text: String) -> ResponseResult<Message>;
        fn set_my_commands(&self, commands: Vec<BotCommand>) -> ResponseResult<bool>;
    }
}

// Helper function to create a test message
fn create_test_message(chat_id: ChatId, text: &str) -> Message {
    Message {
        id: MessageId(1),
        chat: Chat {
            id: chat_id,
            kind: ChatKind::Private(PrivateChat {
                id: chat_id,
                username: None,
                first_name: "Test".to_string(),
                last_name: None,
            }),
        },
        date: 0,
        kind: MessageKind::Common(CommonMessage {
            from: None,
            sender_chat: None,
            forward: None,
            reply_to_message: None,
            edit_date: None,
            media_kind: MediaKind::Text(Text {
                text: text.to_string(),
                entities: vec![],
            }),
            reply_markup: None,
        }),
    }
}

// Helper function to create a test response message
fn create_response_message(chat_id: ChatId, text: &str) -> Message {
    Message {
        id: MessageId(2),
        chat: Chat {
            id: chat_id,
            kind: ChatKind::Private(PrivateChat {
                id: chat_id,
                username: None,
                first_name: "Test".to_string(),
                last_name: None,
            }),
        },
        date: 0,
        kind: MessageKind::Common(CommonMessage {
            from: None,
            sender_chat: None,
            forward: None,
            reply_to_message: None,
            edit_date: None,
            media_kind: MediaKind::Text(Text {
                text: text.to_string(),
                entities: vec![],
            }),
            reply_markup: None,
        }),
    }
}

#[tokio::test]
async fn test_delete_all_messages_success() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let message_ids = vec![MessageId(1), MessageId(2)];

    // Setup expectations
    mock_bot.expect_delete_message()
        .times(2)
        .returning(|_, _| Ok(true));

    // Setup test data
    {
        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
        chat_message_ids.insert(chat_id, message_ids);
    }

    // Execute test
    let result = delete_all_messages(chat_id, &mock_bot).await;
    assert!(result.is_ok());

    // Verify cleanup
    let chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    assert!(!chat_message_ids.contains_key(&chat_id));
}

#[tokio::test]
async fn test_delete_all_messages_failure() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let message_ids = vec![MessageId(1), MessageId(2)];

    // Setup expectations - first delete succeeds, second fails
    mock_bot.expect_delete_message()
        .times(2)
        .returning(|_, message_id| {
            if message_id == MessageId(1) {
                Ok(true)
            } else {
                Err(teloxide::RequestError::InvalidResponse("Test error".to_string()))
            }
        });

    // Setup test data
    {
        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
        chat_message_ids.insert(chat_id, message_ids);
    }

    // Execute test - should still succeed as we handle individual message deletion failures
    let result = delete_all_messages(chat_id, &mock_bot).await;
    assert!(result.is_ok());

    // Verify cleanup still happens
    let chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    assert!(!chat_message_ids.contains_key(&chat_id));
}

#[tokio::test]
async fn test_delete_all_messages_empty() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);

    // Setup expectations - no delete calls should be made
    mock_bot.expect_delete_message()
        .times(0)
        .returning(|_, _| Ok(true));

    // Execute test
    let result = delete_all_messages(chat_id, &mock_bot).await;
    assert!(result.is_ok());

    // Verify no messages were stored
    let chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    assert!(!chat_message_ids.contains_key(&chat_id));
}

#[tokio::test]
async fn test_print_keys_not_logged_in() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ No keys available. Please log in first."))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = print_keys(chat_id, &mock_bot).await;
    assert!(result.is_ok());

    // Verify message was stored
    let chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    assert!(chat_message_ids.contains_key(&chat_id));
}

#[tokio::test]
async fn test_print_keys_error_handling() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);

    // Setup expectations - send_message fails
    mock_bot.expect_send_message()
        .times(1)
        .returning(|_, _| Err(teloxide::RequestError::InvalidResponse("Test error".to_string())));

    // Execute test
    let result = print_keys(chat_id, &mock_bot).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_logout_not_logged_in() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ You are not logged in!"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = logout(chat_id, &mock_bot).await;
    assert!(result.is_ok());

    // Verify message was stored
    let chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    assert!(chat_message_ids.contains_key(&chat_id));
}

#[tokio::test]
async fn test_logout_success() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("👋 You have been logged out successfully!"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    mock_bot.expect_set_my_commands()
        .times(1)
        .returning(|_| Ok(true));

    // Setup logged in state
    {
        let mut states = log_in_state::USER_STATES.lock().await;
        states.insert(chat_id.0, log_in_state::AwaitingState::None);
    }

    // Execute test
    let result = logout(chat_id, &mock_bot).await;
    assert!(result.is_ok());

    // Verify state was cleaned up
    let states = log_in_state::USER_STATES.lock().await;
    assert_eq!(states.get(&chat_id.0), Some(&log_in_state::AwaitingState::None));
}

#[tokio::test]
async fn test_process_message_logout_command() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());
    let msg = create_test_message(chat_id, "/logout");

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ You are not logged in!"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = process_message(mock_bot, msg, Me::default(), config_store).await;
    assert!(result.is_ok());

    // Verify message was stored
    let chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    assert!(chat_message_ids.contains_key(&chat_id));
}

#[tokio::test]
async fn test_process_message_invalid_command() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());
    let msg = create_test_message(chat_id, "/invalid");

    // Execute test - should succeed but do nothing
    let result = process_message(mock_bot, msg, Me::default(), config_store).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_message_empty_text() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());
    let msg = create_test_message(chat_id, "");

    // Execute test - should succeed but do nothing
    let result = process_message(mock_bot, msg, Me::default(), config_store).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_message_whitespace_only() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());
    let msg = create_test_message(chat_id, "   ");

    // Execute test - should succeed but do nothing
    let result = process_message(mock_bot, msg, Me::default(), config_store).await;
    assert!(result.is_ok());
} 