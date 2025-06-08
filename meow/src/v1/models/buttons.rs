use crate::keyboard::{logged_in_operations, logged_out_operations};
use crate::v1::constants::MAN_PAGE;
use crate::v1::models::{log_in_state, password_handler::PasswordHandler, PASSWORD_HANDLERS};
use crate::v1::processors::message_processor::{CHAT_MESSAGE_IDS, logout, print_keys};
use crate::v1::services::user_config_store::UserConfigStore;
use crate::v1::commands::{CommandLoggedIn, CommandLoggedOut};
use std::sync::Arc;
use teloxide::prelude::ResponseResult;
use teloxide::prelude::*;
use teloxide::types::BotCommandScope;
use teloxide::utils::command::BotCommands;
use std::error::Error;

#[derive(Debug)]
pub enum Button {
    // Logged in buttons
    List,
    Trade,
    Create,
    LogOut,
    PrintKeys,
    // Logged out buttons
    LogIn,
    SignUp,
    Faq,
    // Unknown button
    UnRecognized,
}

impl Button {
    pub fn from_str(input: &str, is_logged_in: bool) -> Self {
        if is_logged_in {
            match input {
                "List" => Self::List,
                "Trade" => Self::Trade,
                "Create" => Self::Create,
                "Log Out" => Self::LogOut,
                "Print Keys" => Self::PrintKeys,
                _ => Self::UnRecognized,
            }
        } else {
            match input {
                "Log In" => Self::LogIn,
                "Sign Up" => Self::SignUp,
                "FAQ" => Self::Faq,
                _ => Self::UnRecognized,
            }
        }
    }

    pub async fn execute(
        &self,
        bot: Bot,
        chat_id: ChatId,
        config_store: Arc<UserConfigStore>,
        is_logged_in: bool,
    ) -> ResponseResult<()> {
        log::debug!("Executing Button: {:?}", self);
        match self {
            // Logged in buttons
            Button::List => {
                log::debug!("Executing List button");
                let message = bot
                    .send_message(chat_id, "📋 Listing your items...")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("List button execution completed");
            }
            Button::Trade => {
                log::debug!("Executing Trade button");
                let message = bot
                    .send_message(chat_id, "🔄 Trading interface coming soon...")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("Trade button execution completed");
            }
            Button::Create => {
                log::debug!("Executing Create button");
                let message = bot
                    .send_message(chat_id, "✨ Create interface coming soon...")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("Create button execution completed");
            }
            Button::LogOut => {
                log::info!("Button::LogOut pressed for chat_id={}", chat_id);
                log::debug!("Executing LogOut button");
                match logout(chat_id, &bot).await {
                    Ok(_) => {
                        log::debug!("Logout successful");
                        bot.set_my_commands(CommandLoggedOut::bot_commands())
                            .scope(BotCommandScope::Chat { chat_id: chat_id.into() })
                            .await?;
                        // Remove handler for this user
                        let mut handlers = PASSWORD_HANDLERS.lock().await;
                        handlers.remove(&chat_id.0);
                    }
                    Err(e) => {
                        log::error!("Logout failed: {}", e);
                        let message = bot
                            .send_message(chat_id, format!("Failed to logout: {}", e))
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(chat_id, vec![message.id]);
                    }
                }
                log::info!("Button::LogOut completed for chat_id={}", chat_id);
                log::debug!("LogOut button execution completed");
            }
            Button::PrintKeys => {
                log::info!("Button::PrintKeys pressed for chat_id={}", chat_id);
                log::debug!("Executing PrintKeys button");
                match print_keys(chat_id, &bot).await {
                    Ok(_) => {
                        log::debug!("Print keys successful");
                        bot.set_my_commands(CommandLoggedIn::bot_commands())
                            .scope(BotCommandScope::Chat {
                                chat_id: chat_id.into(),
                            })
                            .await?;
                        // Show logged in keyboard after printing keys
                        let keyboard = logged_in_operations();
                        let message = bot
                            .send_message(
                                chat_id,
                                "🔑 Keys printed above. What else would you like to do?",
                            )
                            .reply_markup(keyboard)
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(chat_id, vec![message.id]);
                    }
                    Err(e) => {
                        log::error!("Print keys failed: {}", e);
                        let message = bot
                            .send_message(chat_id, format!("Failed to print keys: {}", e))
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(chat_id, vec![message.id]);
                    }
                }
                log::info!("Button::PrintKeys completed for chat_id={}", chat_id);
                log::debug!("PrintKeys button execution completed");
            }
            // Logged out buttons
            Button::Faq => {
                log::debug!("Executing FAQ button");
                let message = bot.send_message(chat_id, MAN_PAGE).await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("FAQ button execution completed");
            }
            Button::LogIn => {
                log::debug!("Executing LogIn button");
                let message = bot
                    .send_message(chat_id, "Please enter your password:")
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("LogIn button execution completed");
            }
            Button::SignUp => {
                log::debug!("Executing SignUp button");
                let message = bot.send_message(chat_id, "Choose your password:").await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);

                if let Err(e) = PasswordHandler::new(config_store.clone()) {
                    log::error!("Failed to create password handler: {}", e);
                    let error_message = bot
                        .send_message(chat_id, "Failed to initialize password handler")
                        .await?;
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(chat_id, vec![error_message.id]);
                    return Ok(());
                }

                let mut states = log_in_state::USER_STATES.lock().await;
                states.insert(
                    chat_id.0,
                    log_in_state::AwaitingState::AwaitingSignUpPassword,
                );
                log::debug!("SignUp button execution completed");
            }
            Button::UnRecognized => {
                log::debug!("Executing unrecognized button");
                let message = bot
                    .send_message(chat_id, "❌ Not a valid command")
                    .reply_markup(if is_logged_in {
                        logged_in_operations()
                    } else {
                        logged_out_operations()
                    })
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("Unrecognized button execution completed");
            }
        }
        log::debug!("Button execution finished");
        Ok(())
    }
} 