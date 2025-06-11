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

pub static CHAT_MESSAGE_IDS: Lazy<Mutex<HashMap<ChatId, Vec<MessageId>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

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
        for message_id in ids {
            if let Err(e) = bot.delete_message(chat_id, *message_id).await {
                log::warn!(
                    "Failed to delete message {} for chat_id={}: {}",
                    message_id,
                    chat_id,
                    e
                );
            }
        }
    } else {
        log::debug!("No messages found to delete for chat_id={}", chat_id);
    }
    chat_message_ids.remove(&chat_id);
    log::debug!("Message deletion completed for chat_id={}", chat_id);
    Ok(())
}

pub async fn print_keys(chat_id: ChatId, bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("print_keys called for chat_id={}", chat_id);
    // Delete all previous messages before sending new one
    delete_all_messages(chat_id, bot).await?;

    let handler = PASSWORD_HANDLERS.lock().await;
    if let Some(handler) = handler.get(&chat_id.0) {
        if let Some(handler) = handler {
            log::debug!("print_keys: handler is present");
            let priv_key = handler.get_private_key().await?;
            let pub_key = handler.get_public_key().await?;
            log::debug!("print_keys: priv_key={:?}, pub_key={:?}", priv_key, pub_key);
            match (priv_key, pub_key) {
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
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(chat_id, vec![msg.id]);
                }
                _ => {
                    let msg = bot
                        .send_message(chat_id, "❌ No keys available. Please log in first.")
                        .await?;
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(chat_id, vec![msg.id]);
                }
            }
        } else {
            log::debug!("print_keys: handler is None");
            let msg = bot
                .send_message(chat_id, "❌ No keys available. Please log in first.")
                .await?;
            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
            chat_message_ids.insert(chat_id, vec![msg.id]);
        }
    } else {
        log::debug!("print_keys: handler is None");
        let msg = bot
            .send_message(chat_id, "❌ No keys available. Please log in first.")
            .await?;
        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
        chat_message_ids.insert(chat_id, vec![msg.id]);
    }
    log::info!("print_keys completed for chat_id={}", chat_id);
    Ok(())
}

pub async fn logout(chat_id: ChatId, bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Starting logout process for chat_id={}", chat_id);

    // Check if user is actually logged in first
    let is_logged_in = {
        let handlers = PASSWORD_HANDLERS.lock().await;
        handlers.get(&chat_id.0).and_then(|h| h.as_ref()).is_some()
    };

    if !is_logged_in {
        log::info!("User {} tried to logout but is not logged in", chat_id.0);
        // Send error message
        let message = bot
            .send_message(chat_id, "❌ You are not logged in!")
            .reply_markup(logged_out_operations())
            .await?;
        
        // Store the message ID
        if std::env::var("TEST_MODE").is_err() {
            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
            chat_message_ids.insert(chat_id, vec![message.id]);
        }
        
        return Ok(());
    }

    // Clear state first
    {
        let mut states = log_in_state::USER_STATES.lock().await;
        states.insert(chat_id.0, log_in_state::AwaitingState::None);
        log::info!(
            "State after logout for chat_id={}: {:?}",
            chat_id,
            states.get(&chat_id.0)
        );
    }

    // Clear the password handler
    {
        let mut handler = PASSWORD_HANDLERS.lock().await;
        handler.remove(&chat_id.0);
        log::info!("Password handler cleared for chat_id={}", chat_id);
    }

    // Set command menu to logged-out for this specific chat
    log::debug!(
        "Setting commands to logged-out state for chat_id={}",
        chat_id
    );
    if let Err(e) = bot
        .set_my_commands(CommandLoggedOut::bot_commands())
        .scope(BotCommandScope::Chat {
            chat_id: chat_id.into(),
        })
        .await
    {
        log::warn!("Failed to set commands for chat_id={}: {}", chat_id, e);
    } else {
        log::info!("Commands set to logged-out state for chat_id={}", chat_id);
    }

    // Send logout confirmation message with logged-out keyboard
    log::debug!("Sending logout confirmation message to chat_id={}", chat_id);
    let message = match bot
        .send_message(chat_id, "👋 You have been logged out successfully!")
        .reply_markup(logged_out_operations())
        .await
    {
        Ok(msg) => {
            log::info!(
                "Logout confirmation message sent successfully to chat_id={}",
                chat_id
            );
            msg
        }
        Err(e) => {
            log::error!(
                "Failed to send logout message for chat_id={}: {}",
                chat_id,
                e
            );
            return Err(Box::new(e));
        }
    };

    // Store the message ID
    if std::env::var("TEST_MODE").is_err() {
        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
        chat_message_ids.insert(chat_id, vec![message.id]);
        log::debug!("Message ID stored for chat_id={}", chat_id);
    }

    // Delete previous messages last, as it's not critical
    if std::env::var("TEST_MODE").is_err() {
        if let Err(e) = delete_all_messages(chat_id, bot).await {
            log::warn!("Failed to delete messages for chat_id={}: {}", chat_id, e);
        } else {
            log::debug!("Previous messages deleted for chat_id={}", chat_id);
        }
    }

    log::info!(
        "Logout process completed successfully for chat_id={}",
        chat_id
    );
    Ok(())
}

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

        // Handle /logout command directly first
        if text.trim().to_lowercase() == "/logout" {
            log::info!(
                "Handling /logout command directly for user {}",
                msg.chat.id.0
            );
            
            // Check if user is actually logged in first
            let is_logged_in = {
                let handlers = PASSWORD_HANDLERS.lock().await;
                handlers.get(&msg.chat.id.0).and_then(|h| h.as_ref()).is_some()
            };

            if !is_logged_in {
                log::info!("User {} tried to logout but is not logged in", msg.chat.id.0);
                // Send error message
                let message = bot
                    .send_message(msg.chat.id, "❌ You are not logged in!")
                    .reply_markup(logged_out_operations())
                    .await?;
                
                // Store message IDs even in test mode
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                return Ok(());
            }
            
            // Clear state first
            {
                let mut states = log_in_state::USER_STATES.lock().await;
                states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                log::info!("Reset state to None for user {}", msg.chat.id.0);
            }

            // Clear any existing password handler
            {
                let mut handlers = PASSWORD_HANDLERS.lock().await;
                handlers.remove(&msg.chat.id.0);
                log::info!("Cleared password handler for user {}", msg.chat.id.0);
            }
            // Set logged-out commands
            if let Err(e) = bot
                .set_my_commands(CommandLoggedOut::bot_commands())
                .scope(BotCommandScope::Chat {
                    chat_id: msg.chat.id.into(),
                })
                .await
            {
                log::warn!(
                    "Failed to set logged-out commands for user {}: {}",
                    msg.chat.id.0,
                    e
                );
            } else {
                log::info!("Set logged-out commands for user {}", msg.chat.id.0);
            }
            // Send logout confirmation message
            let message = bot
                .send_message(msg.chat.id, "👋 You have been logged out successfully!")
                .reply_markup(logged_out_operations())
                .await?;
            log::info!(
                "Logout confirmation message sent successfully to user {}",
                msg.chat.id.0
            );
            // Store message IDs even in test mode
            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
            return Ok(());
        }

        // Only delete messages if not in test mode
        if std::env::var("TEST_MODE").is_err() {
            log::debug!("Not in test mode, deleting previous messages");
            delete_all_messages(msg.chat.id, &bot).await?;
        } else {
            log::debug!("In test mode, skipping message deletion");
        }

        // Track the user message for deletion next time (only if not in test mode)
        if std::env::var("TEST_MODE").is_err() {
            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
            chat_message_ids.insert(msg.chat.id, vec![msg.id]);
        }

        let user_id = msg.chat.id.0;
        let user_state = {
            let states = log_in_state::USER_STATES.lock().await;
            states
                .get(&user_id)
                .copied()
                .unwrap_or(log_in_state::AwaitingState::None)
        };
        log::info!("User {} state: {:?}", user_id, user_state);

        // Always check for commands first, regardless of state
        let is_logged_in = {
            let handlers = PASSWORD_HANDLERS.lock().await;
            handlers.get(&user_id).and_then(|h| h.as_ref()).is_some()
        };
        log::info!("User {} is_logged_in: {}", user_id, is_logged_in);

        // Try to parse as CommandLoggedOut first for /start and /logout commands
        log::debug!("Attempting to parse as CommandLoggedOut: {}", text);
        match CommandLoggedOut::parse(text, me.username()) {
            Ok(cmd) => {
                log::info!("Successfully parsed as CommandLoggedOut: {:?}", cmd);
                match cmd {
                    CommandLoggedOut::Start => {
                        log::info!("Handling /start command for user {}", user_id);
                        // Reset state to None to ensure clean state
                        {
                            let mut states = log_in_state::USER_STATES.lock().await;
                            states.insert(user_id, log_in_state::AwaitingState::None);
                            log::info!("Reset state to None for user {}", user_id);
                        }
                        // Clear any existing password handler
                        {
                            let mut handlers = PASSWORD_HANDLERS.lock().await;
                            handlers.remove(&user_id);
                            log::info!("Cleared password handler for user {}", user_id);
                        }
                        // Set logged-out commands
                        if let Err(e) = bot
                            .set_my_commands(CommandLoggedOut::bot_commands())
                            .scope(BotCommandScope::Chat {
                                chat_id: msg.chat.id.into(),
                            })
                            .await
                        {
                            log::warn!(
                                "Failed to set logged-out commands for user {}: {}",
                                user_id,
                                e
                            );
                        } else {
                            log::info!("Set logged-out commands for user {}", user_id);
                        }
                        let keyboard = logged_out_operations();
                        log::debug!("Sending welcome message to user {}", user_id);
                        let message = bot
                            .send_message(msg.chat.id, "💻 gm anon, whatchu wanna do? 🐈")
                            .reply_markup(keyboard)
                            .await?;
                        log::info!("Welcome message sent successfully to user {}", user_id);
                        if std::env::var("TEST_MODE").is_err() {
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        }
                        return Ok(());
                    }
                    CommandLoggedOut::LogOut => {
                        log::info!("Handling /logout command for user {}", user_id);
                        // Clear state first
                        {
                            let mut states = log_in_state::USER_STATES.lock().await;
                            states.insert(user_id, log_in_state::AwaitingState::None);
                            log::info!("Reset state to None for user {}", user_id);
                        }
                        // Clear any existing password handler
                        {
                            let mut handlers = PASSWORD_HANDLERS.lock().await;
                            handlers.remove(&user_id);
                            log::info!("Cleared password handler for user {}", user_id);
                        }
                        // Set logged-out commands
                        if let Err(e) = bot
                            .set_my_commands(CommandLoggedOut::bot_commands())
                            .scope(BotCommandScope::Chat {
                                chat_id: msg.chat.id.into(),
                            })
                            .await
                        {
                            log::warn!(
                                "Failed to set logged-out commands for user {}: {}",
                                user_id,
                                e
                            );
                        } else {
                            log::info!("Set logged-out commands for user {}", user_id);
                        }
                        // Send logout confirmation message
                        let message = bot
                            .send_message(msg.chat.id, "👋 You have been logged out successfully!")
                            .reply_markup(logged_out_operations())
                            .await?;
                        log::info!(
                            "Logout confirmation message sent successfully to user {}",
                            user_id
                        );
                        if std::env::var("TEST_MODE").is_err() {
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        }
                        return Ok(());
                    }
                    _ => {
                        log::debug!("Unhandled CommandLoggedOut variant: {:?}", cmd);
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to parse as CommandLoggedOut: '{}', error: {}",
                    text,
                    e
                );
                // Fallback: reply to user for unknown command
                let message = bot
                    .send_message(msg.chat.id, format!("❌ Unrecognized command: {}", text))
                    .reply_markup(logged_out_operations())
                    .await?;
                if std::env::var("TEST_MODE").is_err() {
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                }
                return Ok(());
            }
        }

        // Try logged-in commands if logged in
        if is_logged_in {
            log::debug!(
                "User is logged in, attempting to parse as CommandLoggedIn: {}",
                text
            );
            if let Ok(cmd) = CommandLoggedIn::parse(text, me.username()) {
                log::info!("Successfully parsed as CommandLoggedIn: {:?}", cmd);
                match cmd {
                    CommandLoggedIn::LogOut => {
                        log::info!("Handling logged-in /logout command for user {}", user_id);
                        logout(msg.chat.id, &bot).await?;
                        log::info!("Logout completed for user {}", user_id);
                        return Ok(());
                    }
                    CommandLoggedIn::PrintKeys => {
                        print_keys(msg.chat.id, &bot).await?;
                        return Ok(());
                    }
                    CommandLoggedIn::Start => {
                        let message = bot
                            .send_message(msg.chat.id, "😺 Welcome back! Here are your keys:")
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        print_keys(msg.chat.id, &bot).await?;
                        return Ok(());
                    }
                    CommandLoggedIn::Help => {
                        let message = bot
                            .send_message(msg.chat.id, CommandLoggedIn::descriptions().to_string())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                    CommandLoggedIn::List => {
                        log::info!("Handling /list command for user {}", user_id);
                        // Verify login state
                        let is_still_logged_in = {
                            let handlers = PASSWORD_HANDLERS.lock().await;
                            handlers.get(&user_id).and_then(|h| h.as_ref()).is_some()
                        };

                        if !is_still_logged_in {
                            log::warn!("User {} tried to use /list but is not logged in", user_id);
                            let message = bot
                                .send_message(msg.chat.id, "❌ Please log in first!")
                                .reply_markup(logged_out_operations())
                                .await?;
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                            return Ok(());
                        }

                        let message = bot
                            .send_message(msg.chat.id, "📋 Listing your items...")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        log::info!("Successfully sent list response to user {}", user_id);
                        return Ok(());
                    }
                    CommandLoggedIn::Trade => {
                        let message = bot
                            .send_message(msg.chat.id, "🔄 Trading interface coming soon...")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                    CommandLoggedIn::Create => {
                        let message = bot
                            .send_message(msg.chat.id, "✨ Create interface coming soon...")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                }
            } else {
                log::debug!("Failed to parse as CommandLoggedIn: {}", text);
            }
        }

        // Now try the rest of the logged-out commands if not already handled
        log::debug!("Trying to parse as CommandLoggedOut: {}", text);
        if let Ok(cmd) = CommandLoggedOut::parse(text, me.username()) {
            log::info!("User {} parsed logged-out command: {:?}", user_id, cmd);
            match cmd {
                CommandLoggedOut::Help => {
                    let message = bot
                        .send_message(msg.chat.id, CommandLoggedOut::descriptions().to_string())
                        .await?;
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                    return Ok(());
                }
                // Start and LogOut are already handled above
                CommandLoggedOut::SignUp { password } => {
                    // Check if already logged in
                    if is_logged_in {
                        log::info!("User {} tried to signup but is already logged in", user_id);
                        let message = bot
                            .send_message(msg.chat.id, "❌ You are already logged in! Please logout first.")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                    
                    let handler = PasswordHandler::new(config_store.clone())?;
                    let user_id = msg.chat.id.0.to_string();
                    match handler.sign_up(&user_id, &password).await {
                        Ok(config) => {
                            let message = bot.send_message(msg.chat.id, "Account created successfully! 🎉\nNow enter your password again to log in.").await?;
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                            log::info!(
                                "User {} created account with config: {}",
                                msg.chat.id.0,
                                config
                            );
                            {
                                let mut states = log_in_state::USER_STATES.lock().await;
                                states.insert(
                                    msg.chat.id.0,
                                    log_in_state::AwaitingState::AwaitingLoginPassword,
                                );
                            }
                            // Store the handler
                            {
                                let mut handler_lock = PASSWORD_HANDLERS.lock().await;
                                handler_lock.insert(msg.chat.id.0, Some(handler));
                            }
                        }
                        Err(e) => {
                            let message = bot
                                .send_message(
                                    msg.chat.id,
                                    format!("Failed to create account: {}", e),
                                )
                                .await?;
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                            log::error!(
                                "Failed to create account for user {}: {}",
                                msg.chat.id.0,
                                e
                            );
                        }
                    }
                    return Ok(());
                }
                CommandLoggedOut::LogIn { password } => {
                    // Check if already logged in
                    if is_logged_in {
                        log::info!("User {} tried to login but is already logged in", user_id);
                        let message = bot
                            .send_message(msg.chat.id, "❌ You are already logged in!")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                    
                    let handler = PasswordHandler::new(config_store.clone())?;
                    let user_id = msg.chat.id.0.to_string();
                    match handler.login(&user_id, &password).await {
                        Ok(true) => {
                            // Set logged-in commands first
                            bot.set_my_commands(CommandLoggedIn::bot_commands())
                                .scope(BotCommandScope::Chat {
                                    chat_id: msg.chat.id.into(),
                                })
                                .await?;

                            // Store the handler before sending messages
                            {
                                let mut handler_lock = PASSWORD_HANDLERS.lock().await;
                                handler_lock.insert(msg.chat.id.0, Some(handler));
                            }

                            // Update user state
                            {
                                let mut states = log_in_state::USER_STATES.lock().await;
                                states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                            }

                            // Send success message
                            let message = bot
                                .send_message(msg.chat.id, "Logged in successfully! 🎉")
                                .reply_markup(logged_in_operations())
                                .await?;
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);

                            log::info!("User {} logged in successfully", msg.chat.id.0);

                            // Print keys after successful login
                            print_keys(msg.chat.id, &bot).await?;
                            return Ok(());
                        }
                        Ok(false) => {
                            let message = bot
                                .send_message(msg.chat.id, "Invalid password! ❌")
                                .await?;
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                            log::warn!("User {} failed to log in", msg.chat.id.0);
                        }
                        Err(e) => {
                            let message = bot
                                .send_message(msg.chat.id, format!("Failed to log in: {}", e))
                                .await?;
                            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                            chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                            log::error!("Failed to log in user {}: {}", msg.chat.id.0, e);
                        }
                    }
                    return Ok(());
                }
                // Start and LogOut are already handled above
                _ => {}
            }
        }

        // If not a command, handle password input based on state
        match user_state {
            log_in_state::AwaitingState::AwaitingLoginPassword => {
                let handler = PasswordHandler::new(config_store.clone())?;
                let user_id = msg.chat.id.0.to_string();
                match handler.login(&user_id, text).await {
                    Ok(true) => {
                        bot.set_my_commands(CommandLoggedIn::bot_commands())
                            .scope(BotCommandScope::Chat {
                                chat_id: msg.chat.id.into(),
                            })
                            .await?;
                        let message = bot
                            .send_message(msg.chat.id, "Logged in successfully! 🎉")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        log::info!("User {} logged in successfully", msg.chat.id.0);
                        // Store the handler
                        {
                            let mut handler_lock = PASSWORD_HANDLERS.lock().await;
                            handler_lock.insert(msg.chat.id.0, Some(handler));
                        }
                        print_keys(msg.chat.id, &bot).await?;
                        {
                            let mut states = log_in_state::USER_STATES.lock().await;
                            states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                        }
                        return Ok(());
                    }
                    Ok(false) => {
                        let message = bot
                            .send_message(msg.chat.id, "Invalid password! ❌")
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                    Err(e) => {
                        let message = bot
                            .send_message(msg.chat.id, format!("Login failed: {}", e))
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        return Ok(());
                    }
                }
            }
            log_in_state::AwaitingState::AwaitingSignUpPassword => {
                let handler = PasswordHandler::new(config_store.clone())?;
                let user_id = msg.chat.id.0.to_string();
                match handler.sign_up(&user_id, text).await {
                    Ok(config) => {
                        let message = bot.send_message(msg.chat.id, "Account created successfully! 🎉\nNow enter your password again to log in.").await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        log::info!(
                            "User {} created account with config: {}",
                            msg.chat.id.0,
                            config
                        );
                        {
                            let mut states = log_in_state::USER_STATES.lock().await;
                            states.insert(
                                msg.chat.id.0,
                                log_in_state::AwaitingState::AwaitingLoginPassword,
                            );
                        }
                        // Store the handler
                        {
                            let mut handler_lock = PASSWORD_HANDLERS.lock().await;
                            handler_lock.insert(msg.chat.id.0, Some(handler));
                        }
                    }
                    Err(e) => {
                        let message = bot
                            .send_message(msg.chat.id, format!("Failed to create account: {}", e))
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                        log::error!("Failed to create account for user {}: {}", msg.chat.id.0, e);
                    }
                }
                return Ok(());
            }
            log_in_state::AwaitingState::None => {
                let message = bot.send_message(msg.chat.id, "Command not found!").await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
            }
        }
    }

    Ok(())
}
