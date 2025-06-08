use crate::keyboard::{logged_in_operations, logged_out_operations};
use crate::v1::commands::{CommandLoggedIn, CommandLoggedOut};
use crate::v1::models::{PASSWORD_HANDLERS, log_in_state, password_handler::PasswordHandler};
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
    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    if let Some(ids) = chat_message_ids.get(&chat_id) {
        for message_id in ids {
            let _ = bot.delete_message(chat_id, *message_id).await;
        }
    }
    chat_message_ids.remove(&chat_id);
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
    log::info!("logout called for chat_id={}", chat_id);
    // Delete all previous messages
    delete_all_messages(chat_id, bot).await?;

    // Clear the password handler
    {
        let mut handler = PASSWORD_HANDLERS.lock().await;
        handler.remove(&chat_id.0);
    }

    // Update user state
    {
        let mut states = log_in_state::USER_STATES.lock().await;
        states.insert(chat_id.0, log_in_state::AwaitingState::None);
    }

    // Set command menu to logged-out
    bot.set_my_commands(CommandLoggedOut::bot_commands())
        .await?;

    // Send logout confirmation message with logged-out keyboard
    let message = bot
        .send_message(chat_id, "👋 You have been logged out successfully!")
        .reply_markup(logged_out_operations())
        .await?;

    // Store the message ID
    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    chat_message_ids.insert(chat_id, vec![message.id]);

    log::info!("logout completed for chat_id={}", chat_id);
    Ok(())
}

pub async fn process_message(
    bot: Bot,
    msg: Message,
    me: Me,
    config_store: std::sync::Arc<crate::v1::services::user_config_store::UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        log::debug!("Received message: {}", text);

        // Delete all previous messages (bot and user) before processing new one
        delete_all_messages(msg.chat.id, &bot).await?;

        // Track the user message for deletion next time
        {
            let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
            chat_message_ids.insert(msg.chat.id, vec![msg.id]);
        }

        // --- FIX: Check login state before parsing as logged in ---
        let user_id = msg.chat.id.0;
        let user_state = {
            let states = log_in_state::USER_STATES.lock().await;
            states
                .get(&user_id)
                .copied()
                .unwrap_or(log_in_state::AwaitingState::None)
        };
        log::info!("User {} state: {:?}", user_id, user_state);

        // Handle awaiting states first
        match user_state {
            log_in_state::AwaitingState::AwaitingLoginPassword => {
                let handler = PasswordHandler::new(config_store.clone())?;
                let user_id = msg.chat.id.0.to_string();
                match handler.login(&user_id, text).await {
                    Ok(true) => {
                        bot.set_my_commands(CommandLoggedIn::bot_commands())
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
            _ => {}
        }

        let is_logged_in = {
            let handlers = PASSWORD_HANDLERS.lock().await;
            handlers.get(&user_id).and_then(|h| h.as_ref()).is_some()
        };
        log::info!("User {} is_logged_in: {}", user_id, is_logged_in);

        if is_logged_in {
            if let Ok(cmd) = CommandLoggedIn::parse(text, me.username()) {
                log::info!("User {} parsed logged-in command: {:?}", user_id, cmd);
                match cmd {
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
                        let message = bot
                            .send_message(msg.chat.id, "📋 Listing your items...")
                            .reply_markup(logged_in_operations())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
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
                    CommandLoggedIn::LogOut => {
                        logout(msg.chat.id, &bot).await?;
                        return Ok(());
                    }
                }
            } else {
                log::warn!("User {} sent unrecognized command while logged in: {}", user_id, text);
                let message = bot
                    .send_message(msg.chat.id, format!("❌ Not a valid command. Use /help to see available commands."))
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                return Ok(());
            }
        }

        match CommandLoggedOut::parse(text, me.username()) {
            Ok(cmd) => {
                log::info!("User {} parsed logged-out command: {:?}", user_id, cmd);
                match cmd {
                    CommandLoggedOut::Help => {
                        let message = bot
                            .send_message(msg.chat.id, CommandLoggedOut::descriptions().to_string())
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                    }
                    CommandLoggedOut::Start => {
                        log::debug!("Handling /start command");
                        let keyboard = logged_out_operations();
                        let message = bot
                            .send_message(msg.chat.id, "💻 gm anon, whatchu wanna do? 🐈")
                            .reply_markup(keyboard)
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                    }
                    CommandLoggedOut::SignUp { password } => {
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
                    }
                    CommandLoggedOut::LogIn { password } => {
                        let handler = PasswordHandler::new(config_store.clone())?;
                        let user_id = msg.chat.id.0.to_string();
                        match handler.login(&user_id, &password).await {
                            Ok(true) => {
                                bot.set_my_commands(CommandLoggedIn::bot_commands())
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
                    }
                }
            }
            Err(e) => {
                log::warn!("User {} failed to parse command: {:?}", user_id, e);
                let user_state = {
                    let states = log_in_state::USER_STATES.lock().await;
                    states
                        .get(&msg.chat.id.0)
                        .copied()
                        .unwrap_or(log_in_state::AwaitingState::None)
                };
                match user_state {
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
                    }
                    log_in_state::AwaitingState::AwaitingLoginPassword => {
                        let handler = PasswordHandler::new(config_store.clone())?;
                        let user_id = msg.chat.id.0.to_string();
                        match handler.login(&user_id, text).await {
                            Ok(true) => {
                                bot.set_my_commands(CommandLoggedIn::bot_commands())
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
                    }
                    log_in_state::AwaitingState::None => {
                        let message = bot.send_message(msg.chat.id, "Command not found!").await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(msg.chat.id, vec![msg.id, message.id]);
                    }
                }
            }
        }
    }

    Ok(())
}
