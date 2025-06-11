use crate::models::buttons::Button;
use crate::services::user_config_store::UserConfigStore;
use crate::models::{PASSWORD_HANDLERS, log_in_state};
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{ChatId, Message, Me, MessageId},
};
use mockall::predicate::*;
use mockall::mock;

// Mock Bot for testing
mock! {
    Bot {
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

#[test]
fn test_button_from_str_logged_in() {
    assert!(matches!(Button::from_str("List", true), Button::List));
    assert!(matches!(Button::from_str("Trade", true), Button::Trade));
    assert!(matches!(Button::from_str("Create", true), Button::Create));
    assert!(matches!(Button::from_str("Log Out", true), Button::LogOut));
    assert!(matches!(Button::from_str("Print Keys", true), Button::PrintKeys));
    assert!(matches!(Button::from_str("Invalid", true), Button::UnRecognized));
}

#[test]
fn test_button_from_str_logged_out() {
    assert!(matches!(Button::from_str("Log In", false), Button::LogIn));
    assert!(matches!(Button::from_str("Sign Up", false), Button::SignUp));
    assert!(matches!(Button::from_str("FAQ", false), Button::Faq));
    assert!(matches!(Button::from_str("Invalid", false), Button::UnRecognized));
}

#[test]
fn test_button_from_str_case_sensitivity() {
    assert!(matches!(Button::from_str("list", true), Button::UnRecognized));
    assert!(matches!(Button::from_str("LIST", true), Button::UnRecognized));
    assert!(matches!(Button::from_str("log in", false), Button::UnRecognized));
    assert!(matches!(Button::from_str("LOG IN", false), Button::UnRecognized));
}

#[test]
fn test_button_from_str_whitespace() {
    assert!(matches!(Button::from_str(" List ", true), Button::UnRecognized));
    assert!(matches!(Button::from_str("Log In ", false), Button::UnRecognized));
    assert!(matches!(Button::from_str("", true), Button::UnRecognized));
    assert!(matches!(Button::from_str("   ", true), Button::UnRecognized));
}

#[tokio::test]
async fn test_button_execute_list() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("📋 Listing your items..."))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::List.execute(mock_bot, chat_id, config_store, true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_button_execute_list_error() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations - send_message fails
    mock_bot.expect_send_message()
        .times(1)
        .returning(|_, _| Err(teloxide::RequestError::InvalidResponse("Test error".to_string())));

    // Execute test
    let result = Button::List.execute(mock_bot, chat_id, config_store, true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_button_execute_faq() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq(crate::constants::MAN_PAGE))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::Faq.execute(mock_bot, chat_id, config_store, false).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_button_execute_signup() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("Choose your password:"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::SignUp.execute(mock_bot, chat_id, config_store, false).await;
    assert!(result.is_ok());

    // Verify state was set
    let states = log_in_state::USER_STATES.lock().await;
    assert_eq!(states.get(&chat_id.0), Some(&log_in_state::AwaitingState::AwaitingSignUpPassword));
}

#[tokio::test]
async fn test_button_execute_signup_error() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations - first message succeeds, error message fails
    mock_bot.expect_send_message()
        .times(2)
        .returning(|_, text| {
            if text == "Choose your password:" {
                Ok(create_response_message(chat_id, &text))
            } else {
                Err(teloxide::RequestError::InvalidResponse("Test error".to_string()))
            }
        });

    // Execute test
    let result = Button::SignUp.execute(mock_bot, chat_id, config_store, false).await;
    assert!(result.is_ok()); // Should still succeed as we handle the error
}

#[tokio::test]
async fn test_button_execute_unrecognized() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ Not a valid command"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::UnRecognized.execute(mock_bot, chat_id, config_store, true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_button_execute_unrecognized_logged_out() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ Not a valid command"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::UnRecognized.execute(mock_bot, chat_id, config_store, false).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_button_execute_logout() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ You are not logged in!"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::LogOut.execute(mock_bot, chat_id, config_store, true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_button_execute_print_keys() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("❌ No keys available. Please log in first."))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::PrintKeys.execute(mock_bot, chat_id, config_store, true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_button_execute_login() {
    let mut mock_bot = MockBot::new();
    let chat_id = ChatId(123);
    let config_store = Arc::new(UserConfigStore::new());

    // Setup expectations
    mock_bot.expect_send_message()
        .with(eq(chat_id), eq("Please enter your password:"))
        .times(1)
        .returning(|_, text| Ok(create_response_message(chat_id, &text)));

    // Execute test
    let result = Button::LogIn.execute(mock_bot, chat_id, config_store, false).await;
    assert!(result.is_ok());
} 