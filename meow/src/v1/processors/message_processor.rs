use crate::keyboard::logged_out_operations;
use crate::v1::commands::{CommandLoggedIn, CommandLoggedOut};
use crate::v1::models::{log_in_state, password_handler::PasswordHandler};
use std::error::Error;
use std::sync::Arc;
use teloxide::{payloads::SendMessageSetters, prelude::*, types::Me, utils::command::BotCommands};

pub async fn process_message(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        match CommandLoggedOut::parse(text, me.username()) {
            Ok(CommandLoggedOut::Help) => {
                bot.send_message(msg.chat.id, CommandLoggedOut::descriptions().to_string())
                    .await?;
            }
            Ok(CommandLoggedOut::Start) => {
                let keyboard = logged_out_operations();
                bot.send_message(msg.chat.id, "💻 gm anon, whatchu wanna do? 🐈")
                    .reply_markup(keyboard)
                    .await?;
            }
            Ok(CommandLoggedOut::SignUp { password }) => {
                let handler = PasswordHandler::new()?;
                match handler.sign_up(&password).await {
                    Ok(config) => {
                        bot.send_message(msg.chat.id, "Account created successfully! 🎉")
                            .await?;
                        log::info!("User {} created account with config: {}", msg.chat.id.0, config);
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Failed to create account: {}", e))
                            .await?;
                        log::error!("Failed to create account for user {}: {}", msg.chat.id.0, e);
                    }
                }
            }
            Ok(CommandLoggedOut::LogIn { password }) => {
                let handler = PasswordHandler::new()?;
                match handler.login(&password).await {
                    Ok(true) => {
                        bot.send_message(msg.chat.id, "Logged in successfully! 🎉")
                            .await?;
                        log::info!("User {} logged in successfully", msg.chat.id.0);
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
            Err(_) => {
                let states = log_in_state::USER_STATES.lock().await;
                match states
                    .get(&msg.chat.id.0)
                    .copied()
                    .unwrap_or(log_in_state::AwaitingState::None)
                {
                    log_in_state::AwaitingState::AwaitingSignUpPassword => {
                        let handler = PasswordHandler::new()?;
                        match handler.sign_up(text).await {
                            Ok(config) => {
                                bot.send_message(msg.chat.id, "Account created successfully! 🎉")
                                    .await?;
                                log::info!("User {} created account with config: {}", msg.chat.id.0, config);
                            }
                            Err(e) => {
                                bot.send_message(msg.chat.id, format!("Failed to create account: {}", e))
                                    .await?;
                                log::error!("Failed to create account for user {}: {}", msg.chat.id.0, e);
                            }
                        }
                        let mut states = log_in_state::USER_STATES.lock().await;
                        states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
                    }
                    log_in_state::AwaitingState::AwaitingLoginPassword => {
                        let handler = PasswordHandler::new()?;
                        match handler.login(text).await {
                            Ok(true) => {
                                bot.send_message(msg.chat.id, "Logged in successfully! 🎉")
                                    .await?;
                                log::info!("User {} logged in successfully", msg.chat.id.0);
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
                        let mut states = log_in_state::USER_STATES.lock().await;
                        states.insert(msg.chat.id.0, log_in_state::AwaitingState::None);
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
