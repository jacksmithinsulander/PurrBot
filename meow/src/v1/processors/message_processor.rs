use crate::keyboard::{logged_out_operations, logged_in_operations};
use crate::v1::commands::{CommandLoggedIn, CommandLoggedOut};
use crate::v1::models::{log_in_state, password_handler::PasswordHandler, PASSWORD_HANDLER};
use hex;
use std::error::Error;
use teloxide::{payloads::SendMessageSetters, prelude::*, types::Me, utils::command::BotCommands};

async fn print_keys(chat_id: ChatId, bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    let handler = PASSWORD_HANDLER.lock().await;
    if let Some(handler) = handler.as_ref() {
        log::debug!("print_keys: handler is present");
        let priv_key = handler.get_private_key().await?;
        let pub_key = handler.get_public_key().await?;
        log::debug!("print_keys: priv_key={:?}, pub_key={:?}", priv_key, pub_key);
        match (priv_key, pub_key) {
            (Some(private_key), Some(public_key)) => {
                bot.send_message(
                    chat_id,
                    format!(
                        "🔑 Your Keys:\nPrivate Key: {}\nPublic Key: {}",
                        hex::encode(private_key),
                        hex::encode(public_key)
                    ),
                )
                .await?;
            }
            _ => {
                bot.send_message(chat_id, "❌ No keys available. Please log in first.")
                    .await?;
            }
        }
    } else {
        log::debug!("print_keys: handler is None");
        bot.send_message(chat_id, "❌ No keys available. Please log in first.")
            .await?;
    }
    Ok(())
}

pub async fn process_message(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        log::debug!("Received message: {}", text);
        
        // --- FIX: Check login state before parsing as logged in ---
        let user_id = msg.chat.id.0;
        let user_state = {
            let states = log_in_state::USER_STATES.lock().await;
            states.get(&user_id).copied().unwrap_or(log_in_state::AwaitingState::None)
        };
        let is_logged_in = match user_state {
            log_in_state::AwaitingState::None => {
                let handler_is_some = {
                    let handler = PASSWORD_HANDLER.lock().await;
                    handler.is_some()
                };
                handler_is_some
            },
            _ => false,
        };
        // lock dropped before any await

        if is_logged_in {
            if let Ok(cmd) = CommandLoggedIn::parse(text, me.username()) {
                log::debug!("Parsed as logged in command: {:?}", cmd);
                match cmd {
                    CommandLoggedIn::PrintKeys => {
                        print_keys(msg.chat.id, &bot).await?;
                        return Ok(());
                    }
                    CommandLoggedIn::Start => {
                        bot.send_message(msg.chat.id, "😺 Welcome back! Here are your keys:").await?;
                        print_keys(msg.chat.id, &bot).await?;
                        return Ok(());
                    }
                    // ... handle other logged in commands ...
                    _ => {
                        bot.send_message(msg.chat.id, "Command not implemented yet!").await?;
                        return Ok(());
                    }
                }
            } else {
                // Fallback: handle normal chat messages for logged-in users
                bot.send_message(msg.chat.id, format!("You said: {}", text)).await?;
                return Ok(());
            }
        }

        match CommandLoggedOut::parse(text, me.username()) {
            Ok(cmd) => {
                log::debug!("Parsed as logged out command: {:?}", cmd);
                match cmd {
                    CommandLoggedOut::Help => {
                        bot.send_message(msg.chat.id, CommandLoggedOut::descriptions().to_string())
                            .await?;
                    }
                    CommandLoggedOut::Start => {
                        log::debug!("Handling /start command");
                        let keyboard = logged_out_operations();
                        bot.send_message(msg.chat.id, "💻 gm anon, whatchu wanna do? 🐈")
                            .reply_markup(keyboard)
                            .await?;
                    }
                    CommandLoggedOut::SignUp { password } => {
                        let handler = PasswordHandler::new()?;
                        let user_id = msg.chat.id.0.to_string();
                        match handler.sign_up(&user_id, &password).await {
                            Ok(config) => {
                                bot.send_message(msg.chat.id, "Account created successfully! 🎉\nNow enter your password again to log in.").await?;
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
                                    let mut handler_lock = PASSWORD_HANDLER.lock().await;
                                    *handler_lock = Some(handler);
                                }
                            }
                            Err(e) => {
                                bot.send_message(msg.chat.id, format!("Failed to create account: {}", e))
                                    .await?;
                                log::error!("Failed to create account for user {}: {}", msg.chat.id.0, e);
                            }
                        }
                    }
                    CommandLoggedOut::LogIn { password } => {
                        let handler = PasswordHandler::new()?;
                        let user_id = msg.chat.id.0.to_string();
                        match handler.login(&user_id, &password).await {
                            Ok(true) => {
                                bot.send_message(msg.chat.id, "Logged in successfully! 🎉")
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                log::info!("User {} logged in successfully", msg.chat.id.0);
                                // Store the handler
                                {
                                    let mut handler_lock = PASSWORD_HANDLER.lock().await;
                                    *handler_lock = Some(handler);
                                }
                                print_keys(msg.chat.id, &bot).await?;
                                {
                                    let mut states = log_in_state::USER_STATES.lock().await;
                                    states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                                }
                            }
                            Ok(false) => {
                                bot.send_message(msg.chat.id, "Invalid password! ❌")
                                    .await?;
                                log::warn!("User {} failed to log in", msg.chat.id.0);
                            }
                            Err(e) => {
                                bot.send_message(msg.chat.id, format!("Failed to log in: {}", e))
                                    .await?;
                                log::error!("Failed to log in user {}: {}", msg.chat.id.0, e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::debug!("Failed to parse command: {:?}", e);
                let user_state = {
                    let states = log_in_state::USER_STATES.lock().await;
                    states.get(&msg.chat.id.0).copied().unwrap_or(log_in_state::AwaitingState::None)
                };
                match user_state {
                    log_in_state::AwaitingState::AwaitingSignUpPassword => {
                        let handler = PasswordHandler::new()?;
                        let user_id = msg.chat.id.0.to_string();
                        match handler.sign_up(&user_id, text).await {
                            Ok(config) => {
                                bot.send_message(msg.chat.id, "Account created successfully! 🎉\nNow enter your password again to log in.").await?;
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
                                    let mut handler_lock = PASSWORD_HANDLER.lock().await;
                                    *handler_lock = Some(handler);
                                }
                            }
                            Err(e) => {
                                bot.send_message(
                                    msg.chat.id,
                                    format!("Failed to create account: {}", e),
                                )
                                .await?;
                                log::error!(
                                    "Failed to create account for user {}: {}",
                                    msg.chat.id.0,
                                    e
                                );
                            }
                        }
                    }
                    log_in_state::AwaitingState::AwaitingLoginPassword => {
                        let handler = PasswordHandler::new()?;
                        let user_id = msg.chat.id.0.to_string();
                        match handler.login(&user_id, text).await {
                            Ok(true) => {
                                bot.send_message(msg.chat.id, "Logged in successfully! 🎉")
                                    .reply_markup(logged_in_operations())
                                    .await?;
                                log::info!("User {} logged in successfully", msg.chat.id.0);
                                // Store the handler
                                {
                                    let mut handler_lock = PASSWORD_HANDLER.lock().await;
                                    *handler_lock = Some(handler);
                                }
                                print_keys(msg.chat.id, &bot).await?;
                                {
                                    let mut states = log_in_state::USER_STATES.lock().await;
                                    states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                                }
                            }
                            Ok(false) => {
                                bot.send_message(msg.chat.id, "Invalid password! ❌")
                                    .await?;
                                log::warn!("User {} failed to log in", msg.chat.id.0);
                            }
                            Err(e) => {
                                bot.send_message(msg.chat.id, format!("Failed to log in: {}", e))
                                    .await?;
                                log::error!("Failed to log in user {}: {}", msg.chat.id.0, e);
                            }
                        }
                    }
                    log_in_state::AwaitingState::None => {
                        bot.send_message(msg.chat.id, "Command not found!").await?;
                    }
                }
            }
        }
    }

    Ok(())
}
