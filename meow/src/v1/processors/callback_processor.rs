use crate::v1::models::logged_out_buttons::LoggedOutButtons;
use crate::v1::processors::message_processor::{delete_all_messages, CHAT_MESSAGE_IDS, logout, print_keys};
use crate::keyboard::{logged_in_operations, logged_out_operations};
use std::error::Error;
use teloxide::prelude::*;
use std::sync::Arc;
use crate::v1::services::user_config_store::UserConfigStore;

pub async fn process_callback(
    bot: Bot,
    q: CallbackQuery,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(data) = q.data.as_deref() {
        // Answer the callback to avoid the loading animation
        bot.answer_callback_query(&q.id).await?;

        if let Some(user_id) = Some(q.from.id) {
            let chat_id = ChatId(user_id.0 as i64);

            // Delete all previous messages (bot and user) before processing new button press
            delete_all_messages(chat_id, &bot).await?;

            // Track the callback query message for deletion next time
            if let Some(message) = q.message {
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id()]);
            }

            match data {
                "Log Out" => {
                    logout(chat_id, &bot).await?;
                    // Show logged out keyboard after logout
                    let keyboard = logged_out_operations();
                    let message = bot
                        .send_message(chat_id, "💻 gm anon, whatchu wanna do? 🐈")
                        .reply_markup(keyboard)
                        .await?;
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(chat_id, vec![message.id]);
                }
                "Print Keys" => {
                    print_keys(chat_id, &bot).await?;
                    // Show logged in keyboard after printing keys
                    let keyboard = logged_in_operations();
                    let message = bot
                        .send_message(chat_id, "🔑 Keys printed above. What else would you like to do?")
                        .reply_markup(keyboard)
                        .await?;
                    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                    chat_message_ids.insert(chat_id, vec![message.id]);
                }
                _ => {
                    LoggedOutButtons::from_str(data)
                        .execute(bot, chat_id, config_store.clone())
                        .await?;
                }
            }
        }
    }

    Ok(())
}
