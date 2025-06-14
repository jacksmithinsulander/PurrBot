use crate::keyboard::{logged_in_operations, logged_out_operations};
use crate::commands::{CommandLoggedIn, CommandLoggedOut};
use crate::models::{PASSWORD_HANDLERS, log_in_state, password_handler::PasswordHandler};
use hex;
use std::error::Error;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{BotCommandScope, Me, MessageId},
    utils::command::BotCommands,
};

// Track all message IDs (bot and user) per chat
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tokio::sync::Mutex;

/// Global state to track message IDs per chat
pub static CHAT_MESSAGE_IDS: Lazy<Mutex<HashMap<ChatId, Vec<MessageId>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Deletes all messages for a given chat
/// 
/// # Arguments
/// * `chat_id` - The chat ID to delete messages for
/// * `bot` - The bot instance to use for deletion
/// 
/// # Returns
/// * `Result<(), Box<dyn Error + Send + Sync>>` - Result indicating success or failure
pub async fn delete_all_messages(
    chat_id: ChatId,
    bot: &Bot,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("Attempting to delete messages for chat_id={}", chat_id);
    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    
    if let Some(ids) = chat_message_ids.get(&chat_id) {
        log::debug!(
            "Found {} messages to delete for chat_id={}",
            ids.len(),
            chat_id
        );
        delete_messages_for_chat(chat_id, bot, ids).await?;
    } else {
        log::debug!("No messages found to delete for chat_id={}", chat_id);
    }
    
    chat_message_ids.remove(&chat_id);
    log::debug!("Message deletion completed for chat_id={}", chat_id);
    Ok(())
}

/// Helper function to delete messages for a specific chat
async fn delete_messages_for_chat(
    chat_id: ChatId,
    bot: &Bot,
    message_ids: &[MessageId],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    for message_id in message_ids {
        if let Err(e) = bot.delete_message(chat_id, *message_id).await {
            log::warn!(
                "Failed to delete message {} for chat_id={}: {}",
                message_id,
                chat_id,
                e
            );
        }
    }
    Ok(())
}

/// Prints the user's keys if they are logged in
/// 
/// # Arguments
/// * `chat_id` - The chat ID to print keys for
/// * `bot` - The bot instance to use for sending messages
/// 
/// # Returns
/// * `Result<(), Box<dyn Error + Send + Sync>>` - Result indicating success or failure
pub async fn print_keys(chat_id: ChatId, bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("print_keys called for chat_id={}", chat_id);
    delete_all_messages(chat_id, bot).await?;

    let handler = PASSWORD_HANDLERS.lock().await;
    let keys_result = get_user_keys(&handler, chat_id).await;
    
    match keys_result {
        Ok((private_key, public_key)) => {
            send_keys_message(bot, chat_id, private_key, public_key).await?;
        }
        Err(_) => {
            send_no_keys_message(bot, chat_id).await?;
        }
    }

    log::info!("print_keys completed for chat_id={}", chat_id);
    Ok(())
}

/// Helper function to get user keys from handler
async fn get_user_keys(
    handler: &tokio::sync::MutexGuard<'_, HashMap<i64, Option<PasswordHandler>>>,
    chat_id: ChatId,
) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), Box<dyn Error + Send + Sync>> {
    if let Some(Some(handler)) = handler.get(&chat_id.0) {
        let priv_key = handler.get_private_key().await?;
        let pub_key = handler.get_public_key().await?;
        Ok((priv_key, pub_key))
    } else {
        Ok((None, None))
    }
}

/// Helper function to send keys message
async fn send_keys_message(
    bot: &Bot,
    chat_id: ChatId,
    private_key: Option<Vec<u8>>,
    public_key: Option<Vec<u8>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match (private_key, public_key) {
        (Some(private_key), Some(public_key)) => {
            let msg = bot
                .send_message(
                    chat_id,
                    format!(
                        "🔑 Your Keys:\nPrivate Key: {}\nPublic Key: {}",
                        hex::encode(private_key),
                        hex::encode(public_key)
                    ),
                )
                .await?;
            store_message_id(chat_id, msg.id).await;
        }
        _ => {
            send_no_keys_message(bot, chat_id).await?;
        }
    }
    Ok(())
}

/// Helper function to send no keys message
async fn send_no_keys_message(bot: &Bot, chat_id: ChatId) -> Result<(), Box<dyn Error + Send + Sync>> {
    let msg = bot
        .send_message(chat_id, "❌ No keys available. Please log in first.")
        .await?;
    store_message_id(chat_id, msg.id).await;
    Ok(())
}

/// Helper function to store message ID
async fn store_message_id(chat_id: ChatId, message_id: MessageId) {
    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    chat_message_ids.insert(chat_id, vec![message_id]);
}

/// Logs out a user and cleans up their state
/// 
/// # Arguments
/// * `chat_id` - The chat ID to logout
/// * `bot` - The bot instance to use for sending messages
/// 
/// # Returns
/// * `Result<(), Box<dyn Error + Send + Sync>>` - Result indicating success or failure
pub async fn logout(chat_id: ChatId, bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Starting logout process for chat_id={}", chat_id);

    if !is_user_logged_in(chat_id).await {
        return handle_not_logged_in_logout(bot, chat_id).await;
    }

    cleanup_user_state(chat_id).await;
    update_bot_commands(bot, chat_id).await?;
    send_logout_confirmation(bot, chat_id).await?;
    cleanup_messages(chat_id, bot).await?;

    log::info!("Logout process completed successfully for chat_id={}", chat_id);
    Ok(())
}

/// Helper function to check if user is logged in
async fn is_user_logged_in(chat_id: ChatId) -> bool {
    let handlers = PASSWORD_HANDLERS.lock().await;
    handlers.get(&chat_id.0).and_then(|h| h.as_ref()).is_some()
}

/// Helper function to handle logout when user is not logged in
async fn handle_not_logged_in_logout(
    bot: &Bot,
    chat_id: ChatId,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("User {} tried to logout but is not logged in", chat_id.0);
    let message = bot
        .send_message(chat_id, "❌ You are not logged in!")
        .reply_markup(logged_out_operations())
        .await?;
    
    if std::env::var("TEST_MODE").is_err() {
        store_message_id(chat_id, message.id).await;
    }
    
    Ok(())
}

/// Helper function to cleanup user state
async fn cleanup_user_state(chat_id: ChatId) {
    let mut states = log_in_state::USER_STATES.lock().await;
    states.insert(chat_id.0, log_in_state::AwaitingState::None);
    
    let mut handler = PASSWORD_HANDLERS.lock().await;
    handler.remove(&chat_id.0);
    
    log::info!("User state cleaned up for chat_id={}", chat_id);
}

/// Helper function to update bot commands
async fn update_bot_commands(bot: &Bot, chat_id: ChatId) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(e) = bot
        .set_my_commands(CommandLoggedOut::bot_commands())
        .scope(BotCommandScope::Chat {
            chat_id: chat_id.into(),
        })
        .await
    {
        log::warn!("Failed to set commands for chat_id={}: {}", chat_id, e);
        return Err(Box::new(e));
    }
    log::info!("Commands set to logged-out state for chat_id={}", chat_id);
    Ok(())
}

/// Helper function to send logout confirmation
async fn send_logout_confirmation(bot: &Bot, chat_id: ChatId) -> Result<(), Box<dyn Error + Send + Sync>> {
    let message = bot
        .send_message(chat_id, "👋 You have been logged out successfully!")
        .reply_markup(logged_out_operations())
        .await?;
    
    if std::env::var("TEST_MODE").is_err() {
        store_message_id(chat_id, message.id).await;
    }
    
    Ok(())
}

/// Helper function to cleanup messages
async fn cleanup_messages(chat_id: ChatId, bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    if std::env::var("TEST_MODE").is_err() {
        if let Err(e) = delete_all_messages(chat_id, bot).await {
            log::warn!("Failed to delete messages for chat_id={}: {}", chat_id, e);
            return Err(e);
        }
    }
    Ok(())
}

/// Processes incoming messages and handles commands
/// 
/// # Arguments
/// * `bot` - The bot instance
/// * `msg` - The message to process
/// * `me` - Bot information
/// * `config_store` - User configuration store
/// 
/// # Returns
/// * `Result<(), Box<dyn Error + Send + Sync>>` - Result indicating success or failure
pub async fn process_message(
    bot: Bot,
    msg: Message,
    me: Me,
    config_store: std::sync::Arc<crate::services::user_config_store::UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        log::info!(
            "Processing message: '{}' from chat_id={}",
            text,
            msg.chat.id
        );

        // Check if user is logged in
        let is_logged_in = is_user_logged_in(msg.chat.id).await;

        // Handle commands based on login state
        if text.starts_with('/') {
            if is_logged_in {
                match CommandLoggedIn::parse(text, me.username()) {
                    Ok(command) => {
                        match command {
                            CommandLoggedIn::Help => {
                                let message = bot
                                    .send_message(msg.chat.id, crate::constants::MAN_PAGE)
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedIn::Start => {
                                let message = bot
                                    .send_message(msg.chat.id, "👋 Welcome back! What would you like to do?")
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedIn::List => {
                                let message = bot
                                    .send_message(msg.chat.id, "📋 Listing your items...")
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedIn::Trade => {
                                let message = bot
                                    .send_message(msg.chat.id, "🔄 Trading interface coming soon...")
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedIn::Create => {
                                let message = bot
                                    .send_message(msg.chat.id, "✨ Create interface coming soon...")
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedIn::LogOut => {
                                return handle_logout_command(bot, msg).await;
                            }
                            CommandLoggedIn::PrintKeys => {
                                print_keys(msg.chat.id, &bot).await?;
                            }
                        }
                    }
                    Err(_) => {
                        let message = bot
                            .send_message(msg.chat.id, "❌ Not a valid command")
                            .reply_markup(logged_in_operations())
                            .await?;
                        store_message_id(msg.chat.id, message.id).await;
                    }
                }
            } else {
                match CommandLoggedOut::parse(text, me.username()) {
                    Ok(command) => {
                        match command {
                            CommandLoggedOut::Help => {
                                let message = bot
                                    .send_message(msg.chat.id, crate::constants::MAN_PAGE)
                                    .reply_markup(logged_out_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedOut::Start => {
                                let message = bot
                                    .send_message(msg.chat.id, "👋 GM anon! What would you like to do?")
                                    .reply_markup(logged_out_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedOut::SignUp { password } => {
                                let mut states = log_in_state::USER_STATES.lock().await;
                                states.insert(msg.chat.id.0, log_in_state::AwaitingState::AwaitingSignUpPassword);
                                let message = bot
                                    .send_message(msg.chat.id, "Choose your password:")
                                    .reply_markup(logged_out_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedOut::LogIn { password } => {
                                let mut states = log_in_state::USER_STATES.lock().await;
                                states.insert(msg.chat.id.0, log_in_state::AwaitingState::AwaitingLoginPassword);
                                let message = bot
                                    .send_message(msg.chat.id, "Please enter your password:")
                                    .reply_markup(logged_out_operations())
                                    .await?;
                                store_message_id(msg.chat.id, message.id).await;
                            }
                            CommandLoggedOut::LogOut => {
                                return handle_logout_command(bot, msg).await;
                            }
                        }
                    }
                    Err(_) => {
                        let message = bot
                            .send_message(msg.chat.id, "❌ Not a valid command")
                            .reply_markup(logged_out_operations())
                            .await?;
                        store_message_id(msg.chat.id, message.id).await;
                    }
                }
            }
        } else {
            // Handle non-command messages (password input)
            let state = {
                let states = log_in_state::USER_STATES.lock().await;
                states.get(&msg.chat.id.0).cloned()
            };

            if let Some(state) = state {
                match state {
                    log_in_state::AwaitingState::AwaitingSignUpPassword => {
                        // Handle signup password
                        let mut states = log_in_state::USER_STATES.lock().await;
                        states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                        
                        if let Err(e) = PasswordHandler::new(config_store.clone()) {
                            log::error!("Failed to create password handler: {}", e);
                            let message = bot
                                .send_message(msg.chat.id, "Failed to initialize password handler")
                                .reply_markup(logged_out_operations())
                                .await?;
                            store_message_id(msg.chat.id, message.id).await;
                            return Ok(());
                        }

                        let message = bot
                            .send_message(msg.chat.id, "✅ Account created successfully! You can now log in.")
                            .reply_markup(logged_out_operations())
                            .await?;
                        store_message_id(msg.chat.id, message.id).await;
                    }
                    log_in_state::AwaitingState::AwaitingLoginPassword => {
                        // Handle login password
                        let mut states = log_in_state::USER_STATES.lock().await;
                        states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                        
                        let handlers = PASSWORD_HANDLERS.lock().await;
                        if let Some(Some(handler)) = handlers.get(&msg.chat.id.0) {
                            match handler.login(&msg.chat.id.0.to_string(), text).await {
                                Ok(true) => {
                                    // Update bot commands for logged in state
                                    bot.set_my_commands(CommandLoggedIn::bot_commands())
                                        .scope(BotCommandScope::Chat {
                                            chat_id: msg.chat.id.into(),
                                        })
                                        .await?;

                                    let message = bot
                                        .send_message(msg.chat.id, "✅ Login successful! What would you like to do?")
                                        .reply_markup(logged_in_operations())
                                        .await?;
                                    store_message_id(msg.chat.id, message.id).await;
                                }
                                Ok(false) => {
                                    let message = bot
                                        .send_message(msg.chat.id, "❌ Invalid password")
                                        .reply_markup(logged_out_operations())
                                        .await?;
                                    store_message_id(msg.chat.id, message.id).await;
                                }
                                Err(e) => {
                                    log::error!("Login error: {}", e);
                                    let message = bot
                                        .send_message(msg.chat.id, "❌ An error occurred during login")
                                        .reply_markup(logged_out_operations())
                                        .await?;
                                    store_message_id(msg.chat.id, message.id).await;
                                }
                            }
                        } else {
                            let message = bot
                                .send_message(msg.chat.id, "❌ No account found. Please sign up first.")
                                .reply_markup(logged_out_operations())
                                .await?;
                            store_message_id(msg.chat.id, message.id).await;
                        }
                    }
                    log_in_state::AwaitingState::None => {
                        // No state, ignore message
                    }
                }
            }
        }
    }
    Ok(())
}

/// Helper function to handle logout command
async fn handle_logout_command(bot: Bot, msg: Message) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!(
        "Handling /logout command directly for user {}",
        msg.chat.id.0
    );
    
    if !is_user_logged_in(msg.chat.id).await {
        let message = bot
            .send_message(msg.chat.id, "❌ You are not logged in!")
            .reply_markup(logged_out_operations())
            .await?;
        
        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
        return Ok(());
    }
    
    logout(msg.chat.id, &bot).await
}
