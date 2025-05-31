use crate::v1::constants::MAN_PAGE;
use crate::v1::models::{log_in_state, password_handler::PasswordHandler};
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

    pub async fn execute(&self, bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
        match self {
            LoggedOutButtons::Faq => {
                bot.send_message(chat_id, MAN_PAGE).await?;
            }
            LoggedOutButtons::LogIn => {
                bot.send_message(chat_id, "Please enter your password:")
                    .await?;
            }
            LoggedOutButtons::SignUp => {
                bot.send_message(chat_id, "Choose your password:").await?;
                if let Err(e) = PasswordHandler::new() {
                    log::error!("Failed to create password handler: {}", e);
                    bot.send_message(chat_id, "Failed to initialize password handler").await?;
                    return Ok(());
                }
                let mut states = log_in_state::USER_STATES.lock().await;
                states.insert(
                    chat_id.0,
                    log_in_state::AwaitingState::AwaitingSignUpPassword,
                );
            }
            _ => {
                bot.send_message(chat_id, "Not a valid command").await?;
            }
        }

        Ok(())
    }
}
