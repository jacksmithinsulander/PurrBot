use crate::v1::processors::callback_processor::process_callback;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;
use std::sync::Arc;
use crate::v1::services::user_config_store::UserConfigStore;

pub async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    process_callback(bot, q, config_store).await
}
