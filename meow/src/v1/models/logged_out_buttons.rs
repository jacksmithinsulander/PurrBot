use crate::v1::constants::MAN_PAGE;
use crate::v1::models::{log_in_state, password_handler::PasswordHandler};
use crate::v1::processors::message_processor::CHAT_MESSAGE_IDS;
use crate::v1::services::user_config_store::UserConfigStore;
use std::sync::Arc;
use teloxide::prelude::ResponseResult;
use teloxide::prelude::*;

pub enum LoggedOutButtons {
    LogIn,
    SignUp,
    Faq,
    UnRecognized,
}

impl LoggedOutButtons {
    pub fn from_str(input: &str) -> Self {
        match input {
            "Log In" => Self::LogIn,
            "Sign Up" => Self::SignUp,
            "FAQ" => Self::Faq,
            _ => Self::UnRecognized,
        }
    }

    pub async fn execute(
        &self,
        bot: Bot,
        chat_id: ChatId,
        config_store: Arc<UserConfigStore>,
    ) -> ResponseResult<()> {
        match self {
            LoggedOutButtons::Faq => {
                let message = bot.send_message(chat_id, MAN_PAGE).await?;

                // Store the new message ID
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
            }
            LoggedOutButtons::LogIn => {
                let message = bot
                    .send_message(chat_id, "Please enter your password:")
                    .await?;

                // Store the new message ID
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
            }
            LoggedOutButtons::SignUp => {
                let message = bot.send_message(chat_id, "Choose your password:").await?;

                // Store the new message ID
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

                // PasswordHandler::new now requires a config_store argument. This must be updated at call sites.
                let mut states = log_in_state::USER_STATES.lock().await;
                states.insert(
                    chat_id.0,
                    log_in_state::AwaitingState::AwaitingSignUpPassword,
                );
            }
            _ => {
                let message = bot.send_message(chat_id, "Not a valid command").await?;

                // Store the new message ID
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
            }
        }

        Ok(())
    }
}
