use super::super::commands::{CommandLoggedIn, CommandLoggedOut};
use crate::keyboard::{logged_in_operations, logged_out_operations};
use crate::v1::models::log_in_state;
use crate::v1::models::logged_in_buttons::LoggedInButtons;
use crate::v1::models::logged_out_buttons::LoggedOutButtons;
use crate::v1::processors::message_processor::{
    CHAT_MESSAGE_IDS, delete_all_messages, logout, print_keys,
};
use crate::v1::services::user_config_store::UserConfigStore;
use std::error::Error;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::BotCommandScope;
use teloxide::utils::command::BotCommands;

pub async fn process_callback(
    bot: Bot,
    q: CallbackQuery,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("Processing callback query: {:?}", q);
    if let Some(data) = q.data.as_deref() {
        log::debug!("Callback data: {}", data);
        // Answer the callback to avoid the loading animation
        bot.answer_callback_query(&q.id).await?;

        if let Some(user_id) = Some(q.from.id) {
            let chat_id = ChatId(user_id.0 as i64);
            log::debug!("Processing callback for user: {}", chat_id);

            // Check login state
            let user_state = {
                let states = log_in_state::USER_STATES.lock().await;
                let state = states
                    .get(&chat_id.0)
                    .copied()
                    .unwrap_or(log_in_state::AwaitingState::None);
                log::debug!("User state from USER_STATES: {:?}", state);
                state
            };

            let password_handler_exists = {
                let handler = crate::v1::models::PASSWORD_HANDLER.lock().await;
                let exists = handler.is_some();
                log::debug!("PASSWORD_HANDLER exists: {}", exists);
                exists
            };

            let is_logged_in = match user_state {
                log_in_state::AwaitingState::None => {
                    log::debug!("User state is None, checking PASSWORD_HANDLER");
                    password_handler_exists
                }
                _ => {
                    log::debug!("User state is not None, user is not logged in");
                    false
                }
            };
            log::debug!("Final login determination - is_logged_in: {}", is_logged_in);
            log::debug!(
                "User login state: {:?}, is_logged_in: {}",
                user_state,
                is_logged_in
            );

            // Delete all previous messages (bot and user) before processing new button press
            delete_all_messages(chat_id, &bot).await?;

            // Track the callback query message for deletion next time
            if let Some(message) = q.message {
                let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
                chat_message_ids.insert(chat_id, vec![message.id()]);
            }

            if is_logged_in {
                match data {
                    "Print Keys" => {
                        log::debug!("Handling Print Keys callback");
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
                    }
                    _ => {
                        log::debug!("Handling logged in callback: {}", data);
                        let button = LoggedInButtons::from_str(data);
                        log::debug!("Button parsed as: {:?}", std::mem::discriminant(&button));
                        match button.execute(bot, chat_id, config_store.clone()).await {
                            Ok(_) => log::debug!("Button execution successful"),
                            Err(e) => log::error!("Button execution failed: {}", e),
                        }
                    }
                }
            } else {
                log::debug!("Handling logged out callback: {}", data);
                LoggedOutButtons::from_str(data)
                    .execute(bot, chat_id, config_store.clone())
                    .await?;
            }
        } else {
            log::error!("No user ID in callback query");
        }
    } else {
        log::error!("No data in callback query");
    }
    Ok(())
}
