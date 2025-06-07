use crate::v1::processors::callback_processor::process_callback;
use crate::v1::services::user_config_store::UserConfigStore;
use std::error::Error;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

pub async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("callback_handler called! q: {:?}", q);

    // Extract callback data for logging
    if let Some(data) = q.data.as_deref() {
        log::info!("Callback data received: {}", data);
    } else {
        log::warn!("Callback query without data");
    }

    // Extract message info for logging
    if let Some(msg) = &q.message {
        log::info!("Callback from message ID: {}", msg.id());
    }

    // Process the callback
    let result = process_callback(bot, q, config_store).await;

    // Log the result
    match &result {
        Ok(_) => log::info!("Callback processing completed successfully"),
        Err(e) => log::error!("Callback processing failed: {}", e),
    }

    result
}
