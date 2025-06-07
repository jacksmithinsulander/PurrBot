use crate::keyboard::logged_in_operations;
use crate::v1::processors::message_processor::{CHAT_MESSAGE_IDS, logout};
use crate::v1::services::user_config_store::UserConfigStore;
use std::sync::Arc;
use teloxide::prelude::ResponseResult;
use teloxide::prelude::*;

#[derive(Debug)]
pub enum LoggedInButtons {
    List,
    Trade,
    Create,
    LogOut,
    PrintKeys,
    UnRecognized,
}

impl LoggedInButtons {
    pub fn from_str(input: &str) -> Self {
        match input {
            "List" => Self::List,
            "Trade" => Self::Trade,
            "Create" => Self::Create,
            "Log Out" => Self::LogOut,
            "Print Keys" => Self::PrintKeys,
            _ => Self::UnRecognized,
        }
    }

    pub async fn execute(
        &self,
        bot: Bot,
        chat_id: ChatId,
        config_store: Arc<UserConfigStore>,
    ) -> ResponseResult<()> {
        log::debug!("Executing LoggedInButton: {:?}", self);
        match self {
            LoggedInButtons::List => {
                log::debug!("Executing List button");
                let message = bot
                    .send_message(chat_id, "📋 Listing your items...")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("List button execution completed");
            }
            LoggedInButtons::Trade => {
                log::debug!("Executing Trade button");
                let message = bot
                    .send_message(chat_id, "🔄 Trading interface coming soon...")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("Trade button execution completed");
            }
            LoggedInButtons::Create => {
                log::debug!("Executing Create button");
                let message = bot
                    .send_message(chat_id, "✨ Create interface coming soon...")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("Create button execution completed");
            }
            LoggedInButtons::LogOut => {
                log::debug!("Executing LogOut button");
                match logout(chat_id, &bot).await {
                    Ok(_) => {
                        log::debug!("Logout successful");
                    }
                    Err(e) => {
                        log::error!("Logout failed: {}", e);
                        let message = bot
                            .send_message(chat_id, format!("Failed to log out: {}", e))
                            .await?;
                        let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                        chat_message_ids.insert(chat_id, vec![message.id]);
                    }
                }
                log::debug!("LogOut button execution completed");
            }
            LoggedInButtons::PrintKeys => {
                log::debug!("Executing PrintKeys button");
                let message = bot
                    .send_message(chat_id, "🔑 Printing your keys...")
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("PrintKeys button execution completed");
                // The actual key printing is handled in the callback processor
            }
            _ => {
                log::debug!("Executing unrecognized button");
                let message = bot
                    .send_message(chat_id, "❌ Not a valid command")
                    .reply_markup(logged_in_operations())
                    .await?;
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id]);
                log::debug!("Unrecognized button execution completed");
            }
        }
        log::debug!("LoggedInButton execution finished");
        Ok(())
    }
}
