use crate::v1::models::PASSWORD_HANDLERS;
use crate::v1::models::buttons::Button;
use crate::v1::processors::message_processor::{CHAT_MESSAGE_IDS, delete_all_messages};
use crate::v1::services::user_config_store::UserConfigStore;
use std::error::Error;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{BotCommandScope, MaybeInaccessibleMessage, Message};

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

        if let Some(message) = q.message {
            match message {
                MaybeInaccessibleMessage::Regular(msg) => {
                    delete_all_messages(msg.chat.id, &bot).await?;
                    let is_logged_in = {
                        let handlers = PASSWORD_HANDLERS.lock().await;
                        handlers
                            .get(&msg.chat.id.0)
                            .and_then(|h| h.as_ref())
                            .is_some()
                    };
                    log::info!(
                        "Callback for user {} is_logged_in: {}",
                        msg.chat.id.0,
                        is_logged_in
                    );
                    let button = Button::from_str(data, is_logged_in);
                    button
                        .execute(bot, msg.chat.id, config_store, is_logged_in)
                        .await?;
                }
                MaybeInaccessibleMessage::Inaccessible(_) => {
                    log::error!("Message is inaccessible");
                }
            }
        }
    }
    Ok(())
}
