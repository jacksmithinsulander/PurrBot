use crate::v1::processors::callback_processor::process_callback;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

pub async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    process_callback(bot, q).await
}
